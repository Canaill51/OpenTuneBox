"""Tests des modèles de filtres Betaflight (analysis/bf_filters.py)."""


import numpy as np
import pytest

from opentunebox.analysis import bf_filters as bf
from opentunebox.log.settings import parse_settings

from corpus_paths import LOGS  # noqa: E402
HOVER = LOGS / "1. PTB/Apri/btfl_008.bbl"  # stationnaire puis wobbles, 2 kHz, gyro non filtré


def _db(h):
    return 20 * np.log10(np.abs(h))


@pytest.mark.parametrize("kind", ["pt1", "pt2", "pt3", "biquad_lpf"])
def test_lowpass_minus_3db_at_cutoff(kind):
    # fc ≪ fs : la formule ω/(ω+1) et la correction PT2/PT3 donnent −3 dB à fc
    fs, fc = 32000.0, 100.0
    h = bf.freq_response([bf.FilterSpec(kind, fc)], [0.0, fc], fs)
    assert abs(h[0]) == pytest.approx(1.0, abs=1e-9)
    assert _db(h[1]) == pytest.approx(-3.01, abs=0.15)


def test_pt1_coefficients():
    b, a = bf.coefficients(bf.FilterSpec("pt1", 100.0), 8000.0)
    w = 2 * np.pi * 100 / 8000
    k = w / (w + 1)
    assert np.allclose(b, [k]) and np.allclose(a, [1, -(1 - k)])
    b2, a2 = bf.coefficients(bf.FilterSpec("pt3", 100.0), 8000.0)
    assert len(b2) == 1 and len(a2) == 4


def test_notch_depth_and_weight():
    fs, f0 = 8000.0, 200.0
    h = bf.freq_response([bf.FilterSpec("notch", f0, 5.0)], [f0, 20.0, 2000.0], fs)
    assert abs(h[0]) < 1e-6
    assert abs(h[1]) == pytest.approx(1.0, abs=1e-3)
    for w in (0.0, 0.3, 0.75, 1.0):
        hw = bf.freq_response([bf.FilterSpec("notch", f0, 5.0, weight=w)], [f0], fs)
        assert abs(hw[0]) == pytest.approx(1.0 - w, abs=1e-6)


def test_static_notch_q():
    q = bf.notch_q(260, 160)
    assert q == pytest.approx(260 * 160 / (260**2 - 160**2))
    # bande −3 dB : la coupure basse tombe à −3 dB
    h = bf.freq_response([bf.FilterSpec("notch", 260.0, q)], [160.0], 32000.0)
    assert _db(h[0]) == pytest.approx(-3.01, abs=0.2)


def test_dyn_lpf_curve():
    assert bf.dyn_lpf_cutoff(250, 500, 5, 0.0) == pytest.approx(250)
    assert bf.dyn_lpf_cutoff(250, 500, 5, 1.0) == pytest.approx(500)
    # expo 5 à mi-gaz : courbe = 0,25·0,5 + 0,5 = 0,625
    assert bf.dyn_lpf_cutoff(250, 500, 5, 0.5) == pytest.approx(250 + 250 * 0.625)
    # quantification au centième
    assert bf.dyn_lpf_cutoff(250, 500, 5, 0.5004) == bf.dyn_lpf_cutoff(250, 500, 5, 0.5)
    # ancienne loi (expo 0)
    assert bf.dyn_lpf_cutoff(250, 500, 0, 0.0) == pytest.approx(250)
    assert bf.dyn_lpf_cutoff(250, 500, 0, 1.0) == pytest.approx(500)
    arr = bf.dyn_lpf_cutoff(75, 150, 5, np.array([0.0, 1.0]))
    assert np.allclose(arr, [75, 150])


def _settings(**extra):
    raw = {
        "Firmware revision": "Betaflight 2025.12.2 (x) STM32F7X2",
        "looptime": "125", "pid_process_denom": "1",
        "gyro_lpf1_type": "0", "gyro_lpf1_static_hz": "0", "gyro_lpf1_dyn_hz": "250,500",
        "gyro_lpf1_dyn_expo": "5", "gyro_lpf2_type": "0", "gyro_lpf2_static_hz": "500",
        "gyro_notch_hz": "0,0", "gyro_notch_cutoff": "0,0",
        "dterm_lpf1_type": "0", "dterm_lpf1_static_hz": "75", "dterm_lpf1_dyn_hz": "75,150",
        "dterm_lpf1_dyn_expo": "5", "dterm_lpf2_type": "0", "dterm_lpf2_static_hz": "150",
        "dterm_notch_hz": "0", "dterm_notch_cutoff": "0",
        "dyn_notch_count": "3", "dyn_notch_q": "300", "dyn_notch_min_hz": "100", "dyn_notch_max_hz": "600",
        "rpm_filter_harmonics": "3", "rpm_filter_weights": "100,50,0", "rpm_filter_q": "500",
        "rpm_filter_min_hz": "100", "rpm_filter_fade_range_hz": "50",
    }
    raw.update(extra)
    return parse_settings(raw)


def test_gyro_chain_order_and_rates():
    s = _settings(gyro_notch_hz="300,0", gyro_notch_cutoff="200,0")
    chain = bf.gyro_chain(s, throttle_frac=0.0, motor_hz=[200.0, 400.0])
    kinds = [(sp.kind, round(sp.f_hz), fs) for sp, fs in chain]
    assert kinds[0] == ("pt1", 500, 8000.0)
    # RPM : poids 0 sur l'harmonique 3 → omis ; 2 harmoniques × 2 moteurs
    rpm = [sp for sp, _ in chain[1:5]]
    assert [round(sp.f_hz) for sp in rpm] == [200, 400, 400, 800]
    assert [sp.weight for sp in rpm] == [1.0, 0.5, 1.0, 0.5]
    assert rpm[0].q == pytest.approx(5.0)
    assert chain[5][0].kind == "notch" and chain[5][0].f_hz == 300
    assert chain[6][0].kind == "pt1" and chain[6][0].f_hz == pytest.approx(250)
    dyn = [sp for sp, _ in chain[7:]]
    assert len(dyn) == 3 and all(sp.q == pytest.approx(3.0) for sp in dyn)
    assert [round(sp.f_hz) for sp in dyn] == [183, 350, 517]


def test_rpm_fade():
    s = _settings()
    f, w = bf.rpm_notch_freq_weight(np.array([80.0, 125.0, 300.0]), 0, s, 8000.0)
    assert np.allclose(f, [100, 125, 300])
    assert np.allclose(w, [0.0, 0.5, 1.0])


def test_dterm_chain_and_delay():
    s = _settings(dterm_notch_hz="260", dterm_notch_cutoff="160")
    chain = bf.dterm_chain(s, throttle_frac=1.0)
    assert [sp.kind for sp, _ in chain] == ["notch", "pt1", "pt1"]
    assert chain[1][0].f_hz == pytest.approx(150)
    # Mélange de cadences : réponse = produit des étages
    h = bf.chain_response(chain, [10.0])
    assert abs(h[0]) == pytest.approx(1.0, abs=0.02)
    d = bf.chain_delay_ms(chain, f_hz=5.0)
    # PT1 : retard ≈ 1/(2π fc) ; deux PT1 à 150 Hz ≈ 2,1 ms + notch
    assert 2.0 < d.group_ms < 3.5
    assert d.phase_ms == pytest.approx(d.group_ms, rel=0.05)
    d1 = bf.chain_delay_ms([bf.FilterSpec("pt1", 100.0)], 8000.0, f_hz=1.0)
    assert d1.group_ms == pytest.approx(1e3 / (2 * np.pi * 100), rel=0.1)


def test_simulation_static_matches_lfilter():
    from scipy.signal import lfilter

    fs = 2000.0
    rng = np.random.default_rng(0)
    x = rng.normal(size=(3, 4000))
    x[:, 0] = 0.0  # état initial nul identique à lfilter
    spec = bf.FilterSpec("pt2", 150.0)
    y = bf._apply(x.copy(), "pt2", 150.0, fs)
    b, a = bf.coefficients(spec, fs)
    assert np.allclose(y, lfilter(b, a, x, axis=1), atol=1e-9)
    spec = bf.FilterSpec("notch", 300.0, 4.0, weight=0.6)
    y = bf._apply(x.copy(), "notch", 300.0, fs, 4.0, 0.6)
    b, a = bf.coefficients(spec, fs)
    assert np.allclose(y, lfilter(b, a, x, axis=1), atol=1e-9)


# ---------------------------------------------------------------- logs réels (corpus)


@pytest.mark.corpus
@pytest.mark.skipif(not HOVER.exists(), reason="log Apri/btfl_008 absent")
def test_simulate_gyro_matches_log():
    from opentunebox.log.flightlog import load

    lg = load(HOVER)[0]
    sim = bf.simulate_gyro(lg)
    assert sim.shape == lg.gyro.shape
    corr = [np.corrcoef(sim[ax], lg.gyro[ax])[0, 1] for ax in range(3)]
    raw = [np.corrcoef(lg.gyro_unfilt[ax], lg.gyro[ax])[0, 1] for ax in range(3)]
    assert corr[0] > 0.9  # mesuré : 0,962 roulis, 0,898 tangage, 0,818 lacet (D : 0,956)
    assert all(c > r for c, r in zip(corr, raw))
    dsim = bf.simulate_dterm(lg)
    assert np.corrcoef(dsim[0], lg.d_term[0])[0, 1] > 0.9


def test_scale_and_disable_lowpass():
    from opentunebox.analysis.bf_filters import lowpass_off, scale_lowpass
    from opentunebox.log.settings import LowpassSetting

    lp = LowpassSetting("PT1", 0, 250, 500)
    assert scale_lowpass(lp, 1.25) == LowpassSetting("PT1", 0, 312, 625)
    # coupure plafonnée à 1000 Hz ; un filtre coupé reste coupé (comme le curseur du firmware)
    assert scale_lowpass(LowpassSetting("PT1", 900), 1.5).static_hz == 1000
    assert not scale_lowpass(LowpassSetting("PT1"), 2).enabled
    assert not lowpass_off(lp).enabled


def _fake_log(n=20000, fs=2000.0, motor_hz=200.0, **extra):
    """Log minimal pour `whatif_spectrum` : gaz constant, moteurs à `motor_hz`, pas de debug FFT_FREQ."""
    from types import SimpleNamespace

    return SimpleNamespace(fs=fs, n=n, throttle=np.full(n, 50.0), motor_hz=np.full((4, n), motor_hz),
                           debug=np.zeros((0, n)), settings=_settings(rpm_filter_weights="100,100,100", **extra))


def test_whatif_spectrum_identity_when_nothing_changes():
    """Méthode relative (D-008) : avec les mêmes réglages, le spectre « modifié » est exactement le spectre
    enregistré, même sur les cases qui tombent pile sur un notch (|H|² ≈ 0 des deux côtés)."""
    rng = np.random.default_rng(0)
    log = _fake_log()
    t = np.arange(log.n) / log.fs
    # raie à 200 Hz = centre d'un notch RPM et multiple exact du pas de fréquence (fs / 600 = 3,33 Hz)
    x = rng.normal(0, 1, log.n) + 0.5 * np.sin(2 * np.pi * 200.0 * t)
    w = bf.whatif_spectrum(x, None, log, slice(None), log.settings, log.settings)
    assert w.freqs.size and np.all(np.isfinite(w.current_db))
    assert np.allclose(w.modified_db, w.current_db, atol=1e-9)


def test_whatif_spectrum_follows_filter_change():
    """LPF2 du gyro abaissé de 500 à 250 Hz : l'écart de spectre suit |H250|² / |H500|² à 0,1 dB près."""
    rng = np.random.default_rng(1)
    log = _fake_log()
    mod = _settings(rpm_filter_weights="100,100,100", gyro_lpf2_static_hz="250")
    x = rng.normal(0, 1, log.n)
    w = bf.whatif_spectrum(x, None, log, slice(None), log.settings, mod)
    f = w.freqs
    h250 = bf.chain_response([(bf.FilterSpec("pt1", 250.0), 8000.0)], f)
    h500 = bf.chain_response([(bf.FilterSpec("pt1", 500.0), 8000.0)], f)
    expected = 20 * np.log10(np.abs(h250) / np.abs(h500))
    band = (f > 50) & (f < 400) & (np.abs(f - 200) > 10) & (np.abs(f - 400) > 10)  # loin des notchs RPM
    assert np.allclose((w.modified_db - w.current_db)[band], expected[band], atol=0.1)
