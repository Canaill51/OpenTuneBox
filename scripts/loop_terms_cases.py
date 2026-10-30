#!/usr/bin/env python3
"""Cas de test de `loop_from_terms` (et du jackknife) de `analysis/loop.py` (étape 4c-B(c)) pour `rust/otb-analysis/src/open_loop.rs`.

Usage : loop_terms_cases.py SORTIE.json

Boucles fermées **simulées** (régulateur P + I + D + F sur un drone du premier ordre avec retard, bruit sur le drone) à plusieurs
gains, bruits et durées : la boucle ouverte mesurée a alors un croisement et des marges. Cas de repli : trop peu de fenêtres, cohérence
insuffisante (forte perturbation), terme D nul (`d_ratio` = 0), moins de 3 blocs (pas de jackknife), repli avec notes déjà présentes,
`approx` issu de `loop_from_T` ou vide, instrument, raccords, seuils de consigne, NaN. Un flottant s'écrit en hexadécimal ;
`"error": true` = Python a levé une exception. Le test `rust/otb-analysis/tests/open_loop_terms.rs` mesure l'écart à Python.
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, call, ref
from loop_cases import cx

from opentunebox.analysis import loop
from opentunebox.log.pid_terms import PidTerms

NAN = float("nan")


def simulate(rng, n, fs, kp=3.0, ki=2.0, kd=0.002, kf=0.05, tau=0.05, delay=2, noise=2.0, amp=250.0):
    """Boucle fermée simulée : (r, y, p, i, d, f) en float32 relus en float64, comme un journal réel."""
    r = np.zeros(n)
    k = 0
    while k < n:
        hold = int(rng.uniform(0.12, 0.4) * fs)
        r[k:k + hold] = rng.choice([-1.0, 1.0, 0.0]) * rng.uniform(5, amp)
        k += hold + int(rng.uniform(0.05, 0.3) * fs)
    dt = 1.0 / fs
    y = np.zeros(n)
    p = np.zeros(n)
    i = np.zeros(n)
    d = np.zeros(n)
    f = np.zeros(n)
    u = np.zeros(n)
    integ = 0.0
    for t in range(1, n):
        e = r[t - 1] - y[t - 1]
        integ += ki * e * dt
        p[t - 1] = kp * e
        i[t - 1] = integ
        d[t - 1] = -kd * (y[t - 1] - (y[t - 2] if t > 1 else 0.0)) / dt
        f[t - 1] = kf * (r[t - 1] - (r[t - 2] if t > 1 else 0.0)) / dt
        u[t - 1] = p[t - 1] + i[t - 1] + d[t - 1] + f[t - 1]
        ud = u[t - 1 - delay] if t - 1 - delay >= 0 else 0.0
        y[t] = y[t - 1] + dt * (ud - y[t - 1]) / tau + rng.normal(0, noise) * dt * 20
    return [a.astype(np.float32).astype(np.float64) for a in (r, y, p, i, d, f)]


def enc_lm(lm) -> dict:
    return {"method": lm.method, "notes": list(lm.notes), "band": [hx(lm.band_hz[0]), hx(lm.band_hz[1])],
            "bandwidth": hx(lm.bandwidth_hz), "tracking": hx(lm.tracking_peak_db), "ms_db": hx(lm.sensitivity_peak_db),
            "ms_hz": hx(lm.sensitivity_peak_hz), "edge": bool(lm.sensitivity_peak_at_edge), "crossover": hx(lm.crossover_hz),
            "pm": hx(lm.phase_margin_deg), "gm": hx(lm.gain_margin_db), "d_ratio": hx(lm.d_ratio),
            "std": {k: hx(v) for k, v in lm.std.items()}, "freqs": ref(lm.freqs), "T": cx(lm.T), "coh": ref(lm.coherence),
            "L": cx(lm.L), "valid": [bool(v) for v in lm.valid]}


def main():
    rng = np.random.default_rng(20261108)
    out = {"cases": []}

    def lt(sig, fs, label, seg_s=0.5, min_input=20.0, max_input=None, breaks=None, instrument=None, approx="empty"):
        r, y, p, i, d, f = sig
        terms = PidTerms(p, i, d, f, instrument=instrument)
        if approx == "empty":
            ap = loop.LoopMetrics()
        else:  # un `approx` issu de loop_from_T (bande passante et pic de suivi réels), avec une note déjà présente
            fr = np.linspace(0.0, 120.0, 61)
            ap = loop.loop_from_T(fr, 1.0 / (1 + 1j * fr / 35.0), np.full(fr.size, 0.9))
            ap.notes.append("termes PID non fournis")
        res, e = call(loop.loop_from_terms, r, y, terms, fs, ap, seg_s, min_input, max_input, breaks)
        out["cases"].append({"label": label, "r": ref(r), "y": ref(y), "p": ref(p), "i": ref(i), "d": ref(d), "f": ref(f),
                             "z": None if instrument is None else ref(instrument), "fs": hx(fs), "seg_s": hx(seg_s),
                             "min_input": hx(min_input), "max_input": None if max_input is None else hx(max_input),
                             "breaks": None if breaks is None else [int(b) for b in breaks],
                             "approx": None if approx == "empty" else enc_lm(ap) | {"notes_before": 1},
                             "approx_kind": approx, "error": e, "out": None if e else enc_lm(res)})

    fs = 250.0
    base = simulate(rng, 2500, fs)
    lt(base, fs, "boucle simulée de référence")
    lt(base, fs, "approx issu de loop_from_T", approx="T")
    for kp in (0.5, 1.5, 4.0, 7.0):
        for tau in (0.06, 0.1):
            lt(simulate(rng, 1800, fs, kp=kp, tau=tau), fs, f"kp={kp} tau={tau}")
    for noise in (0.0, 0.5, 5.0, 40.0):
        lt(simulate(rng, 1800, fs, noise=noise), fs, f"bruit={noise}", approx="T")
    for kd in (0.0, 0.0005, 0.01):
        lt(simulate(rng, 1800, fs, kd=kd), fs, f"kd={kd}")
    for n in (150, 260, 400, 520, 1000):
        lt(simulate(rng, n, fs), fs, f"n={n} (peu de fenêtres, peu de blocs)")
    for n, nz in ((250, 0.0), (250, 0.2), (250, 1.0), (300, 0.0), (310, 0.2), (190, 0.0)):  # 1 à 3 fenêtres : 2 blocs seulement, pas de jackknife
        lt(simulate(rng, n, fs, noise=nz), fs, f"n={n} bruit={nz} (peu de blocs)")
    lt(simulate(rng, 2500, 500.0), 500.0, "fs=500", seg_s=0.25)
    lt(simulate(rng, 2500, 1000.0), 1000.0, "fs=1000", seg_s=0.128)
    r, y, p, i, d, f = base
    for mi, ma in ((20.0, None), (100.0, None), (0.0, None), (1e9, None), (20.0, 150.0), (20.0, 40.0)):
        lt(base, fs, f"min={mi} max={ma}", min_input=mi, max_input=ma)
    lt(base, fs, "raccord", breaks=[1200])
    lt(base, fs, "raccords nombreux", breaks=[300, 600, 900, 1200, 1500, 1800, 2100, 2400])
    lt(base, fs, "instrument (commande radio fabriquée)", instrument=r + rng.normal(0, 10, r.size))
    lt(base, fs, "instrument indépendant", instrument=rng.normal(0, 100, r.size))
    lt((r, y, p, i, np.zeros_like(d), f), fs, "D nul")
    lt((r, y, np.zeros_like(p), i, d, f), fs, "P nul")
    lt((r, y, p, i, d, np.zeros_like(f)), fs, "F nul")
    lt((r, np.zeros_like(y), p, i, d, f), fs, "gyro nul")
    lt((r, y, p, i, d, f), fs, "seg_s = 0,5 avec approx T", approx="T")
    yn = y.copy()
    yn[1800] = NAN
    lt((r, yn, p, i, d, f), fs, "NaN dans le gyro")
    rn = r.copy()
    rn[1234] = NAN
    lt((rn, y, p, i, d, f), fs, "NaN dans la consigne")
    lt((r, y, p[:1500], i, d, f), fs, "P plus court (diffusion impossible)")
    lt((r, y, p[:1], i, d, f), fs, "P d'un point")
    for seg in (0.004, 0.0, NAN):
        lt(base, fs, f"seg_s={seg}", seg_s=seg)
    # beaucoup d'essais tirés au hasard : régimes variés (marges présentes, absentes, bord de bande)
    for _ in range(30):
        lt(simulate(rng, int(rng.integers(1200, 2500)), fs, kp=float(rng.uniform(0.5, 6)), ki=float(rng.uniform(0, 8)),
                    kd=float(rng.choice([0.0, 0.001, 0.004])), kf=float(rng.uniform(0, 0.2)), tau=float(rng.uniform(0.04, 0.1)),
                    delay=int(rng.integers(0, 3)), noise=float(rng.uniform(0, 6))), fs, "tirage libre")
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
