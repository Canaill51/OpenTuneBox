#!/usr/bin/env python3
"""Cas de test de `find_series` et `Series.steps` de `analysis/step_series.py` (phase 4c-B(e)) pour `rust/otb-analysis/src/step_series.rs`.

Usage : step_series_cases.py SORTIE.json

Des listes de vols fabriquées (curseurs et signature de filtres seulement : `find_series` ne lit rien d'autre) : un curseur qui change,
plusieurs séries dans la même liste, vols de filtres différents, autres curseurs qui diffèrent de moins ou de plus de 0,005 (arrondi à
0,01), valeurs égales (tri stable), séries d'une seule valeur, curseurs arrondis au demi (0,285, 0,295…), listes vides ou d'un vol.
`flight_step`, `flight_conditions` et `filter_signature` se jugent sur les 668 sessions du corpus (`otb-parity check-analysis`).

Un flottant s'écrit en hexadécimal. Le test `rust/otb-analysis/tests/step_series.rs` exige l'identité.
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx

from opentunebox.analysis import step_series
from opentunebox.analysis.sliders import PTB_CATEGORIES
from opentunebox.analysis.step_series import FlightConditions, FlightStep

CATS = list(PTB_CATEGORIES)


def sig(variant: int):
    lp = lambda a, b, c: (a, b, c)  # noqa: E731
    return (lp(0, 0, 500), lp(650, 0, 0), lp(75, 75, 150), lp(150, 0, 0), ((250, 200),) * (variant % 3), (0, 0), 3, 300, 100,
            3, 500, 90, (100, 50, 25)[: 1 + variant % 3])


def flight(sliders: dict, variant=0, label="v"):
    return FlightStep(label, dict(sliders), sig(variant), FlightConditions((100, 100), 0.0, sliders.get("Drift-Wobble", 1.0)))


def enc_sig(t):
    return [enc_sig(x) for x in t] if isinstance(t, tuple) else int(t)


def enc_flights(fl):
    return [{"sliders": [hx(f.sliders[c]) for c in CATS], "filters": enc_sig(f.filters)} for f in fl]


def main():
    rng = np.random.default_rng(20261110)
    out = {"cases": []}
    base = {c: 1.0 for c in CATS}

    def case(fl, label):
        series = step_series.find_series(fl)
        out["cases"].append({"label": label, "flights": enc_flights(fl), "out": [
            {"key": s.key, "flights": [int(i) for i in s.flights], "values": [hx(v) for v in s.values],
             "steps": [hx(v) for v in s.steps]} for s in series]})

    case([], "aucun vol")
    case([flight(base)], "un vol")
    for key in ("Damping", "Master", "Pitch T.", "Pitch D.", "Stick Resp.", "Tracking", "Dynamic D."):
        for vals in ((1.0, 1.1), (1.2, 0.9, 1.0), (0.8, 0.8, 1.2), (1.0, 1.0, 1.0), (0.5, 0.75, 1.0, 1.25, 1.5)):
            fl = []
            for v in vals:
                s = dict(base)
                s[key] = v
                fl.append(flight(s))
            case(fl, f"{key} : {vals}")
    # curseurs arrondis : 0,285 / 0,295 (demi-cas), 0,1 + 0,2 (flottants sales), 0,004 d'écart sur un autre curseur
    for vals in ((0.285, 0.295, 0.305), (0.1 + 0.2, 0.3, 0.30000000000000004), (0.994, 1.004, 1.014), (0.995, 1.005), (1.005, 0.995)):
        case([flight(dict(base, Damping=v)) for v in vals], f"arrondis {vals}")
    case([flight(dict(base, Damping=1.0, Tracking=1.0)), flight(dict(base, Damping=1.1, Tracking=1.004))], "autre curseur à 0,004 près")
    case([flight(dict(base, Damping=1.0, Tracking=1.0)), flight(dict(base, Damping=1.1, Tracking=1.006))], "autre curseur à 0,006 près")
    case([flight(dict(base, Damping=1.0, Tracking=0.995)), flight(dict(base, Damping=1.1, Tracking=1.005))], "autre curseur : demi-cas")
    # filtres différents : pas de série commune
    case([flight(dict(base, Damping=1.0), variant=0), flight(dict(base, Damping=1.1), variant=1)], "filtres différents")
    case([flight(dict(base, Damping=1.0), 0), flight(dict(base, Damping=1.1), 0), flight(dict(base, Damping=1.2), 1)], "filtres 2 + 1")
    # plusieurs séries dans la même liste (deux curseurs, deux jeux de filtres)
    fl = [flight(dict(base, Damping=v), 0) for v in (0.8, 1.0, 1.2)] + [flight(dict(base, Master=v), 0) for v in (0.9, 1.0, 1.1)]
    fl += [flight(dict(base, Damping=v, Master=0.8), 1) for v in (1.0, 1.4)]
    case(fl, "plusieurs séries")
    # stabilité du tri : valeurs égales gardent l'ordre des vols
    case([flight(dict(base, Master=v), label=str(i)) for i, v in enumerate((1.0, 0.8, 1.0, 0.8, 1.2, 1.0))], "valeurs égales (tri stable)")
    for _ in range(40):  # tirages libres
        n = int(rng.integers(2, 9))
        fl = []
        for _i in range(n):
            s = {c: float(rng.choice([0.5, 0.75, 1.0, 1.25, 1.5, 0.285, 1.005, 0.9949])) for c in CATS}
            fl.append(flight(s, int(rng.integers(0, 2))))
        case(fl, "tirage libre")
    # conditions du protocole : les propriétés de FlightConditions (angle, ff_active, ff_gain) pour des valeurs aux frontières
    out["conditions"] = []
    for ff in ((0, 0), (100, 0), (0, 100), (120, 125)):
        for share in (0.0, 0.5, 0.89, 0.8999999999999999, 0.9, 0.9000000000000001, 1.0, float("nan")):
            c = FlightConditions(ff, share, 1.0)
            out["conditions"].append({"ff": list(ff), "share": hx(share), "angle": bool(c.angle), "ff_active": bool(c.ff_active),
                                      "ff_gain": [int(c.ff_gain(0)), int(c.ff_gain(1))]})
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print(len(out["cases"]), "cas", sum(1 for c in out["cases"] if c["out"]), "avec séries")


if __name__ == "__main__":
    main()
