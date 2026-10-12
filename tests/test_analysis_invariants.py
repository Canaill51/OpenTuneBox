"""Invariants des analyses (analysis/), sans corpus : échelle des DSP (L-009), valeurs par défaut de la réponse
indicielle, horizon du pic, bande passante, retards par la phase (L-006, L-007), marges de stabilité."""

import inspect
import re
from pathlib import Path
from types import SimpleNamespace

import numpy as np
import pytest
from scipy import signal

from opentunebox.analysis import delay, loop, spectral, step
from opentunebox.advisor.rules import band_peak_ptb_db

ROOT = Path(__file__).resolve().parents[1] / "src" / "opentunebox"


# ---------------------------------------------------------------- DSP et échelle de PIDtoolbox


@pytest.mark.parametrize("fs, res", [(2000.0, 0.5), (2000.0, 2.0), (1999.0, 1.0)])
def test_psd_equals_scipy_welch(fs, res):
    """Nos DSP sont la densité physique de `scipy.signal.welch` (Hann, recouvrement 50 %), L pair ou impair."""
    rng = np.random.default_rng(0)
    t = np.arange(int(10 * fs)) / fs
    x = rng.normal(0, 1, t.size) + 3.0 * np.sin(2 * np.pi * 100 * t)
    s = spectral.spectrum(x, fs, resolution_hz=res)
    L = int(round(fs / res))
    f, p = signal.welch(x, fs, window="hann", nperseg=L, noverlap=L - L // 2, detrend="constant")  # pas = L // 2
    assert np.allclose(s.freqs, f)
    assert np.allclose(s.values, 10 * np.log10(p), atol=1e-9)


def test_psd_sine_power_and_parseval():
    fs, A = 2000.0, 3.0
    t = np.arange(int(20 * fs)) / fs
    x = A * np.sin(2 * np.pi * 100 * t)
    s = spectral.spectrum(x, fs, resolution_hz=0.5)
    df = s.freqs[1] - s.freqs[0]
    assert np.sum(10 ** (s.values / 10)) * df == pytest.approx(A**2 / 2, rel=0.02)
    rng = np.random.default_rng(1)
    x = rng.normal(0, 2.0, t.size)
    s = spectral.spectrum(x, fs, resolution_hz=2.0)
    assert np.sum(10 ** (s.values / 10)) * (s.freqs[1] - s.freqs[0]) == pytest.approx(np.var(x), rel=0.03)
    assert spectral.spectrum(x, fs, mode="amp").mode == "amp"


def test_ptb_scale_applied_exactly_once():
    """L'assistant convertit lui-même (`band_peak_ptb_db`) ; l'interface par `to_ptb_db` dans `compute` ; jamais
    les deux (L-009, D-016)."""
    fs = 2000.0
    t = np.arange(int(10 * fs)) / fs
    x = np.random.default_rng(2).normal(0, 1, t.size) + 5.0 * np.sin(2 * np.pi * 150 * t)
    s = spectral.spectrum(x, fs, resolution_hz=2.0, smoothing=2)
    band = (s.freqs >= 80) & (s.freqs <= 500)
    assert band_peak_ptb_db(x, fs, 80, 500) == pytest.approx(s.values[band].max() - spectral.PTB_PSD_OFFSET_DB, abs=1e-9)
    p = np.array([1e-3, 1.0, np.nan])
    out = spectral.to_ptb_db(spectral.to_db(p))
    assert out[0] == pytest.approx(-30 - 6.5) and out[1] == pytest.approx(-6.5) and np.isnan(out[2])
    # contrôle statique : la conversion n'apparaît que dans spectral.py, rules.py et les pages d'outils
    users = sorted(str(p.relative_to(ROOT)) for p in ROOT.rglob("*.py")
                   if re.search(r"to_ptb_db\(|PTB_PSD_OFFSET_DB", p.read_text(encoding="utf-8")))
    allowed = {"analysis/spectral.py", "advisor/rules.py"}
    assert all(u in allowed or u.startswith("gui/tools/") for u in users), users
    assert not any(re.search(r"to_ptb_db\(.*to_ptb_db\(", line) for u in users
                   for line in (ROOT / u).read_text(encoding="utf-8").splitlines())


# ---------------------------------------------------------------- réponse indicielle


def test_step_response_defaults_frozen():
    """Fenêtres de 2 s, pas de 62,5 ms, consigne min 20 °/s, régularisation 1e-4 (AGENTS.md, Pièges connus)."""
    p = inspect.signature(step.step_response).parameters
    assert (p["seg_s"].default, p["hop_ms"].default, p["min_input"].default, p["reg"].default) == (2.0, 62.5, 20.0, 1e-4)
    assert p["resp_ms"].default == 500.0
    q = inspect.signature(step.input_windows).parameters
    assert (q["seg_s"].default, q["min_input"].default, q["hop_ms"].default) == (2.0, 20.0, 62.5)


def test_input_windows_exact_count():
    fs = 1000.0
    sp = np.full(10_000, 100.0)  # 10 s de consigne : fenêtres de 2000 échantillons, pas de 62 (62,5 ms arrondi)
    assert step.input_windows(sp, fs) == (10_000 - 2000) // 62 + 1
    assert step.input_windows(np.full(10_000, 19.9), fs) == 0
    sp = np.zeros(10_000)
    sp[5000] = 20.0  # un seul pic : toutes les fenêtres qui le contiennent
    assert step.input_windows(sp, fs) == len([s for s in range(0, 8001, 62) if s <= 5000 < s + 2000])
    assert step.input_windows(np.full(1000, 100.0), fs) == 0  # plus court qu'une fenêtre


def test_step_metrics_peak_horizon():
    """Pic cherché dans max(150 ms, 3 × latence) : une lente dérive du terme I n'est pas un dépassement."""
    t = np.arange(0, 501, 1.0)
    r = 1 - np.exp(-t / 8.0)
    r = r + np.where(t > 150, 0.2 * (t - 150) / 350, 0.0)  # dérive lente après 150 ms
    m = step.step_metrics(t, r)
    assert m.latency_ms == pytest.approx(8 * np.log(2), abs=1.0)
    assert m.peak == pytest.approx(1.0, abs=0.01) and m.peak_ms <= 150
    # drone lent (latence 70 ms) : dépassement à 190 ms trouvé grâce à l'horizon de 3 × latence
    tau = 70 / np.log(2)
    r = 1 - np.exp(-t / tau) + 0.3 * np.exp(-((t - 190) / 15) ** 2)
    m = step.step_metrics(t, r)
    assert 60 < m.latency_ms < 80 and m.peak > 1.1 and 180 <= m.peak_ms <= 200
    assert step.step_metrics(t, np.full(t.size, np.nan)).peak != m.peak  # réponse invalide : métriques vides


def test_step_response_identity_and_pure_delay():
    """Wiener sans bruit : consigne = gyro → réponse 1 ; gyro retardé de k échantillons → marche à k/fs."""
    fs = 1000.0
    rng = np.random.default_rng(3)
    t = np.arange(int(12 * fs)) / fs
    sp = np.zeros(t.size)
    k = 0
    while k < t.size:
        hold, gap = int(rng.uniform(0.2, 0.5) * fs), int(rng.uniform(0.3, 0.8) * fs)
        sp[k:k + hold] = rng.choice([-1, 1]) * rng.uniform(100, 400)
        k += hold + gap
    res = step.step_response(sp, sp, fs)
    assert res.n > 20 and np.allclose(res.mean[(res.t_ms > 5) & (res.t_ms < 500)], 1.0, atol=0.02)
    gy = np.roll(sp, 10)
    res = step.step_response(sp, gy, fs)
    assert res.metrics.latency_ms == pytest.approx(9.5, abs=0.3)  # 50 % lu entre les échantillons 9 et 10
    assert step.step_response(sp[:500], gy[:500], fs).n == 0  # plus court qu'une fenêtre : vide
    assert step.step_response(sp, 0.3 * sp, fs).n == 0  # palier 0,3 : refusé par le contrôle qualité (0,5–3)


# ---------------------------------------------------------------- bande passante et marges


def test_bandwidth_from_2hz():
    f = np.arange(0, 150.001, 0.25)
    T = 1 / (1 + 1j * f / 30.0)
    coh = np.ones(f.size)
    assert loop.loop_from_T(f, T, coh).bandwidth_hz == pytest.approx(30.0, abs=1.5)
    T2 = T.copy()
    T2[f < 2] = 0.3  # sous 2 Hz : ignoré
    assert loop.loop_from_T(f, T2, coh).bandwidth_hz == pytest.approx(30.0, abs=1.5)
    assert np.isnan(loop.loop_from_T(f, T, np.full(f.size, 0.4)).bandwidth_hz)  # cohérence insuffisante partout
    assert loop.loop_from_T(f, T, coh).band_hz[0] == 2.0


def test_margins_gain_and_delay_invariants():
    """Multiplier L par k abaisse la marge de gain de 20·log10 k ; un retard pur θ abaisse la marge de phase
    de 360·fc·θ, sans déplacer le croisement."""
    f = np.arange(1, 400.0, 0.1)
    w = 2 * np.pi * f
    L = 300.0 / (1j * w) / (1 + 1j * f / 60.0) ** 2
    m = loop.margins(f, L)
    m2 = loop.margins(f, 2.0 * L)
    assert m2.gain_margin_db == pytest.approx(m.gain_margin_db - 20 * np.log10(2), abs=0.1)
    theta = 0.002
    m3 = loop.margins(f, L * np.exp(-1j * w * theta))
    assert m3.crossover_hz == pytest.approx(m.crossover_hz, abs=0.2)
    assert m3.phase_margin_deg == pytest.approx(m.phase_margin_deg - 360 * m.crossover_hz * theta, abs=0.5)


# ---------------------------------------------------------------- retards (L-006, L-007)


def _pt1(x, fc, fs):
    k = 2 * np.pi * fc / fs
    k = k / (k + 1)
    return signal.lfilter([k], [1, -(1 - k)], x), ([k], [1, -(1 - k)])


def test_phase_delay_pure_delay_pt1_and_band():
    fs = 2000.0
    rng = np.random.default_rng(4)
    x = rng.normal(0, 1, int(20 * fs))
    assert delay.phase_delay_ms(x, np.roll(x, 5), fs) == pytest.approx(2.5, abs=0.05)
    assert delay.phase_delay_ms(np.roll(x, 5), x, fs) == pytest.approx(-2.5, abs=0.05)
    y, (b, a) = _pt1(x, 100.0, fs)
    nper = 2 ** int(np.ceil(np.log2(fs * 0.5)))
    f = np.fft.rfftfreq(nper, 1 / fs)
    band = (f >= 10) & (f <= 100)
    _, h = signal.freqz(b, a, worN=f[band], fs=fs)
    expected = np.mean(-np.unwrap(np.angle(h)) / (2 * np.pi * f[band])) * 1000
    assert delay.phase_delay_ms(x, y, fs) == pytest.approx(expected, abs=0.05)
    # bande : seules les fréquences 10–100 Hz comptent
    assert delay.phase_delay_ms(x, y, fs, band=(200, 400)) < delay.phase_delay_ms(x, y, fs)
    assert np.isnan(delay.phase_delay_ms(x, rng.normal(0, 1, x.size), fs))  # signaux indépendants
    assert np.isnan(delay.phase_delay_ms(x[:40], y[:40], fs))


def test_xcorr_delay_antisymmetric_and_bounded():
    fs = 2000.0
    x = np.random.default_rng(5).normal(0, 1, 4000)
    y = np.roll(x, 7)
    assert delay.xcorr_delay_ms(x, y, fs) == pytest.approx(3.5, abs=0.05)
    assert delay.xcorr_delay_ms(y, x, fs) == pytest.approx(-3.5, abs=0.05)
    assert abs(delay.xcorr_delay_ms(x, np.roll(x, 100), fs, max_ms=30.0)) <= 30.0
    assert np.isnan(delay.xcorr_delay_ms(x[:10], y[:10], fs)) and np.isnan(delay.xcorr_delay_ms(x, np.zeros(x.size), fs))


def test_compute_delays_on_fake_log():
    fs = 2000.0
    rng = np.random.default_rng(6)
    n = int(20 * fs)
    raw = rng.normal(0, 1, (3, n))
    gyro = np.vstack([_pt1(raw[a], 100.0, fs)[0] for a in range(3)])
    pre = rng.normal(0, 1, (3, n))
    d = np.vstack([_pt1(pre[a], 75.0, fs)[0] for a in range(3)])
    sp = np.vstack([200 * np.sin(2 * np.pi * 2 * np.arange(n) / fs), 200 * np.cos(2 * np.pi * 2 * np.arange(n) / fs),
                    np.zeros(n)])
    log = SimpleNamespace(fs=fs, gyro_unfilt=raw, gyro=gyro, d_term=d, d_term_prefilter=pre, setpoint=sp,
                          breaks=lambda sel: np.zeros(0, dtype=int))  # aucun trou
    dl = delay.compute_delays(log)
    assert dl.gyro_ms == pytest.approx(delay.phase_delay_ms(raw[0], gyro[0], fs), abs=0.05)
    assert dl.dterm_ms > dl.gyro_ms  # 75 Hz retarde plus que 100 Hz
    assert np.isnan(dl.sp_gyro_ms[2]) and np.isfinite(dl.sp_gyro_ms[0])  # yaw immobile : pas de latence mesurable
    assert dl.moving and dl.stick_p95_dps == pytest.approx(200 * np.sin(np.pi * 0.95 / 2), rel=0.02)
    log.gyro_unfilt = None
    log.d_term = np.zeros((3, n))
    dl = delay.compute_delays(log)
    assert np.isnan(dl.gyro_ms) and np.isnan(dl.dterm_ms)
    assert delay.Delays(0, 0, [], 49.9).moving is False and delay.Delays(0, 0, [], 50.0).moving is True
