"""Parité des noyaux Rust `_native.rpm_response`, `harmonic_levels`, `masked_mean`, `fold_mean` et `numpy_mean`
(`rust/otb-analysis`) avec le code numpy d'origine de `analysis/motors.py`, gardé tel quel dans `tests/ref_motors.py`
(D-031, phase 3b). TDD : écrits avant les noyaux.

Les noyaux doivent donner des résultats IDENTIQUES AU BIT PRÈS (aucune tolérance) : une somme dans un autre ordre
changerait la dernière décimale d'un spectre, donc un seuil de l'assistant. Plan :
  A. `numpy_mean` (la somme par paires de numpy, brique des moyennes de `harmonic_levels`) ;
  B. `rpm_attenuation` ; C. `harmonic_levels` ; D. `masked_mean` ; E. `fold_mean` : chacun face à un oracle numpy
     écrit ici (mêmes boucles que l'étalon) sur des tableaux aléatoires, puis face à l'étalon sur des vols ;
  F. erreurs : tableaux non contigus, formes incohérentes, entrées absurdes (jamais de panique) ;
  G. intégration : `motors.*` branché sur les noyaux = `*_ref` (code numpy d'origine)."""

import dataclasses
import warnings
from pathlib import Path

import numpy as np
import pytest

_native = pytest.importorskip("opentunebox._native")

from opentunebox.analysis import motors  # noqa: E402
from opentunebox.analysis.bf_filters import _K_NOTCH, _sections_response, rpm_notch_freq_weight  # noqa: E402
from opentunebox.analysis.spectral import _seg_spectra, _segments, to_db  # noqa: E402
from opentunebox.log import flightlog  # noqa: E402
from ref_motors import (  # noqa: E402
    MOTOR_LINE_MARGIN,
    _masked_spectrum_ref,
    harmonic_prominence_ref,
    rpm_attenuation_ref,
    rpm_filtered_spectrum_ref,
)
from synthetic import make_flightlog  # noqa: E402

REAL = Path(__file__).resolve().parent / "data" / "jeno5_5s.bbl"
NAN = float("nan")


def c64(a):
    return np.ascontiguousarray(a, dtype=np.float64)


def cc(a):
    """Contigu, dtype conservé : `lo_m`, `hi_m`, `mean_m` suivent `log.motor_hz` (float32 sur un vrai log) et les
    noyaux reproduisent l'arithmétique dans cette précision (NumPy 2, NEP 50 : les scalaires Python s'y convertissent)."""
    return np.ascontiguousarray(a)


DTYPES = [np.float32, np.float64]  # float32 d'abord : c'est le cas des vrais logs


@pytest.fixture(params=DTYPES, ids=["f32", "f64"])
def dt(request):
    return request.param


def same(a, b, what=""):
    """Même dtype, même forme, mêmes valeurs au bit près (NaN égaux, signe du zéro compris)."""
    a, b = np.asarray(a), np.asarray(b)
    assert a.dtype == b.dtype and a.shape == b.shape, (what, a.dtype, b.dtype, a.shape, b.shape)
    assert np.array_equal(a, b, equal_nan=a.dtype.kind == "f"), what
    if a.dtype.kind == "f":
        assert np.array_equal(np.signbit(a), np.signbit(b)), what


# ----------------------------------------------------------------------------------------------------------
# Fabrication des entrées : mêmes découpages en fenêtres que l'étalon

def spectra(x, fs, seg_ms=300.0, breaks=None):
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop, breaks)
    if segs.shape[0] == 0:
        return L, hop, None, None
    f, P = _seg_spectra(segs, fs, "psd")
    return L, hop, f, P


def win_stat(series, L, hop, breaks, how):
    return getattr(_segments(series, L, hop, breaks)[0], how)(axis=1)


def build_fkwk(motor_hz, settings):
    """Fréquences et poids des étages, (fenêtres, moteurs × harmoniques), colonnes dans l'ordre de l'étalon :
    `fk = fr[:, :, k].T.ravel()` (moteur puis harmonique)."""
    rate, n_h = settings.pid_rate_hz, settings.rpm_harmonics
    m, nw = motor_hz.shape
    if n_h == 0:
        return np.zeros((nw, 0)), np.zeros((nw, 0))
    fw = [rpm_notch_freq_weight(motor_hz, h, settings, rate) for h in range(n_h)]
    fr = np.stack([x[0] for x in fw])
    w = np.broadcast_to(np.stack([x[1] for x in fw]), fr.shape)
    if nw == 0:
        return np.zeros((0, m * n_h)), np.zeros((0, m * n_h))
    fk = np.stack([fr[:, :, k].T.ravel() for k in range(nw)])
    wk = np.stack([w[:, :, k].T.ravel() for k in range(nw)])
    return c64(fk), c64(wk)


def rpm_att(*args):
    """|H| des notchs RPM : le noyau rend la réponse complexe, `np.abs` reste à numpy (SIMD, L-042)."""
    return np.abs(_native.rpm_response(*args))


def attenuation_native(f, motor_hz, settings):
    fk, wk = build_fkwk(motor_hz, settings)
    return rpm_att(c64(f), fk, wk, settings.rpm_q / 100.0, float(settings.pid_rate_hz))


def prominence_native(x, mhz, fs, n_harm=3, seg_ms=300.0, min_hz=20.0, breaks=None):
    """`harmonic_prominence` reconstruite sur `harmonic_levels` (assemblage Python : médiane de dB sur `used`)."""
    L, hop, f, P = spectra(x, fs, seg_ms, breaks)
    if f is None:
        return np.full(n_harm, np.nan)
    lo = win_stat(mhz.min(axis=0), L, hop, breaks, "mean")
    hi = win_stat(mhz.max(axis=0), L, hop, breaks, "mean")
    mean = win_stat(mhz.mean(axis=0), L, hop, breaks, "mean")
    line, floor, used = _native.harmonic_levels(c64(f), c64(P), cc(lo), cc(hi), cc(mean), n_harm, min_hz, float(fs))
    return assemble_prominence(line, floor, used)


def assemble_prominence(line, floor, used):
    out = []
    for h in range(line.shape[0]):
        u = used[h]
        vals = to_db(line[h][u]) - to_db(floor[h][u])
        out.append(float(np.median(vals)) if vals.size else np.nan)
    return np.array(out)


def masked_native(x, mhz, fs, n_harm, seg_ms, breaks=None):
    L, hop, f, P = spectra(x, fs, seg_ms, breaks)
    if f is None:
        return np.zeros(0), np.zeros(0)
    lo = win_stat(mhz.min(axis=0), L, hop, breaks, "min")
    hi = win_stat(mhz.max(axis=0), L, hop, breaks, "max")
    return f, _native.masked_mean(c64(f), c64(P), cc(lo), cc(hi), n_harm, MOTOR_LINE_MARGIN, float(fs))


def rpm_filtered_native(x, mhz, fs, settings, seg_ms=300.0, breaks=None):
    """`rpm_filtered_spectrum` (filtre RPM actif) reconstruite sur `rpm_attenuation` et `fold_mean`."""
    L, hop, f, P = spectra(x, fs, seg_ms, breaks)
    if f is None:
        return np.zeros(0), np.zeros(0)
    mean_m = np.vstack([_segments(np.asarray(m, dtype=np.float64), L, hop, breaks)[0].mean(axis=1) for m in mhz])
    P = P * attenuation_native(f, mean_m, settings) ** 2
    lo = win_stat(mhz.min(axis=0), L, hop, breaks, "min")
    hi = win_stat(mhz.max(axis=0), L, hop, breaks, "max")
    return f, _native.fold_mean(c64(f), c64(P), cc(lo), cc(hi), settings.rpm_harmonics, 2 * fs / L, float(fs))


# ----------------------------------------------------------------------------------------------------------
# Oracles numpy des noyaux (les boucles de l'étalon, appliquées à des tableaux donnés)

def levels_oracle(f, P, lo, hi, mean, n_harm, min_hz, fs):
    nw = P.shape[0]
    line_m, floor_m = np.full((n_harm, nw), np.nan), np.full((n_harm, nw), np.nan)
    used = np.zeros((n_harm, nw), dtype=bool)
    df = f[1] - f[0]
    for h in range(1, n_harm + 1):
        for k in range(nw):
            fc = h * mean[k]
            if fc < min_hz or 1.3 * fc > 0.95 * fs / 2:
                continue
            line = (f >= h * lo[k] - df) & (f <= h * hi[k] + df)
            floor = ((f >= 0.7 * fc) & (f <= 0.85 * fc)) | ((f >= 1.15 * fc) & (f <= 1.3 * fc))
            if line.any() and floor.sum() >= 2:
                line_m[h - 1, k], floor_m[h - 1, k], used[h - 1, k] = P[k, line].mean(), P[k, floor].mean(), True
    return line_m, floor_m, used


def masked_oracle(f, P, lo, hi, n_harm, margin, fs):
    P = P.copy()
    for k in range(P.shape[0]):
        for h in range(1, n_harm + 1):
            a, b = h * lo[k] - margin * lo[k], h * hi[k] + margin * hi[k]
            P[k, (f >= a) & (f <= b)] = np.nan
            for j in range(1, int(b // fs) + 2):
                P[k, (f >= j * fs - b) & (f <= j * fs - a)] = np.nan
                P[k, (f >= a - j * fs) & (f <= b - j * fs)] = np.nan
    with warnings.catch_warnings(), np.errstate(all="ignore"):
        warnings.simplefilter("ignore")
        return np.nanmean(P, axis=0)


def fold_oracle(f, P, lo, hi, n_harm, lobe, fs):
    P = P.copy()
    for k in range(P.shape[0]):
        for h in range(1, n_harm + 1):
            a, b = h * lo[k] - lobe, h * hi[k] + lobe
            P[k, (f >= fs - b) & (f <= fs - a)] = np.nan
    with warnings.catch_warnings(), np.errstate(all="ignore"):
        warnings.simplefilter("ignore")
        return np.nanmean(P, axis=0)


def attenuation_oracle(f, fk, wk, q, rate):
    out = np.ones((fk.shape[0], len(f)))
    for k in range(fk.shape[0]):
        on = wk[k] > 0
        n = int(on.sum())
        if n:
            out[k] = np.abs(_sections_response(np.full(n, _K_NOTCH, np.int64), fk[k][on], np.full(n, q), wk[k][on],
                                               np.full(n, float(rate)), f))
    return out


# ----------------------------------------------------------------------------------------------------------
# Données : tableaux aléatoires, vols synthétiques, vrai log

def random_spectra(dt, seed, nw=40, nf=120, fs=2000.0, hi_frac=0.9, nan_frac=0.0):
    """f régulière jusqu'à fs/2, P positive (log-normale, float64), moteurs lo <= mean <= hi répartis sur
    5 Hz…hi_frac·fs/2, rendus dans le dtype `dt` (valeurs « quelconques » : non représentables exactement)."""
    rng = np.random.default_rng(seed)
    f = np.linspace(0.0, fs / 2, nf)
    P = rng.lognormal(0.0, 1.5, (nw, nf))
    if nan_frac:
        P[rng.random(P.shape) < nan_frac] = np.nan
    mean = rng.uniform(5.0, hi_frac * fs / 2, nw)
    lo = mean * rng.uniform(0.7, 1.0, nw)
    hi = mean * rng.uniform(1.0, 1.3, nw)
    return f, P, lo.astype(dt), hi.astype(dt), mean.astype(dt)


SYNTH = {"rapide": dict(fs=2000.0, motor_hz=220.0), "lent": dict(fs=400.0, motor_hz=70.0),
         "tres_lent": dict(fs=150.0, motor_hz=40.0)}


@pytest.fixture(scope="module", params=[(n, d) for d in DTYPES for n in sorted(SYNTH)],
                ids=lambda v: f"{v[0]}-{v[1].__name__}")
def vol(request):
    """(x, mhz, fs) d'un vol synthétique : gyro axe 0 et fréquence des quatre moteurs (float32 ou float64)."""
    name, d = request.param
    p = SYNTH[name]
    log = make_flightlog(seconds=4.0, fs=p["fs"], motor_hz=p["motor_hz"], seed=3)
    return log.gyro[0].astype(np.float64), log.motor_hz.astype(d), float(log.fs)


@pytest.fixture(scope="module")
def real():
    logs = flightlog.load(REAL)
    assert logs
    return logs[0]


@pytest.fixture(scope="module", params=DTYPES, ids=["f32", "f64"])
def real_data(real, request):
    """(x, mhz, fs, breaks) du vrai log : `motor_hz` en float32 (SAMPLE_DTYPE) puis en float64."""
    return real.gyro[0].astype(np.float64), real.motor_hz.astype(request.param), float(real.fs), real.breaks()


def test_real_log_motor_hz_is_float32():
    """Garde-fou : le cas « vrai log » exerce bien l'arithmétique float32."""
    assert flightlog.load(REAL)[0].motor_hz.dtype == np.float32


# ----------------------------------------------------------------------------------------------------------
# A. numpy_mean

def bits_equal(got, want):
    if np.isnan(want):
        return bool(np.isnan(got))
    return np.float64(got).tobytes() == np.float64(want).tobytes()


@pytest.mark.parametrize("scale", [1e-8, 1.0, 1e8])
def test_numpy_mean_equals_numpy_for_all_lengths_0_to_2000(scale):
    """Ordre de sommation de numpy : bloc de 8 cumulateurs puis récursion par paires (au-delà de 128 valeurs)."""
    rng = np.random.default_rng(int(scale * 10) % 1000)
    big = rng.normal(0.0, scale, 2001) * np.exp(rng.normal(0, 3, 2001))  # échelles très mélangées
    bad = []
    for n in range(0, 2001):
        a = np.ascontiguousarray(big[:n])
        got = _native.numpy_mean(a)
        if n == 0:
            assert np.isnan(got)
        elif not bits_equal(got, a.mean()):
            bad.append(n)
    assert not bad, bad[:10]


def test_numpy_mean_is_not_a_naive_sum():
    """Garde-fou : sur ces données une somme séquentielle diffère de numpy, donc le test ci-dessus discrimine."""
    rng = np.random.default_rng(1)
    a = rng.normal(0, 1, 1000) * np.exp(rng.normal(0, 4, 1000))
    naive = 0.0
    for v in a:
        naive += v
    naive /= a.size
    assert not bits_equal(naive, a.mean())
    assert bits_equal(_native.numpy_mean(a), a.mean())


@pytest.mark.parametrize("n", [1, 7, 8, 9, 127, 128, 129, 1000])
def test_numpy_mean_on_unaligned_contiguous_view_equals_numpy(n):
    big = np.random.default_rng(n).normal(0, 1e3, n + 5)
    a = big[3:3 + n]
    assert a.flags.c_contiguous and a.ctypes.data % 16 != 0
    assert bits_equal(_native.numpy_mean(a), a.mean())


@pytest.mark.parametrize("values, expected", [([1.0, NAN, 3.0], "nan"), ([1.0, np.inf], "inf"), ([np.inf, -np.inf], "nan"),
                                              ([-0.0, -0.0], "zero"), ([5.0], "5"), ([], "nan")])
def test_numpy_mean_special_values(values, expected):
    a = np.array(values, dtype=np.float64)
    got = _native.numpy_mean(a)
    with warnings.catch_warnings(), np.errstate(all="ignore"):
        warnings.simplefilter("ignore")
        want = a.mean() if a.size else NAN
    if expected == "nan":
        assert np.isnan(got) and np.isnan(want)
    else:
        assert bits_equal(got, want)  # y compris le signe du zéro, tel que numpy le rend
        assert {"inf": got == np.inf, "zero": got == 0, "5": got == 5.0}[expected]


# ----------------------------------------------------------------------------------------------------------
# B. rpm_attenuation

def settings_variants(base):
    r = dataclasses.replace
    return {
        "reel": base,
        "poids_nul_premiere": r(base, rpm_weights=[0, 100, 100]),
        "poids_nul_dernieres": r(base, rpm_weights=[100, 0, 0]),
        "moins_de_poids_que_d_harmoniques": r(base, rpm_harmonics=3, rpm_weights=[80]),
        "plus_d_harmoniques_que_de_poids": r(base, rpm_harmonics=6, rpm_weights=[100, 50]),
        "une_harmonique": r(base, rpm_harmonics=1, rpm_weights=[100]),
        "q_serre": r(base, rpm_q=1200), "q_large": r(base, rpm_q=120),
        "plancher_haut_fondu_large": r(base, rpm_min_hz=300, rpm_fade_hz=200),
        "fondu_nul": r(base, rpm_fade_hz=0),
    }


@pytest.mark.parametrize("name", sorted(settings_variants(flightlog.parse_settings({}) if hasattr(flightlog, "parse_settings")
                                                          else make_flightlog().settings)))
def test_rpm_attenuation_matches_reference_on_real_motor_frequencies(name, real, real_data):
    settings = settings_variants(real.settings)[name]
    x, mhz, fs, breaks = real_data
    L, hop, f, _ = spectra(x, fs, breaks=breaks)
    mean_m = np.vstack([_segments(m, L, hop, breaks)[0].mean(axis=1) for m in mhz])
    got = attenuation_native(f, mean_m, settings)
    ref = rpm_attenuation_ref(f, mean_m, settings)
    same(got, ref, name)
    assert (ref < 1.0).any()  # le filtre agit : le test ne compare pas des 1.0 contre des 1.0


@pytest.mark.parametrize("seed", range(4))
def test_rpm_attenuation_random_motor_frequencies_match_reference(seed, real):
    """Fréquences moteur de 0 à bien au-delà de 0,48 × cadence (bornées par le firmware), dont des zéros."""
    rng = np.random.default_rng(seed)
    mhz = rng.uniform(0, 600, (4, 60))
    mhz[rng.random(mhz.shape) < 0.15] = 0.0
    mhz[0, :5] = 5000.0  # écrêté à 0,48 × cadence
    f = np.linspace(0, 1500, 700)
    got = attenuation_native(f, mhz, real.settings)
    ref = rpm_attenuation_ref(f, mhz, real.settings)
    same(got, ref)
    assert (ref < 1.0).any() and (ref == 1.0).any()


def test_rpm_attenuation_motors_at_0_hz_omit_every_stage():
    """Moteur à 0 Hz : fréquence ramenée au plancher, marge nulle, fondu 0 -> poids 0 -> étage omis -> ligne de 1."""
    s = make_flightlog().settings
    mhz = np.zeros((4, 5))
    f = np.linspace(0, 1000, 300)
    got = attenuation_native(f, mhz, s)
    same(got, rpm_attenuation_ref(f, mhz, s))
    assert (got == 1.0).all()
    mhz[:, 2] = 250.0  # une fenêtre en vol : seule celle-là est filtrée
    got = attenuation_native(f, mhz, s)
    same(got, rpm_attenuation_ref(f, mhz, s))
    assert (got[2] < 1.0).any() and (got[[0, 1, 3, 4]] == 1.0).all()


def test_rpm_attenuation_zero_windows_gives_empty_array_of_right_shape():
    s = make_flightlog().settings
    f = np.linspace(0, 1000, 50)
    got = attenuation_native(f, np.zeros((4, 0)), s)
    same(got, rpm_attenuation_ref(f, np.zeros((4, 0)), s))
    assert got.shape == (0, 50)


def test_rpm_attenuation_without_harmonics_gives_rows_of_ones():
    """Aucune colonne d'étage : chaque fenêtre vaut 1.0 (l'étalon ne gère pas ce cas, l'appelant l'évite)."""
    got = rpm_att(np.linspace(0, 100, 9), np.zeros((3, 0)), np.zeros((3, 0)), 5.0, 8000.0)
    assert got.shape == (3, 9) and (got == 1.0).all()


def test_rpm_attenuation_zero_frequencies_array_gives_empty_second_axis():
    got = rpm_att(np.zeros(0), np.full((2, 3), 200.0), np.ones((2, 3)), 5.0, 8000.0)
    assert got.shape == (2, 0)


def test_rpm_response_zero_weight_stage_is_omitted_even_with_nan_frequency():
    """Un notch de poids nul vaut exactement 1 : seul un étage gardé à tort par une fréquence NaN le trahit."""
    f = np.linspace(0, 1000, 50)
    fk = np.array([[300.0, np.nan, 500.0], [np.nan, np.nan, np.nan]])
    wk = np.array([[0.8, 0.0, 0.6], [0.0, 0.0, 0.0]])
    got = rpm_att(f, fk, wk, 5.0, 8000.0)
    assert np.isfinite(got).all() and (got[1] == 1.0).all() and (got[0] < 1.0).any()
    same(got, attenuation_oracle(f, fk, wk, 5.0, 8000.0))


@pytest.mark.parametrize("seed", range(5))
def test_rpm_attenuation_arbitrary_stage_columns_match_oracle(seed):
    """Étages quelconques, dont des poids nuls ou négatifs (omis), des fenêtres sans aucun étage, l'ordre des
    colonnes étant celui de la multiplication (au bit près, donc l'ordre compte)."""
    rng = np.random.default_rng(seed)
    nw, ns = 30, 12
    fk = rng.uniform(80, 1500, (nw, ns))
    wk = rng.uniform(0, 1, (nw, ns))
    wk[rng.random(wk.shape) < 0.3] = 0.0
    wk[rng.random(wk.shape) < 0.05] = -0.5
    wk[0] = 0.0  # fenêtre sans étage
    f = np.concatenate([[0.0], np.sort(rng.uniform(0, 2000, 300))])
    got = rpm_att(f, fk, wk, 4.0, 8000.0)
    want = attenuation_oracle(f, fk, wk, 4.0, 8000.0)
    same(got, want)
    assert (got[0] == 1.0).all() and (got[1:] < 1.0).any()


def test_rpm_response_follows_the_column_order_of_the_oracle():
    """Avant et après permutation des colonnes d'une fenêtre, le noyau égale l'oracle qui multiplie dans cet ordre."""
    rng = np.random.default_rng(9)
    fk, wk = rng.uniform(100, 900, (1, 24)), rng.uniform(0.2, 1, (1, 24))
    f = np.linspace(0, 1200, 500)
    p = rng.permutation(24)
    same(rpm_att(f, fk, wk, 5.0, 8000.0), attenuation_oracle(f, fk, wk, 5.0, 8000.0))
    same(rpm_att(f, fk[:, p], wk[:, p], 5.0, 8000.0),
         attenuation_oracle(f, fk[:, p], wk[:, p], 5.0, 8000.0))


# ----------------------------------------------------------------------------------------------------------
# C. harmonic_levels

def check_levels(f, P, lo, hi, mean, n_harm, min_hz, fs):
    got = _native.harmonic_levels(f, P, lo, hi, mean, n_harm, min_hz, fs)
    want = levels_oracle(f, P, lo, hi, mean, n_harm, min_hz, fs)
    assert len(got) == 3
    assert got[0].shape == got[1].shape == got[2].shape == (n_harm, P.shape[0])
    assert got[2].dtype == np.bool_
    same(got[0], want[0], "line_mean")
    same(got[1], want[1], "floor_mean")
    assert np.array_equal(got[2], want[2])
    return got


@pytest.mark.parametrize("fs", [2000.0, 400.0, 150.0])
@pytest.mark.parametrize("n_harm", [1, 3, 6])
@pytest.mark.parametrize("seed", range(3))
def test_harmonic_levels_random_spectra_match_oracle(seed, n_harm, fs, dt):
    f, P, lo, hi, mean = random_spectra(dt, seed, fs=fs)
    _, _, used = check_levels(f, P, lo, hi, mean, n_harm, 20.0, fs)
    assert used.any() and not used.all()  # fenêtres retenues ET écartées : les deux branches sont exercées


def test_harmonic_levels_outputs_are_nan_where_window_is_not_used(dt):
    f, P, lo, hi, mean = random_spectra(dt, 5)
    line, floor, used = check_levels(f, P, lo, hi, mean, 4, 20.0, 2000.0)
    assert np.isnan(line[~used]).all() and np.isnan(floor[~used]).all()
    assert np.isfinite(line[used]).all() and np.isfinite(floor[used]).all()


def test_harmonic_levels_nan_in_spectrum_keeps_window_used_with_nan_means(dt):
    """P contient des NaN : la fenêtre a passé les tests de l'étalon (`used`), mais la moyenne vaut NaN."""
    f, P, lo, hi, mean = random_spectra(dt, 6, nan_frac=0.0)
    P[3, :] = np.nan  # fenêtre entièrement NaN
    P[7, 10:60] = np.nan
    line, floor, used = check_levels(f, P, lo, hi, mean, 3, 20.0, 2000.0)
    assert (used & np.isnan(line)).any()  # utilisée mais NaN : distinct de « écartée »
    assert used[:, 3].any() and np.isnan(line[used[:, 3], 3]).all()


@pytest.mark.parametrize("seed", range(3))
def test_harmonic_levels_spectra_with_scattered_nan_match_oracle(seed, dt):
    f, P, lo, hi, mean = random_spectra(dt, seed, nan_frac=0.05)
    line, _, used = check_levels(f, P, lo, hi, mean, 3, 20.0, 2000.0)
    assert (used & np.isnan(line)).any() and (used & np.isfinite(line)).any()


def test_harmonic_levels_harmonic_beyond_nyquist_is_not_used(dt):
    f, P, lo, hi, _ = random_spectra(dt, 7, fs=400.0)
    mean = np.full(P.shape[0], 70.0, dtype=dt)  # harmonique 3 : 1,3 × 210 = 273 > 0,95 × 200
    _, _, used = check_levels(f, P, lo, hi, mean, 3, 20.0, 400.0)
    assert used[0].any() and not used[2].any()


def test_harmonic_levels_below_min_hz_is_not_used(dt):
    f = np.linspace(0, 1000.0, 2001)
    P = np.random.default_rng(8).lognormal(0, 1, (10, f.size))
    mean = lo = hi = np.full(10, 15.0, dtype=dt)  # raie et bruit de h = 2 (30 Hz) bien dans la grille
    _, _, used = check_levels(f, P, lo, hi, mean, 3, 20.0, 2000.0)
    assert not used[0].any() and used[1].any()  # h = 2 : 30 Hz >= 20


def test_harmonic_levels_limits_are_inclusive_exactly_as_reference(dt):
    """fc == min_hz est retenu (`fc < min_hz` strict) ; 1,3 × fc == 0,95 × fs/2 est retenu (`>` strict)."""
    f = np.linspace(0, 1000.0, 501)
    P = np.random.default_rng(0).lognormal(0, 1, (2, 501))
    lo = hi = np.array([100.0, 100.0], dtype=dt)
    mean = np.array([20.0, np.nextafter(dt(20.0), dt(0))], dtype=dt)
    lo, hi = lo.astype(dt), hi.astype(dt)
    _, _, used = check_levels(f, P, lo, hi, mean, 1, 20.0, 2000.0)
    assert used[0, 0] and not used[0, 1]
    fs = 2000.0
    edge = dt(0.95 * fs / 2 / 1.3)
    mean = np.array([edge, np.nextafter(edge, dt(1e9)), np.nextafter(edge, dt(0))], dtype=dt)
    lo, hi = np.full(2, 100.0, dtype=dt), np.full(2, 100.0, dtype=dt)
    P3 = np.random.default_rng(1).lognormal(0, 1, (3, 501))
    check_levels(f, P3, np.full(3, 100.0, dtype=dt), np.full(3, 100.0, dtype=dt), mean, 1, 20.0, fs)


def test_harmonic_levels_bounds_exactly_on_a_bin_follow_inclusive_comparisons(dt):
    """df = 2 Hz : `f >= lo - df` et `f <= hi + df` retiennent la case qui tombe pile sur la borne."""
    f = np.arange(0, 400, 2.0)
    P = np.random.default_rng(1).lognormal(0, 1, (3, f.size))
    lo, hi, mean = np.full(3, 50.0, dt), np.full(3, 60.0, dt), np.full(3, 55.0, dt)
    assert (f == 48.0).any() and (f == 62.0).any()  # bornes pile sur des cases
    line, _, used = check_levels(f, P, lo, hi, mean, 2, 20.0, 2000.0)
    assert used.all()
    assert line[0, 0] == P[0, (f >= 48) & (f <= 62)].mean()  # 48 et 62 inclus : 8 cases


def test_harmonic_levels_floor_with_fewer_than_two_bins_is_not_used(dt):
    f = np.arange(0, 1000, 25.0)  # grille grossière : la bande de bruit contient 0 ou 1 case
    P = np.random.default_rng(2).lognormal(0, 1, (6, f.size))
    mean = np.linspace(40.0, 90.0, 6).astype(dt)
    lo, hi = (mean * 0.95).astype(dt), (mean * 1.05).astype(dt)
    _, _, used = check_levels(f, P, lo, hi, mean, 1, 20.0, 2000.0)
    assert used.any() and not used.all()


def test_harmonic_levels_line_band_without_bin_is_not_used(dt):
    f = np.arange(0, 1000, 25.0)
    P = np.random.default_rng(3).lognormal(0, 1, (4, f.size))
    mean = np.array([100.0, 107.0, 112.0, 130.0], dtype=dt)  # raie étroite entre deux cases selon la valeur
    lo, hi = mean + dt(1.0), mean + dt(2.0)
    _, _, used = check_levels(f, P, lo, hi, mean, 1, 20.0, 2000.0)
    assert used.any() and not used.all()


def test_harmonic_levels_moteurs_a_0_hz_are_not_used(dt):
    f, P, _, _, _ = random_spectra(dt, 9)
    z = np.zeros(P.shape[0])
    _, _, used = check_levels(f, P, z, z, z, 3, 20.0, 2000.0)
    assert not used.any()


def test_harmonic_levels_nan_motor_frequencies_follow_nan_comparisons(dt):
    """mean NaN : les deux tests d'écart sont faux (NaN), mais les masques de bande sont vides -> pas utilisée ;
    lo/hi NaN avec mean valide : bande de la raie vide -> pas utilisée."""
    f, P, lo, hi, mean = random_spectra(dt, 10)
    mean[::4] = np.nan
    lo[1::4] = np.nan
    hi[2::4] = np.nan
    _, _, used = check_levels(f, P, lo, hi, mean, 3, 20.0, 2000.0)
    assert used.any() and not used[:, ::4].any() and not used[:, 1::4].any() and not used[:, 2::4].any()


def test_harmonic_levels_zero_windows_and_zero_harmonics(dt):
    f, P, lo, hi, mean = random_spectra(dt, 11)
    got = _native.harmonic_levels(f, P[:0], lo[:0], hi[:0], mean[:0], 3, 20.0, 2000.0)
    assert [g.shape for g in got] == [(3, 0)] * 3
    got = _native.harmonic_levels(f, P, lo, hi, mean, 0, 20.0, 2000.0)
    assert [g.shape for g in got] == [(0, P.shape[0])] * 3


def test_harmonic_levels_swapped_bounds_lo_above_hi_match_oracle(dt):
    f, P, lo, hi, mean = random_spectra(dt, 12)
    check_levels(f, P, hi, lo, mean, 3, 20.0, 2000.0)


def test_harmonic_levels_does_not_modify_its_inputs(dt):
    f, P, lo, hi, mean = random_spectra(dt, 13)
    keep = [a.copy() for a in (f, P, lo, hi, mean)]
    _native.harmonic_levels(f, P, lo, hi, mean, 3, 20.0, 2000.0)
    for a, b in zip((f, P, lo, hi, mean), keep):
        assert a.tobytes() == b.tobytes()


def assert_prominence_matches(x, mhz, fs, **kw):
    ref = harmonic_prominence_ref(x, mhz, fs, **kw)
    got = prominence_native(x, mhz, fs, **kw)
    same(got, ref)
    return ref


def test_harmonic_prominence_synthetic_flights_match_reference(vol):
    x, mhz, fs = vol
    ref = assert_prominence_matches(x, mhz, fs, n_harm=3)
    assert np.isfinite(ref).any()
    assert_prominence_matches(x, mhz, fs, n_harm=8)  # harmoniques au-delà de Nyquist : NaN des deux côtés
    assert_prominence_matches(x, mhz, fs, n_harm=3, min_hz=60.0, seg_ms=200.0)


def test_harmonic_prominence_has_finite_and_nan_harmonics_on_slow_flight(dt):
    p = SYNTH["lent"]
    log = make_flightlog(seconds=4.0, fs=p["fs"], motor_hz=p["motor_hz"], seed=3)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    ref = assert_prominence_matches(x, mhz, 400.0, n_harm=8)
    assert np.isfinite(ref).any() and np.isnan(ref).any()


def test_harmonic_prominence_real_log_matches_reference(real_data):
    x, mhz, fs, breaks = real_data
    ref = assert_prominence_matches(x, mhz, fs, n_harm=3, breaks=breaks)
    assert np.isfinite(ref).any()
    assert_prominence_matches(x, mhz, fs, n_harm=5, breaks=None)


@pytest.mark.parametrize("breaks", [np.array([1000]), np.array([700, 3000, 3100])])
def test_harmonic_prominence_with_breaks_matches_reference(breaks, dt):
    log = make_flightlog(seconds=4.0, fs=2000.0, seed=4)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    full = harmonic_prominence_ref(x, mhz, 2000.0)
    ref = assert_prominence_matches(x, mhz, 2000.0, breaks=breaks)
    assert np.isfinite(ref).any() and not np.array_equal(ref, full)  # les fenêtres écartées changent bien le résultat


def test_harmonic_prominence_nan_in_signal_propagates_like_reference(dt):
    """Fenêtre à spectre NaN : retenue par l'étalon, sa valeur NaN entre dans la médiane qui devient NaN."""
    log = make_flightlog(seconds=4.0, fs=2000.0, seed=5)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    x[1500:1510] = np.nan
    ref = assert_prominence_matches(x, mhz, 2000.0)
    assert np.isnan(ref).any()
    clean = harmonic_prominence_ref(np.nan_to_num(x), mhz, 2000.0)
    assert np.isfinite(clean).any()


def test_harmonic_prominence_nan_motor_frequencies_and_zero_motors_match_reference(dt):
    log = make_flightlog(seconds=4.0, fs=2000.0, seed=6)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    mhz_nan = mhz.copy()
    mhz_nan[:, 2000:2400] = np.nan
    assert np.isfinite(assert_prominence_matches(x, mhz_nan, 2000.0)).any()
    assert np.isnan(assert_prominence_matches(x, np.zeros_like(mhz), 2000.0)).all()  # 0 Hz : rien d'exploitable


def test_harmonic_prominence_no_windows_and_zero_harmonics_match_reference():
    x = np.random.default_rng(0).normal(size=100)
    mhz = np.full((4, 100), 200.0)
    same(prominence_native(x, mhz, 2000.0), harmonic_prominence_ref(x, mhz, 2000.0))
    log = make_flightlog(seconds=2.0, fs=2000.0)
    same(prominence_native(log.gyro[0].astype(float), log.motor_hz.astype(float), 2000.0, n_harm=0),
         harmonic_prominence_ref(log.gyro[0].astype(float), log.motor_hz.astype(float), 2000.0, n_harm=0))


# ----------------------------------------------------------------------------------------------------------
# D. masked_mean

def check_masked(f, P, lo, hi, n_harm, margin, fs):
    got = _native.masked_mean(f, P, lo, hi, n_harm, margin, fs)
    same(got, masked_oracle(f, P, lo, hi, n_harm, margin, fs))
    assert got.shape == (f.size,)
    return got


@pytest.mark.parametrize("fs", [2000.0, 400.0, 150.0])
@pytest.mark.parametrize("n_harm", [0, 1, 3, 8])
@pytest.mark.parametrize("seed", range(3))
def test_masked_mean_random_spectra_match_oracle(seed, n_harm, fs, dt):
    f, P, lo, hi, _ = random_spectra(dt, seed, fs=fs, hi_frac=0.5)
    got = check_masked(f, P, lo, hi, n_harm, MOTOR_LINE_MARGIN, fs)
    if n_harm:
        assert np.isfinite(got).any()
        assert not np.array_equal(got, np.mean(P, axis=0))  # le masque agit


def test_masked_mean_with_n_harm_zero_is_the_plain_nanmean(dt):
    f, P, lo, hi, _ = random_spectra(dt, 1, nan_frac=0.1)
    same(_native.masked_mean(f, P, lo, hi, 0, 0.05, 2000.0), np.nanmean(P, axis=0))


def test_masked_mean_folded_images_matter_on_slow_logs(dt):
    """Log lent : la boucle j de 1 à int(b // fs) + 1 retire les images repliées ; sans elle le résultat diffère."""
    fs = 150.0
    f, P, lo, hi, _ = random_spectra(dt, 2, fs=fs, hi_frac=0.9)
    lo, hi = lo * 3, hi * 3  # b >> fs : plusieurs repliements
    assert int((8 * hi.max() * 1.05) // fs) >= 4
    got = check_masked(f, P, lo, hi, 8, 0.05, fs)
    no_fold = masked_oracle(f, P, lo, hi, 8, 0.05, 1e9)
    assert not np.array_equal(got, no_fold, equal_nan=True)


def test_masked_mean_bounds_exactly_on_a_bin_are_masked(dt):
    f = np.arange(0, 100, 1.0)
    lo, hi = np.array([20.0, 20.0], dtype=dt), np.array([30.0, 30.0], dtype=dt)
    P = np.random.default_rng(0).lognormal(0, 1, (2, f.size))
    a = lo[0] - 0.05 * lo[0]
    assert a == 19.0 and (f == 19.0).any()  # borne basse pile sur la case 19 (comparaison >= inclusive)
    got = check_masked(f, P, lo, hi, 1, 0.05, 2000.0)
    assert np.isnan(got[19:32]).all()  # 19 à 31,5 masqués dans les deux fenêtres
    assert np.isfinite(got[18]) and np.isfinite(got[32])


def test_masked_mean_fully_masked_bins_and_all_nan_columns_give_nan(dt):
    f, P, lo, hi, _ = random_spectra(dt, 3)
    P[:, 5] = np.nan
    got = check_masked(f, P, lo, hi, 2, 0.05, 2000.0)
    assert np.isnan(got[5]) and np.isfinite(got).any()
    lo0, hi0 = np.zeros_like(lo), np.full_like(hi, 1000.0)  # 1 kHz : tout est dans la bande de h = 1
    got = check_masked(f, P, lo0, hi0, 1, 0.05, 2000.0)
    assert np.isnan(got).all()


def test_masked_mean_margin_zero_and_nan_bands_match_oracle(dt):
    f, P, lo, hi, _ = random_spectra(dt, 4)
    check_masked(f, P, lo, hi, 3, 0.0, 2000.0)
    lo[::5] = np.nan
    got = check_masked(f, P, lo, hi, 3, 0.05, 2000.0)
    assert np.isfinite(got).any()


def test_masked_mean_alias_count_uses_python_floor_division_not_floor_of_quotient(dt):
    """1.0 // 0.1 vaut 9.0 (reste de fmod) alors que floor(1.0 / 0.1) vaut 10 : l'image j = 11, qui tombe exactement sur
    la case 11·0,1 − 1,0, masquerait une case que l'étalon laisse intacte (bande ponctuelle : seule cette case la trahit)."""
    edge = 11 * 0.1 - 1.0
    f = np.array([0.0, edge, 0.3, 0.5, 1.0, 1.5])
    P = np.ones((1, f.size))
    lo, hi = np.array([1.0], dtype=dt), np.array([1.0], dtype=dt)
    got = check_masked(f, P, lo, hi, 1, 0.0, 0.1)
    if dt == np.float64:  # en float32, 0,1 arrondi change le quotient : l'oracle seul fait foi
        assert np.isfinite(got[1])


def test_masked_mean_zero_windows_gives_nan_array_like_nanmean_of_nothing(dt):
    f, P, lo, hi, _ = random_spectra(dt, 5)
    got = _native.masked_mean(f, P[:0], lo[:0], hi[:0], 3, 0.05, 2000.0)
    assert got.shape == f.shape and np.isnan(got).all()


def test_masked_mean_does_not_modify_its_inputs(dt):
    f, P, lo, hi, _ = random_spectra(dt, 6)
    keep = [a.copy() for a in (f, P, lo, hi)]
    _native.masked_mean(f, P, lo, hi, 3, 0.05, 2000.0)
    for a, b in zip((f, P, lo, hi), keep):
        assert a.tobytes() == b.tobytes()


@pytest.mark.parametrize("fs", [0.0, -2000.0, NAN, np.inf])
def test_masked_mean_rejects_invalid_sampling_rate(fs, dt):
    f, P, lo, hi, _ = random_spectra(dt, 7)
    with pytest.raises(ValueError):
        _native.masked_mean(f, P, lo, hi, 3, 0.05, fs)


@pytest.mark.parametrize("bad", [np.inf, NAN])
def test_masked_mean_rejects_non_finite_upper_bound(bad, dt):
    f, P, lo, hi, _ = random_spectra(dt, 8)
    hi[3] = bad
    with pytest.raises(ValueError):
        _native.masked_mean(f, P, lo, hi, 3, 0.05, 2000.0)


def assert_masked_matches(x, mhz, fs, n_harm, seg_ms, breaks=None):
    f_ref, ref = _masked_spectrum_ref(x, mhz, fs, n_harm, seg_ms, breaks)
    f_got, got = masked_native(x, mhz, fs, n_harm, seg_ms, breaks)
    same(f_got, f_ref, "f")
    same(got, ref, "spectre")
    return ref


def test_masked_spectrum_synthetic_flights_match_reference(vol):
    x, mhz, fs = vol
    ref = assert_masked_matches(x, mhz, fs, 3, 300.0)
    assert np.isfinite(ref).any()
    assert_masked_matches(x, mhz, fs, 8, 300.0)
    assert_masked_matches(x, mhz, fs, 0, 200.0)


def test_masked_spectrum_real_log_and_breaks_match_reference(real_data):
    x, mhz, fs, breaks = real_data
    ref = assert_masked_matches(x, mhz, fs, 3, 300.0, breaks)
    assert np.isfinite(ref).any()
    assert_masked_matches(x, mhz, fs, 5, 300.0, np.array([1500, 4000]))


def test_masked_spectrum_no_window_matches_reference():
    x = np.zeros(50)
    f_ref, ref = _masked_spectrum_ref(x, np.full((4, 50), 200.0), 2000.0, 3, 300.0)
    f, got = masked_native(x, np.full((4, 50), 200.0), 2000.0, 3, 300.0)
    same(f, f_ref)
    same(got, ref)


# ----------------------------------------------------------------------------------------------------------
# E. fold_mean

def check_fold(f, P, lo, hi, n_harm, lobe, fs):
    got = _native.fold_mean(f, P, lo, hi, n_harm, lobe, fs)
    same(got, fold_oracle(f, P, lo, hi, n_harm, lobe, fs))
    assert got.shape == (f.size,)
    return got


@pytest.mark.parametrize("fs", [400.0, 150.0, 90.0])
@pytest.mark.parametrize("n_harm", [0, 1, 3, 8])
@pytest.mark.parametrize("seed", range(3))
def test_fold_mean_random_spectra_match_oracle(seed, n_harm, fs, dt):
    f, P, lo, hi, _ = random_spectra(dt, seed, fs=fs, hi_frac=0.9)
    lo, hi = lo * 1.5, hi * 1.5  # b dépasse fs / 2 : repliements dans la plage de f
    got = check_fold(f, P, lo, hi, n_harm, 2 * fs / 45.0, fs)
    if n_harm:
        assert np.isnan(got).any() or not np.array_equal(got, np.mean(P, axis=0))


def test_fold_mean_masks_exactly_the_folded_band_and_nothing_else(dt):
    fs = 100.0
    f = np.arange(0, 51.0)
    P = np.random.default_rng(0).lognormal(0, 1, (1, f.size))
    lo, hi = np.array([10.0], dtype=dt), np.array([14.0], dtype=dt)
    got = check_fold(f, P, lo, hi, 1, 2.0, fs)  # a = 8, b = 16 : f dans [84, 92], hors grille 0..50
    assert np.array_equal(got, P[0])
    lo, hi = np.array([60.0], dtype=dt), np.array([64.0], dtype=dt)  # a = 58, b = 66 -> [34, 42] : bornes pile sur des cases
    got = check_fold(f, P, lo, hi, 1, 2.0, fs)
    assert np.isnan(got[34:43]).all() and np.isfinite(got[:34]).all() and np.isfinite(got[43:]).all()


def test_fold_mean_all_masked_bin_and_all_nan_column_give_nan(dt):
    fs = 100.0
    f = np.arange(0, 51.0)
    P = np.random.default_rng(1).lognormal(0, 1, (3, f.size))
    P[:, 7] = np.nan
    lo, hi = np.full(3, 60.0, dtype=dt), np.full(3, 64.0, dtype=dt)
    got = check_fold(f, P, lo, hi, 1, 2.0, fs)
    assert np.isnan(got[7]) and np.isfinite(got[0])


def test_fold_mean_zero_windows_gives_nan_array(dt):
    f, P, lo, hi, _ = random_spectra(dt, 2)
    got = _native.fold_mean(f, P[:0], lo[:0], hi[:0], 3, 5.0, 2000.0)
    assert got.shape == f.shape and np.isnan(got).all()


def test_fold_mean_does_not_modify_its_inputs(dt):
    f, P, lo, hi, _ = random_spectra(dt, 3)
    keep = [a.copy() for a in (f, P, lo, hi)]
    _native.fold_mean(f, P, lo, hi, 3, 5.0, 2000.0)
    for a, b in zip((f, P, lo, hi), keep):
        assert a.tobytes() == b.tobytes()


@pytest.mark.parametrize("fs", [0.0, -400.0, NAN, np.inf])
def test_fold_mean_with_invalid_sampling_rate_still_equals_oracle(fs, dt):
    """L'ancien code ne calcule aucun `int(b // fs)` ici : une cadence absurde masque (ou non) des cases, sans erreur."""
    f, P, lo, hi, _ = random_spectra(dt, 4)
    check_fold(f, P, lo, hi, 3, 5.0, fs)


@pytest.mark.parametrize("bad", [np.inf, NAN])
def test_fold_mean_with_non_finite_motor_bound_still_equals_oracle(bad, dt):
    f, P, lo, hi, _ = random_spectra(dt, 5)
    hi[2] = bad
    check_fold(f, P, lo, hi, 3, 5.0, 2000.0)


def assert_filtered_matches(x, mhz, fs, settings, breaks=None, seg_ms=300.0):
    f_ref, ref = rpm_filtered_spectrum_ref(x, mhz, fs, settings, seg_ms, breaks)
    f_got, got = rpm_filtered_native(x, mhz, fs, settings, seg_ms, breaks)
    same(f_got, f_ref, "f")
    same(got, ref, "spectre")
    return ref


def test_rpm_filtered_spectrum_synthetic_flights_match_reference(vol):
    x, mhz, fs = vol
    s = make_flightlog().settings
    ref = assert_filtered_matches(x, mhz, fs, s)
    assert np.isfinite(ref).any()
    assert_filtered_matches(x, mhz, fs, dataclasses.replace(s, rpm_harmonics=8, rpm_weights=[100, 50, 0, 100]))


def test_rpm_filtered_spectrum_slow_flight_has_folded_bins_removed(dt):
    p = SYNTH["tres_lent"]
    log = make_flightlog(seconds=4.0, fs=p["fs"], motor_hz=p["motor_hz"], seed=3)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    s = dataclasses.replace(log.settings, rpm_harmonics=8, rpm_weights=[100])
    ref = assert_filtered_matches(x, mhz, p["fs"], s)
    L, hop, f, P = spectra(x, p["fs"])
    mean_m = np.vstack([_segments(m, L, hop, None)[0].mean(axis=1) for m in mhz])
    attenuated = (P * rpm_attenuation_ref(f, mean_m, s) ** 2).mean(axis=0)
    assert not np.array_equal(ref, attenuated)  # l'étape de repliement agit


@pytest.mark.parametrize("name", sorted(settings_variants(make_flightlog().settings)))
def test_rpm_filtered_spectrum_real_log_matches_reference(name, real, real_data):
    x, mhz, fs, breaks = real_data
    s = settings_variants(real.settings)[name]
    ref = assert_filtered_matches(x, mhz, fs, s, breaks)
    assert np.isfinite(ref).any()


def test_rpm_filtered_spectrum_with_breaks_matches_reference(dt):
    log = make_flightlog(seconds=4.0, fs=2000.0, seed=2)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    assert_filtered_matches(x, mhz, 2000.0, log.settings, np.array([1000, 5000]))


# ----------------------------------------------------------------------------------------------------------
# F. Erreurs : tableaux non contigus, formes incohérentes, entrées absurdes

def valid_calls(nw=6, nf=20, dt=np.float64):
    """Un appel valide de chaque noyau : (nom, liste d'arguments) ; les tableaux sont aux positions `ARRAYS`."""
    f, P, lo, hi, mean = random_spectra(dt, 0, nw=nw, nf=nf)
    fk, wk = np.full((nw, 4), 300.0), np.full((nw, 4), 0.5)
    return {
        "rpm_response": [f, fk, wk, 5.0, 8000.0],
        "harmonic_levels": [f, P, lo, hi, mean, 3, 20.0, 2000.0],
        "masked_mean": [f, P, lo, hi, 3, 0.05, 2000.0],
        "fold_mean": [f, P, lo, hi, 3, 5.0, 2000.0],
        "numpy_mean": [f],
    }


def array_positions(args):
    return [i for i, a in enumerate(args) if isinstance(a, np.ndarray)]


CASES = [(name, i) for name, args in valid_calls().items() for i in array_positions(args)]
IDS = [f"{n}[{i}]" for n, i in CASES]


@pytest.mark.parametrize("name, i", CASES, ids=IDS)
def test_valid_call_works(name, i):
    """Garde-fou des tests d'erreur : l'appel de base est accepté (sinon les `raises` passeraient à vide)."""
    args = valid_calls()[name]
    assert getattr(_native, name)(*args) is not None


@pytest.mark.parametrize("name, i", CASES, ids=IDS)
def test_non_contiguous_array_is_rejected_with_value_error(name, i):
    args = valid_calls()[name]
    a = args[i]
    if a.ndim == 2:
        bad = np.asfortranarray(a)  # ordre Fortran (L-036)
        assert not bad.flags.c_contiguous
    else:
        bad = np.repeat(a, 2)[::2]  # 1D avec pas
        assert not bad.flags.c_contiguous
    args[i] = bad
    with pytest.raises(ValueError):
        getattr(_native, name)(*args)


# lo_m, hi_m (et mean_m) : float32 OU float64, tous du même dtype ; tout le reste est en float64
MOTOR_POS = {"harmonic_levels": (2, 3, 4), "masked_mean": (2, 3), "fold_mean": (2, 3)}


@pytest.mark.parametrize("name, i", CASES, ids=IDS)
def test_wrong_dtype_array_is_rejected(name, i):
    """int32 : TypeError partout ; float32 : TypeError sauf sur lo_m/hi_m/mean_m, où il est permis mais ne peut pas
    être mélangé avec du float64 (ValueError)."""
    args = valid_calls()[name]
    int_args = list(args)
    int_args[i] = args[i].astype(np.int32)
    with pytest.raises((TypeError, ValueError) if i in MOTOR_POS.get(name, ()) else TypeError):
        getattr(_native, name)(*int_args)
    args[i] = args[i].astype(np.float32)
    with pytest.raises(ValueError if i in MOTOR_POS.get(name, ()) else TypeError):
        getattr(_native, name)(*args)


@pytest.mark.parametrize("name", sorted(MOTOR_POS))
def test_all_motor_arrays_in_float32_are_accepted(name):
    args = valid_calls(dt=np.float32)[name]
    assert all(args[i].dtype == np.float32 for i in MOTOR_POS[name])
    assert getattr(_native, name)(*args) is not None


@pytest.mark.parametrize("name", sorted(MOTOR_POS))
@pytest.mark.parametrize("k", [0, 1, 2])
def test_mixing_float32_and_float64_motor_arrays_is_a_value_error(name, k):
    pos = MOTOR_POS[name]
    if k >= len(pos):
        pytest.skip("pas de 3e tableau moteur")
    args = valid_calls(dt=np.float32)[name]
    args[pos[k]] = args[pos[k]].astype(np.float64)
    with pytest.raises(ValueError):
        getattr(_native, name)(*args)


@pytest.mark.parametrize("name", sorted(MOTOR_POS))
def test_float16_motor_arrays_are_rejected(name):
    args = valid_calls()[name]
    for i in MOTOR_POS[name]:
        args[i] = args[i].astype(np.float16)
    with pytest.raises((TypeError, ValueError)):
        getattr(_native, name)(*args)


# ----------------------------------------------------------------------------------------------------------
# F ter. Le dtype des fréquences moteur change le résultat : le noyau suit l'arithmétique de chaque précision

def ulps(x, rng, k=4):
    """`x` (float32) décalé de quelques ulps : une borne de bande tombe à un souffle d'une case de f."""
    x = np.float32(x)
    for _ in range(int(rng.integers(0, k + 1))):
        x = np.nextafter(x, np.float32(np.inf if rng.random() < 0.5 else -np.inf))
    return np.float32(x)


def sensitive_windows(run, make, n=12, seed=0, tries=20000):
    """Fréquences moteur float32 dont une fenêtre seule donne un résultat différent quand le même calcul est fait en
    float64 (une borne de bande passe d'un côté ou de l'autre d'une case de f). `make(rng)` propose (lo, hi) près
    d'une case ; on garde ceux que `run` (oracle, une fenêtre) distingue."""
    rng = np.random.default_rng(seed)
    picks = []
    for _ in range(tries):
        lo, hi = make(rng)
        a, b = np.array([lo], dtype=np.float32), np.array([hi], dtype=np.float32)
        if not np.array_equal(run(a, b), run(a.astype(np.float64), b.astype(np.float64)), equal_nan=True):
            picks.append((lo, hi))
            if len(picks) == n:
                break
    assert len(picks) == n, len(picks)
    lo, hi = zip(*picks)
    return np.array(lo, dtype=np.float32), np.array(hi, dtype=np.float32)


def test_masked_mean_float32_arithmetic_differs_from_float64_and_kernel_follows_float32():
    f = np.arange(0.0, 1000.0, 0.25)
    rng = np.random.default_rng(1)
    P1 = rng.lognormal(0, 1, (1, f.size))

    def run(lo, hi):
        return masked_oracle(f, P1, lo, hi, 2, 0.05, 2000.0)

    def make(rng):  # a = lo - 0,05·lo et b = hi + 0,05·hi près de cases de f (pas de 0,25 Hz)
        return (ulps(rng.integers(40, 1600) * 0.25 / 0.95, rng), ulps(rng.integers(40, 1600) * 0.25 / 1.05, rng))

    lo, hi = sensitive_windows(run, make)
    P = rng.lognormal(0, 1, (lo.size, f.size))
    want32 = masked_oracle(f, P, lo, hi, 2, 0.05, 2000.0)
    want64 = masked_oracle(f, P, lo.astype(np.float64), hi.astype(np.float64), 2, 0.05, 2000.0)
    assert not np.array_equal(want32, want64, equal_nan=True)  # ce cas distingue bien les deux précisions
    same(_native.masked_mean(f, P, lo, hi, 2, 0.05, 2000.0), want32)
    got64 = _native.masked_mean(f, P, lo.astype(np.float64), hi.astype(np.float64), 2, 0.05, 2000.0)
    same(got64, want64)
    assert not np.array_equal(got64, want32, equal_nan=True)


def test_fold_mean_float32_arithmetic_differs_from_float64_and_kernel_follows_float32():
    fs = 100.0
    f = np.arange(0.0, 50.0, 0.0625)
    rng = np.random.default_rng(2)
    P1 = rng.lognormal(0, 1, (1, f.size))

    def run(lo, hi):
        return fold_oracle(f, P1, lo, hi, 1, 2.0, fs)

    def make(rng):  # b = hi + 2 franchit 64 : float32 arrondit, fs - b tombe près d'une case entre 34 et 36 Hz
        hi = ulps(100.0 - rng.integers(544, 576) * 0.0625 - 2.0, rng)
        return hi - np.float32(3.0), hi

    lo, hi = sensitive_windows(run, make)
    P = rng.lognormal(0, 1, (lo.size, f.size))
    want32 = fold_oracle(f, P, lo, hi, 1, 2.0, fs)
    want64 = fold_oracle(f, P, lo.astype(np.float64), hi.astype(np.float64), 1, 2.0, fs)
    assert not np.array_equal(want32, want64, equal_nan=True)
    same(_native.fold_mean(f, P, lo, hi, 1, 2.0, fs), want32)
    got64 = _native.fold_mean(f, P, lo.astype(np.float64), hi.astype(np.float64), 1, 2.0, fs)
    same(got64, want64)
    assert not np.array_equal(got64, want32, equal_nan=True)


def test_harmonic_levels_float32_arithmetic_differs_from_float64_and_kernel_follows_float32():
    fs = 2000.0
    f = np.arange(0.0, 1000.0, 0.5)
    rng = np.random.default_rng(3)
    P1 = rng.lognormal(0, 1, (1, f.size))

    def run(lo, hi):
        mean = ((lo + hi) / lo.dtype.type(2)).astype(lo.dtype)
        return np.concatenate([a.ravel() for a in levels_oracle(f, P1, lo, hi, mean, 3, 20.0, fs)])

    def make(rng):  # h = 3 : 3·lo s'arrondit en float32 ; bornes 3·lo - df et 3·hi + df près de cases de f (df = 0,5)
        return (ulps((rng.integers(100, 400) * 0.5 + 0.5) / 3, rng), ulps((rng.integers(100, 400) * 0.5 - 0.5) / 3, rng))

    lo, hi = sensitive_windows(run, make)
    mean = ((lo + hi) / np.float32(2)).astype(np.float32)
    P = rng.lognormal(0, 1, (lo.size, f.size))
    want32 = levels_oracle(f, P, lo, hi, mean, 3, 20.0, fs)
    lo64, hi64, mean64 = (a.astype(np.float64) for a in (lo, hi, mean))
    want64 = levels_oracle(f, P, lo64, hi64, mean64, 3, 20.0, fs)
    assert any(not np.array_equal(a, b, equal_nan=True) for a, b in zip(want32, want64))
    for got, want in zip(_native.harmonic_levels(f, P, lo, hi, mean, 3, 20.0, fs), want32):
        same(got, want)
    for got, want in zip(_native.harmonic_levels(f, P, lo64, hi64, mean64, 3, 20.0, fs), want64):
        same(got, want)


def test_reference_filtered_spectrum_differs_between_float32_and_float64_motors():
    """Sur l'étalon lui-même, le dtype des moteurs change le spectre filtré (l'arithmétique de `lo`/`hi` en float32
    modifie le repliement) : les comparaisons d'égalité stricte sont sensibles à la précision. Pour la prominence et
    le spectre masqué, la sensibilité est établie au niveau des noyaux (tests `..._float32_arithmetic_differs...`)."""
    log = make_flightlog(seconds=4.0, fs=2000.0, seed=0)
    x, m64 = log.gyro[0].astype(np.float64), log.motor_hz.astype(np.float64)
    a = rpm_filtered_spectrum_ref(x, m64, 2000.0, log.settings)[1]
    b = rpm_filtered_spectrum_ref(x, m64.astype(np.float32), 2000.0, log.settings)[1]
    assert not np.array_equal(a, b, equal_nan=True)


@pytest.mark.parametrize("name, i", CASES, ids=IDS)
def test_array_of_wrong_rank_is_rejected(name, i):
    args = valid_calls()[name]
    a = args[i]
    args[i] = a[:, 0].copy() if a.ndim == 2 else np.stack([a, a])
    with pytest.raises((ValueError, TypeError)):
        getattr(_native, name)(*args)


def test_rpm_attenuation_rejects_inconsistent_shapes():
    f, fk, wk = valid_calls()["rpm_response"][:3]
    for fk2, wk2 in ((fk, wk[:, :3].copy()), (fk, wk[:5].copy()), (fk[:, :3].copy(), wk)):
        with pytest.raises(ValueError):
            rpm_att(f, fk2, wk2, 5.0, 8000.0)


@pytest.mark.parametrize("name", ["harmonic_levels", "masked_mean", "fold_mean"])
def test_spectrum_kernels_reject_inconsistent_shapes(name):
    args = valid_calls()[name]
    f, P = args[0], args[1]
    cuts = [(0, f[:-1].copy()), (1, P[:, :-1].copy()), (1, P[:-1].copy())]
    cuts += [(i, args[i][:-1].copy()) for i in (2, 3)]  # lo_m, hi_m trop courts
    if name == "harmonic_levels":
        cuts.append((4, args[4][:-1].copy()))
    for i, bad in cuts:
        a2 = list(args)
        a2[i] = bad
        with pytest.raises(ValueError):
            getattr(_native, name)(*a2)


def survives(fn, *args):
    """Le noyau rend un résultat ou une ValueError ; une panique Rust (PanicException) fait échouer le test."""
    try:
        return fn(*args)
    except ValueError:
        return None


def absurd_calls_for(dt):
    f, P, lo, hi, mean = random_spectra(dt, 0, nw=6, nf=20)
    out = []

    def add(name, **changes):
        args = valid_calls(dt=dt)[name]
        names = {"rpm_response": ["f", "fk", "wk", "q", "rate"],
                 "harmonic_levels": ["f", "P", "lo", "hi", "mean", "n_harm", "min_hz", "fs"],
                 "masked_mean": ["f", "P", "lo", "hi", "n_harm", "margin", "fs"],
                 "fold_mean": ["f", "P", "lo", "hi", "n_harm", "lobe", "fs"]}[name]
        for key, val in changes.items():
            args[names.index(key)] = np.asarray(val, dtype=dt) if key in ("lo", "hi", "mean") else val
        out.append(pytest.param(name, args, id=f"{name}-{'-'.join(changes)}-{dt.__name__}"))

    nanf = f.copy()
    nanf[3] = np.nan
    inff = f.copy()
    inff[0] = np.inf
    for name in ("harmonic_levels", "masked_mean", "fold_mean"):
        add(name, P=P[:0], lo=lo[:0], hi=hi[:0], **({"mean": mean[:0]} if name == "harmonic_levels" else {}))
        add(name, f=f[:0], P=P[:, :0])
        add(name, f=f[:1], P=P[:, :1])
        add(name, n_harm=0)
        add(name, f=nanf)
        add(name, f=inff)
        add(name, P=np.where(P > 1, np.inf, P))
        add(name, lo=np.full(6, np.nan), hi=np.full(6, np.nan))
        add(name, lo=np.full(6, np.inf), hi=np.full(6, np.inf))
        add(name, lo=-lo, hi=-hi)
        add(name, fs=0.0)
        add(name, fs=-1.0)
        add(name, fs=NAN)
        add(name, fs=np.inf)
        add(name, fs=1e-300)
    add("harmonic_levels", min_hz=NAN)
    add("harmonic_levels", mean=np.full(6, np.inf))
    add("masked_mean", margin=NAN)
    add("masked_mean", margin=-1.0)
    add("fold_mean", lobe=NAN)
    add("fold_mean", lobe=np.inf)
    add("rpm_response", rate=0.0)
    add("rpm_response", rate=-8000.0)
    add("rpm_response", rate=NAN)
    add("rpm_response", q=0.0)
    add("rpm_response", q=NAN)
    add("rpm_response", f=np.zeros(0))
    add("rpm_response", f=nanf)
    add("rpm_response", f=inff)
    add("rpm_response", fk=np.full((6, 4), np.nan))
    add("rpm_response", fk=np.full((6, 4), np.inf))
    add("rpm_response", fk=np.zeros((6, 4)))
    add("rpm_response", fk=np.full((6, 4), -50.0))
    add("rpm_response", wk=np.full((6, 4), np.nan))
    add("rpm_response", wk=np.full((6, 4), np.inf))
    add("rpm_response", fk=np.zeros((0, 4)), wk=np.zeros((0, 4)))
    add("rpm_response", fk=np.zeros((6, 0)), wk=np.zeros((6, 0)))
    return out


def absurd_calls():
    return absurd_calls_for(np.float32) + absurd_calls_for(np.float64)


@pytest.mark.parametrize("name, args", absurd_calls())
def test_absurd_inputs_give_a_result_or_a_value_error_never_a_panic(name, args):
    got = survives(getattr(_native, name), *args)
    if got is None:
        return
    arrays = got if isinstance(got, tuple) else (got,)
    assert all(isinstance(g, np.ndarray) for g in arrays)
    if name == "rpm_response":
        assert got.shape == (args[1].shape[0], args[0].size)
    elif name == "harmonic_levels":
        assert [g.shape for g in got] == [(args[5], args[1].shape[0])] * 3
    else:
        assert got.shape == (args[0].size,)


def test_numpy_mean_of_empty_and_nan_arrays_never_panics():
    for a in (np.zeros(0), np.array([np.nan]), np.array([np.inf, -np.inf, 1.0]), np.full(1000, np.nan)):
        survives(_native.numpy_mean, a)


@pytest.mark.parametrize("name", ["harmonic_levels", "masked_mean", "fold_mean"])
def test_negative_harmonic_count_is_an_error_not_a_panic(name):
    args = valid_calls()[name]
    args[5 if name == "harmonic_levels" else 4] = -1
    with pytest.raises((ValueError, OverflowError, TypeError)):
        getattr(_native, name)(*args)


# ----------------------------------------------------------------------------------------------------------
# F bis. Déterminisme et propriété de la sortie

@pytest.mark.parametrize("name", ["rpm_response", "harmonic_levels", "masked_mean", "fold_mean"])
def test_kernels_are_deterministic_and_return_fresh_arrays(name):
    args = valid_calls(nw=40, nf=200)[name]
    a = getattr(_native, name)(*args)
    b = getattr(_native, name)(*args)
    a, b = (a if isinstance(a, tuple) else (a,)), (b if isinstance(b, tuple) else (b,))
    for x, y in zip(a, b):
        assert x.tobytes() == y.tobytes() and x is not y
        assert not any(np.shares_memory(x, z) for z in args if isinstance(z, np.ndarray))


# ----------------------------------------------------------------------------------------------------------
# G. Intégration : `motors.*` branché sur les noyaux = `*_ref` (code numpy d'origine)

def count_calls(monkeypatch, names):
    """Espionne `motors._native.<nom>` : les noyaux doivent être réellement appelés par `motors.*`."""
    calls = dict.fromkeys(names, 0)
    for n in names:
        orig = getattr(motors._native, n)

        def spy(*a, _orig=orig, _n=n):
            calls[_n] += 1
            return _orig(*a)

        monkeypatch.setattr(motors._native, n, spy)
    return calls


def test_motors_harmonic_prominence_uses_native_and_equals_reference(vol, monkeypatch):
    x, mhz, fs = vol
    calls = count_calls(monkeypatch, ["harmonic_levels"])
    got = motors.harmonic_prominence(x, mhz, fs, n_harm=3)
    ref = harmonic_prominence_ref(x, mhz, fs, n_harm=3)
    same(got, ref)
    assert calls["harmonic_levels"] >= 1 and np.isfinite(ref).any()


def test_motors_harmonic_prominence_real_log_with_breaks_equals_reference(real_data):
    x, mhz, fs, breaks = real_data
    for n_harm in (3, 6):
        same(motors.harmonic_prominence(x, mhz, fs, n_harm=n_harm, breaks=breaks),
             harmonic_prominence_ref(x, mhz, fs, n_harm=n_harm, breaks=breaks))


def test_motors_harmonic_prominence_nan_in_signal_equals_reference(dt):
    log = make_flightlog(seconds=4.0, fs=2000.0, seed=5)
    x, mhz = log.gyro[0].astype(np.float64), log.motor_hz.astype(dt)
    x[1500:1510] = np.nan
    got = motors.harmonic_prominence(x, mhz, 2000.0)
    same(got, harmonic_prominence_ref(x, mhz, 2000.0))
    assert np.isnan(got).any()


@pytest.mark.parametrize("name", sorted(settings_variants(make_flightlog().settings)))
def test_motors_rpm_attenuation_uses_native_and_equals_reference(name, real, real_data, monkeypatch):
    x, mhz, fs, breaks = real_data
    s = settings_variants(real.settings)[name]
    L, hop, f, _ = spectra(x, fs, breaks=breaks)
    mean_m = np.vstack([_segments(m, L, hop, breaks)[0].mean(axis=1) for m in mhz])
    calls = count_calls(monkeypatch, ["rpm_response"])
    got = motors.rpm_attenuation(f, mean_m, s)
    ref = rpm_attenuation_ref(f, mean_m, s)
    same(got, ref)
    assert calls["rpm_response"] >= 1 and (ref < 1.0).any()


def test_motors_rpm_attenuation_zero_windows_equals_reference():
    s = make_flightlog().settings
    f = np.linspace(0, 1000, 40)
    same(motors.rpm_attenuation(f, np.zeros((4, 0)), s), rpm_attenuation_ref(f, np.zeros((4, 0)), s))


def test_motors_rpm_filtered_spectrum_uses_native_and_equals_reference(vol, monkeypatch):
    x, mhz, fs = vol
    s = make_flightlog().settings
    calls = count_calls(monkeypatch, ["rpm_response", "fold_mean"])
    f, got = motors.rpm_filtered_spectrum(x, mhz, fs, s)
    f_ref, ref = rpm_filtered_spectrum_ref(x, mhz, fs, s)
    same(f, f_ref)
    same(got, ref)
    assert calls["rpm_response"] >= 1 and calls["fold_mean"] >= 1 and np.isfinite(ref).any()


def test_motors_rpm_filtered_spectrum_real_log_with_breaks_equals_reference(real, real_data):
    x, mhz, fs, breaks = real_data
    for s in settings_variants(real.settings).values():
        f, got = motors.rpm_filtered_spectrum(x, mhz, fs, s, 300.0, breaks)
        f_ref, ref = rpm_filtered_spectrum_ref(x, mhz, fs, s, 300.0, breaks)
        same(f, f_ref)
        same(got, ref)


def test_motors_rpm_filtered_spectrum_without_rpm_filter_equals_reference(vol):
    x, mhz, fs = vol
    s = dataclasses.replace(make_flightlog().settings, rpm_harmonics=0)
    for args in ((None, None), (mhz, None), (mhz, s)):
        f, got = motors.rpm_filtered_spectrum(x, args[0], fs, args[1])
        f_ref, ref = rpm_filtered_spectrum_ref(x, args[0], fs, args[1])
        same(f, f_ref)
        same(got, ref)


def test_motors_masked_spectrum_uses_native_and_equals_reference(vol, monkeypatch):
    x, mhz, fs = vol
    calls = count_calls(monkeypatch, ["masked_mean"])
    f, got = motors._masked_spectrum(x, mhz, fs, 3, 300.0)
    f_ref, ref = _masked_spectrum_ref(x, mhz, fs, 3, 300.0)
    same(f, f_ref)
    same(got, ref)
    assert calls["masked_mean"] >= 1 and np.isfinite(ref).any()


def test_motors_masked_spectrum_real_log_with_breaks_equals_reference(real_data):
    x, mhz, fs, breaks = real_data
    for n_harm, br in ((3, breaks), (6, np.array([1500, 4000]))):
        f, got = motors._masked_spectrum(x, mhz, fs, n_harm, 300.0, br)
        f_ref, ref = _masked_spectrum_ref(x, mhz, fs, n_harm, 300.0, br)
        same(f, f_ref)
        same(got, ref)


def test_motors_no_window_cases_equal_reference():
    x = np.zeros(50)
    mhz = np.full((4, 50), 200.0)
    s = make_flightlog().settings
    for got, ref in (
        (motors._masked_spectrum(x, mhz, 2000.0, 3, 300.0), _masked_spectrum_ref(x, mhz, 2000.0, 3, 300.0)),
        (motors.rpm_filtered_spectrum(x, mhz, 2000.0, s), rpm_filtered_spectrum_ref(x, mhz, 2000.0, s)),
    ):
        same(got[0], ref[0])
        same(got[1], ref[1])
    same(motors.harmonic_prominence(x, mhz, 2000.0), harmonic_prominence_ref(x, mhz, 2000.0))


@pytest.mark.parametrize("name", ["harmonic_levels", "masked_mean", "fold_mean"])
def test_absurd_harmonic_count_is_a_value_error_not_a_giant_allocation(name):
    """Plafond de 1000 harmoniques : `n_harm × fenêtres` démesuré ferait terminer le processus (échec d'allocation)."""
    args = list(valid_calls()[name])
    pos = {"harmonic_levels": 5, "masked_mean": 4, "fold_mean": 4}[name]
    args[pos] = 10**12
    with pytest.raises((ValueError, OverflowError)):
        getattr(_native, name)(*args)
