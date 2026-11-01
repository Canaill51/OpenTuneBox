#!/usr/bin/env python3
"""Cas de test de `analysis/step.py` (phase 4c-B(d)) pour `rust/otb-analysis/src/step.rs`.

Usage : step_cases.py SORTIE.json

`gradient` (`np.gradient(f, x)`, pas égaux ou non, `t_ms` réels de plusieurs cadences, moins de 2 points), `step_metrics` (réponses de premier
et deuxième ordre, jamais à 50 %, niveaux atteints exactement, hors de ±5 % à la fin, NaN, vide), `shape_metrics` (de 0 à 120 réponses :
blocs du jackknife, latence NaN, moins de 3 instants), `window_starts` et `input_windows` (raccords, seuils, NaN) et `step_response` entier
(boucles fermées simulées, avec ou sans termes PID, note de repli, raccords, régularisation, correction du palier, fenêtres trop courtes,
aucune fenêtre retenue, `smooth > 1`), `box_smooth_same` (`np.convolve(gy, ones(n)/n, mode="same")` du lissage du gyro) et
`segment_metrics` (pic et latence de chaque réponse, `gui/tools/_step_common.py`).

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
from opentunebox.gui.tools._step_common import segment_metrics
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
    out = {"gradient": [], "metrics": [], "shape": [], "starts": [], "response": [], "box": [], "segment": []}

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
    for n_smooth in (2, 5, 20, 40, 60):
        sr(sig, f"smooth = {n_smooth}", seg_s=0.5, smooth=n_smooth)
    sr(sims["b"], "smooth = 20, fenêtres de 2 s", smooth=20)
    sr(sims["c"], "smooth = 61 (impair), sans termes", terms="none", seg_s=0.5, smooth=61)
    sr(tuple(a[:40] for a in sig), "smooth = 60 plus long que le signal (gyro rallongé)", seg_s=0.1, smooth=60)
    sr(sig, "réponse 0 ms", seg_s=0.5, resp_ms=0.0)
    sr(sig, "réponse négative", seg_s=0.5, resp_ms=-100.0)
    sr(sig, "fs NaN", seg_s=0.5, fs=NAN)
    r, y, p, i, d, f = sig
    yn = y.copy()
    yn[1500] = NAN
    sr((r, yn, p, i, d, f), "NaN dans le gyro", seg_s=0.5)
    sr((r[:1200], y, p, i, d, f), "consigne plus courte que le gyro", seg_s=0.5)
    sr((r, y[:1200], p, i, d, f), "gyro plus court que la consigne", seg_s=0.5)

    # ---- lissage du gyro : np.convolve(gy, ones(n)/n, mode="same"), n pair ou impair, signal plus court que le noyau
    def bx(gy, n, label):
        gy = np.asarray(gy, float)
        r, e = call(lambda g, k: np.convolve(g, np.ones(k) / k, mode="same"), gy, n)
        out["box"].append({"label": label, "gy": ref(gy), "n": n, "error": e, "out": None if e else ref(r)})

    for nn in (1, 2, 3, 19, 20, 21, 60, 61, 500, 2500):
        for k in (2, 3, 5, 20, 40, 60, 61):
            bx(rng.normal(0, 200, nn), k, f"{nn} points, noyau {k}")
    bx(np.zeros(0), 20, "signal vide")
    bx(np.full(100, 3.0), 20, "constante")
    bx(np.r_[np.zeros(50), np.full(50, 1e6)], 40, "marche")
    bx(np.r_[rng.normal(size=30), NAN, rng.normal(size=30)], 20, "NaN dans le signal")

    # ---- segment_metrics
    def sg(t, R, label):
        t = np.asarray(t, float)
        R = np.asarray(R, float)
        r, e = call(segment_metrics, t, R)
        out["segment"].append({"label": label, "t": ref(t), "R": ref(R.ravel()), "rows": int(R.shape[0]), "error": e,
                               "out": None if e else {"peak": ref(r[0]), "lat": ref(r[1])}})

    for fs in (250.0, 1000.0, 3200.0):
        t = tgrid(fs)
        R = np.array([first_order(t, tau, delay=d, over=o) for tau, d, o in
                      ((15, 0, 0), (30, 5, 0.3), (8, 2, 0.6), (60, 0, 0), (20, 10, 1.2))])
        sg(t, R, f"cinq réponses simulées fs={fs}")
        sg(t, R + rng.normal(0, 0.05, R.shape), f"bruitées fs={fs}")
    t = tgrid(1000.0)
    sg(t, np.clip(t / 20, 0, 1)[None, :], "rampe (latence 10 ms, pic 1)")
    sg(t, (1 - np.exp(-t / 10) * np.cos(t / 8))[None, :], "oscillation amortie")
    sg(t, np.zeros((3, t.size)), "jamais à 50 % (toutes à 0)")
    sg(t, np.full((2, t.size), 0.7), "à 50 % dès le premier instant (k = 0)")
    sg(t, np.full((2, t.size), 0.5), "exactement 0,5 partout")
    R = np.zeros((1, t.size))
    R[0, 100:] = 1.0
    sg(t, R, "marche à 100 ms (r1 - r0 = 1)")
    R = np.zeros((1, t.size))
    R[0, 100:] = 0.5
    sg(t, R, "marche exactement à 0,5")
    R = np.zeros((1, t.size))
    R[0, 400:] = 1.0
    R[0, 30] = 3.0
    sg(t, R, "pic avant la latence, horizon de 3 × latence")
    R = np.zeros((1, t.size))
    R[0, 40:] = 1.0
    R[0, 300] = 5.0
    sg(t, R, "pic après l'horizon de 150 ms (ignoré)")
    R = np.r_[np.zeros(300), np.full(t.size - 300, 1.0)][None, :]
    R[0, 100] = NAN
    sg(t, R, "NaN avant l'horizon")
    R = np.r_[np.zeros(40), np.full(t.size - 40, 1.0)][None, :]
    R[0, 450] = NAN
    sg(t, R, "NaN après l'horizon de 150 ms")
    R = np.full((1, t.size), NAN)
    sg(t, R, "que des NaN")
    sg(t, np.zeros((0, t.size)), "aucune réponse")
    sg(t[:1], np.array([[0.8], [0.2]]), "un seul instant")
    sg(t[:2], np.array([[0.2, 0.9], [0.9, 0.9]]), "deux instants")
    sg(t, np.vstack([np.zeros(t.size), np.clip(t / 40, 0, 1.4), np.full(t.size, 0.6)]), "mélange : jamais, rampe, déjà au-dessus")
    sg(t[:300], np.vstack([np.clip(t[:300] / 20, 0, 1)] * 2), "horizon au-delà de la fin")
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
