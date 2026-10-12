"""Analyses « de vol » sans corpus : fréquences moteur (motors.py), statistiques (stats.py), période et crash
(period.py), sur le vol synthétique de `tests/synthetic.py` modifié à la demande."""

import dataclasses

import numpy as np
import pytest

from opentunebox.analysis.motors import distribution, hover_motor_hz, motor_hz, motor_noise
from opentunebox.analysis.period import auto_period, crash_time
from opentunebox.analysis.stats import flight_stats, pid_error_stats
from opentunebox.log.settings import parse_settings
from synthetic import BASE_HEADER, make_flightlog


@pytest.fixture(scope="module")
def hover():
    return make_flightlog(seconds=8.0, moving=False)


# ---------------------------------------------------------------- moteurs


def test_motor_hz_measured_or_estimated(hover):
    mhz, estimated = motor_hz(hover)
    assert mhz is hover.motor_hz and not estimated
    no_rpm = dataclasses.replace(hover, motor_hz=None)
    est, estimated = motor_hz(no_rpm)
    assert estimated and est.shape == hover.motors.shape
    assert np.allclose(est, 2.1 * hover.motors * 4.0)  # multiplicateur par défaut : ≈ 8,4 Hz par %
    assert np.allclose(motor_hz(no_rpm, 3.0)[0], 3.0 * hover.motors * 4.0)
    # KV connu (2025.12+) et tension loggée : tr/min ≈ KV × V × commande × 0,85
    kv = dataclasses.replace(no_rpm, settings=parse_settings({**BASE_HEADER, "motor_kv": "1800"}),
                             vbat=np.full(hover.n, 16.0))
    est, estimated = motor_hz(kv)
    assert estimated and np.allclose(est, 1800 * 16.0 * np.clip(hover.motors / 100, 0, 1) * 0.85 / 60)
    assert np.allclose(motor_hz(kv, 2.0)[0], 2.0 * hover.motors * 4.0)  # multiplicateur donné : prioritaire
    assert motor_hz(dataclasses.replace(no_rpm, motors=np.zeros((0, hover.n)))) == (None, True)


def test_hover_motor_hz_median_in_flight(hover):
    m = hover_motor_hz(hover)
    flying = hover.throttle > 5
    assert m == pytest.approx(float(np.median(hover.motor_hz.mean(axis=0)[flying])), rel=1e-9)
    assert hover_motor_hz(dataclasses.replace(hover, motor_hz=None)) is None  # estimée : pas une mesure
    assert hover_motor_hz(dataclasses.replace(hover, throttle=np.zeros(hover.n))) is None  # jamais en vol


def test_distribution_is_a_cdf():
    v = np.array([100.0, 200.0, 300.0, np.nan, -5.0, 0.0])
    f = np.array([50.0, 100.0, 250.0, 400.0])
    assert distribution(v, f).tolist() == [0.0, 1 / 3, 2 / 3, 1.0]  # nan et valeurs ≤ 0 ignorées
    assert distribution(np.array([np.nan]), f).tolist() == [0.0] * 4


def test_motor_noise_reads_psd_at_harmonics():
    fs, n = 2000.0, 20000
    t = np.arange(n) / fs
    mhz = np.full((4, n), 150.0)
    x = np.random.default_rng(0).normal(0, 1, n) + 3.0 * np.sin(2 * np.pi * 150 * t) + 2.0 * np.sin(2 * np.pi * 450 * t)
    mn = motor_noise(x, mhz, fs)
    assert mn.harmonics.tolist() == [1, 2, 3] and np.all(np.isfinite(mn.mean_db))
    assert mn.mean_db[0] > mn.mean_db[1] + 10 and mn.mean_db[2] > mn.mean_db[1] + 10  # raies 1 et 3 seulement
    assert np.all(np.isnan(motor_noise(x[:10], mhz[:, :10], fs).mean_db))  # trop court pour une fenêtre
    # harmonique au-delà de la bande mesurable : nan
    assert np.isnan(motor_noise(x, np.full((4, n), 400.0), fs, n_harm=3).mean_db[2])


# ---------------------------------------------------------------- statistiques


def test_flight_stats_on_synthetic(hover):
    st = flight_stats(hover)
    assert st.duration_s == pytest.approx(hover.duration)
    for name in ("Roll", "Pitch", "Yaw", "Gaz"):
        centres, pct = st.stick_hist[name]
        assert centres.size == 40 and pct.sum() == pytest.approx(100.0)
    assert set(st.terms) == {"gyro", "P", "I", "D", "F"} and len(st.terms["gyro"]) == 3
    assert st.terms["D"][2].mean == 0.0  # pas de D en lacet
    assert len(st.motors) == 4 and st.motor_saturation_pct == 0.0 and st.motor_imbalance_pct < 2
    assert st.vbat is None and st.current is None and st.mah is None
    # un moteur à 99 % un quart du temps, un autre 20 % plus chargé ; 10 A pendant tout le vol
    m = hover.motors.copy()
    q = hover.n // 4
    m[0, :q] = 99.0
    m[1] += 20.0
    v = np.full(hover.n, 16.0)
    v[10] = 14.8
    cur = np.full(hover.n, 10.0)
    st = flight_stats(dataclasses.replace(hover, motors=m, vbat=v, current=cur))
    assert st.motor_saturation_pct == pytest.approx(25.0, abs=0.1)
    means = m[:, hover.throttle > 5].mean(axis=1)  # écart du moteur le plus chargé à la moyenne, en vol
    assert st.motor_imbalance_pct == pytest.approx(100 * (means.max() - means.mean()) / means.mean(), rel=1e-6)
    assert 15 < st.motor_imbalance_pct < 25
    assert st.vbat_min == 14.8 and st.vbat.max == 16.0 and st.current.mean == 10.0
    assert st.mah == pytest.approx(10.0 * hover.duration / 3.6, rel=0.01)
    # sélection : les statistiques ne portent que sur la tranche
    assert flight_stats(dataclasses.replace(hover, motors=m), slice(q, None)).motor_saturation_pct == 0.0


def test_pid_error_stats_robust_sd(hover):
    """En stationnaire synthétique, l'erreur = −(bruit blanc σ = 2 + raie moteur) : σ robuste proche de 2, moins
    sensible que l'écart-type à quelques transitions."""
    st = pid_error_stats(hover)
    assert st.edges.size == 101 and all(h.sum() == pytest.approx(100.0) for h in st.hist)
    for a in range(3):
        assert 1.5 < st.sd[a] < 3.5 and 1.0 < st.mean_abs[a] < 4.0
    assert st.stick_bins.size == 10 and all(v.size == 10 for v in st.err_vs_stick)
    assert np.isfinite(st.err_vs_stick[0][0]) and np.all(np.isnan(st.err_vs_stick[0][5:]))  # manche au repos
    # grands mouvements exclus de l'histogramme (|consigne| ≤ 100 °/s), mais présents dans l'erreur par déflexion
    moving = make_flightlog(seconds=8.0)
    st2 = pid_error_stats(moving)
    assert np.isfinite(st2.err_vs_stick[0][1:4]).any()


# ---------------------------------------------------------------- période et crash


def _with_crash(log, t_crash: float, ms: float = 100.0, kind: str = "pid"):
    k0 = int(t_crash * log.fs)
    k1 = k0 + int(ms * log.fs / 1000)
    gyro, motors = log.gyro.copy(), log.motors.copy()
    if kind == "pid":  # erreur énorme, un moteur saturé, un autre au ralenti
        gyro[0, k0:k1] = 500.0
        motors[0, k0:k1] = 99.0
        motors[1, k0:k1] = 5.0
    else:  # rotation hors de portée du gyro
        gyro[1, k0:k1] = 2500.0
    return dataclasses.replace(log, gyro=gyro, motors=motors)


def test_crash_time_detection(hover):
    assert crash_time(hover) is None
    assert crash_time(_with_crash(hover, 5.0)) == pytest.approx(5.0, abs=0.02)
    assert crash_time(_with_crash(hover, 3.0, kind="gyro")) == pytest.approx(3.0, abs=0.02)
    assert crash_time(_with_crash(hover, 5.0, ms=30.0)) is None  # trop bref : pas un crash
    # erreur énorme mais moteurs normaux (coup de manche violent) : pas un crash
    g = hover.gyro.copy()
    g[0, 10000:10400] = 500.0
    assert crash_time(dataclasses.replace(hover, gyro=g)) is None


def test_auto_period_margins_crash_and_short_flights(hover):
    t0, t1 = auto_period(hover)
    assert t0 == pytest.approx(0.5) and t1 == pytest.approx(hover.duration - 0.5, abs=0.01)
    # décollage à 2 s : début à 2,5 s
    thr = hover.throttle.copy()
    thr[: int(2 * hover.fs)] = 0.0
    assert auto_period(dataclasses.replace(hover, throttle=thr))[0] == pytest.approx(2.5, abs=0.01)
    # crash à 5 s : fin 0,3 s avant
    assert auto_period(_with_crash(hover, 5.0))[1] == pytest.approx(4.7, abs=0.02)
    # jamais en vol, ou vol utile trop court : tout le log
    whole = (0.0, pytest.approx(hover.duration))
    assert auto_period(dataclasses.replace(hover, throttle=np.zeros(hover.n))) == whole
    short = make_flightlog(seconds=1.5, moving=False)
    assert auto_period(short) == (0.0, pytest.approx(short.duration))
