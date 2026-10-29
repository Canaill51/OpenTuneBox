"""Parité des noyaux Rust (`rust/otb-dsp`, via `opentunebox._native`) avec les noyaux numba d'origine
(`tests/ref_bf_kernels.py`) : D-031, phase 2a. Écart attendu : arrondis de sin/cos et de la division complexe."""

from types import SimpleNamespace

import numpy as np
import pytest

_native = pytest.importorskip("opentunebox._native")
ref = pytest.importorskip("ref_bf_kernels")  # exige numba (extra « dev »)

from opentunebox.analysis import bf_filters as bf  # noqa: E402
from synthetic import make_flightlog  # noqa: E402

RTOL = 1e-12  # relatif au maximum du tableau comparé ; écart mesuré ≈ 1e-15 (voir L-034)


def close(a, b):
    a, b = np.asarray(a), np.asarray(b)
    assert a.shape == b.shape
    np.testing.assert_allclose(a, b, rtol=0, atol=RTOL * max(1.0, float(np.max(np.abs(b), initial=0.0))))


def random_stages(rng, n):
    kinds = rng.integers(0, 3, n).astype(np.int64)
    fcs = rng.uniform(-50, 4500, n)
    fcs[rng.random(n) < 0.1] = 0.0
    return (kinds, fcs, rng.uniform(0.2, 12, n), rng.uniform(0, 1, n), rng.choice([2000.0, 4000.0, 8000.0], n))


@pytest.mark.parametrize("seed", range(5))
def test_biquad_identical(seed):
    rng = np.random.default_rng(seed)
    for kind, f, q, w, fs in zip(*random_stages(rng, 300)):
        got = _native.biquad(int(kind), float(f), float(q), float(w), float(fs))
        close(got, ref._biquad(int(kind), float(f), float(q), float(w), float(fs)))


@pytest.mark.parametrize("seed", range(5))
def test_sections_response_matches_numba(seed):
    rng = np.random.default_rng(seed)
    kinds, fcs, qs, ws, rates = random_stages(rng, int(rng.integers(1, 12)))
    freqs = np.concatenate([np.linspace(0, 4000, 800), rng.uniform(0, 4000, 50)])
    got = bf._sections_response(kinds, fcs, qs, ws, rates, freqs)
    assert got.dtype == np.complex128
    close(got.real, ref._sections_response(kinds, fcs, qs, ws, rates, freqs).real)
    close(got.imag, ref._sections_response(kinds, fcs, qs, ws, rates, freqs).imag)


@pytest.mark.parametrize("fs,fc,q", [(2000.0, 150.0, 1.0), (2000.0, 112.5, 3.0), (4000.0, 300.0, 1.0)])
def test_sections_response_floor_at_exact_notch_centre(fs, fc, q):
    """Creux exact d'un notch : le gain vaut exactement le plancher 1e-15 (pas zéro, L-024), comme chez numba.
    Comparaison exacte : la tolérance de `close` (1e-12) masquerait un plancher modifié."""
    args = (np.array([2], np.int64), np.array([fc]), np.array([q]), np.array([1.0]), np.array([fs]))
    freqs = np.array([fc])
    got = bf._sections_response(*args, freqs)
    assert got[0] == 1e-15 == ref._sections_response(*args, freqs)[0]


def test_sections_response_empty_chain_and_empty_freqs():
    e = (np.zeros(0, np.int64), np.zeros(0), np.zeros(0), np.zeros(0), np.zeros(0))
    assert np.all(bf._sections_response(*e, np.linspace(0, 100, 5)) == 1.0)
    assert bf._sections_response(np.array([0], np.int64), np.array([100.0]), np.array([0.0]), np.array([1.0]),
                                 np.array([8000.0]), np.zeros(0)).size == 0


@pytest.mark.parametrize("seed", range(6))
def test_run_section_matches_numba(seed):
    rng = np.random.default_rng(seed)
    n_ax, n = 3, int(rng.integers(50, 2000))
    x = rng.normal(0, 200, (n_ax, n))
    # fréquences et poids variables, par paliers (cas du filtre RPM) et avec des zéros / valeurs hors limite
    freq = np.repeat(rng.uniform(-10, 1200, n // 10 + 1), 10)[:n]
    weight = np.repeat(rng.uniform(0, 1, n // 7 + 1), 7)[:n]
    for kind in (0, 1, 2):
        for limit in (np.inf, 960.0):
            got = _native.run_section(x, kind, freq, 0.7, weight, 2000.0, limit)
            close(got, ref._run_section(x, kind, freq, 0.7, weight, 2000.0, limit))


def test_pt1_track_matches_numba():
    x = np.random.default_rng(0).uniform(100, 600, 5000)
    for k in (0.001, 0.2, 1.0):
        close(_native.pt1_track(x, k), ref._pt1_track(x, k))


def test_bad_shapes_raise_value_error_not_panic():
    f = np.ones(4)
    with pytest.raises(ValueError):  # 4 fréquences mais 5 colonnes
        _native.run_section(np.ones((2, 5)), 0, f, 1.0, f, 2000.0, np.inf)
    with pytest.raises(ValueError):  # 2 types d'étage pour 4 fréquences de coupure
        _native.sections_response(np.zeros(2, np.int64), f, f, f, f, f)


def test_run_section_rejects_fortran_order_instead_of_reading_it_wrong():
    """Un tableau rangé colonne par colonne serait lu ligne par ligne : résultat faux sans erreur (L-036)."""
    x = np.random.default_rng(0).normal(size=(3, 40))
    f = np.full(40, 120.0)
    with pytest.raises(ValueError):
        _native.run_section(np.asfortranarray(x), 1, f, 0.7, np.ones(40), 2000.0, np.inf)
    assert _native.run_section(np.ascontiguousarray(x), 1, f, 0.7, np.ones(40), 2000.0, np.inf).shape == (3, 40)


def test_run_section_needs_contiguous_float64():
    f = np.ones(4)
    with pytest.raises((ValueError, TypeError)):
        _native.run_section(np.ones((2, 8))[:, ::2], 0, f, 1.0, f, 2000.0, np.inf)


@pytest.mark.parametrize("seed,rpm", [(0, True), (1, True), (2, False)])
def test_chain_and_simulation_match_numba_end_to_end(seed, rpm, monkeypatch):
    log = make_flightlog(seconds=4, fs=2000.0, seed=seed, rpm=rpm)
    f = np.linspace(0, 1000, 600)
    chains = [bf.gyro_chain(log.settings, 0.5, motor_hz=300.0), bf.dterm_chain(log.settings, 0.4)]
    rust_h = [bf.chain_response(c, f) for c in chains]
    rust_gyro = bf.simulate_gyro(log)
    rust_d = bf.simulate_dterm(log, gyro=rust_gyro)
    # même code Python, moteur numba d'origine à la place du Rust
    monkeypatch.setattr(bf, "_native", SimpleNamespace(
        biquad=ref._biquad, sections_response=ref._sections_response, run_section=ref._run_section,
        pt1_track=ref._pt1_track))
    for c, h in zip(chains, rust_h):
        close(h.real, bf.chain_response(c, f).real)
        close(h.imag, bf.chain_response(c, f).imag)
    close(rust_gyro, bf.simulate_gyro(log))
    close(rust_d, bf.simulate_dterm(log, gyro=rust_gyro))
