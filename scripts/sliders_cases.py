#!/usr/bin/env python3
"""Cas de test des curseurs de `analysis/sliders.py` pour `rust/otb-analysis/src/sliders.rs` (D-035, phase 4b-5).

Usage : sliders_cases.py SORTIE.json

Couvre `default_pids`, `pids_from_sliders`, `filters_from_sliders`, `sliders_from_pids`, `slider_view` et `cli_commands` :
balayage de chaque curseur de 0 à 200 dans chaque famille de versions, tirages au hasard (graine fixe) de curseurs et de PID,
`base` quelconque, trop court (Python lève `IndexError`), curseurs absents, négatifs, NaN, infinis, fractionnaires (demi-
entiers pour le `round()` au pair), PID nuls, D max < D, listes de moins ou de plus de trois axes, en-têtes dont les curseurs
sont illisibles.

Un curseur s'écrit : `null` (absent), un entier JSON, `"nan"` / `"inf"` / `"-inf"`, ou une chaîne de 16 caractères
hexadécimaux (bits du float64). Un résultat flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception.
Le test `rust/otb-analysis/tests/sliders.rs` exige les PID, les filtres et les commandes identiques ; les estimations de
`sliders_from_pids` (SVD de Jacobi au lieu de LAPACK) à la tolérance mesurée.
"""
import dataclasses
import json
import math
import struct
import sys
import warnings

import numpy as np
from bf_filters_cases import enc_settings as enc_filter_settings
from bf_filters_cases import random_settings

from opentunebox.analysis import sliders as sl
from opentunebox.log.settings import AxisPID, LowpassSetting, parse_settings

FAMILIES = ["4.4", "4.5", "2025.12", "2026.6"]
VERSION = {"4.4": "Betaflight 4.4.3 (abc1234) STM32F7X2", "4.5": "Betaflight 4.5.1 (abc1234) STM32F7X2",
           "2025.12": "Betaflight 2025.12.0 (abc1234) STM32H743", "2026.6": "Betaflight 2026.6.1 (abc1234) STM32H743"}
KEYS = ("mode", "master", "i_gain", "d_gain", "pi_gain", "ff_gain", "pitch_d", "pitch_pi", "dmax_gain",
        "dterm_filter", "dterm_mult", "gyro_filter", "gyro_mult")
VALUES = [0, 1, 5, 10, 33, 50, 80, 99, 100, 101, 120, 133, 150, 199, 200, 250, 1000, 37.5, 99.999, 100.5, 0.5, 1.5, 2.5,
          -1, math.nan, None]


def hx(x) -> str:
    return struct.pack("<d", float(x)).hex()


def enc(v):
    """Un curseur ou un flottant isolé, dans le format du fichier."""
    if v is None:
        return None
    v = float(v)
    if math.isnan(v):
        return "nan"
    if math.isinf(v):
        return "inf" if v > 0 else "-inf"
    if v == int(v) and abs(v) < 1e12 and not (v == 0 and math.copysign(1, v) < 0):
        return int(v)
    return hx(v)


def call(fn, *args, **kw):
    """(résultat, False) ou (None, True) si Python lève une exception."""
    try:
        return fn(*args, **kw), False
    except Exception:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        return None, True


def pid5(p: AxisPID) -> list[int]:
    return [p.P, p.I, p.D, p.Dmax, p.F]


def enc_sliders(d: dict) -> list:
    return [[k, enc(v)] for k, v in d.items()]


def enc_pids_out(pids, err) -> dict:
    return {"error": True} if err else {"out": [pid5(p) for p in pids]}


def rand_sliders(rng, n_keys=None) -> dict:
    keys = list(KEYS[: (n_keys or len(KEYS))])
    d = {}
    for k in keys:
        if rng.random() < 0.15:
            continue  # clé absente
        d[k] = VALUES[int(rng.integers(len(VALUES)))]
    if "mode" in d and rng.random() < 0.8:
        d["mode"] = [0, 1, 2, 2, 2, 3][int(rng.integers(6))]
    return d


def rand_base(rng):
    r = rng.random()
    if r < 0.6:
        return None
    n = [3, 3, 3, 4, 2, 1, 0][int(rng.integers(7))]
    return [AxisPID(*[int(x) for x in rng.integers(0, 320, 5)]) for _ in range(n)]


def case_pids(sliders, fam, base):
    pids, err = call(sl.pids_from_sliders, sliders, fam, base)
    return {"fam": fam, "s": enc_sliders(sliders), "base": None if base is None else [pid5(p) for p in base],
            **enc_pids_out(pids, err)}


def raw_from(pids, fam, rng=None, sliders=None, junk=False) -> dict:
    """En-tête d'un log : PID normalisés (D, Dmax) rangés comme le firmware de cette famille, curseurs facultatifs."""
    new = fam in ("2025.12", "2026.6")
    raw = {"Firmware revision": VERSION[fam]}
    for ax, name in enumerate(("rollPID", "pitchPID", "yawPID")):
        p = pids[ax]
        raw[name] = f"{p.P},{p.I},{p.D if new else p.Dmax}"
    raw["ff_weight"] = ",".join(str(p.F) for p in pids)
    raw["d_max" if new else "d_min"] = ",".join(str(p.Dmax if new else p.D) for p in pids)
    if sliders is not None:
        names = {"mode": "simplified_pids_mode", "master": "simplified_master_multiplier", "i_gain": "simplified_i_gain",
                 "d_gain": "simplified_d_gain", "pi_gain": "simplified_pi_gain", "ff_gain": "simplified_feedforward_gain",
                 "pitch_d": "simplified_pitch_d_gain", "pitch_pi": "simplified_pitch_pi_gain",
                 "dmax_gain": "simplified_d_max_gain" if new else "simplified_dmax_gain",
                 "dterm_filter": "simplified_dterm_filter", "dterm_mult": "simplified_dterm_filter_multiplier",
                 "gyro_filter": "simplified_gyro_filter", "gyro_mult": "simplified_gyro_filter_multiplier"}
        for k, v in sliders.items():
            if k in names and v is not None:
                raw[names[k]] = str(int(v)) if not (isinstance(v, float) and math.isnan(v)) else "abc"
        if junk and rng is not None and "simplified_master_multiplier" in raw:
            raw["simplified_master_multiplier"] = "abc"  # illisible → -1
        if junk and rng is not None and "simplified_pi_gain" in raw:
            del raw["simplified_pi_gain"]
    return raw


def main():
    rng = np.random.default_rng(20261031)
    warnings.simplefilter("ignore")
    np.seterr(all="ignore")
    out = {"default": [], "pids": [], "filters": [], "from_pids": [], "view": [], "cli": []}

    for fam in FAMILIES:
        out["default"].append({"fam": fam, "out": [pid5(p) for p in sl.default_pids(fam)]})

    # --- pids_from_sliders : un curseur à la fois, de 0 à 200 (pas 1 sur 4.5 et 2026.6, pas 3 sur les autres)
    for fam in FAMILIES:
        step = 2 if fam in ("4.5", "2026.6") else 5
        for key in ("master", "pi_gain", "i_gain", "d_gain", "ff_gain", "pitch_d", "pitch_pi", "dmax_gain"):
            for v in range(0, 201, step):
                out["pids"].append(case_pids({"mode": 2, key: v}, fam, None))
    # croisements maître × autre curseur (les produits réassociés)
    for fam in FAMILIES:
        for a in range(0, 201, 25):
            for b in range(0, 201, 25):
                out["pids"].append(case_pids({"mode": 2, "master": a, "d_gain": b, "pi_gain": 200 - b, "ff_gain": a}, fam, None))
    # tirages
    for fam in FAMILIES:
        for _ in range(600):
            out["pids"].append(case_pids(rand_sliders(rng), fam, rand_base(rng)))
    # cas à la main
    specials = [
        {"mode": 2, "master": math.inf}, {"mode": 2, "master": math.inf, "d_gain": 0}, {"mode": 0, "master": math.inf},
        {"mode": math.inf}, {"mode": -math.inf}, {"mode": 2, "master": -math.inf}, {"mode": 2, "master": 1e300},
        {"mode": 2, "master": 1e10}, {"mode": 2, "dmax_gain": 1e10, "d_gain": 200}, {"mode": 2, "master": None},
        {"mode": 1.9}, {"mode": 0.9}, {"mode": 7}, {"mode": -3, "master": 90}, {}, {"master": 120}, {"mode": 2, "pi_gain": 0},
        {"mode": 2, "master": math.inf, "pi_gain": 0, "ff_gain": 0, "d_gain": 0},  # NaN sans infini : Python lève
        {"mode": 2, "d_gain": 0, "dmax_gain": 0}, {"mode": 2, "master": 0}, {"mode": 1, "pitch_d": 200, "pitch_pi": 200},
    ]
    for fam in FAMILIES:
        for s in specials:
            for base in (None, [], [AxisPID(1, 2, 3, 4, 5)], [AxisPID(1, 2, 3, 4, 5)] * 2,
                         [AxisPID(60, 70, 80, 90, 100), AxisPID(1, 1, 1, 1, 1), AxisPID(10, 20, 30, 20, 50)],
                         [AxisPID(60, 70, 80, 90, 100), AxisPID(1, 1, 1, 1, 1), AxisPID(10, 20, 30, 40, 50)] * 2,
                         [AxisPID(0, 0, 0, 0, 0)] * 3,
                         [AxisPID(45, 80, 30, 40, 120), AxisPID(47, 84, 34, 46, 125), AxisPID(45, 80, 25, 25, 120)],
                         [AxisPID(45, 80, 30, 40, 120), AxisPID(47, 84, 34, 46, 125), AxisPID(45, 80, 25, 10, 120)]):
                out["pids"].append(case_pids(s, fam, base))
    # --- filters_from_sliders
    mults = [-200, -1, 0, 1, 5, 9, 10, 11, 49, 50, 99, 100, 101, 133, 150, 199, 200, 201, 333, 400, 1000, 10**6, 10**12]
    settings = random_settings(rng, 40)
    # un seul des deux bornes dynamiques à 0 : le firmware ne lit que `dyn_min_hz` (voir `filters_from_sliders`)
    for base in settings[:6]:
        settings.append(dataclasses.replace(base, gyro_lpf1=LowpassSetting("PT1", 0, 0, 300, 5),
                                            dterm_lpf1=LowpassSetting("PT1", 0, 80, 0, 5)))
        settings.append(dataclasses.replace(base, gyro_lpf1=LowpassSetting("PT1", 0, 200, 0, 5),
                                            dterm_lpf1=LowpassSetting("PT1", 0, 0, 150, 5)))
    out["settings"] = [enc_filter_settings(s) for s in settings]
    for g in mults:
        for d in (0, 10, 77, 100, 150, 200, 777, -50):
            out["filters"].append({"g": g, "d": d, "set": None, "out": list(sl.filters_from_sliders(g, d, "2026.6").items())})
    for i, s in enumerate(settings):
        for g, d in ((100, 100), (10, 10), (200, 200), (120, 80), (37, 183)):
            out["filters"].append({"g": g, "d": d, "set": i, "out": list(sl.filters_from_sliders(g, d, "4.5", s).items())})

    # --- sliders_from_pids
    def add_from_pids(fam, pids):
        r, err = call(sl.sliders_from_pids, pids, fam)
        out["from_pids"].append({"fam": fam, "pids": [pid5(p) for p in pids],
                                 **({"error": True} if err else {"out": [[k, hx(v)] for k, v in r.items()]})})

    for fam in FAMILIES:
        add_from_pids(fam, sl.default_pids(fam))
        for pids in ([], [AxisPID()], [AxisPID()] * 3, [AxisPID(0, 0, 0, 0, 0), AxisPID(10, 10, 10, 10, 10), AxisPID()],
                     [AxisPID(45, 80, 30, 20, 120)] * 3, [AxisPID(-5, 80, 30, 40, 120)] * 3,
                     [AxisPID(45, 80, 30, 40, 120)] * 5, [AxisPID(1, 1, 1, 1, 1)] * 3,
                     [AxisPID(250, 250, 250, 250, 1000)] * 3, [AxisPID(0, 0, 0, 0, 0), AxisPID(47, 84, 34, 46, 125)],
                     [AxisPID(45, 80, 30, 40, 120), AxisPID(0, 0, 0, 0, 0), AxisPID(45, 80, 0, 0, 120)],
                     [AxisPID(45, 0, 30, 40, 0), AxisPID(47, 0, 34, 46, 0), AxisPID(45, 0, 0, 0, 0)]):
            add_from_pids(fam, pids)
        # PID issus de curseurs : le cas réel (estimation d'un réglage simplifié)
        for _ in range(250):
            s = {"mode": 2, **{k: int(rng.integers(0, 201)) for k in sl.PID_SLIDER_KEYS}}
            add_from_pids(fam, sl.pids_from_sliders(s, fam))
        # PID quelconques (réglage manuel)
        for _ in range(250):
            n = int(rng.choice([3, 3, 3, 2, 1, 4]))
            lo = int(rng.choice([0, 1, 10]))
            pids = []
            for _ in range(n):
                p, i, d, f = (int(x) for x in rng.integers(lo, 300, 4))
                pids.append(AxisPID(p, i, d, d + int(rng.integers(-5, 40)), f))
            add_from_pids(fam, pids)

    # --- slider_view : en-têtes complets
    for fam in FAMILIES:
        for _ in range(250):
            s = {"mode": int(rng.choice([0, 1, 2, 2, 2])), **{k: int(rng.integers(0, 201)) for k in sl.PID_SLIDER_KEYS},
                 "gyro_mult": 100, "dterm_mult": 100, "gyro_filter": 1, "dterm_filter": 1}
            pids = sl.pids_from_sliders(s, fam)
            r = rng.random()
            if r < 0.25:  # PID qui ne correspondent plus aux curseurs
                pids = [AxisPID(p.P + int(rng.integers(0, 3)), p.I, p.D, p.Dmax, p.F) for p in pids]
            elif r < 0.5:  # réglage manuel : curseurs absents ou à 0
                pids = [AxisPID(*[int(x) for x in rng.integers(0, 200, 5)]) for _ in range(3)]
                if rng.random() < 0.5:
                    s = {"mode": 0}
            raw = raw_from(pids, fam, rng, s if rng.random() < 0.9 else None, junk=bool(rng.random() < 0.15))
            st = parse_settings(raw)
            v, err = call(sl.slider_view, st)
            out["view"].append({"raw": raw, **({"error": True} if err else {
                "values": [[k, hx(x)] for k, x in v.values.items()], "from_header": v.from_header, "consistent": v.consistent})})
        # en-têtes sans PID du tout
        for raw in ({"Firmware revision": VERSION[fam]}, {"Firmware revision": VERSION[fam], "simplified_pids_mode": "2"},
                    {"simplified_pids_mode": "1", "simplified_master_multiplier": "130"}, {}):
            st = parse_settings(raw)
            v, err = call(sl.slider_view, st)
            out["view"].append({"raw": raw, **({"error": True} if err else {
                "values": [[k, hx(x)] for k, x in v.values.items()], "from_header": v.from_header, "consistent": v.consistent})})

    # --- cli_commands
    cli_values = [0, 1, 2, 3, 5, 10, 99, 100, 150, 200, 201, 1000, 0.5, 1.5, 2.5, 99.5, 100.5, 3.4999, -1, -0.1, None,
                  math.nan, math.inf, -math.inf, 1e30]
    for fam in FAMILIES:
        for _ in range(250):
            d = {}
            for k in KEYS:
                if rng.random() < 0.8:
                    d[k] = cli_values[int(rng.integers(len(cli_values)))]
            # NaN et infinis : peu de cas (Python lève) pour garder des cas de réussite en nombre
            if rng.random() < 0.8:
                d = {k: v for k, v in d.items() if not (isinstance(v, float) and not math.isfinite(v))}
            r, err = call(sl.cli_commands, d, fam)
            out["cli"].append({"fam": fam, "s": enc_sliders(d), **({"error": True} if err else {"out": r})})
        for d in ({}, {"mode": 2}, {"dmax_gain": 80, "gyro_filter": 1, "gyro_mult": 5}, {"mode": math.nan}, {"master": math.inf}):
            r, err = call(sl.cli_commands, d, fam)
            out["cli"].append({"fam": fam, "s": enc_sliders(d), **({"error": True} if err else {"out": r})})

    json.dump(out, open(sys.argv[1], "w"), separators=(",", ":"))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, file=sys.stderr)


if __name__ == "__main__":
    main()
