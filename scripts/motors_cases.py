#!/usr/bin/env python3
"""Cas de test de `analysis/motors.py` pour `rust/otb-analysis/src/motors.rs` (4c-3).

Usage : motors_cases.py SORTIE.json

- `motor_hz`, `dyn_notch_centres`, `hover_motor_hz` : petits journaux fabriqués (télémétrie eRPM présente ou non, KV et tension,
  multiplicateur, gaz et moteurs hors bornes, NaN, versions 4.x / 2025.12 / 2026.6, nombre de notchs et de champs debug).
- `distribution` : valeurs `float32` avec NaN, infinis, zéros, négatifs, doublons ; fréquences NaN et infinies.
- « scènes » (bruit, harmoniques moteur qui suivent un régime variable, résonances fixes) passées à `motor_noise`,
  `harmonic_prominence`, `rpm_attenuation`, `rpm_filtered_spectrum`, `dyn_notch_targets`, `line_shapes`, `line_heights` et
  `spectrum_lines` avec des variantes : nombre d'harmoniques, durée de fenêtre, raccords, bandes, réglages RPM (harmoniques,
  poids, Q, cadence PID), moteurs de même vitesse, NaN dans la vitesse, signal plus court qu'une fenêtre.

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception. Les courbes d'un journal et les vitesses
moteur sont des float32 (8 caractères par valeur). Le test `rust/otb-analysis/tests/motors.rs` exige l'égalité pour ce qui est
exact et une tolérance mesurée (FFT, `hypot`, `log10`, `pow` : L-048) pour le reste.
"""
import dataclasses
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from bf_sim_cases import enc_settings as enc_sim_settings
from bf_sim_cases import fft_settings
from flight_stats_cases import INPUTS, call, enc_log, make_log, ref, with_

from opentunebox.analysis import motors
from opentunebox.log.settings import parse_settings

V26, V4, V2512 = (2026, 6, 2), (4, 5, 1), (2025, 12, 0)


def enc_settings(s) -> dict:
    return {**enc_sim_settings(s), "motor_kv": s.motor_kv}


def f64r(a) -> str:
    return ref(np.asarray(a, dtype=np.float64))


def f32r(a) -> str:
    return ref(np.asarray(a, dtype=np.float32), True)


def rec_rows(a, f32=True):
    return None if a is None else {"k": int(a.shape[0]), "x": f32r(a) if f32 else f64r(a)}


def log_record(lg) -> dict:
    d = enc_log(lg)
    d["settings"] = enc_settings(lg.settings)
    return d


# ---------------------------------------------------------------- scènes de signaux


def scene(rng, n, fs, f0, k=4, lines=(130.0, 420.0), nan=False, same=False, noise=4.0, fixed=False):
    """(x float32, mhz float32 (k, n)) : bruit, harmoniques 1 à 3 d'un régime qui varie lentement, résonances fixes."""
    t = np.arange(n) / fs
    drift = 1.0 + 0.06 * np.sin(2 * np.pi * 0.7 * t) + 0.02 * np.sin(2 * np.pi * 3.1 * t)
    spread = np.zeros(k) if same else np.linspace(-0.04, 0.04, k)
    mhz = np.vstack([f0 * (1.0 + s) * drift for s in spread])
    phase = 2 * np.pi * np.cumsum(mhz.mean(axis=0)) / fs
    x = rng.normal(0, noise, n) + 8 * np.sin(3.0 * t * 2 * np.pi) * 6
    for h, a in ((1, 9.0), (2, 5.0), (3, 2.5)):
        x += a * np.sin(h * phase)
    for fl in lines:
        if fl < fs / 2 * 0.95:
            x += 3.0 * np.sin(2 * np.pi * fl * t + 0.3)
    mhz = mhz.astype(np.float32)
    if fixed:  # régime constant, identique pour tous les moteurs : fh / df tombe pile sur des valeurs choisies
        mhz = np.full((k, n), f0, np.float32)
    if nan:
        mhz[1, n // 3: n // 3 + 5] = np.nan
    return x.astype(np.float32), mhz


def rsets(rng):
    return {
        "3h": fft_settings(rng, V26, rpm_harmonics=3, rpm_weights=[100, 60, 30]),
        "1h": fft_settings(rng, V26, rpm_harmonics=1, rpm_weights=[100]),
        "2h_q7": fft_settings(rng, V26, rpm_harmonics=2, rpm_weights=[50, 100], rpm_q=700, rpm_min_hz=50, rpm_fade_hz=10),
        "pid1k": fft_settings(rng, V26, rpm_harmonics=2, rpm_weights=[100, 100], pid_denom=8),
        "poids0": fft_settings(rng, V26, rpm_harmonics=2, rpm_weights=[0, 100]),
        "0h": fft_settings(rng, V26, rpm_harmonics=0),
    }


def main():
    rng = np.random.default_rng(20261106)
    out = {"logs": [], "log_names": [], "motor_hz": [], "dyn_centres": [], "hover": [], "distribution": [], "settings": {},
           "scenes": [], "noise": [], "prominence": [], "attenuation": [], "filtered": [], "targets": [], "shapes": [],
           "heights": [], "lines": [], "weight": []}

    # ---- journaux
    names = []

    def add(name, lg):
        out["logs"].append(log_record(lg))
        names.append(name)
        return len(names) - 1

    base_s = parse_settings({})
    n = 400

    def mk(**kw):
        return make_log(rng, n, 100.0, flags=kw.pop("flags", "all"), **kw)

    def mot(k=4, fill=None):
        m = (rng.uniform(5, 95, (k, n)) if fill is None else np.full((k, n), fill)).astype(np.float32)
        return m

    hz = (np.vstack([200 + 40 * np.sin(np.arange(n) / 17.0 + i) for i in range(4)])).astype(np.float32)
    logs = {
        "erpm": with_(mk(), motor_hz=hz),
        "kv_vbat": with_(mk(), settings=dataclasses.replace(base_s, motor_kv=2300)),
        "kv_sans_vbat": with_(mk(vbat=False), settings=dataclasses.replace(base_s, motor_kv=2300)),
        "sans_kv": mk(),
        "kv_neg": with_(mk(), settings=dataclasses.replace(base_s, motor_kv=-5)),
        "sans_moteur": mk(motors=0),
        "moteurs_hors_bornes": with_(mk(), motors=np.linspace(-40, 160, 4 * n).reshape(4, n).astype(np.float32),
                                     settings=dataclasses.replace(base_s, motor_kv=1800)),
        "moteurs_nan": with_(mk(), motors=np.where(np.arange(4 * n).reshape(4, n) % 37 == 0, np.nan, mot()).astype(np.float32),
                             settings=dataclasses.replace(base_s, motor_kv=1800)),
        "vbat_nan": with_(mk(), vbat=np.where(np.arange(n) % 11 == 0, np.nan, 15.5).astype(np.float32),
                          settings=dataclasses.replace(base_s, motor_kv=1800)),
        "un_moteur": with_(mk(), motors=mot(1), settings=dataclasses.replace(base_s, motor_kv=2000)),
    }
    for nm, lg in logs.items():
        add(nm, lg)
    for i, (nm, lg) in enumerate(logs.items()):
        for mult in (None, 2.0, 1.7, 0.0, -1.0, 3.3333333333333335):
            r, e = call(motors.motor_hz, lg, mult)
            rec = {"log": i, "mult": None if mult is None else hx(mult), "error": e}
            if not e:
                a, est = r
                assert a is None or a.dtype == np.float32, (nm, a.dtype)
                rec["out"] = {"hz": rec_rows(a), "est": bool(est)}
            out["motor_hz"].append(rec)

    # ---- centres des notchs dynamiques
    for ver in (V4, V2512, V26, (4, 4, 0)):
        for mode in ("FFT_FREQ", "NONE"):
            for count in (0, 1, 3, 4, 6, 9):
                for krows in (0, 1, 3, 5, 8):
                    s = fft_settings(rng, ver, debug_mode=mode, count=count)
                    dbg = rng.uniform(50, 600, (krows, 8)).astype(np.float32)  # 8 échantillons : seuls réglages et debug comptent
                    lg = with_(make_log(rng, 8, 100.0, flags="all"), settings=s, debug=dbg)
                    i = add(f"debug {ver} {mode} count={count} rows={krows}", lg)
                    r, e = call(motors.dyn_notch_centres, lg)
                    rec = {"log": i, "error": e}
                    if not e:
                        rec["out"] = None if r is None else {"k": int(r.shape[0]), "x": f64r(r)}
                    out["dyn_centres"].append(rec)

    # ---- hover_motor_hz
    big = 600
    for nm, kw in {"mesure_stable": {}, "mesure_courte": {"short": True}, "mesure_basse": {"low": True},
                   "estimee": {"est": True}, "gaz_bas": {"thr": 3.0}, "gaz_5_pile": {"thr": 5.0},
                   "seuil_10": {"edge10": True}, "99_trames": {"n99": True}, "100_trames": {"n100": True}}.items():
        lg = make_log(rng, big, 100.0, flags="all")
        hzv = (np.vstack([180 + 20 * np.sin(np.arange(big) / 31.0 + i) for i in range(4)])).astype(np.float32)
        thr = np.full(big, kw.get("thr", 40.0), np.float32)
        if kw.get("short"):
            thr[60:] = 0.0
        if kw.get("low"):
            hzv = (hzv * 0.03).astype(np.float32)
        if kw.get("edge10"):
            hzv = np.full((4, big), 10.0, np.float32)
            hzv[:, ::2] = np.float32(10.0000009)
        if kw.get("n99") or kw.get("n100"):
            thr[:] = 0.0
            thr[: 99 if kw.get("n99") else 100] = 40.0
        lg = with_(lg, motor_hz=None if kw.get("est") else hzv, throttle=thr)
        i = add("hover " + nm, lg)
        r, e = call(motors.hover_motor_hz, lg)
        out["hover"].append({"log": i, "error": e, "out": None if (e or r is None) else hx(r), "none": (not e and r is None)})
    out["log_names"] = names

    # ---- distribution
    vals = [rng.uniform(0, 600, 300).astype(np.float32), np.array([], np.float32), np.zeros(10, np.float32),
            np.array([np.nan, np.inf, -np.inf, 0.0, -1.0, 5.0, 5.0, 5.0], np.float32),
            np.array([1, 2, 3, 4, 5], np.float32), -np.abs(rng.normal(0, 1, 20)).astype(np.float32),
            np.repeat(np.float32(100.0), 50), np.array([np.nan] * 4, np.float32)]
    fq = [np.linspace(0, 700, 71), np.array([]), np.array([np.nan, np.inf, -np.inf, 0.0, 5.0, 4.999999999, 5.000000001]),
          np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), np.array([100.0, 99.99999, 100.00001])]
    for v in vals:
        for f in fq:
            r, e = call(motors.distribution, v, f)
            out["distribution"].append({"values": f32r(v), "freqs": f64r(f), "error": e, "out": None if e else f64r(r)})

    # ---- rpm_weight_needed
    for p in (-5.0, 0.0, 1e-12, 0.5, 3.0, 6.0, 10.0, 20.0, 40.0, 100.0, 1000.0, float("nan"), float("inf"), -float("inf")):
        out["weight"].append({"in": hx(p), "out": hx(motors.rpm_weight_needed(p))})

    # ---- scènes
    rs = rsets(rng)
    out["settings"] = {k: enc_settings(v) for k, v in rs.items()}
    scenes = [("2kHz", 2000.0, 3000, 180.0, {}), ("4kHz", 4000.0, 4000, 300.0, {}), ("1kHz", 1000.0, 2500, 120.0, {}),
              ("8kHz", 8000.0, 5000, 600.0, {}), ("nan", 2000.0, 3000, 180.0, {"nan": True}),
              ("memes_moteurs", 2000.0, 3000, 180.0, {"same": True}), ("court", 2000.0, 400, 180.0, {}),
              ("sans_bruit", 2000.0, 3000, 180.0, {"noise": 1e-3}), ("bas_regime", 2000.0, 3000, 40.0, {"lines": (30.0, 70.0)}),
              ("repliement", 1000.0, 2500, 300.0, {}),  # harmoniques 2 et 3 au-delà de Nyquist : images repliées
              ("constant_500Hz", 1000.0, 2000, 500.0, {"fixed": True}),  # fh = Nyquist pile
              ("constant_200Hz", 2000.0, 2000, 200.0, {"fixed": True}),  # fh = min_hz pile
              ("pair_10Hz", 1024.0, 2048, 10.0, {"fixed": True}),  # L = 256, df = 4 : fh / df = 2,5, 5, 7,5
              ("pair_18Hz", 1024.0, 2048, 18.0, {"fixed": True})]  # fh / df = 4,5
    sc = []
    for nm, fs, ns, f0, kw in scenes:
        x, mhz = scene(rng, ns, fs, f0, **kw)
        out["scenes"].append({"name": nm, "fs": hx(fs), "n": ns, "x": f32r(x), "mhz": rec_rows(mhz)})
        sc.append((nm, fs, ns, x, mhz))

    def brk(ns, which):
        return None if which == "none" else ([] if which == "vide" else [ns // 3, 2 * ns // 3])

    # motor_noise / harmonic_prominence
    for si, (nm, fs, ns, x, mhz) in enumerate(sc):
        for n_harm in (1, 3, 8):
            for seg_ms in (300.0, 120.0):
                for min_hz in (20.0, 250.0):
                    for which in ("none", "deux"):
                        if which == "deux" and (seg_ms != 300.0 or min_hz != 20.0):
                            continue
                        b = brk(ns, which)
                        args = (np.asarray(x, np.float64), mhz, fs, n_harm, seg_ms, min_hz,
                                None if b is None else np.asarray(b, np.int64))
                        base = {"scene": si, "n_harm": n_harm, "seg_ms": hx(seg_ms), "min_hz": hx(min_hz), "breaks": b}
                        r, e = call(motors.motor_noise, *args)
                        out["noise"].append({**base, "error": e, "out": None if e else {
                            "h": [int(v) for v in r.harmonics], "mean": f64r(r.mean_db), "std": f64r(r.std_db)}})
                        r, e = call(motors.harmonic_prominence, *args)
                        out["prominence"].append({**base, "error": e, "out": None if e else f64r(r)})
    # n_harm nul
    x, mhz = sc[0][3], sc[0][4]
    for n_harm in (0, -2):
        r, e = call(motors.motor_noise, np.asarray(x, np.float64), mhz, 2000.0, n_harm)
        out["noise"].append({"scene": 0, "n_harm": n_harm, "seg_ms": hx(300.0), "min_hz": hx(20.0), "breaks": None, "error": e,
                             "out": None if e else {"h": [int(v) for v in r.harmonics], "mean": f64r(r.mean_db), "std": f64r(r.std_db)}})
        r, e = call(motors.harmonic_prominence, np.asarray(x, np.float64), mhz, 2000.0, n_harm)
        out["prominence"].append({"scene": 0, "n_harm": n_harm, "seg_ms": hx(300.0), "min_hz": hx(20.0), "breaks": None,
                                  "error": e, "out": None if e else f64r(r)})

    # rpm_attenuation direct
    for key, s in rs.items():
        for nwin, k in ((1, 1), (7, 4), (20, 2)):
            f = np.linspace(0.0, 1000.0, 129)
            mh = rng.uniform(80, 700, (k, nwin))
            r, e = call(motors.rpm_attenuation, f, mh, s)
            out["attenuation"].append({"settings": key, "k": k, "nwin": nwin, "f": f64r(f), "mhz": f64r(mh), "error": e,
                                       "out": None if e else f64r(r)})
    f = np.linspace(0.0, 1000.0, 33)
    for mh in (np.array([[np.nan, 200.0, 300.0]]), np.array([[0.0, 1e-9, -50.0]]), np.array([[5000.0, 1e5, 250.0]])):
        r, e = call(motors.rpm_attenuation, f, mh, rs["3h"])
        out["attenuation"].append({"settings": "3h", "k": 1, "nwin": 3, "f": f64r(f), "mhz": f64r(mh), "error": e,
                                   "out": None if e else f64r(r)})

    # rpm_filtered_spectrum / dyn_notch_targets / raies
    for si, (nm, fs, ns, x, mhz) in enumerate(sc):
        xd = np.asarray(x, np.float64)
        for key in (None, "3h", "1h", "2h_q7", "pid1k", "poids0", "0h"):
            if key not in (None, "3h") and si not in (0, 1, 4, 5):
                continue
            s = None if key is None else rs[key]
            for use_m in (True, False):
                if not use_m and key != "3h":
                    continue
                for which in ("none", "deux"):
                    if which == "deux" and si not in (0, 4):
                        continue
                    b = brk(ns, which)
                    bb = None if b is None else np.asarray(b, np.int64)
                    r, e = call(motors.rpm_filtered_spectrum, xd, mhz if use_m else None, fs, s, 300.0, bb)
                    out["filtered"].append({"scene": si, "rpm": key, "use_m": use_m, "breaks": b, "seg_ms": hx(300.0), "error": e, "out": None if e else {
                        "f": f64r(r[0]), "s": f64r(r[1])}})
                    for band in ((80.0, 600.0), (0.0, 80.0), (100.0, 110.0), (50.0, 5000.0), (300.0, 200.0), (50.75, 600.0)):
                        if band != (80.0, 600.0) and (which != "none" or key not in (None, "3h")):
                            continue
                        for bin_hz in (18.5, 7.0):
                            if bin_hz != 18.5 and band != (80.0, 600.0):
                                continue
                            r, e = call(motors.dyn_notch_targets, xd, mhz if use_m else None, fs, band, s, 300.0, bin_hz, bb)
                            out["targets"].append({"scene": si, "rpm": key, "use_m": use_m, "breaks": b, "seg_ms": hx(300.0), "band": [hx(band[0]), hx(band[1])],
                                                   "bin_hz": hx(bin_hz), "error": e, "out": None if e else {
                                "f": f64r(r.freqs), "db": f64r(r.ratios_db), "band": [hx(r.band[0]), hx(r.band[1])]}})
        freqs_sets = ([130.0, 420.0, 180.0, 333.0], [f0 * 2 for f0 in (60.0, 90.0)], [0.0, 1.0, 5000.0, float("nan")], [],
                      list(np.arange(15.0, 120.0, 1.7)))
        for n_harm in (0, 1, 3, 8):
            for key in (None, "3h", "2h_q7"):
                for which in ("none", "deux"):
                    if which == "deux" and (si not in (0, 4) or n_harm != 3):
                        continue
                    b = brk(ns, which)
                    bb = None if b is None else np.asarray(b, np.int64)
                    s = None if key is None else rs[key]
                    for fi, fr in enumerate(freqs_sets):
                        if fi > 0 and (n_harm != 3 or key != "3h" or which != "none"):
                            continue
                        r, e = call(motors.line_shapes, np.asarray(x, np.float64), mhz, fs, fr, n_harm, 300.0, bb, s)
                        out["shapes"].append({"scene": si, "n_harm": n_harm, "rpm": key, "breaks": b, "seg_ms": hx(300.0), "freqs": f64r(fr),
                                              "error": e, "out": None if e else f64r(np.asarray(r, float).ravel())})
                r, e = call(motors.spectrum_lines, xd, np.asarray(x, np.float64) * 0.7, mhz, fs, (80.0, 700.0), 6.0, n_harm, 300.0, None)
                out["lines"].append({"scene": si, "n_harm": n_harm, "xf": hx(0.7), "seg_ms": hx(300.0), "min_db": hx(6.0), "band": [hx(80.0), hx(700.0)], "error": e,
                                     "out": None if e else [[hx(l.freq_hz), hx(l.raw_db), hx(l.left_db)] for l in r]})
        for fr in ([130.0, 420.0, 180.0], [], list(np.arange(15.0, 120.0, 1.7))):
            r, e = call(motors.line_heights, xd, fs, fr, 300.0, None)
            out["heights"].append({"scene": si, "seg_ms": hx(300.0), "freqs": f64r(fr), "error": e, "out": None if e else f64r(r)})
        for min_db, band, n_harm in ((1.0, (0.0, 1000.0), 8), (20.0, (80.0, 700.0), 8), (6.0, (300.0, 200.0), 3), (0.0, (0.0, 400.0), 2)):
            r, e = call(motors.spectrum_lines, xd, xd * 0.5, mhz, fs, band, min_db, n_harm, 300.0, None)
            out["lines"].append({"scene": si, "n_harm": n_harm, "xf": hx(0.5), "seg_ms": hx(300.0), "min_db": hx(min_db), "band": [hx(band[0]), hx(band[1])], "error": e,
                                 "out": None if e else [[hx(l.freq_hz), hx(l.raw_db), hx(l.left_db)] for l in r]})


    # entrées dégénérées : Python lève (NaN dans la bande, pas nul, n_harm négatif sur un signal trop court)
    x1, mh1 = sc[0][3], sc[0][4]
    xd1 = np.asarray(x1, np.float64)
    nan_ = float("nan")
    for band, bin_hz in (((50.0, nan_), 18.5), ((nan_, 800.0), 18.5), ((50.0, 800.0), 0.0), ((50.0, 800.0), -18.5), ((50.0, float("inf")), 18.5)):
        r, e = call(motors.dyn_notch_targets, xd1, mh1, 2000.0, band, None, 300.0, bin_hz, None)
        out["targets"].append({"scene": 0, "rpm": None, "use_m": True, "breaks": None, "seg_ms": hx(300.0),
                               "band": [hx(band[0]), hx(band[1])], "bin_hz": hx(bin_hz), "error": e, "out": None if e else {
            "f": f64r(r.freqs), "db": f64r(r.ratios_db), "band": [hx(r.band[0]), hx(r.band[1])]}})
    r, e = call(motors.spectrum_lines, xd1, xd1 * 0.5, mh1, 2000.0, (20.0, nan_), 1.0, 3, 300.0, None)
    out["lines"].append({"scene": 0, "n_harm": 3, "xf": hx(0.5), "seg_ms": hx(300.0), "min_db": hx(1.0), "band": [hx(20.0), hx(nan_)],
                         "error": e, "out": None if e else [[hx(l.freq_hz), hx(l.raw_db), hx(l.left_db)] for l in r]})
    short = next(i for i, c in enumerate(sc) if c[0] == "court")
    for n_harm in (-1, -3):
        args = (np.asarray(sc[short][3], np.float64), sc[short][4], sc[short][1], n_harm, 300.0, 20.0, None)
        r, e = call(motors.motor_noise, *args)
        out["noise"].append({"scene": short, "n_harm": n_harm, "seg_ms": hx(300.0), "min_hz": hx(20.0), "breaks": None, "error": e,
                             "out": None if e else {"h": [int(v) for v in r.harmonics], "mean": f64r(r.mean_db), "std": f64r(r.std_db)}})
    # fenêtres de 16 points au minimum (`max(16, ...)`) : 2, 5, 8 et 8,5 ms à 2 kHz donnent 4, 10, 16 et 17 points
    x0, mh0 = sc[0][3], sc[0][4]
    xd0 = np.asarray(x0, np.float64)
    for seg_ms in (2.0, 5.0, 8.0, 8.5, 20.0):
        for key in (None, "3h"):
            s_ = None if key is None else rs[key]
            args = (xd0, mh0, 2000.0, 3, seg_ms, 20.0, None)
            r, e = call(motors.motor_noise, *args)
            out["noise"].append({"scene": 0, "n_harm": 3, "seg_ms": hx(seg_ms), "min_hz": hx(20.0), "breaks": None, "error": e,
                                 "out": None if e else {"h": [int(v) for v in r.harmonics], "mean": f64r(r.mean_db), "std": f64r(r.std_db)}})
            r, e = call(motors.harmonic_prominence, *args)
            out["prominence"].append({"scene": 0, "n_harm": 3, "seg_ms": hx(seg_ms), "min_hz": hx(20.0), "breaks": None, "error": e,
                                      "out": None if e else f64r(r)})
            r, e = call(motors.rpm_filtered_spectrum, xd0, mh0, 2000.0, s_, seg_ms, None)
            out["filtered"].append({"scene": 0, "rpm": key, "use_m": True, "breaks": None, "seg_ms": hx(seg_ms), "error": e,
                                    "out": None if e else {"f": f64r(r[0]), "s": f64r(r[1])}})
            fr = [130.0, 420.0, 180.0]
            r, e = call(motors.line_shapes, xd0, mh0, 2000.0, fr, 3, seg_ms, None, s_)
            out["shapes"].append({"scene": 0, "n_harm": 3, "rpm": key, "breaks": None, "seg_ms": hx(seg_ms), "freqs": f64r(fr),
                                  "error": e, "out": None if e else f64r(np.asarray(r, float).ravel())})
            r, e = call(motors.line_heights, xd0, 2000.0, fr, seg_ms, None)
            out["heights"].append({"scene": 0, "seg_ms": hx(seg_ms), "freqs": f64r(fr), "error": e, "out": None if e else f64r(r)})
            r, e = call(motors.spectrum_lines, xd0, xd0 * 0.5, mh0, 2000.0, (0.0, 900.0), 1.0, 3, seg_ms, None)
            out["lines"].append({"scene": 0, "n_harm": 3, "xf": hx(0.5), "seg_ms": hx(seg_ms), "min_db": hx(1.0), "band": [hx(0.0), hx(900.0)],
                                 "error": e, "out": None if e else [[hx(l.freq_hz), hx(l.raw_db), hx(l.left_db)] for l in r]})
            r, e = call(motors.dyn_notch_targets, xd0, mh0, 2000.0, (50.0, 800.0), s_, seg_ms, 18.5, None)
            out["targets"].append({"scene": 0, "rpm": key, "use_m": True, "breaks": None, "seg_ms": hx(seg_ms),
                                   "band": [hx(50.0), hx(800.0)], "bin_hz": hx(18.5), "error": e, "out": None if e else {
                "f": f64r(r.freqs), "db": f64r(r.ratios_db), "band": [hx(r.band[0]), hx(r.band[1])]}})
    # vitesse moteur constante : fh pile sur min_hz, sur Nyquist, ou à une demi-case (arrondi au pair)
    for nm in ("constant_500Hz", "constant_200Hz", "pair_10Hz", "pair_18Hz"):
        si = next(i for i, c in enumerate(sc) if c[0] == nm)
        _, fs_, ns_, x_, mh_ = sc[si]
        for seg_ms in (250.0, 300.0):
            for min_hz in (5.0, 200.0, 500.0):
                for n_harm in (1, 2, 3):
                    args = (np.asarray(x_, np.float64), mh_, fs_, n_harm, seg_ms, min_hz, None)
                    base = {"scene": si, "n_harm": n_harm, "seg_ms": hx(seg_ms), "min_hz": hx(min_hz), "breaks": None}
                    r, e = call(motors.motor_noise, *args)
                    out["noise"].append({**base, "error": e, "out": None if e else {
                        "h": [int(v) for v in r.harmonics], "mean": f64r(r.mean_db), "std": f64r(r.std_db)}})
                    r, e = call(motors.harmonic_prominence, *args)
                    out["prominence"].append({**base, "error": e, "out": None if e else f64r(r)})

    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
