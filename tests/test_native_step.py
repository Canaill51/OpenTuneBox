"""Parité du noyau Rust `_native.step_windows` (`rust/otb-dsp`) avec la boucle numpy d'origine de
`step.step_response`, gardée dans `tests/ref_step.py` (D-031, phase 2b, pilote). TDD : écrits avant le noyau.

Écart attendu ≈ 1e-12 (rustfft et pocketfft n'arrondissent pas pareil) : tolérance RELATIVE au maximum du
tableau comparé. À justifier après mesure (le chiffre définitif sera noté ici une fois le noyau écrit)."""

import os
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

_native = pytest.importorskip("opentunebox._native")

from opentunebox.analysis import _numeric, step  # noqa: E402
from opentunebox.log.flightlog import load  # noqa: E402
from ref_step import step_windows_numpy  # noqa: E402
from synthetic import make_flightlog  # noqa: E402

RTOL = 1e-9  # relatif au maximum du tableau ; écart mesuré (2026-10-29) : 1,7e-14 sur R, 6e-16 sur les sommes ; la marge couvre une autre FFT
TESTS_DIR = Path(__file__).resolve().parent
REAL_LOG = TESTS_DIR / "data" / "jeno5_5s.bbl"


def close(a, b):
    """Égalité à `RTOL` près relativement au maximum de la référence `b` (plancher 1 pour un tableau presque nul)."""
    a, b = np.asarray(a), np.asarray(b)
    assert a.shape == b.shape
    np.testing.assert_allclose(a, b, rtol=0, atol=RTOL * max(1.0, float(np.max(np.abs(b), initial=0.0))))


def steady_range(fs, lr):
    """Plage d'indices de la réponse (200–500 ms) où l'on contrôle la qualité, comme `step_response`."""
    t = np.arange(lr) * 1000.0 / fs
    ii = np.flatnonzero((t >= 200) & (t <= 500))
    return (int(ii[0]), int(ii[-1]) + 1) if ii.size else (0, 0)


def build(sp, gy, fs, seg_s=2.0, pad=100, reg=1e-4, y_correction=False, starts=None, steady=True):
    """Arguments de `step_windows` pour un couple (consigne, gyro) : mêmes choix que `step_response`."""
    sp = np.ascontiguousarray(sp, dtype=np.float64)
    gy = np.ascontiguousarray(gy, dtype=np.float64)
    L = int(round(seg_s * fs))
    lr = int(round(500 * fs / 1000.0)) + 1
    if starts is None:
        starts = step._window_starts(sp, fs, L, 62.5, 20.0, None, None)
    lo, hi = steady_range(fs, lr) if steady else (0, 0)
    return dict(sp=sp, gy=gy, w=_numeric.hann(L), starts=np.ascontiguousarray(starts, dtype=np.int64),
                l=L, lr=lr, pad=pad, reg=reg, y_correction=y_correction, steady_lo=lo, steady_hi=hi)


def run(fn, **kw):
    return fn(kw["sp"], kw["gy"], kw["w"], kw["starts"], kw["l"], kw["lr"], kw["pad"], kw["reg"],
              kw["y_correction"], kw["steady_lo"], kw["steady_hi"])


def assert_same(kw):
    """Même masque de fenêtres retenues (même k, même `maxin`, exact : c'est un max de consigne) puis mêmes nombres."""
    ref = run(step_windows_numpy, **kw)
    got = run(_native.step_windows, **kw)
    assert got[0].shape == ref[0].shape and got[1].shape == ref[1].shape
    assert np.array_equal(got[1], ref[1]), "fenêtres retenues différentes"
    assert got[2].dtype == np.complex128 and got[0].dtype == got[3].dtype == got[4].dtype == np.float64
    close(got[0], ref[0])
    close(got[2].real, ref[2].real)
    close(got[2].imag, ref[2].imag)
    close(got[3], ref[3])
    close(got[4], ref[4])
    # les sommes couvrent plusieurs décades (6e6 mesuré) : la tolérance « au maximum » ne vérifie pas les hautes
    # fréquences ; on exige aussi l'écart relatif case par case (mesuré : 4e-12 au pire, 200 fois sous RTOL)
    for k in (2, 3, 4):
        np.testing.assert_allclose(got[k], ref[k], rtol=RTOL, atol=0)
    return got


def synth_axis(seconds, fs, seed):
    log = make_flightlog(seconds=seconds, fs=fs, seed=seed, moving=True)
    return log.setpoint[0], log.gyro[0], float(log.fs)


# ---------------------------------------------------------------- 1. parité

@pytest.mark.parametrize("fs,seconds,seed", [(2000.0, 12, 0), (1000.0, 10, 1), (4000.0, 6, 2), (2000.0, 15, 7)])
@pytest.mark.parametrize("y_correction", [False, True])
@pytest.mark.parametrize("pad,reg", [(100, 1e-4), (50, 1e-2)])
def test_synthetic_flights_match_reference(fs, seconds, seed, y_correction, pad, reg):
    sp, gy, f = synth_axis(seconds, fs, seed)
    got = assert_same(build(sp, gy, f, pad=pad, reg=reg, y_correction=y_correction))
    assert got[0].shape[0] > 0, "le vol synthétique doit retenir des fenêtres, sinon le test ne prouve rien"


@pytest.mark.parametrize("seg_s,parity", [(1.0, 0), (1.0015, 1)])
def test_even_and_odd_window_lengths_match_reference(seg_s, parity):
    """nfft = L + 2·pad a la parité de L : la FFT réelle inverse (n pair / impair) doit suivre l'étalon."""
    sp, gy, f = synth_axis(12, 2000.0, 3)
    kw = build(sp, gy, f, seg_s=seg_s)
    assert kw["l"] % 2 == parity
    assert run(_native.step_windows, **kw)[0].shape[0] > 0
    assert_same(kw)


@pytest.mark.parametrize("y_correction", [False, True])
def test_real_log_matches_reference(y_correction):
    """Vrai log (1618 Hz, L = 3236) : 5 s donnent peu ou pas de fenêtre ; le noyau doit s'accorder dans les deux cas."""
    (lg,) = load(REAL_LOG)
    kw = build(lg.setpoint[0], lg.gyro[0], float(lg.fs), y_correction=y_correction)
    assert kw["l"] == 3236
    assert_same(kw)


def test_real_log_all_axes_and_short_windows_match_reference():
    """Fenêtres de 1 s sur le vrai log : assez courtes pour en avoir plusieurs, sur chaque axe."""
    (lg,) = load(REAL_LOG)
    for ax in range(3):
        assert_same(build(lg.setpoint[ax], lg.gyro[ax], float(lg.fs), seg_s=1.0))


# ---------------------------------------------------------------- 2. cas limites

def test_empty_steady_range_keeps_every_window_and_ignores_y_correction():
    sp, gy, f = synth_axis(12, 2000.0, 0)
    a = assert_same(build(sp, gy, f, steady=False, y_correction=False))
    b = assert_same(build(sp, gy, f, steady=False, y_correction=True))
    assert a[0].shape[0] == build(sp, gy, f)["starts"].size, "plage vide : toutes les fenêtres passent"
    assert np.array_equal(a[0], b[0])


def test_no_window_passes_quality_control_gives_empty_result():
    sp, _, f = synth_axis(12, 2000.0, 0)
    kw = build(sp, np.zeros_like(sp), f)
    assert kw["starts"].size > 0
    R, maxin, gh, hh, gg = run(_native.step_windows, **kw)
    assert R.shape == (0, kw["lr"]) and maxin.shape == (0,)
    nbin = kw["l"] + 2 * kw["pad"]
    assert gh.shape == hh.shape == gg.shape == (nbin // 2 + 1,)
    assert not gh.any() and not hh.any() and not gg.any()
    assert_same(kw)


def test_single_window():
    sp, gy, f = synth_axis(12, 2000.0, 0)
    first = build(sp, gy, f)["starts"][:1]
    got = assert_same(build(sp, gy, f, starts=first))
    assert got[0].shape[0] == 1 and got[1].shape == (1,)


def test_empty_starts_gives_empty_result():
    sp, gy, f = synth_axis(12, 2000.0, 0)
    kw = build(sp, gy, f, starts=np.zeros(0, np.int64))
    R, maxin, _, hh, _ = run(_native.step_windows, **kw)
    assert R.shape == (0, kw["lr"]) and maxin.size == 0 and not hh.any()
    assert_same(kw)


def test_more_than_256_windows_are_processed_in_packets_in_order():
    """L'étalon traite 256 fenêtres par paquet : l'ordre et les sommes doivent traverser les frontières de paquet."""
    sp, gy, f = synth_axis(120, 2000.0, 5)
    n = sp.size
    kw = build(sp, gy, f, starts=np.arange(0, n - 4000, 200))
    assert kw["starts"].size > 600
    got = assert_same(kw)
    assert got[0].shape[0] > 256


# ---------------------------------------------------------------- 3. contrôle qualité discontinu

def delayed_gain(a, seconds=8, fs=2000.0, seed=0, delay=4):
    """gyro = a · consigne retardée de `delay` échantillons : le plateau de la réponse vaut ≈ a."""
    sp, _, f = synth_axis(seconds, fs, seed)
    gy = np.zeros_like(sp)
    gy[delay:] = a * sp[:-delay]
    return build(sp, gy, f, starts=build(sp, sp, f)["starts"][:12])


@pytest.mark.parametrize("a,kept", [(0.45, False), (1.0, True), (3.5, False)])
def test_quality_control_verdict_on_clear_cases(a, kept):
    kw = delayed_gain(a)
    ref = run(step_windows_numpy, **kw)
    assert (ref[0].shape[0] > 0) is kept, "l'étalon doit juger comme prévu, sinon la fabrication du plateau est fausse"
    assert_same(kw)


SWEEP = [round(x, 4) for x in (*np.arange(0.48, 0.5201, 0.004), *np.arange(2.98, 3.0201, 0.004))]


@pytest.mark.parametrize("a", SWEEP)
def test_quality_control_threshold_verdict_is_identical_near_limits(a):
    """Seuils 0,5 (min, strict) et 3,0 (max, strict) : à la limite, un arrondi d'FFT ne doit pas changer le verdict."""
    kw = delayed_gain(a)
    ref = run(step_windows_numpy, **kw)
    got = run(_native.step_windows, **kw)
    assert np.array_equal(got[1], ref[1]), f"verdict différent pour a = {a}"


# ---------------------------------------------------------------- 4. déterminisme

SCRIPT = """
import hashlib
from opentunebox import _native
from opentunebox.analysis import _numeric, step
from synthetic import make_flightlog
log = make_flightlog(seconds=130, fs=2000.0, seed=3, moving=True)
sp, gy, fs = log.setpoint[0], log.gyro[0], float(log.fs)
L = int(round(2.0 * fs)); Lr = int(round(500 * fs / 1000.0)) + 1
starts = step._window_starts(sp, fs, L, 62.5, 20.0, None, None).astype('int64')
import numpy as np
t = np.arange(Lr) * 1000.0 / fs
ii = np.flatnonzero((t >= 200) & (t <= 500))
out = _native.step_windows(np.ascontiguousarray(sp, dtype=np.float64), np.ascontiguousarray(gy, dtype=np.float64),
                           _numeric.hann(L), starts, L, Lr, 100, 1e-4, True, int(ii[0]), int(ii[-1]) + 1)
print(starts.size, out[0].shape[0])
for a in out:
    print(hashlib.sha256(np.ascontiguousarray(a).tobytes()).hexdigest())
"""


def run_with_threads(n):
    env = {**os.environ, "RAYON_NUM_THREADS": str(n), "PYTHONUTF8": "1", "PYTHONPATH": str(TESTS_DIR)}
    p = subprocess.run([sys.executable, "-c", SCRIPT], env=env, capture_output=True, text=True,
                       encoding="utf-8", timeout=600)
    assert p.returncode == 0, p.stderr[-2000:]
    return p.stdout.split("\n")


def test_result_is_bit_identical_whatever_the_thread_count():
    """L'ordre des sommes ne doit pas dépendre du nombre de fils : mêmes octets avec 1 et 8 fils."""
    one, eight = run_with_threads(1), run_with_threads(8)
    assert int(one[0].split()[0]) >= 600 and int(one[0].split()[1]) > 0, "pas assez de fenêtres pour le parallélisme"
    assert one == eight
    assert len([h for h in one[1:] if len(h) == 64]) == 5


# ---------------------------------------------------------------- 5. erreurs

@pytest.fixture
def base():
    sp, gy, f = synth_axis(12, 2000.0, 0)
    return build(sp, gy, f)


def call(kw, **over):
    return run(_native.step_windows, **{**kw, **over})


@pytest.mark.parametrize("bad", [-1, "after_end"])
def test_window_outside_array_raises_value_error_not_panic(base, bad):
    n = base["sp"].size
    start = -1 if bad == -1 else n - base["l"] + 1
    with pytest.raises(ValueError):
        call(base, starts=np.array([0, start], dtype=np.int64))


def test_window_ending_exactly_at_array_end_is_accepted(base):
    n = base["sp"].size
    R, *_ = call(base, starts=np.array([n - base["l"]], dtype=np.int64))
    assert R.shape[1] == base["lr"]


def test_window_beyond_shorter_gyro_raises_value_error(base):
    n = base["sp"].size
    with pytest.raises(ValueError):
        call(base, gy=base["gy"][: n - 10], starts=np.array([n - base["l"]], dtype=np.int64))


def test_unequal_lengths_use_the_shorter_without_panic(base):
    """Pas d'exigence d'égalité : une fenêtre valable des deux côtés donne le même résultat que l'étalon."""
    kw = {**base, "gy": base["gy"][:-500], "starts": base["starts"][base["starts"] + base["l"] <= base["gy"].size - 500]}
    assert_same(kw)


@pytest.mark.parametrize("delta", [-1, 1])
def test_window_taper_of_wrong_length_raises_value_error(base, delta):
    w = _numeric.hann(base["l"] + delta)
    with pytest.raises(ValueError):
        call(base, w=w)


def test_steady_range_inverted_raises_value_error(base):
    with pytest.raises(ValueError):
        call(base, steady_lo=10, steady_hi=5)


def test_steady_range_beyond_response_length_raises_value_error(base):
    with pytest.raises(ValueError):
        call(base, steady_lo=0, steady_hi=base["lr"] + 1)


def test_steady_range_up_to_response_length_is_accepted(base):
    call(base, steady_lo=base["lr"] - 5, steady_hi=base["lr"])


@pytest.mark.parametrize("name", ["sp", "gy", "w"])
def test_non_contiguous_array_raises_value_error(base, name):
    arr = base[name]
    strided = np.column_stack([arr, arr])[:, 0]
    assert not strided.flags["C_CONTIGUOUS"]
    with pytest.raises(ValueError):
        call(base, **{name: strided})


def test_non_contiguous_starts_raises_value_error(base):
    s = np.column_stack([base["starts"], base["starts"]])[:, 0]
    assert not s.flags["C_CONTIGUOUS"]
    with pytest.raises(ValueError):
        call(base, starts=s)


@pytest.mark.parametrize("name", ["sp", "gy", "w"])
def test_float32_array_raises_type_error(base, name):
    with pytest.raises(TypeError):
        call(base, **{name: base[name].astype(np.float32)})


# ---------------------------------------------------------------- 6. intégration

def array_fields(obj):
    """Champs numériques (tableaux, flottants) d'un dataclass imbriqué, par nom."""
    out = {}
    for k, v in vars(obj).items():
        if isinstance(v, (np.ndarray, float, int, np.floating)) and not isinstance(v, bool):
            out[k] = np.asarray(v)  # complexes conservés : ne pas écarter la partie imaginaire
    return out


@pytest.mark.parametrize("y_correction", [False, True])
def test_step_response_with_rust_kernel_matches_numpy_reference(monkeypatch, y_correction):
    """`step_response` branché sur le noyau Rust = `step_response` branché sur l'étalon numpy (même code Python autour).
    Ne peut passer qu'une fois `step.py` branché sur `_native.step_windows` (étape suivante de D-031)."""
    sp, gy, f = synth_axis(12, 2000.0, 0)
    rust = step.step_response(sp, gy, f, y_correction=y_correction)
    monkeypatch.setattr(step._native, "step_windows", step_windows_numpy)
    ref = step.step_response(sp, gy, f, y_correction=y_correction)
    assert rust.n == ref.n > 0
    assert np.array_equal(rust.max_inputs, ref.max_inputs)
    for name in ("responses", "mean", "std"):
        close(getattr(rust, name), getattr(ref, name))
    rl, fl = array_fields(rust.loop), array_fields(ref.loop)
    assert rl.keys() == fl.keys()
    for k in rl:
        if rl[k].shape == fl[k].shape:
            np.testing.assert_allclose(rl[k], fl[k], rtol=1e-8, atol=1e-10, equal_nan=True, err_msg=k)
        else:
            pytest.fail(f"forme différente pour loop.{k}")


def test_step_response_without_window_is_unchanged_by_the_kernel():
    """Aucune fenêtre (consigne trop faible) : résultat vide, noyau non appelé ou appelé à vide, sans erreur."""
    f = 2000.0
    res = step.step_response(np.zeros(8000), np.zeros(8000), f)
    assert res.n == 0 and not res.ok
