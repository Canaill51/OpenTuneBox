"""Tests des marges de stabilité (analysis/loop.py) sur des boucles ouvertes connues analytiquement."""

import numpy as np
import pytest

from opentunebox.analysis.loop import margins

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
