#!/usr/bin/env python3
"""Cas de test de `analysis/_numeric.py` pour `rust/otb-dsp/src/numeric.rs` (D-035, phase 4b-1).

Usage : numeric_cases.py SORTIE.json

Écrit, pour `hann`, `uniform_filter1d`, `gaussian_filter` et `xcorr_full`, des entrées (dont des cas courts : longueur 0, 1,
plus petite que la fenêtre ; des NaN et des infinis) et les sorties de Python. Un tableau s'écrit en hexadécimal, 16 caractères
par élément (bits du float64), pour comparer au bit près ; une entrée float32 est écrite en float64 (exact) et son résultat est
écrit en float32 arrondi (`f32`: vrai). Le test `rust/otb-dsp/tests/numeric.rs` exige les mêmes sorties : bit pour bit pour
`hann`, `uniform_filter1d` et `gaussian_filter` ; à 1e-12 du plus grand élément pour `xcorr_full` (FFT, mesuré en 4b-0).
"""
import json
import sys

import numpy as np

from opentunebox.analysis import _numeric


def hexs(a, dtype=np.float64) -> str:
    a = np.ascontiguousarray(a, dtype=dtype).ravel()
    return a.astype("<f8" if dtype == np.float64 else "<f4").tobytes().hex()


def _raises(call):
    """Le cas d'erreur écrit dans le fichier est bien une erreur de Python (pas une supposition)."""
    try:
        call()
    except ValueError:
        return
    raise SystemExit("une ValueError était attendue")


def signal(rng, shape, kind="normal"):
    n = int(np.prod(shape))
    if kind == "constant":
        return np.full(shape, 3.25)
    if kind == "wide":  # grande dynamique : les arrondis se voient
        return (rng.standard_normal(n) * 10.0 ** rng.integers(-8, 9, n)).reshape(shape)
    return (rng.standard_normal(n) * 100.0).reshape(shape)


def main():
    rng = np.random.default_rng(20261030)
    out = {"hann": [], "uniform": [], "gaussian": [], "xcorr": []}

    for length in [-3, 0, 1, 2, 3, 4, 5, 8, 10, 63, 64, 595, 600, 1000, 1001]:
        for sym in (True, False):
            out["hann"].append({"length": length, "sym": sym, "out": hexs(_numeric.hann(length, sym))})

    shapes_axes = [((0,), -1), ((1,), -1), ((2,), 0), ((3,), -1), ((5,), -1), ((17,), 0), ((200,), -1),
                   ((4, 9), 1), ((4, 9), 0), ((9, 4), -1), ((1, 6), 1), ((6, 1), 0), ((3, 40), -1), ((2, 3, 5), 1)]
    for shape, axis in shapes_axes:
        for size in (1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 30):
            for kind in ("normal", "wide", "constant"):
                if np.prod(shape) == 0 and kind != "normal":
                    continue
                x = signal(rng, shape, kind)
                for f32 in (False, True):
                    xin = x.astype(np.float32) if f32 else x
                    out["uniform"].append({"shape": list(shape), "axis": axis, "size": size, "f32": f32,
                                           "x": hexs(xin.astype(np.float64)),
                                           "out": hexs(_numeric.uniform_filter1d(xin, size, axis=axis),
                                                       np.float32 if f32 else np.float64)})
    # NaN et infinis : la somme cumulée propage un NaN jusqu'à la fin
    for bad in (np.nan, np.inf, -np.inf):
        x = signal(rng, (30,))
        x[7] = bad
        for size in (1, 3, 4, 9):
            out["uniform"].append({"shape": [30], "axis": -1, "size": size, "f32": False, "x": hexs(x),
                                   "out": hexs(_numeric.uniform_filter1d(x, size))})
    # zéros négatifs : np.cumsum garde le premier tel quel, une somme 0.0 + (-0.0) donnerait +0.0
    x = np.full(6, -0.0)
    for size in (2, 3, 4):
        out["uniform"].append({"shape": [6], "axis": -1, "size": size, "f32": False, "x": hexs(x),
                               "out": hexs(_numeric.uniform_filter1d(x, size))})
    for size in (0, -1):
        _raises(lambda: _numeric.uniform_filter1d(np.arange(5.0), size))
        out["uniform"].append({"shape": [5], "axis": -1, "size": size, "f32": False, "x": hexs(np.arange(5.0)),
                               "out": None})

    sigmas = [0.0, 1e-16, 2e-15, 0.01, 0.124, 0.125, 0.3, 0.5, 1.0, 1.25, 2.0, 2.5, 3.75, 5.0, 6.25]
    for shape in [(1,), (2,), (3,), (7,), (40,), (200,), (1, 5), (5, 1), (2, 2), (6, 4), (24, 20), (2, 3, 4)]:
        for sg in sigmas:
            for kind in ("normal", "wide"):
                x = signal(rng, shape, kind)
                for f32 in (False, True) if x.size <= 200 else (False,):  # fichier de taille raisonnable
                    xin = x.astype(np.float32) if f32 else x
                    out["gaussian"].append({"shape": list(shape), "sigma": [sg] * len(shape), "f32": f32,
                                            "x": hexs(xin.astype(np.float64)),
                                            "out": hexs(_numeric.gaussian_filter(xin, sg),
                                                        np.float32 if f32 else np.float64)})
    # un sigma par axe, comme `smooth_map` (1,25 × niveau, 0,5 × niveau), dont un axe laissé intact
    for level in range(1, 6):
        for shape in [(24, 30), (33, 50), (3, 100), (200, 4)]:
            x = signal(rng, shape)
            x[rng.random(shape) < 0.2] = 0.0
            sg = (1.25 * level, 0.5 * level)
            out["gaussian"].append({"shape": list(shape), "sigma": list(sg), "f32": False, "x": hexs(x),
                                    "out": hexs(_numeric.gaussian_filter(x, sg))})
    for sg in ([0.0, 2.0], [2.0, 0.0], [0.0, 0.0]):
        x = signal(rng, (10, 12))
        out["gaussian"].append({"shape": [10, 12], "sigma": sg, "f32": False, "x": hexs(x),
                                "out": hexs(_numeric.gaussian_filter(x, sg))})
    for bad in (np.nan, np.inf):
        x = signal(rng, (30,))
        x[11] = bad
        out["gaussian"].append({"shape": [30], "sigma": [1.5], "f32": False, "x": hexs(x),
                                "out": hexs(_numeric.gaussian_filter(x, 1.5))})

    for ly, lx in [(1, 1), (2, 1), (1, 2), (2, 2), (3, 5), (5, 3), (10, 10), (100, 100), (600, 600), (997, 640),
                   (1000, 1000), (3000, 300), (1025, 1024), (2049, 17)]:
        y = signal(rng, (ly,))
        x = np.roll(y, 3)[:lx] if lx <= ly else signal(rng, (lx,))
        c, lags = _numeric.xcorr_full(y, x)
        out["xcorr"].append({"ly": ly, "lx": lx, "y": hexs(y), "x": hexs(x), "out": hexs(c),
                             "lags": [int(lags[0]), int(lags[-1]), int(lags.size)]})
    for ly, lx in [(0, 3), (3, 0), (0, 0)]:
        _raises(lambda: _numeric.xcorr_full(np.zeros(ly), np.zeros(lx)))
        out["xcorr"].append({"ly": ly, "lx": lx, "y": "", "x": "", "out": None, "lags": None})
    # entiers en entrée : Python les convertit en float64
    yi = np.arange(10)
    c, lags = _numeric.xcorr_full(yi, yi[::-1])
    out["xcorr"].append({"ly": 10, "lx": 10, "y": hexs(yi.astype(float)), "x": hexs(yi[::-1].astype(float)),
                         "out": hexs(c), "lags": [int(lags[0]), int(lags[-1]), int(lags.size)]})

    json.dump(out, open(sys.argv[1], "w", encoding="utf-8"), separators=(",", ":"))
    print({k: len(v) for k, v in out.items()})


if __name__ == "__main__":
    main()
