"""Tests des analyses sur signaux synthétiques (réponses connues) et recoupements PIDtoolbox."""


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
    r = dyn_notch_targets(x, mhz, FS, (100, 600), rpm_harmonics=3)
    assert r.freqs.size == 1 and abs(r.freqs[0] - 330) < 15 and r.ratios_db[0] > 3
    # sans filtre RPM, le notch dynamique doit aussi suivre la raie moteur
    assert dyn_notch_targets(x, mhz, FS, (100, 600)).freqs.size >= 2


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
