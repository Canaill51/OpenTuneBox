"""Tests des analyses sur signaux synthétiques (réponses connues) et recoupements PIDtoolbox."""


from types import SimpleNamespace

import numpy as np
import pytest
from scipy import signal

from opentunebox.analysis.delay import phase_delay_ms, xcorr_delay_ms
from opentunebox.analysis.spectral import freq_throttle, freq_time, spectrum
from opentunebox.analysis.loop import loop_from_T
from opentunebox.analysis.step import step_metrics, step_response

FS = 2000.0
from corpus_paths import LOGS  # noqa: E402


def stick_input(seconds=60, seed=1):
    rng = np.random.default_rng(seed)
    n = int(seconds * FS)
    sp = np.zeros(n)
    i = 0
    while i < n:
        d = int(rng.uniform(0.05, 0.6) * FS)
        sp[i : i + d] = rng.choice([0, 0, 1, -1]) * rng.uniform(50, 600)
        i += d
    return signal.lfilter(*signal.butter(2, 30 / (FS / 2)), sp), rng


def second_order(zeta=0.59, f_n=12.0):
    wn = 2 * np.pi * f_n
    return signal.bilinear([wn**2], [1, 2 * zeta * wn, wn**2], FS)


def test_spectrum_finds_sine():
    t = np.arange(int(10 * FS)) / FS
    x = 3.0 * np.sin(2 * np.pi * 180 * t) + np.random.default_rng(0).normal(0, 0.1, t.size)
    s = spectrum(x, FS, mode="amp")
    k = int(np.argmax(s.values))
    assert abs(s.freqs[k] - 180) <= s.resolution_hz
    assert s.values[k] == pytest.approx(3.0, rel=0.1)


def test_band_peak_on_ptb_scale():
    from opentunebox.advisor.rules import band_peak_ptb_db
    from opentunebox.analysis.spectral import PTB_PSD_OFFSET_DB

    # bruit blanc d'écart-type σ : DSP unilatérale 2σ²/fs, plate ; le sommet lissé dépasse à peine ce niveau
    sigma = 3.0
    x = np.random.default_rng(1).normal(0, sigma, int(20 * FS))
    level = 10 * np.log10(2 * sigma**2 / FS) - PTB_PSD_OFFSET_DB
    assert level < band_peak_ptb_db(x, FS, 80, 500) < level + 2.5


def test_harmonic_prominence_and_rpm_weight():
    from opentunebox.analysis.motors import harmonic_prominence, rpm_weight_needed

    # 4 moteurs vers 200 Hz (fréquence variable), raies 1 et 3 seulement, dans un bruit blanc
    n = int(20 * FS)
    t = np.arange(n) / FS
    base = 200 + 30 * np.sin(2 * np.pi * 0.1 * t)
    mhz = np.vstack([base * (1 + d) for d in (-0.02, -0.01, 0.01, 0.02)])
    x = np.random.default_rng(0).normal(0, 1, n)
    for h, a in ((1, 2.0), (3, 1.0)):
        for m in mhz:
            x += a / 2 * np.sin(2 * np.pi * np.cumsum(h * m) / FS)
    p = harmonic_prominence(x, mhz, FS)
    assert p[0] > 15 and abs(p[1]) < 2 and 8 < p[2] < p[0]
    # firmware : il reste (1 − poids) de la raie → 20 dB demandent un poids de 0,9 ; pas de raie, pas de filtre
    assert rpm_weight_needed(20.0) == pytest.approx(0.9)
    assert rpm_weight_needed(-1.0) == 0.0


def _rpm_settings(weights=(100, 100, 100)):
    return SimpleNamespace(rpm_q=500, rpm_harmonics=3, rpm_weights=list(weights), rpm_min_hz=100, rpm_fade_hz=50,
                           pid_rate_hz=8000.0)


def test_dyn_notch_targets_skip_motor_lines():
    from opentunebox.analysis.motors import dyn_notch_targets

    # raie moteur variable (traitée par le filtre RPM) + résonance fixe à 330 Hz : seule la seconde reste
    n = int(20 * FS)
    t = np.arange(n) / FS
    base = 200 + 30 * np.sin(2 * np.pi * 0.1 * t)
    mhz = np.vstack([base * (1 + d) for d in (-0.02, 0.0, 0.02)])
    x = np.random.default_rng(0).normal(0, 1, n) + 1.0 * np.sin(2 * np.pi * 330 * t)
    for m in mhz:
        x += 2.0 * np.sin(2 * np.pi * np.cumsum(m) / FS)
    r = dyn_notch_targets(x, mhz, FS, (100, 600), _rpm_settings())
    assert abs(r.freqs[0] - 330) < 15 and r.ratios_db[0] > 10  # résonance = pic le plus fort
    assert not any(160 < f < 240 for f in r.freqs)  # raies moteur effacées
    # (sur un bruit blanc, les creux des notchs RPM abaissent la moyenne de la plage, comme dans le firmware,
    # dyn_notch_filter.c : quelques cases de bruit passent le seuil)
    # sans filtre RPM, le notch dynamique doit aussi suivre la raie moteur
    assert dyn_notch_targets(x, mhz, FS, (100, 600)).freqs.size >= 2


def test_dyn_notch_targets_follow_rpm_weights():
    """Le notch dynamique suit ce que le filtre RPM laisse passer : fondamentale à poids 0 → raie moteur à suivre
    (le firmware la suit, debug FFT_FREQ du corpus) ; poids 100 → effacée."""
    from opentunebox.analysis.motors import dyn_notch_targets

    n = int(20 * FS)
    t = np.arange(n) / FS
    mhz = np.vstack([np.full(n, 250.0 * (1 + d)) for d in (-0.01, 0.0, 0.01)])
    x = np.random.default_rng(1).normal(0, 1, n)
    for m in mhz:
        x += 2.0 * np.sin(2 * np.pi * m * t)
    near = lambda r: any(abs(f - 250) < 15 for f in r.freqs)  # noqa: E731
    assert near(dyn_notch_targets(x, mhz, FS, (100, 600), _rpm_settings((0, 100, 100))))
    assert not near(dyn_notch_targets(x, mhz, FS, (100, 600), _rpm_settings()))


def test_line_shapes_measure_rpm_leftovers():
    """Forme d'une raie que le masque des harmoniques cache : fondamentale à poids RPM 0 (non traitée, suivie par
    le notch dynamique) → largeur inconnue avec le masque seul, mesurée sur le spectre après filtre RPM."""
    from opentunebox.analysis.motors import line_shapes

    n = int(20 * FS)
    t = np.arange(n) / FS
    mhz = np.vstack([np.full(n, 250.0 * (1 + d)) for d in (-0.01, 0.0, 0.01)])
    x = np.random.default_rng(2).normal(0, 1, n)
    for m in mhz:
        x += 2.0 * np.sin(2 * np.pi * m * t)
    assert not np.isfinite(line_shapes(x, mhz, FS, [250.0], 3)[0][0])
    w, h = line_shapes(x, mhz, FS, [250.0], 3, rpm_settings=_rpm_settings((0, 100, 100)))[0]
    assert 0 < w < 30 and h > 10


def test_stick_movement_criterion():
    from types import SimpleNamespace

    from opentunebox.analysis.delay import MOVING_SETPOINT_DPS, stick_p95_dps

    # stationnaire : consigne de quelques °/s ; vol : flips à 300 °/s 10 % du temps
    rng = np.random.default_rng(0)
    hover = rng.normal(0, 5, (3, 4000))
    flight = hover.copy()
    flight[0, :400] = 300.0
    p = lambda sp: stick_p95_dps(SimpleNamespace(setpoint=sp), slice(None))  # noqa: E731
    assert p(hover) < MOVING_SETPOINT_DPS <= p(flight)
    # wobbles du protocole en mode Angle limité à 30° : consigne plafonnée vers 47 °/s, vol avec mouvements (D-026)
    wobble = hover.copy()
    wobble[0] = 49.0 * np.sin(2 * np.pi * np.arange(4000) / 400)
    assert 45 < p(wobble) < 50 and p(wobble) >= MOVING_SETPOINT_DPS


def test_spectrum_lines_hover():
    from opentunebox.analysis.motors import spectrum_lines

    # stationnaire : moteurs vers 190 Hz (raies 1 à 3), résonance à 330 Hz que le « filtre » laisse à moitié
    n = int(20 * FS)
    t = np.arange(n) / FS
    rng = np.random.default_rng(2)
    mhz = np.vstack([np.full(n, 190.0 * (1 + d)) for d in (-0.01, 0.0, 0.01)])
    raw = rng.normal(0, 1, n) + 1.0 * np.sin(2 * np.pi * 330 * t)
    for h in (1, 2, 3):
        raw += 1.0 * np.sin(2 * np.pi * h * 190 * t)
    filt = rng.normal(0, 1, n) + 0.3 * np.sin(2 * np.pi * 330 * t)
    lines = spectrum_lines(raw, filt, mhz, FS, (20, 900))
    assert len(lines) == 1 and abs(lines[0].freq_hz - 330) < 5
    assert lines[0].raw_db > lines[0].left_db > 3


def test_freq_throttle_tracks_motor_line():
    # raie dont la fréquence suit le gaz : f = 100 + 4 × gaz
    n = int(60 * FS)
    thr = np.linspace(5, 95, n)
    phase = 2 * np.pi * np.cumsum(100 + 4 * thr) / FS
    x = np.sin(phase)
    ft = freq_throttle(x, thr, FS, mode="amp")
    for g in (20, 50, 80):
        col = ft.z[:, g - 1]
        assert abs(ft.freqs[int(np.nanargmax(col))] - (100 + 4 * ft.bins[g - 1])) < 12


def test_freq_time_shape():
    x = np.random.default_rng(0).normal(size=int(5 * FS))
    sg = freq_time(x, np.arange(x.size) / FS, FS)
    assert sg.z.shape == (sg.freqs.size, sg.times.size)


def test_delays_pt1():
    rng = np.random.default_rng(0)
    x = signal.detrend(np.cumsum(rng.normal(size=60000))) + rng.normal(size=60000) * 5
    k = 2 * np.pi * 100 / FS
    k = k / (k + 1)
    y = signal.lfilter([k], [1, -(1 - k)], x)
    # retard de phase moyen d'un PT1 100 Hz sur 10–100 Hz ≈ 1,4 ms
    assert phase_delay_ms(x, y, FS) == pytest.approx(1.4, abs=0.15)
    # retard pur de 7 échantillons
    assert xcorr_delay_ms(x, np.roll(x, 7), FS) == pytest.approx(3.5, abs=0.1)


def test_step_response_second_order():
    sp, rng = stick_input()
    b, a = second_order()
    gy = signal.lfilter(b, a, sp)
    gy = np.concatenate([np.zeros(10), gy[:-10]]) + rng.normal(0, 2, sp.size)
    r = step_response(sp, gy, FS)
    t = np.arange(r.t_ms.size) / FS * 1000
    truth = np.concatenate([np.zeros(10), signal.lfilter(b, a, np.ones(2000))])[: t.size]
    assert r.n > 100
    assert np.max(np.abs(r.mean - truth)) < 0.03
    tm = step_metrics(t, truth)
    assert r.metrics.peak == pytest.approx(tm.peak, abs=0.02)
    assert r.metrics.latency_ms == pytest.approx(tm.latency_ms, abs=0.5)
    f = np.linspace(0, 150, 3001)
    _, h = signal.freqz(b, a, worN=f, fs=FS)
    lt = loop_from_T(f, h * np.exp(-2j * np.pi * f * 0.005), np.ones_like(f))
    assert r.loop.phase_margin_deg == pytest.approx(lt.phase_margin_deg, abs=4)
    assert r.loop.crossover_hz == pytest.approx(lt.crossover_hz, abs=0.6)
    assert r.loop.gain_margin_db == pytest.approx(lt.gain_margin_db, abs=1.0)
    assert r.loop.sensitivity_peak_db == pytest.approx(lt.sensitivity_peak_db, abs=0.5)
    assert r.loop.bandwidth_hz == pytest.approx(lt.bandwidth_hz, rel=0.08)


def _shape_of(b, a, seed=1):
    sp, rng = stick_input(seed=seed)
    gy = signal.lfilter(b, a, sp)
    gy = np.concatenate([np.zeros(10), gy[:-10]]) + rng.normal(0, 2, sp.size)
    return step_response(sp, gy, FS).metrics


def test_step_ripple_and_waves():
    """Cassure (« ripple ») : montée rapide jusqu'à 88 %, puis le reste lentement ; vagues : mode peu amorti."""
    # réponse réglée avec un léger dépassement : pas de cassure (valeur exacte −0,033 ; une réponse plus
    # amortie s'approche lentement de 1 et donne une petite cassure, +0,028 pour ζ = 0,9)
    clean = _shape_of(*second_order(zeta=0.7, f_n=14.0))
    assert clean.ripple == pytest.approx(-0.033, abs=0.02)
    assert clean.waves_ratio < 2.0
    # 0,87 × second ordre rapide + 0,13 × premier ordre lent (τ = 60 ms)
    bf, af = second_order(zeta=0.9, f_n=22.0)
    bs, as_ = signal.bilinear([1.0], [0.060, 1.0], FS)
    b = 0.87 * np.convolve(bf, as_) + 0.13 * np.convolve(bs, af)
    a = np.convolve(af, as_)
    shelf = _shape_of(b, a)
    assert shelf.ripple > 0.05
    assert shelf.ripple_std < 0.03
    assert 15 < shelf.ripple_ms < 40
    # 0,9 × réponse propre + 0,1 × mode à 45 Hz très peu amorti : vagues sur le palier
    bw, aw = second_order(zeta=0.03, f_n=45.0)
    bc, ac = second_order(zeta=0.9, f_n=14.0)
    b = 0.9 * np.convolve(bc, aw) + 0.1 * np.convolve(bw, ac)
    wavy = _shape_of(b, np.convolve(ac, aw))
    assert wavy.waves_ratio > 3.0
    assert wavy.waves > 3 * clean.waves


@pytest.mark.corpus
@pytest.mark.skipif(not (LOGS / "1. PTB/BlackSummer").exists(), reason="corpus absent")
def test_matches_ptb_damping_sweep():
    """Recoupement avec la capture PTB_FIGS_2026-08-16-1 (pics roll 1,23 / 1,11 / 1,00 / 0,98)."""
    from opentunebox.log.flightlog import load

    ptb = {"Damping 0.8.BFL": 1.23, "Damping 1.0.BFL": 1.11, "Damping 1.2.BFL": 1.00, "Damping 1.4.BFL": 0.98}
    peaks = []
    for name, ref in ptb.items():
        lg = load(LOGS / "1. PTB/BlackSummer" / name)[0]
        r = step_response(lg.setpoint[0], lg.gyro[0], lg.fs)
        assert r.metrics.peak == pytest.approx(ref, abs=0.07)
        peaks.append(r.metrics.peak)
    assert peaks[0] > peaks[1] > peaks[2]  # plus de D → moins de dépassement
