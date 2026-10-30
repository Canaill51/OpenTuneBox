#!/usr/bin/env python3
"""Cas de test de `analysis/step.py` (phase 4c-B(d)) pour `rust/otb-analysis/src/step.rs`.

Usage : step_cases.py SORTIE.json

`gradient` (`np.gradient(f, x)`, pas égaux ou non, `t_ms` réels de plusieurs cadences, moins de 2 points), `step_metrics` (réponses de premier
et deuxième ordre, jamais à 50 %, niveaux atteints exactement, hors de ±5 % à la fin, NaN, vide), `shape_metrics` (de 0 à 120 réponses :
blocs du jackknife, latence NaN, moins de 3 instants), `window_starts` et `input_windows` (raccords, seuils, NaN) et `step_response` entier
(boucles fermées simulées, avec ou sans termes PID, note de repli, raccords, régularisation, correction du palier, fenêtres trop courtes,
aucune fenêtre retenue, `smooth > 1` non porté).

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception. Le test `rust/otb-analysis/tests/step.rs` mesure l'écart à
Python.
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, call, ref
from loop_terms_cases import enc_lm, simulate

from opentunebox.analysis import step
from opentunebox.log.pid_terms import PidTerms

NAN = float("nan")


def enc_metrics(m) -> dict:
    return {k: hx(getattr(m, k)) for k in ("peak", "peak_ms", "latency_ms", "rise_ms", "settling_ms", "overshoot_pct", "ripple",
                                           "ripple_std", "ripple_ms", "waves", "waves_ratio")}


def first_order(t, tau, delay=0.0, over=0.0, wn=0.0):
    x = np.clip(t - delay, 0, None)
    r = 1 - np.exp(-x / tau)
    if over:
        r = r + over * np.exp(-x / (tau * 3)) * np.sin(x / (tau * 0.7))
    return r


def tgrid(fs, resp_ms=500.0):
    lr = int(round(resp_ms * fs / 1000.0)) + 1
    return np.arange(lr) * 1000.0 / fs


def main():
    rng = np.random.default_rng(20261109)
    out = {"gradient": [], "metrics": [], "shape": [], "starts": [], "response": []}

    # ---- gradient
    def gr(f, x, label):
        r, e = call(np.gradient, np.asarray(f, float), np.asarray(x, float))
        out["gradient"].append({"label": label, "f": ref(np.asarray(f, float)), "x": ref(np.asarray(x, float)), "error": e,
                                "out": None if e else ref(r)})

    for fs in (250.0, 500.0, 1000.0, 1984.0, 2000.0, 3200.0, 8000.0):
        t = tgrid(fs)
        gr(np.sin(t / 30), t, f"t_ms réels fs={fs}")
    x = np.cumsum(rng.uniform(0.5, 2.0, 60))
    gr(rng.normal(size=60), x, "pas variables")
    gr(rng.normal(size=60), np.arange(60) * 0.5, "pas égaux exacts")
    gr(rng.normal(size=60), np.arange(60) * 0.1, "pas 0,1 (égaux à l'arrondi près)")
    for n in (0, 1, 2, 3, 4):
        gr(rng.normal(size=n), np.arange(n) * 1.0, f"{n} points")
    gr(rng.normal(size=5), np.array([0.0, 1.0, 1.0, 2.0, 3.0]), "pas nul")
    gr(np.array([1.0, NAN, 3.0, 4.0]), np.arange(4.0), "NaN dans f")
    gr(rng.normal(size=4), np.array([0.0, 1.0, NAN, 3.0]), "NaN dans x")
    gr(rng.normal(size=4), np.array([3.0, 2.0, 1.0, 0.0]), "x décroissant")

    # ---- step_metrics
    def sm(t, r, label):
        res, e = call(step.step_metrics, np.asarray(t, float), np.asarray(r, float))
        out["metrics"].append({"label": label, "t": ref(np.asarray(t, float)), "r": ref(np.asarray(r, float)), "error": e,
                               "out": None if e else enc_metrics(res)})

    for fs in (500.0, 1000.0, 2000.0):
        t = tgrid(fs)
        for tau in (5.0, 20.0, 60.0, 200.0, 1000.0):
            for over in (0.0, 0.3):
                sm(t, first_order(t, tau, delay=3.0, over=over), f"ordre 1 tau={tau} dépassement={over} fs={fs}")
        sm(t, first_order(t, 15.0) + rng.normal(0, 0.03, t.size), f"bruit fs={fs}")
    t = tgrid(1000.0)
    sm(t, np.zeros(t.size), "toujours nulle")
    sm(t, np.ones(t.size), "toujours à 1")
    sm(t, np.full(t.size, 0.3), "toujours à 0,3 (jamais 50 %)")
    sm(t, np.full(t.size, 2.0), "toujours à 2")
    sm(t, 1.0 + 0.04 * np.sin(t / 10), "dans ±5 %")
    sm(t, 1.0 + 0.06 * np.sin(t / 10), "hors de ±5 % à la fin")
    sm(t, np.where(t < 100, 0.2, 1.4), "marche à 100 ms, palier 1,4")
    sm(t, np.where(t < 100, 0.2, 1.0), "marche à 100 ms, palier 1")
    r = np.linspace(0, 1.2, t.size)
    sm(t, r, "rampe")
    r2 = np.zeros(t.size)
    r2[10:] = 0.5
    sm(t, r2, "50 % atteint exactement, r1 = r0 ensuite")
    r3 = np.zeros(t.size)
    r3[0] = 0.5
    sm(t, r3, "50 % dès le premier point")
    r4 = first_order(t, 20.0)
    r4[200] = NAN
    sm(t, r4, "un NaN")
    r5 = first_order(t, 20.0)
    r5[100] = np.inf
    sm(t, r5, "un infini")
    sm(t[:0], np.zeros(0), "vide")
    sm(t[:1], np.array([0.7]), "un point")
    sm(t[:2], np.array([0.2, 0.8]), "deux points")
    sm(t[:5], np.array([0.0, 0.2, 0.6, 1.0, 1.1]), "cinq points")
    sm(t, first_order(t, 100.0, over=0.5), "pic hors de la fenêtre de 150 ms (drone lent)")
    r6 = first_order(t, 8.0)
    r6[300:] += 0.2  # dérive lente du terme I après la montée
    sm(t, r6, "dérive après la montée")
    r7 = first_order(t, 20.0)
    r7[-1] = 1.5
    sm(t, r7, "sortie de ±5 % au dernier point")
    sm(t, np.array(list(first_order(t, 20.0)[:-3]) + [1.5, 1.0, 1.0]), "sortie de ±5 % à l'avant-dernier rang")

    # ---- shape_metrics
    def shp(t, R, label, latency=None):
        R = np.asarray(R, float)
        mean = R.mean(axis=0) if R.shape[0] else np.zeros(t.size)
        m = step.step_metrics(t, mean) if latency is None else step.StepMetrics(latency_ms=latency)
        before = enc_metrics(m)
        res, e = call(step.shape_metrics, t, R, m)
        out["shape"].append({"label": label, "t": ref(t), "R": ref(R), "rows": int(R.shape[0]),
                             "latency": hx(m.latency_ms), "error": e,
                             "before": before,
                             "out": None if e else enc_metrics(m)})

    for fs in (1000.0, 2000.0):
        t = tgrid(fs)
        for k in (1, 2, 3, 4, 7, 8, 12, 15, 16, 39, 40, 41, 80, 120):
            base = first_order(t, 18.0, delay=2.0, over=0.25)
            R = base[None, :] + rng.normal(0, 0.15, (k, t.size)) + 0.05 * np.sin(t / 7)[None, :] * rng.normal(size=(k, 1))
            if fs == 2000.0 and k not in (8, 40, 41):
                continue
            shp(t, R, f"{k} réponses fs={fs}")
    t = tgrid(1000.0)
    shp(t, np.zeros((0, t.size)), "aucune réponse")
    shp(t, rng.normal(size=(20, t.size)), "bruit pur (latence NaN ou lointaine)")
    shp(t, np.full((20, t.size), 0.3) + rng.normal(0, 0.01, (20, t.size)), "jamais 50 %")
    shp(t[:2], rng.normal(size=(20, 2)), "deux instants")
    shp(t[:2], np.array([0.2, 0.8]) + rng.normal(0, 0.01, (20, 2)), "deux instants, latence finie")
    shp(t[:3], rng.normal(size=(20, 3)) + np.array([0.1, 0.6, 1.0]), "trois instants")
    shp(t, first_order(t, 18.0)[None, :] + rng.normal(0, 0.1, (40, t.size)), "latence imposée à 12 ms", latency=12.0)
    shp(t, first_order(t, 18.0)[None, :] + rng.normal(0, 0.1, (40, t.size)), "latence imposée à 0", latency=0.0)
    shp(t, first_order(t, 18.0)[None, :] + rng.normal(0, 0.1, (40, t.size)), "latence imposée à 90 ms (palier absent)", latency=90.0)
    shp(t, first_order(t, 18.0)[None, :] + rng.normal(0, 0.1, (40, t.size)), "latence imposée à 400 ms (aucun instant de vague)", latency=400.0)
    Rn = first_order(t, 18.0)[None, :] + rng.normal(0, 0.1, (40, t.size))
    Rn[7, 100] = NAN
    shp(t, Rn, "un NaN dans une réponse")
    Rs = np.tile(first_order(t, 18.0), (40, 1))
    shp(t, Rs, "réponses identiques (bruit nul)")
    # montée en trois blocs de forme différente : cassure variable d'un bloc à l'autre
    Rv = first_order(t, 18.0)[None, :] + np.where(np.arange(40)[:, None] < 20, 0.0, 0.2) * np.exp(-t / 60)[None, :]
    shp(t, Rv, "deux populations de réponses")

    # ---- window_starts / input_windows
    def ws(sp, fs, seg_s, label, hop_ms=62.5, min_input=20.0, max_input=None, breaks=None):
        sp = np.asarray(sp, float)
        r, e = call(lambda: step._window_starts(sp, fs, int(round(seg_s * fs)), hop_ms, min_input, max_input, breaks))
        iw, e2 = call(step.input_windows, sp, fs, seg_s, min_input, hop_ms, breaks)
        out["starts"].append({"label": label, "sp": ref(sp), "fs": hx(fs), "seg_s": hx(seg_s), "hop_ms": hx(hop_ms),
                              "min_input": hx(min_input), "max_input": None if max_input is None else hx(max_input),
                              "breaks": None if breaks is None else [int(b) for b in breaks], "error": e, "error_iw": e2,
                              "out": None if e else [int(v) for v in r], "iw": None if e2 else int(iw)})

    sp0 = np.zeros(3000)
    k = 0
    while k < 3000:
        h = int(rng.uniform(100, 400))
        sp0[k:k + h] = rng.choice([-1, 0, 1]) * rng.uniform(5, 300)
        k += h + int(rng.uniform(50, 300))
    for fs, seg in ((1000.0, 0.5), (1000.0, 2.0), (500.0, 1.0), (2000.0, 0.25), (1000.0, 0.0405)):
        ws(sp0, fs, seg, f"fs={fs} seg={seg}")
    for hop in (62.5, 10.0, 0.1, 0.0, -5.0, 1000.0, 62.4999, 1.5):
        ws(sp0, 1000.0, 0.5, f"hop={hop}", hop_ms=hop)
    for mi, ma in ((20.0, None), (100.0, None), (0.0, None), (1e9, None), (20.0, 150.0), (20.0, 20.0), (NAN, None), (20.0, NAN)):
        ws(sp0, 1000.0, 0.5, f"min={mi} max={ma}", min_input=mi, max_input=ma)
    for br in ([500], [500, 1500], [0], [2999], [250, 251], np.array([], dtype=int), [10 ** 7]):
        ws(sp0, 1000.0, 0.5, f"raccords {list(br)[:3]}", breaks=br)
    spn = sp0.copy()
    spn[700] = NAN
    ws(spn, 1000.0, 0.5, "NaN dans la consigne")
    spi = sp0.copy()
    spi[700] = np.inf
    ws(spi, 1000.0, 0.5, "infini dans la consigne")
    for n in (0, 1, 499, 500, 501, 562, 563):
        ws(sp0[:n], 1000.0, 0.5, f"n={n}")
    ws(sp0, NAN, 0.5, "fs NaN")
    ws(sp0, 1000.0, NAN, "seg_s NaN")
    ws(sp0, 1000.0, 0.0, "seg_s nul (fenêtre vide : Python lève)")
    ws(sp0, 1000.0, -0.5, "seg_s négatif (Python lève)")
    ws(sp0, 1000.0, 0.0, "seg_s nul, seuil 0", min_input=0.0)
    ws(np.full(3000, 25.0), 1000.0, 0.5, "consigne constante à 25 °/s")
    ws(np.full(3000, 20.0), 1000.0, 0.5, "consigne constante à 20 °/s (frontière >=)")
    ws(np.full(3000, -20.0), 1000.0, 0.5, "consigne constante à −20 °/s")
    ws(np.full(3000, 20.0), 1000.0, 0.5, "crête égale à max_input (frontière <)", max_input=20.0)
    ws(np.full(3000, 25.0), 1000.0, 0.5, "crête 25, max_input 25", min_input=0.0, max_input=25.0)

    # ---- step_response entier
    fs = 250.0
    sims = {"a": simulate(rng, 2500, fs), "b": simulate(rng, 2500, fs, kp=1.0, tau=0.1, noise=0.5),
            "c": simulate(rng, 1800, fs, kp=5.0, tau=0.06, noise=3.0)}

    def sr(sig, label, terms="terms", seg_s=2.0, min_input=20.0, max_input=None, resp_ms=500.0, hop_ms=62.5, reg=1e-4, smooth=0,
           y_correction=False, breaks=None, instrument=None, note="", fs=fs):
        r, y, p, i, d, f = sig
        pt = None if terms == "none" else PidTerms(p, i, d, f, note=note if terms == "note" else "", instrument=instrument)
        res, e = call(step.step_response, r, y, fs, seg_s, min_input, max_input, resp_ms, hop_ms, reg, smooth, y_correction, pt,
                      breaks)
        rec = {"label": label, "r": ref(r), "y": ref(y), "p": ref(p), "i": ref(i), "d": ref(d), "f": ref(f),
               "z": None if instrument is None else ref(instrument), "terms": terms, "note": note, "fs": hx(fs), "seg_s": hx(seg_s),
               "min_input": hx(min_input), "max_input": None if max_input is None else hx(max_input), "resp_ms": hx(resp_ms),
               "hop_ms": hx(hop_ms), "reg": hx(reg), "smooth": smooth, "y_correction": y_correction,
               "breaks": None if breaks is None else [int(b) for b in breaks], "error": e, "out": None}
        if not e:
            lm = res.loop
            rec["out"] = {"t": ref(res.t_ms), "R": ref(res.responses), "mean": ref(res.mean), "std": ref(res.std), "n": int(res.n),
                          "metrics": enc_metrics(res.metrics), "max_inputs": ref(res.max_inputs), "k": int(res.responses.shape[0]),
                          "lm": enc_lm(lm)}
        out["response"].append(rec)

    for key, sig in sims.items():
        sr(sig, f"{key} : termes PID, fenêtres de 2 s")
        sr(sig, f"{key} : sans termes", terms="none")
        sr(sig, f"{key} : note de repli", terms="note", note="terme D non enregistré")
        sr(sig, f"{key} : fenêtres de 0,5 s", seg_s=0.5)
        sr(sig, f"{key} : 0,5 s, correction du palier", seg_s=0.5, y_correction=True)
        sr(sig, f"{key} : 0,5 s, réponse de 300 ms", seg_s=0.5, resp_ms=300.0)
    sig = sims["a"]
    sr(sig, "regularisation 1e-2", seg_s=0.5, reg=1e-2)
    sr(sig, "regularisation 0", seg_s=0.5, reg=0.0)
    sr(sig, "pas de 20 ms", seg_s=0.5, hop_ms=20.0)
    sr(sig, "pas de 250 ms", seg_s=0.5, hop_ms=250.0)
    sr(sig, "consigne min 100 °/s", seg_s=0.5, min_input=100.0)
    sr(sig, "consigne max 150 °/s", seg_s=0.5, max_input=150.0)
    sr(sig, "raccord", seg_s=0.5, breaks=[1200])
    sr(sig, "instrument", seg_s=0.5, instrument=rng.normal(0, 100, 2500))
    sr(sig, "aucune fenêtre retenue (consigne min 1e9)", seg_s=0.5, min_input=1e9)
    sr(sig, "signal plus court que la fenêtre", seg_s=20.0)
    for nn in (500, 501, 499, 563):  # n = L à 2 s et fs = 250 : une seule fenêtre possible
        sr(simulate(rng, nn, fs, amp=400.0), f"n={nn} (fenêtre de 2 s)", min_input=5.0)
    sr(tuple(a[:500] for a in sig), "n = L exactement, signal de la boucle a", min_input=5.0)
    sr(sig, "seg_s négatif", seg_s=-0.5)
    sr(sig, "seg_s nul", seg_s=0.0)
    sr(sig, "smooth = 1", seg_s=0.5, smooth=1)
    sr(sig, "smooth = 5 (non porté)", seg_s=0.5, smooth=5)
    sr(sig, "réponse 0 ms", seg_s=0.5, resp_ms=0.0)
    sr(sig, "réponse négative", seg_s=0.5, resp_ms=-100.0)
    sr(sig, "fs NaN", seg_s=0.5, fs=NAN)
    r, y, p, i, d, f = sig
    yn = y.copy()
    yn[1500] = NAN
    sr((r, yn, p, i, d, f), "NaN dans le gyro", seg_s=0.5)
    sr((r[:1200], y, p, i, d, f), "consigne plus courte que le gyro", seg_s=0.5)
    sr((r, y[:1200], p, i, d, f), "gyro plus court que la consigne", seg_s=0.5)
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
