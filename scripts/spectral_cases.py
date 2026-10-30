#!/usr/bin/env python3
"""Cas de test de `analysis/spectral.py` pour `rust/otb-analysis/src/spectral.rs` (D-035, phase 4b-2).

Usage : spectral_cases.py SORTIE.json

Écrit, pour chaque fonction publique de `spectral`, des entrées (dont des cas courts : signal plus court que la fenêtre,
vide ; des raccords du log ; des portes sur une frontière de tranche ; des NaN) et les sorties de Python. Un tableau
s'écrit en hexadécimal (16 caractères par élément, bits du float64) ; `"error": true` = Python a levé une exception. Le
test `rust/otb-analysis/tests/spectral.rs` exige : les mêmes erreurs, les mêmes formes, les mêmes entiers, et les flottants
à la tolérance de leur fonction (FFT, `log10`, `hypot` : docs/migration-rust.md, L-048).
"""
import json
import sys

import numpy as np

from opentunebox.analysis import spectral


def hexs(a) -> str:
    return np.ascontiguousarray(a, dtype="<f8").tobytes().hex()


def call(fn, *args, **kw):
    """(résultat, False) ou (None, True) si Python lève une exception."""
    try:
        return fn(*args, **kw), False
    except Exception:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        return None, True


def signal(rng, n, fs, floor=1e-3):
    """Deux raies fortes sur un plancher faible : grande dynamique en dB."""
    t = np.arange(n) / fs
    return 200.0 * np.sin(2 * np.pi * 37.0 * t) + 40.0 * np.sin(2 * np.pi * 151.3 * t) + floor * rng.standard_normal(n)


def spec_out(res, err):
    if err:
        return {"error": True}
    return {"freqs": hexs(res.freqs), "values": hexs(res.values), "n": [int(res.freqs.size), int(res.values.size)],
            "mode": res.mode, "res": hexs(np.float64(res.resolution_hz))}


def main():
    rng = np.random.default_rng(20261031)
    out = {k: [] for k in ("clear_of_breaks", "ptb_subsample", "to_db", "smooth_freq", "spectrum", "freq_throttle",
                           "smooth_map", "freq_time")}

    # ---- clear_of_breaks : portes sur la frontière d'un raccord
    for length in (1, 4, 10):
        for breaks in (None, [], [0], [5], [5, 5, 9], [3, 10, 20, 21, 400], [10_000_000_000]):
            for starts in ([], [0], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], list(range(0, 40, 3)), [-2, 4, 5, 9]):
                b = None if breaks is None else np.array(breaks, dtype=np.int64)
                s = np.array(starts, dtype=np.int64)
                res, err = call(spectral.clear_of_breaks, s, length, b)
                out["clear_of_breaks"].append({"starts": starts, "length": length, "breaks": breaks, "error": err,
                                               "out": None if err else [bool(v) for v in res]})

    # ---- ptb_subsample
    for d in (0.0, 1.0, 19.999, 20.0, 20.0001, 59.9, 60.0, 60.0001, 1e9, float("nan"), float("inf"), -5.0):
        res, err = call(spectral.ptb_subsample, d, 5, 3, 1)
        out["ptb_subsample"].append({"d": hexs(np.float64(d)), "out": None if err else int(res)})

    # ---- to_db / to_ptb_db
    for p in (np.array([]), np.array([0.0, 1e-13, 1e-12, 1.5e-12, 1.0, 10.0, 1e6, 1e12, np.nan, np.inf, -3.0]),
              10.0 ** rng.uniform(-14, 8, 500), rng.standard_normal(40)):
        db = spectral.to_db(p)
        out["to_db"].append({"p": hexs(p), "db": hexs(db), "ptb": hexs(spectral.to_ptb_db(db))})

    # ---- smooth_freq : 1D et 2D, niveaux de 0 à 5 (et négatif)
    for shape in ((0,), (1,), (3,), (12,), (60,), (4, 33), (1, 20), (7, 1), (3, 2, 15)):
        y = rng.standard_normal(int(np.prod(shape))).reshape(shape) * 30.0
        for level in (-1, 0, 1, 2, 3, 5):
            res = spectral.smooth_freq(y, level)
            out["smooth_freq"].append({"shape": list(shape), "level": level, "y": hexs(y), "out": hexs(res)})

    # ---- spectrum
    cases = []
    for n, fs in ((0, 1000.0), (1, 1000.0), (2, 1000.0), (3, 400.0), (63, 400.0), (64, 400.0), (65, 400.0),
                  (200, 400.0), (401, 400.0), (1500, 400.0), (1800, 3200.0)):
        for mode in ("psd", "amp"):
            cases.append((n, fs, mode, 2.0, 0, None))
    for mode in ("psd", "amp", "x"):
        for res_hz in (1.0, 2.0, 3.3, 0.0):
            for smoothing in (0, 2, 5):
                cases.append((1200, 400.0, mode, res_hz, smoothing, None))
    for fs_half in (129.0, 131.0, 193.5):  # fs / 2 = 64,5 (au pair : 64) ; 65,5 (66) ; 96,75 : demi-entiers de Python
        cases.append((1000, fs_half, "psd", 2.0, 0, None))
        cases.append((1000, fs_half, "amp", 2.0, 0, None))
    for breaks in ([], [100], [100, 900, 901], [0], [1199], list(range(0, 1200, 50))):
        cases.append((1200, 400.0, "psd", 2.0, 0, breaks))
        cases.append((1200, 400.0, "amp", 4.0, 2, breaks))
    for n, fs, mode, res_hz, smoothing, breaks in cases:
        x = signal(rng, n, fs) if n else np.zeros(0)
        for f32 in (False, True) if (n in (1500, 1200) and mode == "psd" and not breaks) else (False,):
            xin = x.astype(np.float32) if f32 else x
            b = None if breaks is None else np.array(breaks, dtype=np.int64)
            res, err = call(spectral.spectrum, xin, fs, mode, res_hz, smoothing, b)
            out["spectrum"].append({"n": n, "fs": hexs(np.float64(fs)), "mode": mode, "res_hz": res_hz,
                                    "smoothing": smoothing, "breaks": breaks, "x": hexs(xin.astype(np.float64)),
                                    "f32": f32, "out": spec_out(res, err)})

    # ---- freq_throttle : durées courte / moyenne / longue (recouvrement 5 / 3 / 1), portes sur les frontières
    def ft_out(res, err):
        if err:
            return {"error": True}
        return {"freqs": hexs(res.freqs), "bins": hexs(res.bins), "z": hexs(res.z), "shape": list(res.z.shape),
                "counts": [int(c) for c in res.counts], "mode": res.mode}

    def ramp(n, lo, hi):
        return np.linspace(lo, hi, n)

    ft_cases = []
    for n, fs in ((0, 200.0), (10, 200.0), (59, 200.0), (60, 200.0), (61, 200.0), (800, 200.0), (4000, 200.0),
                  (12200, 200.0)):
        for mode in ("amp", "psd") if n < 10000 else ("amp",):
            ft_cases.append((n, fs, mode, "ramp", 300.0, 20, (0.0, 100.0), None))
    for kind in ("const0", "const50", "const29", "const100", "const7.3", "step", "nan", "inf", "hi"):
        ft_cases.append((1200, 200.0, "amp", kind, 300.0, 100, (0.0, 100.0), None))
        ft_cases.append((1200, 200.0, "psd", kind, 300.0, 10, (0.0, 100.0), None))
    for nbins, rng_ in ((1, (0.0, 100.0)), (3, (0.0, 100.0)), (100, (10.0, 90.0)), (100, (5.0, 5.0)), (50, (100.0, 0.0))):
        ft_cases.append((1200, 200.0, "amp", "ramp", 300.0, nbins, rng_, None))
    for fs_half in (335.0, 345.0):  # 335 × 300 ms = 100,5 (au pair : 100) ; 345 × 300 ms = 103,5 (104)
        ft_cases.append((1500, fs_half, "amp", "ramp", 300.0, 20, (0.0, 100.0), None))
    for seg_ms in (1.0, 10.0, 100.0, 1000.0):
        ft_cases.append((1200, 200.0, "amp", "ramp", seg_ms, 20, (0.0, 100.0), None))
    for breaks in ([], [400], [400, 401, 900], list(range(0, 1200, 120))):
        ft_cases.append((1200, 200.0, "psd", "ramp", 300.0, 20, (0.0, 100.0), breaks))
    for n, fs, mode, kind, seg_ms, nbins, xr, breaks in ft_cases:
        x = signal(rng, n, fs) if n else np.zeros(0)
        if kind == "ramp":
            xa = ramp(n, 0.0, 100.0)
        elif kind.startswith("const"):
            xa = np.full(n, float(kind[5:]))
        elif kind == "step":
            xa = np.where(np.arange(n) < n // 2, 12.0, 88.0)
        elif kind == "hi":
            xa = ramp(n, -50.0, 250.0)  # hors de la plage : bornes de tranche
        else:
            xa = ramp(n, 0.0, 100.0)
            xa[n // 3: n // 3 + 200] = np.nan if kind == "nan" else np.inf
        b = None if breaks is None else np.array(breaks, dtype=np.int64)
        res, err = call(spectral.freq_throttle, x, xa, fs, mode, seg_ms, nbins, xr, b)
        out["freq_throttle"].append({"n": n, "fs": hexs(np.float64(fs)), "mode": mode, "seg_ms": seg_ms, "nbins": nbins,
                                     "range": list(xr), "breaks": breaks, "x": hexs(x), "xaxis": hexs(xa),
                                     "out": ft_out(res, err)})

    # ---- smooth_map : cartes avec tranches vides (NaN), niveaux de 0 à 5
    for shape in ((1, 1), (3, 5), (33, 20), (17, 100), (2, 1)):
        z = np.abs(rng.standard_normal(shape)) * 20.0
        if shape[1] > 3:
            z[:, 1::4] = np.nan
        for level in (-1, 0, 1, 2, 3, 5):
            for variant in ("nan", "plein") if shape in ((3, 5), (33, 20)) else ("nan",):
                zz = z if variant == "nan" else np.nan_to_num(z)
                res = spectral.smooth_map(zz, level)
                out["smooth_map"].append({"shape": list(shape), "level": level, "z": hexs(zz), "out": hexs(res)})
    zz = np.full((6, 8), np.nan)
    out["smooth_map"].append({"shape": [6, 8], "level": 2, "z": hexs(zz), "out": hexs(spectral.smooth_map(zz, 2))})

    # ---- freq_time
    def tf_out(res, err):
        if err:
            return {"error": True}
        return {"times": hexs(res.times), "freqs": hexs(res.freqs), "z": hexs(res.z), "shape": list(res.z.shape)}

    tf_cases = []
    for n, fs in ((0, 400.0), (5, 400.0), (119, 400.0), (120, 400.0), (121, 400.0), (500, 400.0), (2000, 400.0)):
        tf_cases.append((n, fs, 300.0, 2, 0, None))
    for hop_div in (-1, 0, 1, 2, 3, 4, 7):
        tf_cases.append((1200, 400.0, 300.0, hop_div, 0, None))
    for smoothing in (0, 1, 3):
        tf_cases.append((1200, 400.0, 100.0, 2, smoothing, None))
    for fs_half in (335.0, 345.0):
        tf_cases.append((1500, fs_half, 300.0, 2, 0, None))
    for win_ms in (1.0, 20.0, 600.0):
        tf_cases.append((1200, 400.0, win_ms, 2, 0, None))
    for breaks in ([], [500], [500, 501, 900], list(range(0, 1200, 130)), [0], [1199]):
        tf_cases.append((1200, 400.0, 300.0, 2, 1, breaks))
    for n, fs, win_ms, hop_div, smoothing, breaks in tf_cases:
        x = signal(rng, n, fs) if n else np.zeros(0)
        t = 12.5 + np.arange(n) / fs
        b = None if breaks is None else np.array(breaks, dtype=np.int64)
        res, err = call(spectral.freq_time, x, t, fs, win_ms, hop_div, smoothing, b)
        out["freq_time"].append({"n": n, "fs": hexs(np.float64(fs)), "win_ms": win_ms, "hop_div": hop_div,
                                 "smoothing": smoothing, "breaks": breaks, "x": hexs(x), "t": hexs(t),
                                 "out": tf_out(res, err)})

    json.dump(out, open(sys.argv[1], "w", encoding="utf-8"), separators=(",", ":"))
    print({k: len(v) for k, v in out.items()})


if __name__ == "__main__":
    main()
