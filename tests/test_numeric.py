"""Tests de `opentunebox.analysis._numeric` : remplaçants numpy purs de scipy.

Chaque fonction est comparée à sa référence scipy (scipy n'est importé qu'ici,
dans les tests, jamais dans le module).
"""

import ast
from pathlib import Path

import numpy as np
import pytest
import scipy.ndimage
import scipy.signal
import scipy.signal.windows

from opentunebox.analysis import _numeric

# À resserrer/justifier après mesure de l'écart réel une fois l'implémentation faite.
ATOL_FACTOR = 1e-12

MODULE_PATH = Path(__file__).resolve().parents[1] / "src" / "opentunebox" / "analysis" / "_numeric.py"


def _assert_close(got, ref):
    """Écart absolu borné par ATOL_FACTOR * max(1, max|ref|), sans tolérance relative."""
    got = np.asarray(got)
    ref = np.asarray(ref)
    assert got.shape == ref.shape
    atol = ATOL_FACTOR * max(1.0, float(np.max(np.abs(ref))) if ref.size else 1.0)
    assert np.allclose(got, ref, rtol=0, atol=atol), float(np.max(np.abs(got - ref)))


# --- hann -------------------------------------------------------------------


@pytest.mark.parametrize("sym", [True, False])
@pytest.mark.parametrize("length", [1, 2, 3, 64, 257, 1024])
def test_hann_matches_scipy(length, sym):
    ref = scipy.signal.windows.hann(length, sym=sym)
    got = _numeric.hann(length, sym)
    _assert_close(got, ref)


# --- uniform_filter1d ---------------------------------------------------------


@pytest.mark.parametrize("size", [1, 2, 3, 4, 5, 10, 11])
@pytest.mark.parametrize("dtype", [np.float64, np.float32])
def test_uniform_filter1d_1d_matches_scipy_with_same_dtype(size, dtype):
    x = np.random.default_rng(1).normal(size=200).astype(dtype)
    ref = scipy.ndimage.uniform_filter1d(x, size, mode="nearest")
    got = _numeric.uniform_filter1d(x, size)
    assert got.dtype == ref.dtype
    if dtype == np.float32:
        # float32 : la précision de l'entrée limite l'accord, pas la méthode.
        assert np.allclose(got, ref, rtol=0, atol=1e-5)
    else:
        _assert_close(got, ref)


@pytest.mark.parametrize("size", [2, 3, 4, 5, 10, 11])
def test_uniform_filter1d_signal_shorter_than_size_matches_scipy(size):
    x = np.random.default_rng(2).normal(size=3)
    ref = scipy.ndimage.uniform_filter1d(x, size, mode="nearest")
    _assert_close(_numeric.uniform_filter1d(x, size), ref)


@pytest.mark.parametrize("axis", [0, 1, -1])
@pytest.mark.parametrize("size", [3, 4, 10, 11])
def test_uniform_filter1d_2d_axis_matches_scipy(axis, size):
    x = np.random.default_rng(3).normal(size=(17, 23))
    ref = scipy.ndimage.uniform_filter1d(x, size, axis=axis, mode="nearest")
    got = _numeric.uniform_filter1d(x, size, axis=axis)
    assert got.dtype == ref.dtype
    _assert_close(got, ref)


@pytest.mark.parametrize("size", [2, 4, 10])
def test_uniform_filter1d_even_size_has_same_offset_as_scipy(size):
    # Une impulsion révèle le décalage appliqué aux largeurs paires.
    x = np.zeros(41)
    x[20] = 1.0
    ref = scipy.ndimage.uniform_filter1d(x, size, mode="nearest")
    got = _numeric.uniform_filter1d(x, size)
    assert np.array_equal(np.flatnonzero(got > 1e-9), np.flatnonzero(ref > 1e-9))
    _assert_close(got, ref)


# --- gaussian_filter ----------------------------------------------------------


@pytest.mark.parametrize("sigma", [(1.25, 0.5), (3.7, 1.9), (0.3, 0.3)])
@pytest.mark.parametrize("shape", [(3, 4), (16, 16), (40, 25), (7, 90)])
def test_gaussian_filter_matches_scipy(sigma, shape):
    arr = np.random.default_rng(4).normal(size=shape)
    ref = scipy.ndimage.gaussian_filter(arr, sigma, mode="nearest")
    got = _numeric.gaussian_filter(arr, sigma)
    _assert_close(got, ref)


# --- xcorr_full ---------------------------------------------------------------


@pytest.mark.parametrize("n", [1, 2, 50, 1000, 1001])
def test_xcorr_full_equal_lengths_matches_scipy(n):
    rng = np.random.default_rng(5)
    y, x = rng.normal(size=n), rng.normal(size=n)
    ref = scipy.signal.correlate(y, x, mode="full", method="fft")
    ref_lags = scipy.signal.correlation_lags(len(y), len(x), mode="full")
    corr, lags = _numeric.xcorr_full(y, x)
    assert np.array_equal(lags, ref_lags)
    assert corr.shape == ref.shape
    assert np.allclose(corr, ref, rtol=0, atol=1e-9 * np.max(np.abs(ref)))


@pytest.mark.parametrize("ny,nx", [(100, 37), (37, 100), (1000, 1), (5, 6)])
def test_xcorr_full_unequal_lengths_matches_scipy(ny, nx):
    rng = np.random.default_rng(6)
    y, x = rng.normal(size=ny), rng.normal(size=nx)
    ref = scipy.signal.correlate(y, x, mode="full", method="fft")
    ref_lags = scipy.signal.correlation_lags(ny, nx, mode="full")
    corr, lags = _numeric.xcorr_full(y, x)
    assert np.array_equal(lags, ref_lags)
    assert corr.shape == ref.shape
    assert np.allclose(corr, ref, rtol=0, atol=1e-9 * np.max(np.abs(ref)))


# --- indépendance -------------------------------------------------------------


def test_module_imports_neither_scipy_nor_numba():
    tree = ast.parse(MODULE_PATH.read_text(encoding="utf-8"))
    imported = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported |= {a.name.split(".")[0] for a in node.names}
        elif isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
            imported.add(node.module.split(".")[0])
    assert not imported & {"scipy", "numba"}
