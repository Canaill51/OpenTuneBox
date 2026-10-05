"""Tests de la boucle ouverte (analysis/loop.py) : marges sur des boucles connues analytiquement, puis
mesure sur une boucle Betaflight simulée (tests/bf_loop_sim.py) dont la vraie boucle ouverte est connue."""

from functools import lru_cache
from pathlib import Path

import numpy as np
import pytest
from bf_loop_sim import simulate

from opentunebox.analysis.loop import APPROX, MEASURED, PidTerms, margins
from opentunebox.analysis.step import step_response

F = np.linspace(2.0, 200.0, 4000)


def integrator_lag_delay(f, fc0, tau, theta):
    """L = K / (s (1 + s τ)) · e^(−s θ), K tel que |L| ≈ 1 vers fc0 ; phase exacte connue."""
    s = 2j * np.pi * f
    return 2 * np.pi * fc0 / (s * (1 + s * tau)) * np.exp(-s * theta)


def exact(fc0, tau, theta):
    """Croisement, marge de phase et marge de gain par recherche fine sur la formule."""
    f = np.linspace(0.5, 1000, 2_000_000)
    L = integrator_lag_delay(f, fc0, tau, theta)
    w = 2 * np.pi * f
    ph = -90 - np.degrees(np.arctan(w * tau)) - np.degrees(w * theta)  # phase déroulée exacte
    i = int(np.argmin(np.abs(np.abs(L) - 1)))
    j = int(np.argmin(np.abs(ph + 180)))
    return f[i], 180 + ph[i], -20 * np.log10(np.abs(L[j]))


def test_margins_match_formula():
    fc, pm, gm = exact(40.0, 0.003, 0.004)
    m = margins(F, integrator_lag_delay(F, 40.0, 0.003, 0.004))
    assert m.crossover_hz == pytest.approx(fc, rel=0.002)
    assert m.phase_margin_deg == pytest.approx(pm, abs=0.2)
    assert m.gain_margin_db == pytest.approx(gm, abs=0.05)
    assert 0 < m.phase_margin_deg < 90 and m.gain_margin_db > 0


def test_negative_phase_margin_stays_negative():
    # phase au croisement ≈ −232° : np.angle seul donnait +128°, d'où une marge de +308° au lieu de −52°
    fc, pm, _ = exact(60.0, 0.002, 0.006)
    m = margins(F, integrator_lag_delay(F, 60.0, 0.002, 0.006))
    assert pm < -30
    assert m.phase_margin_deg == pytest.approx(pm, abs=0.3)
    assert m.crossover_hz == pytest.approx(fc, rel=0.002)


def test_first_crossover_only():
    # |L| repasse au-dessus de 1 vers 111–127 Hz (résonance) : seul le premier croisement compte,
    # car sur les vrais logs ces croisements tardifs viennent presque toujours du bruit
    L0 = integrator_lag_delay(F, 30.0, 0.002, 0.002)
    wr, z = 2 * np.pi * 120, 0.05
    s = 2j * np.pi * F
    L = L0 * (s**2 + 2 * 0.6 * wr * s + wr**2) / (s**2 + 2 * z * wr * s + wr**2)
    lg = np.log(np.abs(L))
    assert np.count_nonzero(np.sign(lg[:-1]) != np.sign(lg[1:])) == 3  # vers 29, 111 et 127 Hz
    m = margins(F, L)
    first = margins(F[F < 60], L[F < 60])
    assert m.crossover_hz == pytest.approx(first.crossover_hz) and m.crossover_hz < 35
    assert m.phase_margin_deg == pytest.approx(first.phase_margin_deg)


def test_valid_mask_and_sensitivity_edge():
    L = integrator_lag_delay(F, 40.0, 0.003, 0.004)
    full = margins(F, L)
    assert not full.sensitivity_peak_at_edge
    S = np.abs(1 / (1 + L))
    assert full.sensitivity_peak_db == pytest.approx(20 * np.log10(S.max()), abs=1e-9)
    # bande coupée avant le pic : le pic est au bord, ce n'est qu'un minorant
    cut = margins(F, L, valid=F < 0.6 * full.sensitivity_peak_hz)
    assert cut.sensitivity_peak_at_edge
    assert cut.sensitivity_peak_db < full.sensitivity_peak_db


def test_too_few_points():
    m = margins(F[:1], np.ones(1, complex))
    assert np.isnan(m.phase_margin_deg) and np.isnan(m.sensitivity_peak_db)


# ---------------------------------------------------------------- mesure sur boucle Betaflight simulée

FT = np.linspace(1.0, 300.0, 6000)


@lru_cache(maxsize=None)
def _sim(**kw):
    return simulate(**kw)


def _measure(s, **terms_kw):
    return step_response(s.r, s.y, s.fs, terms=PidTerms(s.p, s.i, s.d, s.f, **terms_kw)).loop


SCENARIOS = {
    "feedforward": {},
    "sans feedforward": dict(F=0),
    "D faible": dict(body_gain=150.0, D=20, F=0),
    "yaw sans D": dict(D=0, F=60, body_gain=60.0),
    "log 1 kHz": dict(log_div=4),
    "log 500 Hz": dict(log_div=8),
    "vol court (20 s)": dict(seconds=20.0),
}


@pytest.mark.parametrize("name", list(SCENARIOS))
def test_measured_loop_matches_truth(name):
    s = _sim(**SCENARIOS[name])
    truth = margins(FT, s.true_L(FT))
    lp = _measure(s)
    assert lp.method == MEASURED and not lp.notes
    assert lp.crossover_hz == pytest.approx(truth.crossover_hz, rel=0.06)
    assert lp.phase_margin_deg == pytest.approx(truth.phase_margin_deg, abs=4.0)
    assert not lp.sensitivity_peak_at_edge
    tol = max(1.0, 2 * lp.std["sensitivity_peak_db"])  # vol court : incertitude plus grande, chiffrée
    assert lp.sensitivity_peak_db == pytest.approx(truth.sensitivity_peak_db, abs=tol)
    assert lp.std["phase_margin_deg"] < 5.0
    assert np.isfinite(lp.tracking_peak_db)  # repris de la méthode approchée (consigne → erreur)


def test_approximate_method_is_wrong_without_feedforward():
    # Sans FF, T/(1 − T) place le croisement bien trop bas et surestime la marge de phase
    s = _sim(F=0)
    truth = margins(FT, s.true_L(FT))
    old = step_response(s.r, s.y, s.fs).loop
    assert old.method == APPROX and old.notes == ["termes PID non fournis"]
    assert abs(old.crossover_hz / truth.crossover_hz - 1) >= 0.30
    assert old.phase_margin_deg - truth.phase_margin_deg >= 10.0


def test_more_d_can_reduce_stability_when_d_dominates():
    # D déjà dominant au croisement : plus de D monte le croisement là où les filtres retardent,
    # le pic de sensibilité augmente. L'ancienne méthode annonce au contraire une marge en hausse.
    lo, hi = _sim(body_gain=150.0, D=20, F=0), _sim(body_gain=150.0, D=35, F=0)
    t_lo, t_hi = margins(FT, lo.true_L(FT)), margins(FT, hi.true_L(FT))
    assert t_hi.sensitivity_peak_db > t_lo.sensitivity_peak_db + 0.3
    m_lo, m_hi = _measure(lo), _measure(hi)
    assert m_hi.d_ratio > 1.0
    assert m_hi.sensitivity_peak_db > m_lo.sensitivity_peak_db
    o_lo, o_hi = step_response(lo.r, lo.y, lo.fs).loop, step_response(hi.r, hi.y, hi.fs).loop
    assert o_hi.phase_margin_deg > o_lo.phase_margin_deg + 5.0


def test_variable_d_between_bounds():
    # D max : le gain du D varie avec le manche ; la mesure donne une boucle entre D de base et D max
    s = _sim(d_boost=0.33)
    lo, hi = (margins(FT, s.true_L(FT, D=d)) for d in (35.0, 35.0 * 1.33))
    lp = _measure(s)
    assert lp.method == MEASURED
    assert lo.crossover_hz * 0.95 <= lp.crossover_hz <= hi.crossover_hz * 1.05
    assert min(lo.phase_margin_deg, hi.phase_margin_deg) - 4 <= lp.phase_margin_deg \
        <= max(lo.phase_margin_deg, hi.phase_margin_deg) + 4


def test_strong_disturbance_no_bias():
    # Forte perturbation (prop wash) : l'estimateur instrumental reste juste, seule l'incertitude grandit,
    # alors que l'estimateur direct S(u, gyro)/S(u, u) est biaisé en boucle fermée
    from scipy import signal

    s = _sim(dist=120.0)
    truth = margins(FT, s.true_L(FT))
    lp = _measure(s)
    assert lp.method == MEASURED
    assert abs(lp.phase_margin_deg - truth.phase_margin_deg) <= max(5.0, 3 * lp.std["phase_margin_deg"])
    u = s.p + s.i + s.d + s.f
    f, Suy = signal.csd(u, s.y, s.fs, nperseg=int(2 * s.fs))
    _, Suu = signal.welch(u, s.fs, nperseg=int(2 * s.fs))
    k = int(np.argmin(np.abs(f - 10.0)))
    G_true = s.true_parts(f[k:k + 1])[0][0]
    assert abs(20 * np.log10(abs(Suy[k] / Suu[k] / G_true))) > 3.0


def test_instrument_other_than_setpoint():
    # Mode angle : instrument = manche. Une fonction linéaire du manche donne la même boucle.
    s = _sim()
    a = _measure(s)
    b = _measure(s, instrument=2.5 * s.r + 7.0)
    assert b.method == MEASURED
    assert b.phase_margin_deg == pytest.approx(a.phase_margin_deg, abs=0.5)


def test_note_forces_approximate_method():
    s = _sim()
    lp = _measure(s, note="terme D non enregistré")
    assert lp.method == APPROX and lp.notes == ["terme D non enregistré"]
    assert np.isfinite(lp.phase_margin_deg)  # ancienne formule, toujours calculée


def test_flightlog_pid_terms():
    from opentunebox.log.flightlog import FlightLog
    from opentunebox.log.settings import parse_settings

    s = _sim(seconds=10.0)
    n = s.r.size
    st = parse_settings({"rollPID": "45,80,35", "pitchPID": "45,80,35", "yawPID": "45,80,0", "d_max": "35,35,0",
                         "ff_weight": "120,120,60", "Firmware revision": "Betaflight 4.5.1 (77d0) STM32F7X2"})
    three = lambda x: np.vstack([x, x, x])  # noqa: E731
    flags = np.full(n, 0b1)  # ARM
    lg = FlightLog(Path("sim.bbl"), 1, st, np.arange(n) / s.fs, s.fs, three(s.y), None, three(s.r),
                   np.vstack([three(s.r / 2), np.full(n, 1500.0)]), three(s.p), three(s.i), three(s.d), three(s.f),
                   np.full(n, 50.0), np.zeros((4, n)), None, np.zeros((0, n)), None, None, None, flags,
                   np.zeros((0, 4)), {}, frozenset({"axisP[0]", "axisI[0]", "axisF[0]", "axisP[2]", "axisI[2]", "axisF[2]"}))
    assert lg.pid_terms(0).note == "terme D non enregistré"  # D = 35 mais axisD[0] absent
    assert lg.pid_terms(2).note == ""  # yaw : D = 0, axisD[2] inutile
    assert lg.pid_terms(1).note == "termes P, I, D, F non enregistrés"
    assert lg.pid_terms(2).instrument is None
    lg.mode_flags = np.where(np.arange(n) < n // 2, 0b11, 0b1)  # ANGLE la moitié du temps
    assert lg.pid_terms(2).instrument is not None
    assert np.array_equal(lg.pid_terms(2).instrument, lg.rc_command[2])
