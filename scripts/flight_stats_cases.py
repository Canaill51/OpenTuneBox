#!/usr/bin/env python3
"""Cas de test de `analysis/datasets.py`, `period.py` et `stats.py` pour `rust/otb-analysis/src/{datasets,period,stats}.rs` (4c-1).

Usage : flight_stats_cases.py SORTIE.json

Petits journaux fabriqués (pas de corpus) : vol complet, journaux de 1 à 25 points, sans tension ni courant, courant nul, tension
négative, sans mode (armé partout), sans moteur, NaN dans le gyro / la consigne / les gaz, valeurs EXACTEMENT sur les seuils
(gaz 8, 5 et 2 %, moteurs 98 et 8 %, consigne 100 et 30 °/s, erreur 300, gyro 1900), crash dont la durée vaut exactement
`0,8 × w` ou `w − 1` trames, gaz et manche hors bornes, périodes (`sel`) pleine, partielle, vide ou au-delà de la fin.

Un flottant s'écrit en hexadécimal (16 caractères, bits du float64) ; `"error": true` = Python a levé une exception. Les courbes
d'un journal sont des float32 écrits en float64 (exact). Le test `rust/otb-analysis/tests/flight_stats.rs` compare les sorties
au bit près (arithmétique float32 de numpy reproduite, voir `npstat.rs`).
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hexs, hx

from opentunebox.analysis import datasets, period, stats
from opentunebox.log.flightlog import FlightLog
from opentunebox.log.settings import parse_settings

INPUTS: dict[str, str] = {}
_IDS: dict[str, str] = {}


def ref(a, f32: bool = False) -> str:
    """Identifiant d'une entrée dans la table `inputs` du fichier : une entrée identique n'y est écrite qu'une fois.
    Un tableau float32 s'écrit sur 8 caractères par valeur (identifiant « s… »), un float64 sur 16 (« i… »)."""
    h = np.ascontiguousarray(a, dtype="<f4").tobytes().hex() if f32 else hexs(a)
    key = ("s" if f32 else "i") + h
    if key not in _IDS:
        _IDS[key] = f"{'s' if f32 else 'i'}{len(_IDS)}"
        INPUTS[_IDS[key]] = h
    return _IDS[key]


def call(fn, *args, **kw):
    """(résultat, False) ou (None, True) si Python lève une exception."""
    try:
        return fn(*args, **kw), False
    except Exception:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        return None, True


def make_log(rng, n, fs=100.0, *, flags="arm", motors=4, vbat=True, current=True, nan_gyro=False, nan_sp=False,
             nan_thr=False, scale=1.0, quiet=False) -> FlightLog:
    t = np.arange(n) / fs
    sp = np.zeros((3, n))
    if not quiet:
        for ax in range(3):
            k = 0
            while k < n:
                hold, gap = int(rng.uniform(0.15, 0.4) * fs), int(rng.uniform(0.1, 0.5) * fs)
                sp[ax, k:k + hold] = rng.choice([-1.0, 1.0]) * rng.uniform(5, 300) * scale
                k += hold + gap
    gyro = (sp + rng.normal(0, 8.0, (3, n))).astype(np.float32)
    sp = sp.astype(np.float32)
    thr = np.clip(30 + 25 * np.sin(2 * np.pi * 0.4 * t) + rng.normal(0, 2.0, n), 0, 100).astype(np.float32)
    thr[: max(1, n // 12)] = 0.0  # au sol au début
    mot = (np.clip(np.vstack([thr + rng.normal(0, 1.5, n) for _ in range(motors)]), 0, 100).astype(np.float32)
           if motors else np.zeros((0, n), np.float32))
    rc = np.vstack([sp / 2.0, 1000.0 + 10.0 * thr]).astype(np.float32)
    rc[0] *= 1.0 + 0.3 * rng.standard_normal(n).astype(np.float32)  # dépasse parfois ±500
    terms = [rng.normal(0, 30, (3, n)).astype(np.float32) for _ in range(4)]
    mf = {"arm": np.where(t > t[-1] * 0.05, 1, 0).astype(np.int32) if n else np.zeros(0, np.int32),
          "none": None, "all": np.ones(n, np.int32), "off": np.zeros(n, np.int32)}[flags]
    if nan_gyro:
        gyro[0, n // 3: n // 3 + 3] = np.nan
    if nan_sp:
        sp[1, n // 2] = np.nan
    if nan_thr:
        thr[n // 4] = np.nan
    vb = (np.float32(16.0) - np.linspace(0, 1.5, n, dtype=np.float32) + rng.normal(0, 0.02, n).astype(np.float32)) if vbat else None
    cu = (np.float32(20.0) + 8 * np.sin(2 * np.pi * 0.3 * t)).astype(np.float32) if current else None
    return FlightLog(Path("x.bbl"), 1, parse_settings({}), t, fs, gyro, None, sp, rc, *terms, thr, mot, None,
                     np.zeros((0, n), dtype=np.float32), vb, cu, None, mf, np.zeros((0, 4)), {}, frozenset())


def with_(lg: FlightLog, **kw) -> FlightLog:
    import dataclasses
    return dataclasses.replace(lg, **kw)


def enc_log(lg: FlightLog) -> dict:
    a = lambda x: None if x is None else {"k": int(x.shape[0]), "x": ref(x, True)}  # noqa: E731
    return {"n": lg.n, "fs": hx(lg.fs), "t": ref(lg.t), "gyro": a(lg.gyro), "setpoint": a(lg.setpoint),
            "gyro_unfilt": a(lg.gyro_unfilt), "motor_hz": a(lg.motor_hz), "rc_command": a(lg.rc_command), "p_term": a(lg.p_term), "i_term": a(lg.i_term), "d_term": a(lg.d_term),
            "f_term": a(lg.f_term), "throttle": ref(lg.throttle, True), "motors": a(lg.motors), "debug": a(lg.debug),
            "vbat": None if lg.vbat is None else ref(lg.vbat, True),
            "current": None if lg.current is None else ref(lg.current, True),
            "mode_flags": None if lg.mode_flags is None else ref(np.asarray(lg.mode_flags, dtype=np.float64))}


def ms(m) -> list:
    return [hx(m.mean), hx(m.sd), hx(m.max)]


def enc_stats(s) -> dict:
    return {"duration_s": hx(s.duration_s),
            "stick_hist": [[k, ref(v[0]), ref(v[1])] for k, v in s.stick_hist.items()],
            "terms": {k: [ms(m) for m in v] for k, v in s.terms.items()},
            "motors": [ms(m) for m in s.motors], "sat": hx(s.motor_saturation_pct), "imb": hx(s.motor_imbalance_pct),
            "throttle": ms(s.throttle), "vbat": None if s.vbat is None else ms(s.vbat),
            "vbat_min": None if s.vbat_min is None else hx(s.vbat_min),
            "current": None if s.current is None else ms(s.current), "mah": None if s.mah is None else hx(s.mah)}


def enc_pid_error(s) -> dict:
    return {"edges": ref(s.edges), "hist": [ref(h) for h in s.hist], "sd": [hx(v) for v in s.sd],
            "mean_abs": [hx(v) for v in s.mean_abs], "stick_bins": ref(s.stick_bins),
            "err_vs_stick": [ref(v) for v in s.err_vs_stick]}


def enc_period(p):
    return [hx(p[0]), hx(p[1])]


def enc_parts(d) -> dict:
    return {k: None if v is None else [hx(v[0]), hx(v[1])] for k, v in d.items()}


def sels(lg: FlightLog, full: bool) -> list:
    """Périodes essayées : la plupart des journaux n'en reçoivent que quelques-unes (le fichier reste léger)."""
    n = lg.n
    out = [None, [n // 4, max(n // 4, 3 * n // 4)], [n + 5, n + 9]]
    if full:
        out += [[0, n], [n, n], [0, 0], [1, min(n, 2)]]
        if n > 3:
            out += [[n // 3, n // 3 + 1], [0, n - 1]]
    return out


def to_slice(s):
    return None if s is None else slice(s[0], s[1])


def primitives(rng) -> dict:
    """Primitives float32 de numpy (`npstat.rs`) : chaque longueur de 0 à 300, puis des longueurs autour des coupures de la
    somme par paires (8, 128, 129, 256…) jusqu'à 2 100, sur trois signaux (normal, grand décalage, avec un NaN)."""
    import warnings
    warnings.simplefilter("ignore")
    lengths = sorted(set(range(0, 301)) | {383, 384, 385, 511, 512, 513, 767, 1000, 1023, 1024, 1025, 1500, 2047, 2048, 2100})
    sigs = {"normal": rng.normal(0, 40, 2100).astype(np.float32),
            "decale": (rng.normal(0, 0.5, 2100) + 1000.0).astype(np.float32),
            "nan": np.where(np.arange(2100) == 150, np.nan, rng.normal(0, 40, 2100)).astype(np.float32),
            "doublons": rng.integers(0, 6, 2100).astype(np.float32)}
    res: dict = {"lengths": lengths, "signals": {}}
    for nm, x in sigs.items():
        d = {"x": ref(x, True), "mean": [], "std": [], "median": [], "p95": [], "max": [], "min": []}
        for n in lengths:
            v = x[:n]
            d["mean"].append(hx(np.mean(v)))
            d["std"].append(hx(np.std(v)))
            d["median"].append(hx(np.median(v)))
            d["p95"].append(hx(np.percentile(v, 95)) if n else None)
            d["max"].append(hx(np.max(v)) if n else None)
            d["min"].append(hx(np.min(v)) if n else None)
        res["signals"][nm] = d
    # centiles d'autres rangs sur de petites longueurs (interpolation, t proche de 0,5)
    res["quantiles"] = []
    for q in (0.0, 5.0, 25.0, 50.0, 75.0, 95.0, 99.0, 100.0):
        for n in (1, 2, 3, 4, 5, 6, 7, 10, 11, 20, 21, 39, 40, 41, 100):
            v = sigs["normal"][:n]
            res["quantiles"].append({"q": hx(q), "n": n, "out": hx(np.percentile(v, q))})
    edges = np.linspace(-100, 100, 41)
    vals = np.concatenate([edges, edges - 1e-9, edges + 1e-9, [np.nan, np.inf, -np.inf, -100.0000001, 100.0000001],
                           rng.uniform(-110, 110, 500)]).astype(np.float32)
    res["histogram"] = {"values": ref(vals, True), "edges": ref(edges), "counts": [int(c) for c in np.histogram(vals, bins=edges)[0]]}
    sb = np.linspace(0, 100, 11)
    dv = np.concatenate([sb, [np.nan, np.inf, 1e9, 0.0, 99.99999, 100.0], rng.uniform(0, 120, 200)]).astype(np.float32)
    res["digitize"] = {"values": ref(dv, True), "bins": ref(sb), "out": [int(k) for k in np.digitize(dv, sb)]}
    res["linspace"] = [{"a": hx(a), "b": hx(b), "n": n, "out": ref(np.linspace(a, b, n))}
                       for a, b, n in ((-100, 100, 41), (0, 100, 41), (0, 100, 11), (-50, 50, 101), (-20, 20, 101), (-0.3, 0.7, 101),
                                       (0, 1, 7), (3, 3, 5), (0, 100, 2), (0, 100, 1), (1e-9, 1.0, 13), (1e-9, 1.0, 11))]
    cl = np.array([np.nan, -np.inf, np.inf, -0.0, 0.0, -100.0, 100.0, 100.00001, -100.00001, 5.5], dtype=np.float32)
    res["clip"] = {"values": ref(cl, True), "out": ref(np.clip(cl, -100, 100), True)}
    return res


def main():
    rng = np.random.default_rng(20261104)
    logs: list[FlightLog] = []
    names: list[str] = []

    def add(name, lg):
        logs.append(lg)
        names.append(name)
        return len(logs) - 1

    add("vol", make_log(rng, 1500))
    add("vol_long", make_log(rng, 2500, 250.0, scale=1.4))
    add("sans_vbat_courant", make_log(rng, 800, vbat=False, current=False))
    add("modes_absents", make_log(rng, 800, flags="none"))
    add("jamais_arme", make_log(rng, 800, flags="off"))
    add("arme_partout", make_log(rng, 800, flags="all"))
    add("sans_moteur", make_log(rng, 800, motors=0))
    add("deux_moteurs", make_log(rng, 800, motors=2))
    add("nan_gyro", make_log(rng, 800, nan_gyro=True))
    add("nan_consigne", make_log(rng, 800, nan_sp=True))
    add("nan_gaz", make_log(rng, 800, nan_thr=True))
    add("immobile", make_log(rng, 1500, quiet=True))
    for n in (1, 2, 3, 9, 10, 11, 12, 25, 63, 128, 129):
        add(f"n{n}", make_log(rng, n))
    add("courant_nul", with_(make_log(rng, 800), current=np.zeros(800, np.float32)))
    add("tension_negative", with_(make_log(rng, 800), vbat=-np.ones(800, np.float32)))
    add("tension_nulle", with_(make_log(rng, 800), vbat=np.zeros(800, np.float32)))
    add("courant_constant", with_(make_log(rng, 800), current=np.full(800, 7.5, np.float32)))
    add("courant_nan", with_(make_log(rng, 800), current=np.where(np.arange(800) == 100, np.nan, 3.0).astype(np.float32)))
    add("gros_manche", make_log(rng, 1200, scale=9.0))
    add("fs_nan", with_(make_log(rng, 800), fs=float("nan")))  # int(min_ms × fs / 1000) lève dans crash_time
    add("fs_inf", with_(make_log(rng, 800), fs=float("inf")))

    # ---- seuils exacts (float32) : chaque valeur est EXACTEMENT sur le seuil comparé
    def edge_log(n, over):
        lg = make_log(rng, n, flags="all")
        return with_(lg, **over(lg))

    def thr_edge(value):
        def f(lg):
            thr = np.full(lg.n, value, np.float32)
            thr[::7] = np.float32(value) + np.float32(1e-3)
            return {"throttle": thr}
        return f

    for v in (8.0, 5.0, 2.0):
        add(f"gaz_seuil_{v}", edge_log(500, over=thr_edge(v)))

    def motors_edge(lg):
        m = lg.motors.copy()
        m[0, ::5], m[1, ::5] = np.float32(98.0), np.float32(8.0)  # saturation et plancher exacts
        m[2, 1::5], m[3, 1::5] = np.float32(97.99), np.float32(8.01)
        return {"motors": m}

    add("moteurs_seuils", edge_log(500, over=motors_edge))

    def sp_edge(lg):
        sp = lg.setpoint.copy()
        sp[:, ::3] = np.float32(100.0)
        sp[:, 1::3] = np.float32(-100.0)
        sp[:, 2::3] = np.float32(30.0)
        g = lg.gyro.copy()
        g[0, ::11] = np.float32(1900.0)
        g[1, 1::11] = np.float32(-1900.0)
        return {"setpoint": sp, "gyro": g}

    add("consigne_seuils", edge_log(700, over=sp_edge))

    # ---- crashs : une suite « mauvaise » (erreur > 300, moteurs saturés ET au plancher) de durée donnée
    fs = 100.0
    w = int(60.0 * fs / 1000.0)  # 120 trames

    def crash_log(run, via="err", at=150, n=600):
        lg = make_log(rng, n, fs, flags="all")
        sp, g, m = lg.setpoint.copy(), lg.gyro.copy(), lg.motors.copy()
        sp[:], g[:] = 0.0, 0.0
        if via == "err":
            g[0, at:at + run] = np.float32(301.0)
            m[0, at:at + run], m[1, at:at + run] = 99.0, 5.0
        elif via == "err_seuil":
            g[0, at:at + run] = np.float32(300.0)  # = seuil : pas mauvais
            m[0, at:at + run], m[1, at:at + run] = 99.0, 5.0
        elif via == "gyro":
            g[1, at:at + run] = np.float32(1901.0)
        elif via == "gyro_seuil":
            g[1, at:at + run] = np.float32(1900.0)
        elif via == "sans_plancher":
            g[0, at:at + run] = np.float32(500.0)
            m[:, at:at + run] = 99.0
        return with_(lg, setpoint=sp, gyro=g, motors=m)

    for run in (w - 1, w, int(0.8 * w) - 1, int(0.8 * w), int(0.8 * w) + 1, 2 * w):
        for via in ("err", "gyro"):
            add(f"crash_{via}_{run}", crash_log(run, via))
    for via in ("err_seuil", "gyro_seuil", "sans_plancher"):
        add(f"crash_{via}", crash_log(2 * w, via))
    add("crash_deux_suites", with_(crash_log(w, "err", 100), gyro=np.where(np.arange(600)[None, :].repeat(3, 0) % 300 < 7, 2000.0, 0.0).astype(np.float32)))
    add("crash_fin", crash_log(w, "gyro", at=600 - w))
    add("crash_debut", crash_log(w, "gyro", at=0))


    # ---- cas dirigés : branches que le tirage au hasard n'atteint pas (survivantes de la batterie de mutations, L-050)
    def mod(n, **kw):
        return with_(make_log(rng, n, flags="all"), **kw)

    base = make_log(rng, 400, flags="all")
    thr_hi = np.full(400, 60.0, np.float32)
    add("moteurs_nuls_en_vol", with_(base, throttle=thr_hi, motors=np.zeros((4, 400), np.float32)))
    tiny = (1e-7 * (1.0 + rng.uniform(0, 1, (4, 400)))).astype(np.float32)
    add("moteurs_minuscules", with_(base, throttle=thr_hi, motors=tiny))
    add("courant_negatif", with_(base, current=-np.abs(base.current)))
    gaz = np.linspace(-10, 130, 400).astype(np.float32)
    add("gaz_hors_bornes", with_(base, throttle=gaz))
    add("un_moteur", with_(base, motors=base.motors[:1]))
    add("debug_1", with_(base, debug=rng.uniform(0, 500, (1, 400)).astype(np.float32)))
    add("debug_8", with_(base, debug=rng.uniform(0, 500, (8, 400)).astype(np.float32)))

    # période d'exactement 1 s : fs = 64 (puissance de 2), vol de i0 à i0 + 128 inclus, marge 0,5 s
    n64 = 512
    lg64 = make_log(rng, n64, 64.0, flags="all", quiet=True)
    thr64 = np.zeros(n64, np.float32)
    thr64[64:64 + 129] = 50.0
    add("periode_1s_exacte", with_(lg64, throttle=thr64))
    thr65 = thr64.copy()
    thr65[64 + 129] = 50.0
    add("periode_1s_et_un_pas", with_(lg64, throttle=thr65))

    # crash : la suite « mauvaise » vaut exactement 0,8 × w (w = 10 à 170 Hz), ou finit sur la toute dernière trame
    for run in (7, 8, 9):
        lg = make_log(rng, 1200, 170.0, flags="all")
        g = np.zeros_like(lg.gyro)
        g[1, 500:500 + run] = 1901.0
        add(f"crash_170Hz_{run}", with_(lg, setpoint=np.zeros_like(lg.setpoint), gyro=g))
    lg = make_log(rng, 600, 100.0, flags="all")
    g = np.zeros_like(lg.gyro)
    g[1, 600 - 5:] = 1901.0  # 5 trames sur w = 6 : 0,8 × w = 4,8 ; la seule fenêtre qui les contient est la dernière
    add("crash_dernieres_trames", with_(lg, setpoint=np.zeros_like(lg.setpoint), gyro=g))
    g1 = g.copy()
    g1[1, 100] = 1901.0  # une trame mauvaise isolée : 6 au total (= w), mais aucune fenêtre antérieure n'atteint 0,8 × w
    add("crash_derniere_fenetre", with_(lg, setpoint=np.zeros_like(lg.setpoint), gyro=g1))
    m8 = lg.motors.copy()
    g2 = np.zeros_like(lg.gyro)
    g2[0, 200:230] = 301.0
    m8[0, 200:230], m8[1, 200:230] = 99.0, 8.0  # plancher EXACTEMENT à 8 %
    add("crash_plancher_8", with_(lg, setpoint=np.zeros_like(lg.setpoint), gyro=g2, motors=m8))

    # découpe en parties : un bloc d'une seconde par lettre (H stationnaire, M mouvement, T = 6 valeurs à 30 °/s pile)
    def blocks_log(pattern, fs=100):
        n = len(pattern) * fs
        lg = make_log(rng, n, float(fs), flags="all", quiet=True)
        sp = np.zeros((3, n), np.float32)
        for b, ch in enumerate(pattern):
            a = b * fs
            if ch == "M":
                sp[0, a:a + fs] = np.where(np.arange(fs) % 2, 200.0, -200.0)
            elif ch == "T":
                sp[0, a:a + 6] = 30.0
            elif ch == "U":
                sp[0, a:a + 5] = 31.0  # 5 valeurs seulement : le centile 95 reste sous 30
        return with_(lg, setpoint=sp, gyro=sp.copy(), throttle=np.full(n, 40.0, np.float32))

    add("parties_egalite_stationnaire", blocks_log("HHHHHHMMMHHHHHH"))
    add("parties_egalite_mouvement", blocks_log("MMMMMMHHHMMMMMM"))
    add("parties_seuil_30", blocks_log("HHHHHHTTTTTTHHHHHH"))
    add("parties_sous_le_seuil", blocks_log("HHHHHHUUUUUUHHHHHH"))
    add("parties_courtes", blocks_log("HHHHMMMMHHHH"))

    out = {"logs": [], "names": names, "datasets": [], "flight_stats": [], "pid_error_stats": [], "auto_period": [],
           "crash_time": [], "flight_parts": []}
    for lg in logs:
        out["logs"].append(enc_log(lg))

    for i, lg in enumerate(logs):
        keys = list(datasets.DATASETS) + ["inconnu"]
        av, err = call(datasets.available, lg)
        rec = {"log": i, "available": av, "error": err, "has_axis": []}
        for k in keys:
            row = []
            for ax in range(4):
                v, e = call(datasets.has_axis, lg, k, ax)
                row.append("E" if e else bool(v))
            rec["has_axis"].append(row)
        out["datasets"].append(rec)
        for s in sels(lg, i < 3 or lg.n < 30):
            r, e = call(stats.flight_stats, lg, to_slice(s))
            out["flight_stats"].append({"log": i, "sel": s, "error": e, "out": None if e else enc_stats(r)})
            r, e = call(stats.pid_error_stats, lg, to_slice(s))
            out["pid_error_stats"].append({"log": i, "sel": s, "error": e, "out": None if e else enc_pid_error(r)})
            if i < 3 and s is not None and s[0] < lg.n and s[1] > s[0]:  # pid_error_stats avec d'autres paramètres
                r, e = call(stats.pid_error_stats, lg, to_slice(s), 50.0, 20.0)
                out["pid_error_stats"].append({"log": i, "sel": s, "error": e, "args": [hx(50.0), hx(20.0)],
                                               "out": None if e else enc_pid_error(r)})
        for args in ((), (4.0, 0.25), (8.0, 0.0), (0.0, 0.5), (100.0, 0.5)):
            r, e = call(period.auto_period, lg, *args)
            out["auto_period"].append({"log": i, "args": [hx(a) for a in args], "error": e,
                                       "out": None if e else enc_period(r)})
        for args in ((), (150.0, 30.0), (300.0, 1000.0), (10.0, 0.01)):
            r, e = call(period.crash_time, lg, *args)
            out["crash_time"].append({"log": i, "args": [hx(a) for a in args], "error": e,
                                      "out": None if (e or r is None) else hx(r)})
        if lg.n >= 1:
            span = (float(lg.t[0]), float(lg.t[-1]))
            wins = [span, (span[0] + 0.1, span[1] - 0.1), (span[0], span[0] + 4.0), (span[1] - 9.0, span[1]),
                    (span[1] + 1.0, span[1] + 2.0), (span[0] - 5.0, span[0] - 1.0)]
            for (a, b) in wins:
                for extra in ((), (0.5,), (1.0, 20.0), (2.0, 30.0), (1.0, 5.0)):
                    r, e = call(period.flight_parts, lg, a, b, *extra)
                    out["flight_parts"].append({"log": i, "t0": hx(a), "t1": hx(b), "args": [hx(x) for x in extra],
                                                "error": e, "out": None if e else enc_parts(r)})
    out["prim"] = primitives(rng)
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
