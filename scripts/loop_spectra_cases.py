#!/usr/bin/env python3
"""Cas de test de `loop_spectra` et `estimate` de `analysis/loop.py` (étape 4c-B(b)) pour `rust/otb-analysis/src/open_loop.rs`.

Usage : loop_spectra_cases.py SORTIE.json

`loop_spectra` : boucles fermées fabriquées (consigne en marches, gyro filtré avec bruit, termes P, I, D, F cohérents), à plusieurs
cadences et longueurs de fenêtre (arrondi au pair de `seg_s × fs`, fenêtres < 8 points, signal plus court que la fenêtre), nombres de blocs
(1, 2, 3, 8, 20, 0 : Python lève), seuils `min_input` et `max_input`, raccords, instrument, terme D nul, NaN, tailles différentes, valeurs
arrondies en `float32` puis relues en `float64` comme les journaux réels.
`estimate` : les spectres ci-dessus (sorties de Python, relus tels quels par Rust) et des spectres tirés au hasard avec une
cohérence réglée (régime où `valid` n'est pas vide), avec ou sans bloc retiré (`leave_out`), terme D nul, un seul bloc.

Un flottant s'écrit en hexadécimal ; un complexe en deux tableaux ; `"error": true` = Python a levé une exception. Le test
`rust/otb-analysis/tests/open_loop_spectra.rs` mesure l'écart à Python (FFT : L-048 ; produits en FMA : L-051).
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, call, ref
from loop_cases import cx
from scipy.signal import lfilter

from opentunebox.analysis import loop
from opentunebox.log.pid_terms import PidTerms

NAN = float("nan")


def closed_loop(rng, n, fs, f32=True, with_d=True, noise=1.0, amp=250.0):
    """(r, y, p, i, d, f) d'une boucle P-I-D fabriquée : y suit r à travers un passe-bas, plus du bruit."""
    r = np.zeros(n)
    k = 0
    while k < n:
        hold = int(rng.uniform(0.12, 0.4) * fs)
        r[k:k + hold] = rng.choice([-1.0, 1.0, 0.0]) * rng.uniform(5, amp)
        k += hold + int(rng.uniform(0.05, 0.3) * fs)
    w = 2 * np.pi * 40.0 / fs
    y = lfilter([w / (w + 1)], [1.0, -(1.0 - w / (w + 1))], r) + rng.normal(0, noise * 4, n)
    e = r - y
    p = 0.8 * e
    i = np.cumsum(0.02 * e) * 0.1
    d = -0.3 * np.gradient(y) * (1 if with_d else 0)
    f = 0.1 * np.gradient(r)
    out = [r, y, p, i, d, f]
    if f32:
        out = [a.astype(np.float32).astype(np.float64) for a in out]
    return out


def enc_spec(s) -> dict:
    nb = s.n_blocks
    return {"freqs": ref(s.freqs), "fs": hx(s.fs), "n_windows": int(s.n_windows), "has_d": bool(s.has_d), "nb": nb,
            "blocks": {k: cx(v) for k, v in s.blocks.items()}}


def main():
    rng = np.random.default_rng(20261107)
    out = {"spectra": [], "estimate": []}
    specs = []  # (index dans out["spectra"], objet Python) des spectres réussis

    def sp(sig, fs, seg_s, label, min_input=20.0, max_input=None, blocks=8, breaks=None, instrument=None, trunc=None):
        r, y, p, i, d, f = sig
        terms = PidTerms(p, i, d, f, instrument=instrument)
        res, e = call(loop.loop_spectra, r, y, terms, fs, seg_s, min_input, max_input, blocks, breaks)
        rec = {"label": label, "r": ref(r), "y": ref(y), "p": ref(p), "i": ref(i), "d": ref(d), "f": ref(f),
               "z": None if instrument is None else ref(instrument), "fs": hx(fs), "seg_s": hx(seg_s), "min_input": hx(min_input),
               "max_input": None if max_input is None else hx(max_input), "blocks": blocks,
               "breaks": None if breaks is None else [int(b) for b in breaks], "error": e, "none": (not e) and res is None,
               "out": enc_spec(res) if (not e and res is not None) else None}
        if rec["out"] is not None:
            specs.append((len(out["spectra"]), res))
        out["spectra"].append(rec)

    # fenêtres de L points à plusieurs cadences, blocs, seuils
    for fs, seg in ((500.0, 0.064), (1000.0, 0.064), (1000.0, 0.1), (2000.0, 0.032), (1000.0, 0.0405), (1000.0, 0.0415), (1000.0, 0.16)):
        sig = closed_loop(rng, int(3 * fs), fs)
        for blocks in (8, 3):
            sp(sig, fs, seg, f"fs={fs} seg={seg} blocs={blocks}", blocks=blocks)
    sig = closed_loop(rng, 2500, 1000.0)
    for blocks in (1, 2, 3, 8, 20, 0, -1):
        sp(sig, 1000.0, 0.064, f"blocs={blocks}", blocks=blocks)
    for mi, ma in ((20.0, None), (100.0, None), (0.0, None), (1e9, None), (20.0, 200.0), (20.0, 50.0), (0.0, 0.0), (NAN, None), (20.0, NAN)):
        sp(sig, 1000.0, 0.064, f"min={mi} max={ma}", min_input=mi, max_input=ma)
    sig64 = closed_loop(rng, 2500, 1000.0, f32=False)
    sp(sig64, 1000.0, 0.064, "float64 pur")
    # raccords : fenêtre qui en contient un, raccord exactement au début ou à la fin d'une fenêtre
    for br in ([500], [500, 1000, 2000], [32, 64], [0], [2499], [10 ** 6], np.array([], dtype=int), [31], [64, 96]):
        sp(sig, 1000.0, 0.064, f"raccords {list(br)[:4]}", breaks=br)
    # une seule fenêtre dépasse le seuil ; un terme D nul sur une partie seulement ; termes P ou I d'un seul point
    r1 = np.zeros(2500)
    r1[5] = 200.0  # une seule fenêtre (début 0) contient cet échantillon
    one = closed_loop(rng, 2500, 1000.0)
    sp((r1,) + tuple(one[1:]), 1000.0, 0.064, "une seule fenêtre au-dessus du seuil")
    r2 = r1.copy()
    r2[1505] = 200.0
    sp((r2,) + tuple(one[1:]), 1000.0, 0.064, "deux fenêtres au-dessus du seuil")
    dh = one[4].copy()
    dh[1250:] = 0.0
    sp((one[0], one[1], one[2], one[3], dh, one[5]), 1000.0, 0.064, "D nul sur la moitié")
    sp((one[0], one[1], one[2][:1], one[3], one[4], one[5]), 1000.0, 0.064, "P d'un point (diffusé)")
    sp((one[0], one[1], one[2], one[3][:1], one[4], one[5]), 1000.0, 0.064, "I d'un point (diffusé)")
    sp((one[0], one[1], one[2][:1], one[3][:1], one[4], one[5]), 1000.0, 0.064, "P et I d'un point")
    sp((one[0], one[1], one[2][:0], one[3][:1], one[4], one[5]), 1000.0, 0.064, "P vide, I d'un point")
    sp((one[0], one[1], one[2][:2], one[3][:3], one[4], one[5]), 1000.0, 0.064, "P et I de tailles 2 et 3")
    # instrument
    z = rng.normal(0, 100, 2500)
    sp(sig, 1000.0, 0.064, "instrument", instrument=z)
    sp(sig, 1000.0, 0.064, "instrument plus court", instrument=z[:1500])
    # terme D nul, termes plus courts que la consigne, NaN
    sp(closed_loop(rng, 2500, 1000.0, with_d=False), 1000.0, 0.064, "D nul")
    r, y, p, i, d, f = sig
    sp((r, y, p[:1500], i, d, f), 1000.0, 0.064, "P plus court")
    sp((r[:1500], y, p, i, d, f), 1000.0, 0.064, "consigne plus courte")
    rn = r.copy()
    rn[1234] = NAN
    sp((rn, y, p, i, d, f), 1000.0, 0.064, "NaN dans la consigne")
    yn = y.copy()
    yn[2000] = NAN
    sp((r, yn, p, i, d, f), 1000.0, 0.064, "NaN dans le gyro")
    dn = d.copy()
    dn[10] = NAN
    sp((r, y, p, i, dn, f), 1000.0, 0.064, "NaN dans D")
    ri = r.copy()
    ri[1500] = np.inf
    sp((ri, y, p, i, d, f), 1000.0, 0.064, "infini dans la consigne")
    # peu de fenêtres : 0, 1, 2, 3 sur le signal
    for n in (40, 64, 95, 96, 128, 160, 200):
        sp(closed_loop(rng, n, 1000.0), 1000.0, 0.064, f"n={n}")
    for seg in (0.007, 0.008, 0.0075, 0.0085, 0.0, -1.0, NAN):
        sp(sig, 1000.0, seg, f"seg_s={seg}")
    sp(sig, NAN, 0.064, "fs NaN")
    sp(sig, 0.0, 0.064, "fs nul")
    # beaucoup de fenêtres, bloc de tailles inégales
    sp(closed_loop(rng, 6000, 1000.0), 1000.0, 0.064, "6 s, blocs inégaux", blocks=7)
    sp(closed_loop(rng, 5000, 1000.0, amp=10.0), 1000.0, 0.064, "consigne faible (peu de fenêtres)")

    # ---- estimate : spectres de Python, avec et sans bloc retiré
    def est(spec_idx, spec, leave_out, label):
        res, e = call(loop.estimate, spec, leave_out)
        rec = {"label": label, "spec": spec_idx, "leave_out": leave_out, "error": e, "out": None}
        if not e:
            rec["out"] = {"Ce": cx(res.Ce), "Cd": cx(res.Cd), "Cf": cx(res.Cf), "G": cx(res.G), "T": cx(res.T), "L": cx(res.L),
                          "S_a": cx(res.S_a), "L_c": cx(res.L_c), "err_g": ref(res.err_g),
                          "coh": {k: ref(v) for k, v in res.coh.items()}, "valid": [bool(v) for v in res.valid]}
        out["estimate"].append(rec)

    for idx, spec in specs:
        est(idx, spec, None, "tous les blocs")
        if idx % 3 != 0:  # les variantes avec un bloc retiré sur un spectre sur trois (taille du fichier)
            continue
        if spec.n_blocks >= 2:
            est(idx, spec, 0, "bloc 0 retiré")
            est(idx, spec, spec.n_blocks - 1, "dernier bloc retiré")
        if spec.n_blocks == 1:
            est(idx, spec, 0, "seul bloc retiré")
        est(idx, spec, 99, "bloc inexistant retiré")

    # spectres tirés au hasard (indices à partir de 1000 : `spec` est alors le spectre inclus dans le cas)
    out["random_specs"] = []

    def rnd(nf, nb, fs, n_windows, has_d, coh, label, df=None, f0=0.0, zero=None):
        f = (f0 + np.arange(nf) * (df if df else fs / 2 / max(nf - 1, 1))).astype(float)

        def power():
            return rng.uniform(0.97, 1.03, (nb, nf)) * rng.uniform(0.5, 5.0) + 0j  # puissances voisines : le lissage garde la cohérence

        def mk(xx, yy, c):
            ph = np.exp(1j * (rng.uniform(-3, 3) + 0.02 * np.arange(nf)))[None, :] * np.ones((nb, 1))  # phase lente : le lissage garde la cohérence
            return c * np.sqrt(xx.real * yy.real) * ph

        blocks = {k: np.zeros((nb, nf), complex) for k in loop._PAIRS}
        rr, yy, ee, uu, pp, dd, zz = (power() for _ in range(7))
        blocks.update({"rr": rr, "yy": yy, "ee": ee, "uu": uu, "pp": pp, "dd": dd if has_d else dd * 0, "zz": zz})
        for k, (a, b) in {"ry": ("rr", "yy"), "rf": ("rr", "rr"), "zy": ("zz", "yy"), "zu": ("zz", "uu"), "zp": ("zz", "pp"),
                          "zd": ("zz", "dd"), "ze": ("zz", "ee"), "ep": ("ee", "pp"), "yd": ("yy", "dd")}.items():
            blocks[k] = mk(blocks[a], blocks[b], coh)
        if zero is not None:  # une case de puissance nulle (Ce ou Cd infinie), coherences laissées
            blocks[zero[0]][:, zero[1]:] = 0  # à partir d'une case : le lissage (≥ 5 points) ne la comble pas
        spec = loop.LoopSpectra(f, fs, blocks, n_windows, has_d)
        rec = enc_spec(spec)
        rec["label"] = label
        out["random_specs"].append(rec)
        j = len(out["random_specs"]) - 1
        for lo in (None, 0, nb - 1):
            res, e = call(loop.estimate, spec, lo)
            r2 = {"label": f"aléatoire {label}", "spec": 1000 + j, "leave_out": lo, "error": e, "out": None}
            if not e:
                r2["out"] = {"Ce": cx(res.Ce), "Cd": cx(res.Cd), "Cf": cx(res.Cf), "G": cx(res.G), "T": cx(res.T), "L": cx(res.L),
                             "S_a": cx(res.S_a), "L_c": cx(res.L_c), "err_g": ref(res.err_g),
                             "coh": {k: ref(v) for k, v in res.coh.items()}, "valid": [bool(v) for v in res.valid]}
            out["estimate"].append(r2)

    for coh in (0.99, 0.9, 0.85, 0.79, 0.5, 0.0, 1.0):
        rnd(40, 8, 1000.0, 120, True, coh, f"coh={coh}")
    rnd(40, 8, 1000.0, 120, False, 0.95, "D nul")
    rnd(40, 1, 1000.0, 40, True, 0.95, "un bloc")
    rnd(40, 3, 1000.0, 40, True, 0.97, "3 blocs")
    rnd(40, 8, 400.0, 120, True, 0.97, "fs = 400 (0,4 fs = 160 Hz)")
    rnd(40, 8, 100.0, 120, True, 0.97, "fs = 100 (0,4 fs = 40 Hz)")
    rnd(40, 8, 1000.0, 120, True, 0.97, "df = 0,5 Hz", df=0.5)
    rnd(40, 8, 1000.0, 120, True, 0.97, "df = 7 Hz", df=7.0)
    rnd(40, 8, 1000.0, 2, True, 0.97, "2 fenêtres")
    rnd(40, 8, 1000.0, 1000, True, 0.97, "1000 fenêtres")
    rnd(5, 4, 1000.0, 40, True, 0.97, "5 fréquences")
    rnd(2, 4, 1000.0, 40, True, 0.97, "2 fréquences")
    rnd(1, 4, 1000.0, 40, True, 0.97, "1 fréquence")
    rnd(1, 4, 1000.0, 40, True, 0.97, "1 fréquence non nulle", f0=50.0)
    rnd(40, 8, 1000.0, 120, True, 0.97, "df = 2 Hz (première fréquence valide pile à 2 Hz)", df=2.0)
    rnd(40, 8, 1000.0, 120, True, 0.97, "ee nul à partir d'une case", df=1.0, zero=("ee", 20))
    rnd(40, 8, 1000.0, 120, True, 0.97, "yy nul à partir d'une case", df=1.0, zero=("yy", 22))
    rnd(40, 8, 1000.0, 120, True, 0.97, "zu nul à partir d'une case", df=1.0, zero=("zu", 24))
    rnd(40, 8, 1000.0, 120, True, 0.97, "dd nul à partir d'une case", df=1.0, zero=("dd", 26))
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
