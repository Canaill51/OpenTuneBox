#!/usr/bin/env python3
"""Cas de test de la simulation de filtres de `analysis/bf_filters.py` pour `rust/otb-analysis/src/bf_filters.rs` (4b-4).

Usage : bf_sim_cases.py SORTIE.json

Couvre `fft_freq_centres`, `simulate_gyro`, `simulate_dterm` et `whatif_spectrum` sur de petits journaux fabriqués (pas de
corpus) : trou d'enregistrement (raccord), télémétrie RPM présente ou non, gyro non filtré absent, debug FFT_FREQ des
versions 4.x (le 4e centre est le gyro) et 2025.12+, journaux d'un ou deux points, séquences d'appels qui partagent un cache
(`whatif_spectrum` réutilise ses réponses de filtres) et cache périmé (réglages changés sous le même cache).

Un flottant s'écrit en hexadécimal (16 caractères, bits du float64) ; `"error": true` = Python a levé une exception. Les
courbes d'un journal sont des float32 écrits en float64 (exact). Le test `rust/otb-analysis/tests/bf_sim.rs` exige des
sorties à la tolérance mesurée (libm, FFT, atan2 : L-048).
"""
import dataclasses
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hexs, hx, lp, random_settings
from bf_filters_cases import enc_settings as enc_filter_settings

from opentunebox.analysis import bf_filters as bf
from opentunebox.log.flightlog import FlightLog
from opentunebox.log.settings import Settings

INPUTS: dict[str, str] = {}
_IDS: dict[str, str] = {}


def ref(a) -> str:
    """Identifiant d'une entrée dans la table `inputs` du fichier : une entrée identique n'y est écrite qu'une fois."""
    h = hexs(a)
    if h not in _IDS:
        _IDS[h] = f"i{len(_IDS)}"
        INPUTS[_IDS[h]] = h
    return _IDS[h]


def call(fn, *args, **kw):
    """(résultat, False) ou (None, True) si Python lève une exception."""
    try:
        return fn(*args, **kw), False
    except Exception:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        return None, True


def enc_settings(s: Settings) -> dict:
    return {**enc_filter_settings(s), "version": list(s.version), "debug_mode": s.debug_mode, "rpm_lpf_hz": s.rpm_lpf_hz}


def make_log(rng, n, fs, settings, motors=4, unfilt=True, debug_rows=8, gap=None, flat_throttle=False,
             zero_debug_from=None, const_debug=False, const_motors=False) -> FlightLog:
    """Journal minimal : seuls les champs que lisent les filtres sont remplis, les autres sont nuls."""
    t = np.arange(n) / fs
    if gap is not None:
        t[gap:] += 20.0 / fs  # un trou de 20 échantillons : un raccord
    gyro = (rng.standard_normal((3, n)) * 80.0 + 120.0 * np.sin(2 * np.pi * 37.0 * t)).astype(np.float32)
    gyro_unfilt = (gyro + rng.standard_normal((3, n)) * 25.0).astype(np.float32) if unfilt else None
    throttle = (50.0 + 45.0 * np.sin(2 * np.pi * 0.7 * t) + rng.standard_normal(n) * 3.0).astype(np.float32)
    throttle[:: max(1, n // 7)] = 120.0  # gaz hors bornes
    if flat_throttle:  # mêmes gaz dans toutes les fenêtres : seuls les moteurs et les centres changent d'une fenêtre à l'autre
        throttle[:] = 50.0
    mhz = (250.0 + 120.0 * np.sin(2 * np.pi * 0.9 * t) + rng.standard_normal((motors, n)) * 4.0).astype(np.float32) if motors else None
    debug = rng.uniform(80.0, 640.0, (debug_rows, n)).astype(np.float32)
    debug[:, :: 5] = 0.0  # centres perdus (0 = notch ignoré)
    if zero_debug_from is not None:
        debug[:, zero_debug_from:] = 0.0  # plus aucun centre lu : tout vient des centres initiaux
    if const_debug:  # mêmes centres dans toutes les fenêtres
        debug[:] = np.linspace(150.0, 450.0, debug_rows)[:, None]
    if const_motors and mhz is not None:  # mêmes vitesses moteur dans toutes les fenêtres
        mhz[:] = 300.0
    z3 = np.zeros((3, n))
    return FlightLog(Path("sim.bbl"), 1, settings, t, fs, gyro, gyro_unfilt, z3, z3, z3, z3, z3, z3, throttle,
                     np.zeros((4, n)), mhz, debug, None, None, None, None, np.zeros((0, 4)), {}, frozenset())


def enc_log(lg: FlightLog, settings_index: int) -> dict:
    f64 = lambda a: None if a is None else {"k": a.shape[0], "x": ref(np.asarray(a, dtype=np.float64))}  # noqa: E731
    return {"fs": hx(lg.fs), "n": lg.n, "t": ref(lg.t), "settings": settings_index, "gyro": f64(lg.gyro),
            "gyro_unfilt": f64(lg.gyro_unfilt), "throttle": ref(np.asarray(lg.throttle, dtype=np.float64)),
            "motor_hz": f64(lg.motor_hz), "debug": f64(lg.debug)}


def fft_settings(rng, version, debug_mode="FFT_FREQ", count=3, **kw) -> Settings:
    base = dict(raw={}, version=version, debug_mode=debug_mode, looptime_us=125, pid_denom=4, dyn_notch_count=count,
                dyn_notch_q=300, dyn_notch_min_hz=90, dyn_notch_max_hz=620, rpm_harmonics=2, rpm_weights=[100, 60],
                rpm_q=400, rpm_min_hz=90, rpm_fade_hz=40, rpm_lpf_hz=150, gyro_lpf1=lp("PT1", 0, 75, 160, 5),
                gyro_lpf2=lp("PT2", 400), gyro_notches=[(250, 200), (300, 270)], dterm_lpf1=lp("PT1", 0, 60, 120, 7),
                dterm_lpf2=lp("PT3", 80), dterm_notch=(260, 190))
    base.update(kw)
    return Settings(**base)


def variants(rng, base: Settings, k: int) -> list[Settings]:
    """Réglages du journal : celui du log puis des variantes tirées au hasard avec le même numéro de version."""
    out = [base]
    for s in random_settings(rng, k):
        out.append(dataclasses.replace(s, version=base.version, debug_mode=str(rng.choice(["FFT_FREQ", "FFT_FREQ", "NONE"])),
                                       rpm_lpf_hz=int(rng.choice([0, 150, 40]))))
    return out


def extras(base: Settings) -> list[Settings]:
    """Réglages choisis pour franchir des branches que le tirage au hasard atteint rarement."""
    return [
        dataclasses.replace(base, rpm_lpf_hz=0, rpm_harmonics=3, rpm_weights=[100, 80, 60]),  # pas de lissage de la vitesse moteur
        dataclasses.replace(base, rpm_lpf_hz=40, rpm_harmonics=2),
        dataclasses.replace(base, pid_denom=2, gyro_notches=[(250, 200), (350, 300), (450, 400)]),  # 3 notchs statiques valides
        dataclasses.replace(base, gyro_lpf1=lp("PT1", 30), gyro_lpf2=lp("PT1", 25), dyn_notch_count=0, rpm_harmonics=0),
        # cadence PID plus basse que celle du log (pid_denom 8 : 1 kHz contre 2 kHz) : le plafond 0,48 × cadence PID des
        # notchs RPM est sous la limite 0,48 × fs du log, ils restent appliqués
        dataclasses.replace(base, pid_denom=8, rpm_harmonics=3, rpm_weights=[100, 100, 100], rpm_lpf_hz=150),
    ]


def main():
    rng = np.random.default_rng(20261102)
    out = {"settings": [], "logs": [], "fft_freq_centres": [], "simulate_gyro": [], "simulate_dterm": [],
           "whatif_spectrum": []}

    def sref(s: Settings) -> int:
        out["settings"].append(enc_settings(s))
        return len(out["settings"]) - 1

    logs: list[FlightLog] = []

    def add_log(lg: FlightLog) -> int:
        logs.append(lg)
        out["logs"].append(enc_log(lg, sref(lg.settings)))
        return len(logs) - 1

    # ---- journaux
    v26, v4, v2512 = (2026, 6, 2), (4, 5, 1), (2025, 12, 0)
    A = add_log(make_log(rng, 300, 2000.0, fft_settings(rng, v26), gap=160))
    B = add_log(make_log(rng, 240, 1000.0, fft_settings(rng, v4, count=4, pid_denom=8), debug_rows=4))  # 4.x : idx 3 = gyro
    C = add_log(make_log(rng, 200, 2000.0, fft_settings(rng, v2512, count=9), unfilt=False, motors=0, debug_rows=3))
    D = add_log(make_log(rng, 2, 2000.0, fft_settings(rng, v26)))
    E = add_log(make_log(rng, 1, 1000.0, fft_settings(rng, v26)))
    F = add_log(make_log(rng, 220, 1000.0, fft_settings(rng, v26, debug_mode="GYRO_SCALED"), motors=2))
    G = add_log(make_log(rng, 160, 2000.0, fft_settings(rng, v26, count=0, rpm_harmonics=0)))
    H = add_log(make_log(rng, 0, 2000.0, fft_settings(rng, v26)))  # vide
    small = [A, B, C, D, E, F, G, H]

    # ---- fft_freq_centres / simulate_gyro / simulate_dterm
    for li in small:
        lg = logs[li]
        alts = [None] + variants(rng, lg.settings, 4 if lg.n > 10 else 2) + (extras(lg.settings) if li in (A, B) else [])
        for si, s in enumerate(alts):
            sx = None if s is None else sref(s)
            res, err = call(bf.fft_freq_centres, lg, s)
            out["fft_freq_centres"].append({"log": li, "s": sx, "error": err,
                                            "out": None if (err or res is None) else {"shape": list(res.shape), "x": hexs(res)}})
            res, err = call(bf.simulate_gyro, lg, s)
            out["simulate_gyro"].append({"log": li, "s": sx, "error": err, "out": None if err else hexs(res)})
            g = None
            if si % 3 == 1 and lg.n > 1:
                g = np.asarray(lg.gyro, dtype=np.float64) * 0.9 + 3.0  # gyro filtré fourni (simulation avant / après)
            res, err = call(bf.simulate_dterm, lg, s, g)
            out["simulate_dterm"].append({"log": li, "s": sx, "gyro": None if g is None else ref(g), "error": err,
                                          "out": None if err else hexs(res)})

    # ---- whatif_spectrum : journaux de 1 200 à 1 500 points à 500 Hz (fenêtres de 300 ms = 150 points), avec et sans trou
    def whatif_log(n=1000, fs=500.0, gap=None, motors=4, debug_mode="FFT_FREQ", count=3):
        return add_log(make_log(rng, n, fs, fft_settings(rng, v26, debug_mode=debug_mode, count=count, pid_denom=4),
                                gap=gap, motors=motors))

    W1 = whatif_log(gap=450)
    W2 = whatif_log(n=700, motors=0, debug_mode="NONE")
    W3 = whatif_log(n=620, fs=1000.0)
    W4 = whatif_log(n=700, count=7)
    W5 = add_log(make_log(rng, 700, 500.0, fft_settings(rng, v26, pid_denom=4), flat_throttle=True))  # gaz constants
    W6 = add_log(make_log(rng, 700, 500.0, fft_settings(rng, v26, pid_denom=4), zero_debug_from=350))  # centres perdus
    # gaz constants et un seul des deux (moteurs, centres) qui varie : la clé du cache doit les distinguer
    W7 = add_log(make_log(rng, 700, 500.0, fft_settings(rng, v26, pid_denom=4), flat_throttle=True, const_debug=True))
    W8 = add_log(make_log(rng, 700, 500.0, fft_settings(rng, v26, pid_denom=4), flat_throttle=True, const_motors=True))
    group_counter = 0

    def changed(base: Settings, how: int) -> Settings:
        if how == 0:
            return dataclasses.replace(base)  # identique : le spectre doit rester celui du log
        if how == 1:
            return dataclasses.replace(base, gyro_lpf1=bf.scale_lowpass(base.gyro_lpf1, 0.6),
                                       gyro_lpf2=bf.scale_lowpass(base.gyro_lpf2, 0.5))
        if how == 2:
            return dataclasses.replace(base, dyn_notch_count=max(1, base.dyn_notch_count - 1), dyn_notch_min_hz=130,
                                       dyn_notch_max_hz=500)
        if how == 3:
            return dataclasses.replace(base, rpm_harmonics=3, rpm_q=250, rpm_weights=[100, 100, 50])
        if how == 6:  # plus de notchs dynamiques que de centres lus : les manquants viennent des centres initiaux
            return dataclasses.replace(base, dyn_notch_count=base.dyn_notch_count + 3, dyn_notch_min_hz=120, dyn_notch_max_hz=560)
        if how == 4:
            return dataclasses.replace(base, gyro_lpf1=bf.lowpass_off(base.gyro_lpf1), gyro_lpf2=bf.lowpass_off(base.gyro_lpf2),
                                       gyro_notches=[], dterm_lpf1=bf.scale_lowpass(base.dterm_lpf1, 2.0))
        return dataclasses.replace(base, dyn_notch_count=0, rpm_harmonics=0, gyro_lpf1=lp("PT1", 40),
                                   gyro_lpf2=lp("BIQUAD", 90), dterm_lpf2=lp("PT1", 30))  # coupe presque tout : |H|² < 1 %

    def add_whatif(li, sel, base, mod, target, seg_ms, prefilt, group, ax=0):
        lg = logs[li]
        xl = np.asarray(lg.gyro[ax], dtype=np.float64).astype(np.float32)  # float32, comme le log
        xp = np.asarray(lg.gyro_unfilt[ax]) if prefilt else None
        res, err = call(bf.whatif_spectrum, xl, xp, lg, slice(*sel), base, mod, target, seg_ms, caches.get(group))
        out["whatif_spectrum"].append({
            "log": li, "x": ref(np.asarray(xl, dtype=np.float64)), "xp": None if xp is None else ref(np.asarray(xp, dtype=np.float64)),
            "sel": list(sel), "base": sref(base), "mod": sref(mod), "target": target, "seg_ms": seg_ms, "cache": group,
            "error": err, "out": None if err else {"n": int(res.freqs.size), "freqs": hexs(res.freqs),
                                                   "current_db": hexs(res.current_db), "modified_db": hexs(res.modified_db)}})

    caches: dict[int, dict] = {}
    for li, sels in ((W1, [(0, 1000), (80, 400), (300, 520), (420, 1000)]), (W2, [(0, 700)]), (W3, [(0, 620)]),
                     (W4, [(0, 700), (0, 100)]), (W5, [(0, 700)]), (W6, [(0, 700)]), (W7, [(0, 700)]), (W8, [(0, 700)])):
        base = logs[li].settings
        for sel in sels:
            for how in range(7):
                for target in ("gyro", "dterm"):
                    if (how + len(sel)) % 2 and target == "dterm" and sel[0] != 0:
                        continue
                    add_whatif(li, sel, base, changed(base, how), target, 300.0, prefilt=(how % 2 == 1), group=None)
    # séquences qui partagent un cache : le 2e appel réutilise les réponses du 1er (et celles du terme D)
    for li in (W1, W3):
        base = logs[li].settings
        group_counter += 1
        caches[group_counter] = {}
        mod = changed(base, 1)
        for target in ("gyro", "dterm", "gyro", "dterm"):
            add_whatif(li, (0, logs[li].n), base, mod, target, 300.0, prefilt=True, group=group_counter)
        # cache périmé : réglages changés sous le même cache. Python rend l'ancien résultat ; le port doit faire de même
        add_whatif(li, (0, logs[li].n), base, changed(base, 4), "gyro", 300.0, prefilt=False, group=group_counter)
    # base très filtrante, modification plus légère : « là où les filtres actuels écrasent tout, on repart du signal avant filtres »
    for li in (W1, W3, W5):
        heavy = changed(logs[li].settings, 5)
        for target in ("gyro", "dterm"):
            add_whatif(li, (0, logs[li].n), heavy, logs[li].settings, target, 300.0, prefilt=True, group=None)
            add_whatif(li, (0, logs[li].n), heavy, changed(logs[li].settings, 1), target, 300.0, prefilt=True, group=None)
    # signal enregistré très faible face à un avant-filtres fort (log très filtré) : c'est là que la branche « signal avant
    # filtres » remplace l'estimation par le rapport des réponses
    for li in (W1, W3):
        lg = logs[li]
        heavy = changed(lg.settings, 5)
        for target in ("gyro", "dterm"):
            for mod in (lg.settings, changed(lg.settings, 1)):
                xl = (np.asarray(lg.gyro[0], dtype=np.float64) * 1e-4).astype(np.float32)
                res, err = call(bf.whatif_spectrum, xl, np.asarray(lg.gyro_unfilt[0]), lg, slice(0, lg.n), heavy, mod, target, 300.0, None)
                out["whatif_spectrum"].append({
                    "log": li, "x": ref(np.asarray(xl, dtype=np.float64)), "xp": ref(np.asarray(lg.gyro_unfilt[0], dtype=np.float64)),
                    "sel": [0, lg.n], "base": sref(heavy), "mod": sref(mod), "target": target, "seg_ms": 300.0, "cache": None,
                    "error": err, "out": None if err else {"n": int(res.freqs.size), "freqs": hexs(res.freqs),
                                                           "current_db": hexs(res.current_db), "modified_db": hexs(res.modified_db)}})
    # NaN dans le signal enregistré : np.maximum le propage, le max de Rust l'ignorerait
    lgn = logs[W1]
    xl = np.asarray(lgn.gyro[0], dtype=np.float64).astype(np.float32)
    xl[200] = np.nan
    heavy = changed(lgn.settings, 5)
    res, err = call(bf.whatif_spectrum, xl, np.asarray(lgn.gyro_unfilt[0]), lgn, slice(0, lgn.n), heavy, lgn.settings, "gyro", 300.0, None)
    out["whatif_spectrum"].append({
        "log": W1, "x": ref(np.asarray(xl, dtype=np.float64)), "xp": ref(np.asarray(lgn.gyro_unfilt[0], dtype=np.float64)),
        "sel": [0, lgn.n], "base": sref(heavy), "mod": sref(lgn.settings), "target": "gyro", "seg_ms": 300.0, "cache": None,
        "error": err, "out": None if err else {"n": int(res.freqs.size), "freqs": hexs(res.freqs),
                                               "current_db": hexs(res.current_db), "modified_db": hexs(res.modified_db)}})
    # fenêtres courtes (32 points minimum), fenêtre plus longue que le signal, axes
    for seg_ms, sel in ((10.0, (0, 1000)), (50.0, (0, 400)), (3000.0, (0, 1000)), (300.0, (0, 120)), (300.0, (5, 5)), (300.0, (3000, 4000))):
        base = logs[W1].settings
        add_whatif(W1, sel, base, changed(base, 1), "gyro", seg_ms, prefilt=True, group=None)
    for ax in (1, 2):
        base = logs[W1].settings
        add_whatif(W1, (0, 1000), base, changed(base, 2), "dterm", 300.0, prefilt=True, group=None, ax=ax)

    out["inputs"] = INPUTS
    json.dump(out, open(sys.argv[1], "w", encoding="utf-8"), separators=(",", ":"))
    print({k: len(v) for k, v in out.items() if k != "inputs"}, len(INPUTS), "entrées distinctes")


if __name__ == "__main__":
    main()
