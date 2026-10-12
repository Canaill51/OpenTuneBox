"""Passage des valeurs brutes d'une session (DecodedSession) aux unités physiques (FlightLog), sans corpus."""

from pathlib import Path

import numpy as np
import pytest

from opentunebox.log.flightlog import from_session

from synthetic import make_flightlog, make_session


def test_high_resolution_scales_axes_and_rc_but_not_mixer_throttle():
    """`blackbox_high_resolution:1` : gyro, consigne des axes et les quatre rcCommand × 10 ; le gaz du mixeur
    (setpoint[3] = mixeur × 1000) ne change pas d'échelle (blackbox.c 4.4 → 2026.12, spec-betaflight.md §7)."""
    hr = {"gyroADC[0]": [1000] * 4, "setpoint[0]": [2000] * 4, "setpoint[3]": [500] * 4,
          "rcCommand[0]": [-3000] * 4, "rcCommand[3]": [15000] * 4}
    lg = from_session(Path("x.bbl"), make_session(hr, {"blackbox_high_resolution": "1"}))
    assert lg.gyro[0, 0] == 100.0 and lg.setpoint[0, 0] == 200.0
    assert lg.rc_command[0, 0] == -300.0 and lg.rc_command[3, 0] == 1500.0
    assert lg.throttle[0] == 50.0  # 500 / 1000 du mixeur = 50 %
    # sans haute résolution : mêmes grandeurs, valeurs brutes dix fois plus petites
    std = {k: [v[0] // 10 if k != "setpoint[3]" else v[0]] * 4 for k, v in hr.items()}
    lg = from_session(Path("x.bbl"), make_session(std, {}))
    assert lg.gyro[0, 0] == 100.0 and lg.setpoint[0, 0] == 200.0 and lg.rc_command[3, 0] == 1500.0
    assert lg.throttle[0] == 50.0


def test_high_resolution_throttle_fallback_from_rc_command():
    """Sans setpoint[3], le gaz vient de rcCommand[3], lui aussi × 10 en haute résolution."""
    lg = from_session(Path("x.bbl"), make_session({"gyroADC[0]": [0] * 4, "rcCommand[3]": [10000, 15000, 20000, 20000]},
                                                  {"blackbox_high_resolution": "1"}))
    assert lg.throttle.tolist() == [0.0, 50.0, 100.0, 100.0]


def test_throttle_falls_back_to_rc_command():
    """Sans setpoint[3] (anciens firmwares), le gaz vient de rcCommand[3] (1000..2000 → 0..100 %)."""
    lg = from_session(Path("x.bbl"), make_session({"gyroADC[0]": [0] * 4, "rcCommand[3]": [1000, 1250, 1500, 2000]}))
    assert lg.throttle.tolist() == [0.0, 25.0, 50.0, 100.0]


# ---------------------------------------------------------------- trames impossibles (L-002)


def test_drop_implausible_thresholds_and_sign():
    """Gyro ou consigne > 4000 °/s (× hr), terme PID > ±65535 : trame écartée ; les champs signés sont d'abord
    ramenés en 32 bits ; setpoint[3] (gaz) n'est pas contrôlé ; main, main_kind et slow restent alignés."""
    from opentunebox.log.flightlog import _drop_implausible

    cols = {"gyroADC[0]": [100, 4001, 2**32 - 5, 100, 100, 100],
            "setpoint[0]": [0, 0, 0, 0, 0, -4001],
            "setpoint[3]": [0, 0, 0, 0, 9000, 0],
            "axisP[0]": [0, 0, 0, 70000, 0, 0]}
    sess = make_session(cols, slow={"flightModeFlags": [1, 2, 3, 4, 5, 6]})
    assert _drop_implausible(sess, 1.0) == 3
    assert sess.main[:, 0].tolist() == [0, 2, 4]  # lignes gardées (itérations)
    assert sess.column("gyroADC[0]").tolist() == [100, -5, 100]  # 2³² − 5 → −5 : plausible
    assert sess.column("flightModeFlags").tolist() == [1, 3, 5] and sess.main_kind.shape == (3,)
    sess = make_session({"gyroADC[0]": [4001, 40001, 0]})
    assert _drop_implausible(sess, 10.0) == 1 and sess.column("gyroADC[0]").tolist() == [4001, 0]


def test_from_session_drops_mostly_corrupt_session():
    assert from_session(Path("x.bbl"), make_session({"gyroADC[0]": [5000, 5000, 5000, 100]})) is None
    lg = from_session(Path("x.bbl"), make_session({"gyroADC[0]": [5000, 100, 100, 100]}))
    assert lg is not None and lg.stats["implausible_frames"] == 1 and lg.n == 3
    assert from_session(Path("x.bbl"), make_session({"gyroADC[0]": [100]})) is None  # moins de 2 lignes


# ---------------------------------------------------------------- unités et replis de from_session


def test_from_session_units_dshot_erpm_debug_and_missing_terms():
    n = 4
    cols = {"gyroADC[0]": [0] * n, "motor[0]": [48, 2047, 1047, 48], "motor[1]": [48] * n, "motor[2]": [48] * n,
            "motor[3]": [48] * n, "eRPM[0]": [100] * n, "eRPM[1]": [0] * n, "eRPM[2]": [0] * n, "eRPM[3]": [0] * n,
            "vbatLatest": [1680] * n, "amperageLatest": [250] * n, "axisP[0]": [1] * n, "axisI[0]": [2] * n,
            "axisF[0]": [3] * n, "debug[0]": [10] * n, "debug[1]": [20] * n, "debug[2]": [30] * n}
    hdr = {"Firmware revision": "Betaflight 4.5.1 (abcd1234) STM32F7X2", "motor_pwm_protocol": "6",
           "motor_poles": "14", "debug_mode": "6"}  # 6 = GYRO_SCALED en 4.x
    lg = from_session(Path("x.bbl"), make_session(cols, hdr))
    assert lg.motors[0].tolist() == pytest.approx([0.0, 100.0, (1047 - 48) / 1999 * 100, 0.0])  # DShot 48..2047
    assert lg.motor_hz[0, 0] == pytest.approx(100 * 100 / 60 / 7) and lg.motor_hz.shape == (4, n)
    assert lg.vbat[0] == 16.8 and lg.current[0] == 2.5
    assert lg.gyro_unfilt is not None and lg.gyro_unfilt[:, 0].tolist() == [10, 20, 30]  # debug GYRO_SCALED
    assert lg.debug.shape == (3, n) and lg.d_term[2].tolist() == [0] * n
    assert lg.pid_columns == {"axisP[0]", "axisI[0]", "axisF[0]"} and not lg.term_present("D", 0)
    assert abs(lg.fs - 2000) < 1e-6 and lg.summary().startswith("x — Betaflight 4.5.1")


def test_from_session_pwm_output_and_rpm_from_debug():
    cols = {"gyroADC[0]": [0] * 3, "motor[0]": [1000, 1500, 2000], "motor[1]": [1000] * 3,
            "debug[0]": [700] * 3, "debug[1]": [1400] * 3}
    hdr = {"Firmware revision": "Betaflight 4.5.1 (abcd1234) STM32F7X2", "motor_pwm_protocol": "0",
           "motorOutput": "1000,2000", "debug_mode": "45", "motor_poles": "14"}  # 45 = DSHOT_RPM_TELEMETRY
    lg = from_session(Path("x.bbl"), make_session(cols, hdr))
    assert lg.motors[0].tolist() == [0.0, 50.0, 100.0] and lg.n_motors == 2
    assert lg.motor_hz.shape == (2, 3) and lg.motor_hz[1, 0] == pytest.approx(1400 * 100 / 60 / 7)


# ---------------------------------------------------------------- modes de vol, instrument, chirp (L-005)


def test_pid_terms_instrument_in_angle_and_horizon():
    """Au-delà de 20 % du temps en Angle OU Horizon, la consigne vient de l'autolevel : l'instrument de la
    boucle ouverte devient la commande radio."""
    lg = make_flightlog(seconds=2.0)
    n = lg.n
    assert lg.pid_terms(0).instrument is None  # Acro
    lg.mode_flags = np.full(n, 0b101)  # ARM + HORIZON
    assert np.array_equal(lg.pid_terms(0).instrument, lg.rc_command[0])
    lg.mode_flags = np.where(np.arange(n) < int(0.15 * n), 0b011, 0b001)  # Angle 15 % du temps : en dessous
    assert lg.pid_terms(0).instrument is None
    lg.mode_flags = np.where(np.arange(n) < int(0.25 * n), 0b011, 0b001)
    assert lg.pid_terms(0).instrument is not None


def test_angle_mask_distinct_from_horizon():
    """Le feedforward n'est nul qu'en Angle : `flight_conditions` ne compte pas Horizon, `pid_terms` compte les deux."""
    from opentunebox.analysis.step_series import flight_conditions
    from opentunebox.log import modes
    from opentunebox.log.flightlog import ANGLE_HORIZON_MASK, ANGLE_MODE_MASK

    lg = make_flightlog(seconds=1.0)
    lg.mode_flags = np.full(lg.n, 0b101)  # Horizon seul
    assert flight_conditions(lg).angle_share == 0.0 and flight_conditions(lg).ff_active
    lg.mode_flags = np.full(lg.n, 0b011)  # Angle
    assert flight_conditions(lg).angle_share == 1.0 and not flight_conditions(lg).ff_active
    lg.mode_flags = None
    assert np.isnan(flight_conditions(lg).angle_share) and lg.pid_terms(0).instrument is None
    assert ANGLE_MODE_MASK & ANGLE_HORIZON_MASK == ANGLE_MODE_MASK
    for fam, table in modes.TABLES.items():  # mêmes rangs dans toutes les versions
        assert 1 << table.index("ANGLE") == ANGLE_MODE_MASK, fam
        assert (1 << table.index("HORIZON")) | ANGLE_MODE_MASK == ANGLE_HORIZON_MASK, fam


def test_chirp_excitation_added_to_instrument():
    """Debug CHIRP : debug[1] = axe excité, debug[3] = excitation × 1000 ; en mode Angle elle s'ajoute à la
    commande radio pour former l'instrument."""
    lg = make_flightlog(seconds=1.0, **{"debug_mode_name": "CHIRP", "chirp_amplitude_roll": "50"})
    n = lg.n
    assert lg.debug_name() == "CHIRP" and lg.chirp_excitation(0) is None  # pas de debug enregistré
    exc = np.sin(2 * np.pi * 20 * lg.t)
    lg.debug = np.vstack([np.zeros(n), np.zeros(n), np.zeros(n), exc * 1000.0])  # axe 0 excité
    assert np.allclose(lg.chirp_excitation(0), 50.0 * exc) and lg.chirp_excitation(1) is None
    lg.mode_flags = np.full(n, 0b011)
    assert np.allclose(lg.pid_terms(0).instrument, lg.rc_command[0] + 50.0 * exc)
    lg.debug[1] = -1  # aucun axe excité
    assert lg.chirp_excitation(0) is None


# ---------------------------------------------------------------- propriétés dérivées


def test_segments_and_armed():
    lg = make_flightlog(seconds=1.0)
    assert lg.segments() == [slice(0, lg.n)]
    lg.t = lg.t.copy()
    lg.t[lg.n // 2:] += 10 / lg.fs  # trou d'enregistrement de 10 échantillons
    assert lg.segments() == [slice(0, lg.n // 2), slice(lg.n // 2, lg.n)]
    assert lg.armed.all()
    lg.mode_flags = None
    del lg.__dict__["armed"]  # propriété mise en cache
    assert lg.armed.all() and lg.armed.shape == (lg.n,)
    lg.mode_flags = np.array([1, 0, 3] + [1] * (lg.n - 3))
    del lg.__dict__["armed"]
    assert lg.armed[:3].tolist() == [True, False, True]


def test_d_term_prefilter_recovers_gain():
    """D = Kd · (−dgyro/dt) : le gain est retrouvé par moindres carrés sur les composantes lentes."""
    lg = make_flightlog(seconds=4.0)
    lg.gyro = np.vstack([100 * np.sin(2 * np.pi * 5 * lg.t)] * 3)
    lg.d_term = -np.gradient(lg.gyro, axis=1) * lg.fs * 0.01
    lg.d_term[2] = 0.0
    pre = lg.d_term_prefilter
    assert np.allclose(pre[0], lg.d_term[0], atol=0.02 * np.abs(lg.d_term[0]).max())
    assert not np.any(pre[2])
