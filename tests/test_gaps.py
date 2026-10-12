"""Trous d'enregistrement : aucune fenêtre d'analyse ne chevauche un raccord (D-020, W-010)."""

import numpy as np
import pytest
from scipy import signal

from opentunebox.analysis.delay import phase_delay_ms
from opentunebox.analysis.spectral import clear_of_breaks, spectrum
from opentunebox.analysis.step import step_response
from synthetic import make_flightlog

FS = 2000.0


def test_clear_of_breaks():
    starts = np.array([0, 50, 100, 150])
    assert clear_of_breaks(starts, 100, None).all()
    # raccord à 120 : touchées les fenêtres [50, 150) et [100, 200) ; [150, 250) commence après lui
    assert clear_of_breaks(starts, 100, np.array([120])).tolist() == [True, False, False, True]
    # un raccord au tout début d'une fenêtre ne la coupe pas
    assert clear_of_breaks(starts, 100, np.array([100])).tolist() == [True, False, True, True]


def test_flightlog_breaks():
    log = make_flightlog(seconds=10.0)
    assert log.breaks().size == 0
    log.t[log.n // 2:] += 0.2
    assert log.breaks().tolist() == [log.n // 2]
    assert log.breaks(slice(100, log.n)).tolist() == [log.n // 2 - 100]


def _gapped(x, cuts, length):
    """Signal privé de `length` échantillons à chaque position de `cuts`, et ses raccords."""
    keep = np.ones(x.size, bool)
    for c in cuts:
        keep[c:c + length] = False
    breaks = [c - sum(length for d in cuts if d < c) for c in cuts]
    return x[keep], np.array(breaks)


def _band(s, lo, hi):
    m = (s.freqs >= lo) & (s.freqs <= hi)
    return float(np.mean(s.values[m]))


def test_spectrum_ignores_gaps():
    """Mouvements lents et forts + petit bruit : un raccord fait un saut, donc un faux bruit large bande."""
    rng = np.random.default_rng(0)
    t = np.arange(int(60 * FS)) / FS
    x = 300 * np.sin(2 * np.pi * 0.7 * t) + rng.normal(0, 0.5, t.size)
    clean = _band(spectrum(x, FS, resolution_hz=2.0), 80, 500)
    xg, br = _gapped(x, [int(FS * k) for k in (7.3, 19.1, 33.7, 48.2)], int(0.1 * FS))
    assert _band(spectrum(xg, FS, resolution_hz=2.0), 80, 500) > clean + 5  # défaut corrigé (W-010)
    assert _band(spectrum(xg, FS, resolution_hz=2.0, breaks=br), 80, 500) == pytest.approx(clean, abs=0.3)


def test_step_response_ignores_gaps():
    log = make_flightlog(seconds=60.0)
    sp, gy = log.setpoint[0], log.gyro[0]
    ref = step_response(sp, gy, log.fs)
    cuts, L = [int(log.fs * k) for k in (11.3, 27.9, 44.6)], int(0.15 * log.fs)
    spg, br = _gapped(sp, cuts, L)
    gyg, _ = _gapped(gy, cuts, L)
    fixed = step_response(spg, gyg, log.fs, breaks=br)
    assert 0 < fixed.n < ref.n
    assert fixed.metrics.peak == pytest.approx(ref.metrics.peak, abs=0.01)
    assert fixed.metrics.latency_ms == pytest.approx(ref.metrics.latency_ms, abs=0.3)


def test_phase_delay_matches_scipy_without_breaks():
    """Mêmes fenêtres que scipy.signal.csd / welch / coherence : résultat identique sans raccord."""
    rng = np.random.default_rng(1)
    x = np.cumsum(rng.normal(0, 1, int(20 * FS))) + rng.normal(0, 5, int(20 * FS))
    b, a = signal.butter(2, 90, fs=FS)
    y = signal.lfilter(b, a, x)
    nper = 1024
    f, sxy = signal.csd(x, y, fs=FS, nperseg=nper)
    _, sxx = signal.welch(x, fs=FS, nperseg=nper)
    _, coh = signal.coherence(x, y, fs=FS, nperseg=nper)
    m = (f >= 10) & (f <= 100) & (coh >= 0.6) & (sxx > 0)
    d = -np.unwrap(np.angle(sxy[m] / sxx[m])) / (2 * np.pi * f[m])
    expected = float(np.average(d, weights=coh[m]) * 1000.0)
    assert phase_delay_ms(x, y, FS) == pytest.approx(expected, rel=1e-9)
