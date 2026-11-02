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

    return SimpleNamespace(fs=fs, n=n, breaks=lambda sel: np.zeros(0, dtype=int), throttle=np.full(n, 50.0), motor_hz=np.full((4, n), motor_hz),
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


# ---------------------------------------------------------------- notchs dynamiques ajoutés (D-041)


def test_complete_dyn_centres_places_added_notches_on_the_highest_peaks():
    """Notchs ajoutés : les pics les plus hauts hors voisinage (± 1,5 case de 18,5 Hz) d'un centre déjà pris, dans la plage ;
    sans pic, le centre initial du firmware. Centres déjà enregistrés d'abord, ramenés dans la plage."""
    s = _settings(dyn_notch_count="3", dyn_notch_min_hz="100", dyn_notch_max_hz="600")
    init = bf.dyn_notch_initial_centres(s)  # 183,3 ; 350 ; 516,7
    assert bf.complete_dyn_centres([200.0], s, [407.0]) == [200.0, 407.0, init[2]]
    assert bf.complete_dyn_centres([200.0], s, [210.0, 407.0]) == [200.0, 407.0, init[2]]  # 210 : à moins de 27,75 Hz de 200
    assert bf.complete_dyn_centres([200.0], s, [650.0, 90.0, 407.0]) == [200.0, 407.0, init[2]]  # hors plage
    assert bf.complete_dyn_centres([200.0], s, [300.0, 407.0, 480.0]) == [200.0, 300.0, 407.0]  # les plus hauts d'abord
    assert bf.complete_dyn_centres([200.0], s, []) == [200.0, init[1], init[2]]  # sans pic : comportement du firmware
    assert bf.complete_dyn_centres([50.0], s, [407.0]) == [100.0, 407.0, init[2]]  # centre enregistré ramené dans la plage
    assert bf.complete_dyn_centres([200.0, 300.0, 400.0], _settings(dyn_notch_count="2"), [500.0]) == [200.0, 300.0]


def _log_with_one_recorded_notch(line_hz=400.0, n=20000, fs=2000.0):
    """Log sans corpus : un seul notch dynamique enregistré (centre 200 Hz, debug FFT_FREQ 2025.12), gyro non filtré avec une raie."""
    from types import SimpleNamespace

    rng = np.random.default_rng(3)
    t = np.arange(n) / fs
    unfilt = np.vstack([rng.normal(0, 1, n) + 3.0 * np.sin(2 * np.pi * line_hz * t) for _ in range(3)])
    s = _settings(rpm_filter_weights="100,100,100", dyn_notch_count="1", debug_mode="16")
    return SimpleNamespace(fs=fs, n=n, breaks=lambda sel: np.zeros(0, dtype=int), throttle=np.full(n, 50.0),
                           motor_hz=np.full((4, n), 90.0), debug=np.vstack([np.zeros(n), np.full(n, 200.0)]),
                           gyro_unfilt=unfilt, settings=s), unfilt[0]


def test_whatif_added_dyn_notch_lands_on_the_peak_not_on_the_initial_centre():
    """1 notch enregistré (200 Hz) monté à 3, raie à 400 Hz : le creux le plus profond entre 300 et 450 Hz est sur la raie, et non
    à 350 Hz (centre initial, creux sans raie du modèle précédent)."""
    log, x = _log_with_one_recorded_notch()
    mod = _settings(rpm_filter_weights="100,100,100", dyn_notch_count="3", debug_mode="16")
    w = bf.whatif_spectrum(x, None, log, slice(None), log.settings, mod)
    diff = w.modified_db - w.current_db
    zone = (w.freqs >= 300) & (w.freqs <= 450)
    deepest = float(w.freqs[zone][np.argmin(diff[zone])])
    assert abs(deepest - 400.0) <= 20.0, deepest


def test_whatif_without_unfiltered_gyro_keeps_the_initial_centres():
    """Sans gyro non filtré, aucun pic à prévoir : les notchs ajoutés restent aux centres initiaux (comme avant D-041)."""
    log, x = _log_with_one_recorded_notch()
    log.gyro_unfilt = None
    mod = _settings(rpm_filter_weights="100,100,100", dyn_notch_count="3", debug_mode="16")
    w = bf.whatif_spectrum(x, None, log, slice(None), log.settings, mod)
    diff = w.modified_db - w.current_db
    zone = (w.freqs >= 300) & (w.freqs <= 450)
    assert abs(float(w.freqs[zone][np.argmin(diff[zone])]) - 350.0) <= 10.0


# ---------------------------------------------------------------- simulation temporelle (sans corpus)


@pytest.mark.parametrize("kind", ["pt1", "pt3", "biquad_lpf"])
def test_simulation_every_lowpass_kind_matches_lfilter(kind):
    """PT1, PT3 et biquad : la forme directe I à coefficients constants reproduit `lfilter` sur les mêmes (b, a)."""
    from scipy.signal import lfilter

    fs = 4000.0
    x = np.random.default_rng(2).normal(size=(2, 3000))
    x[:, 0] = 0.0
    b, a = bf.coefficients(bf.FilterSpec(kind, 120.0), fs)
    assert np.allclose(bf._apply(x.copy(), kind, 120.0, fs), lfilter(b, a, x, axis=1), atol=1e-9)


def test_simulation_filter_beyond_log_rate_is_transparent():
    """Coupure ≥ 0,48 × cadence du log (composante repliée de toute façon) ou nulle : le signal passe tel quel."""
    fs = 2000.0
    x = np.random.default_rng(0).normal(size=(1, 500))
    assert np.array_equal(bf._apply(x.copy(), "pt1", 0.49 * fs, fs), x)
    assert np.array_equal(bf._apply(x.copy(), "notch", 0.0, fs, 5.0), x)
    assert not np.array_equal(bf._apply(x.copy(), "pt1", 0.47 * fs, fs), x)


def test_simulation_initial_state_is_steady():
    """État initial = régime permanent de la première valeur : une constante ressort inchangée dès le 1er point."""
    x = np.full((1, 200), 37.0)
    for kind in ("pt1", "pt2", "biquad_lpf"):
        assert np.allclose(bf._apply(x.copy(), kind, 50.0, 2000.0), 37.0, atol=1e-9)
    assert np.allclose(bf._apply(x.copy(), "notch", 200.0, 2000.0, 3.0), 37.0, atol=1e-9)


def _band_power(x, fs, lo, hi):
    f = np.fft.rfftfreq(x.size, 1 / fs)
    p = np.abs(np.fft.rfft(x * np.hanning(x.size))) ** 2
    return float(p[(f >= lo) & (f <= hi)].sum())


def test_simulation_variable_notch_tracks_a_moving_line():
    """Notch dont la fréquence change à chaque échantillon (filtre RPM) : il efface une raie qui glisse de 150 à
    350 Hz, qu'un notch fixe à 250 Hz laisse presque intacte."""
    fs, n = 2000.0, 20000
    f_inst = np.linspace(150.0, 350.0, n)
    line = np.sin(2 * np.pi * np.cumsum(f_inst) / fs)
    x = (line + 0.05 * np.random.default_rng(4).normal(size=n))[None, :]
    before = _band_power(x[0], fs, 140, 360)
    tracked = bf._apply(x.copy(), "notch", f_inst, fs, 5.0, 1.0)
    fixed = bf._apply(x.copy(), "notch", 250.0, fs, 5.0, 1.0)
    assert 10 * np.log10(before / _band_power(tracked[0], fs, 140, 360)) > 25
    assert 10 * np.log10(before / _band_power(fixed[0], fs, 140, 360)) < 6  # mesuré : 3,7 dB
    # poids 0 : notch transparent même s'il suit la raie
    assert np.allclose(bf._apply(x.copy(), "notch", f_inst, fs, 5.0, 0.0), x)
    assert np.allclose(bf._apply(x.copy(), "notch", f_inst, fs, 5.0, np.zeros(n)), x)


def test_pt1_track_matches_lfilter():
    from opentunebox import _native
    from scipy.signal import lfilter

    k = bf.pt1_gain(150.0, 2000.0)
    x = np.concatenate([np.zeros(10), np.full(200, 100.0)])
    y = _native.pt1_track(x, k)
    assert np.allclose(y, lfilter([k], [1.0, -(1.0 - k)], x), atol=1e-9)
    assert y[-1] == pytest.approx(100.0, rel=1e-6) and 0 < y[11] < 100


def test_static_notch_clamped_under_nyquist():
    """Centre au-dessus de Nyquist : ramené à Nyquist si la coupure y reste sous, sinon le notch disparaît
    (gyro_init.c) ; coupure nulle ou ≥ centre : pas de notch."""
    assert bf._static_notch(300, 200, 1000.0) == bf.FilterSpec("notch", 300.0, bf.notch_q(300, 200))
    assert bf._static_notch(1500, 200, 1000.0).f_hz == 1000.0
    assert bf._static_notch(1500, 1200, 1000.0) is None
    assert bf._static_notch(300, 0, 1000.0) is None and bf._static_notch(300, 300, 1000.0) is None


def test_dyn_notch_centres_and_chain_options():
    s = _settings(dyn_notch_count="3", dyn_notch_min_hz="100", dyn_notch_max_hz="400", dyn_notch_q="300")
    assert bf.dyn_notch_initial_centres(s) == [150.0, 250.0, 350.0]
    with_dyn = bf.gyro_chain(s, dyn_notch_hz=None)
    assert [sp.f_hz for sp, _ in with_dyn if sp.kind == "notch"] == [150.0, 250.0, 350.0]
    assert [sp.f_hz for sp, _ in bf.gyro_chain(s, dyn_notch_hz=[220.0]) if sp.kind == "notch"] == [220.0]
    assert not any(sp.kind == "notch" for sp, _ in bf.gyro_chain(s, dyn_notch_hz=[]))
    slow = _settings(dyn_notch_count="3", pid_process_denom="8")  # boucle PID 1 kHz : notch dynamique inactif
    assert not bf.dyn_notch_enabled(slow) and not any(sp.kind == "notch" for sp, _ in bf.gyro_chain(slow))


def _debug_log(version: str, debug_index: int, rows: int, count: int = 3):
    """Vol synthétique avec `rows` lignes de debug (valeur = 100 × numéro de ligne) et le mode de debug donné."""
    import dataclasses

    from synthetic import make_flightlog

    lg = make_flightlog(seconds=1.0, **{"Firmware revision": f"Betaflight {version} (abcd1234) STM32F7X2",
                                       "debug_mode": str(debug_index), "dyn_notch_count": str(count)})
    debug = np.vstack([np.full(lg.n, 100.0 * (k + 1)) for k in range(rows)]) if rows else np.zeros((0, lg.n))
    return dataclasses.replace(lg, debug=debug)


def test_fft_freq_centres_by_version():
    """FFT_FREQ = 17 en 4.x (debug[0..2], debug[3] = gyro perdu), 16 depuis 2025.12 (debug[1..count]) ; le même
    numéro 17 n'est plus FFT_FREQ en 2025.12 (L-003)."""
    from opentunebox.analysis.motors import dyn_notch_centres

    old = _debug_log("4.5.1", 17, rows=4, count=4)
    assert old.settings.debug_mode == "FFT_FREQ"
    c = bf.fft_freq_centres(old)
    assert c.shape == (4, old.n) and c[:, 0].tolist() == [100.0, 200.0, 300.0, 0.0]  # 4e centre perdu → 0
    assert dyn_notch_centres(old)[:, 0].tolist() == [100.0, 200.0, 300.0]  # ligne 3 écartée
    new = _debug_log("2025.12.0", 16, rows=4, count=3)
    assert new.settings.debug_mode == "FFT_FREQ"
    assert bf.fft_freq_centres(new)[:, 0].tolist() == [200.0, 300.0, 400.0]
    assert dyn_notch_centres(new)[:, 0].tolist() == [200.0, 300.0, 400.0]
    # moins de lignes de debug que de notchs : lignes manquantes à 0 (pas d'erreur d'indice)
    assert bf.fft_freq_centres(_debug_log("2025.12.0", 16, rows=2, count=3))[:, 0].tolist() == [200.0, 0.0, 0.0]
    assert bf.fft_freq_centres(_debug_log("2025.12.0", 17, rows=4)) is None  # 17 = RX_FRSKY_SPI en 2025.12
    assert bf.fft_freq_centres(_debug_log("4.5.1", 17, rows=4, count=0)) is None
    assert dyn_notch_centres(_debug_log("4.5.1", 0, rows=0)) is None


def test_simulate_gyro_on_synthetic_log_removes_motor_harmonic():
    """Le vol synthétique a, dans le gyro non filtré, une raie à 2 × fréquence moteur (≈ 440 Hz) : la chaîne du
    log (LPF2 500 Hz, filtre RPM 3 harmoniques) l'efface ; sans filtre RPM, elle reste."""
    import dataclasses

    from synthetic import make_flightlog

    lg = make_flightlog(seconds=6.0)
    sim = bf.simulate_gyro(lg)
    assert sim.shape == lg.gyro.shape and np.all(np.isfinite(sim))
    m = lg.motor_hz.mean()
    lo, hi = 2 * m * 0.8, 2 * m * 1.2
    drop = 10 * np.log10(_band_power(lg.gyro_unfilt[0], lg.fs, lo, hi) / _band_power(sim[0], lg.fs, lo, hi))
    assert drop > 10
    no_rpm = dataclasses.replace(lg.settings, rpm_harmonics=0)
    sim2 = bf.simulate_gyro(lg, no_rpm)
    drop2 = 10 * np.log10(_band_power(lg.gyro_unfilt[0], lg.fs, lo, hi) / _band_power(sim2[0], lg.fs, lo, hi))
    assert drop2 < drop - 5
    d = bf.simulate_dterm(lg, gyro=sim)
    assert d.shape == lg.gyro.shape and np.all(np.isfinite(d))
    with pytest.raises(ValueError):
        bf.simulate_gyro(make_flightlog(seconds=1.0, unfilt=False))


def test_chain_response_matches_polynomial_product():
    # produit des étages (noyau numba) = polynômes convolués de `coefficients`, cadences mélangées
    rng = np.random.default_rng(1)
    f = np.linspace(0.0, 1900.0, 400)
    for _ in range(50):
        chain = [(bf.FilterSpec(str(rng.choice(bf.FILTER_KINDS)), float(rng.uniform(20, 900)), float(rng.uniform(0.5, 6)),
                                float(rng.uniform(0, 1))), float(rng.choice([2000.0, 4000.0, 8000.0])))
                 for _ in range(int(rng.integers(1, 12)))]
        ref = np.ones(f.size, complex)
        for spec, fs in chain:
            b, a = bf.coefficients(spec, fs)
            z1 = np.exp(-2j * np.pi * f / fs)
            ref *= np.polyval(b[::-1], z1) / np.polyval(a[::-1], z1)
        assert np.allclose(bf.chain_response(chain, f), ref, rtol=1e-7, atol=1e-12)
    assert bf.chain_response([], [10.0, 20.0]) == pytest.approx([1.0, 1.0])


def test_exact_notch_centre_keeps_the_ratio_of_the_other_stages():
    # harmonique pile sur une case : le notch y vaut exactement 0 ; le rapport de deux chaînes qui ont le même
    # notch doit rester celui des autres étages, pas 0/0 (L-024)
    fs, f0 = 8000.0, 450.0
    f = np.array([f0])
    notch = (bf.FilterSpec("notch", f0, 5.0), fs)
    lpf = (bf.FilterSpec("pt1", 300.0), fs)
    cur = np.abs(bf.chain_response([notch, lpf], f)) ** 2
    new = np.abs(bf.chain_response([notch], f)) ** 2
    assert cur[0] > 0
    assert new[0] / cur[0] == pytest.approx(1 / np.abs(bf.chain_response([lpf], f)[0]) ** 2, rel=1e-9)


def test_rpm_attenuation_is_the_product_of_the_rpm_notches():
    from types import SimpleNamespace
    from opentunebox.analysis.motors import rpm_attenuation
    s = SimpleNamespace(rpm_q=500, pid_rate_hz=8000.0, rpm_harmonics=3, rpm_min_hz=100, rpm_fade_hz=50,
                        rpm_weights=[100, 0, 80])
    f = np.linspace(0, 1500, 300)
    mhz = np.array([[120.0, 300.0], [125.0, 310.0], [90.0, 305.0], [130.0, 290.0]])  # 4 moteurs, 2 fenêtres
    out = rpm_attenuation(f, mhz, s)
    for k in range(2):
        ref = np.ones(f.size)
        for m in mhz[:, k]:
            for h in range(3):
                fr, w = bf.rpm_notch_freq_weight(m, h, s, 8000.0)
                if w > 0:
                    ref *= np.abs(bf.freq_response([bf.FilterSpec("notch", float(fr), 5.0, float(w))], f, 8000.0))
        assert np.allclose(out[k], ref, rtol=1e-9)
