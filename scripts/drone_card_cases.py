#!/usr/bin/env python3
"""Cas de test de `advisor/drone_card.py` pour `rust/otb-advisor/src/drone_card.rs` (5b-1).

Usage : drone_card_cases.py SORTIE.json

- `constants`  : les tables du module (choix de la fiche, séries, filtres par défaut, plages du I) ;
- `cards`      : chaque combinaison version × taille × boucle PID × ESC (valeurs du menu, puis des boucles hors menu) avec
  toutes les valeurs conseillées (`log_rate`, `pid_log_rate`, `size_group`, séries, `i_range`, `debug_mode`, `bidir`,
  `angle_limit_setting`) et `to_dict` ;
- `from_dict`  : clés absentes, clés inconnues, taille inconnue, cases cochées ;
- `card_for_log` : casse, espaces (dont U+001C et les blancs Unicode), noms vides ou absents, doublons ;
- `pitch`      : `pitch_series` autour des bornes 0,2 et 2,0, les deux sens, plusieurs pas.

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception.
"""
import json
import struct
import sys

from opentunebox.advisor import drone_card as dc
from opentunebox.advisor.drone_card import DroneCard, card_for_log, pitch_series


def hx(x) -> str:
    return struct.pack("<d", float(x)).hex()


def main(out: str) -> None:
    cases: dict = {}
    cases["constants"] = {
        "bf": dc.BF_FAMILIES, "pid_rates": [[hx(a), b] for a, b in dc.PID_RATES_KHZ], "escs": dc.ESCS,
        "bidir": sorted(dc.BIDIR_ESCS), "large": sorted(dc.LARGE_SIZES),
        "damping": {k: [hx(v) for v in vs] for k, vs in dc.DAMPING_SERIES.items()},
        "master": [hx(v) for v in dc.MASTER_SERIES], "ff": [hx(v) for v in dc.FF_SERIES],
        "pitch_len": dc.PITCH_SERIES_LEN, "defaults": dc.DRONE_FILTER_DEFAULTS,
        "i_range": {k: [hx(v) for v in vs] for k, vs in dc.I_RANGE.items()},
    }
    rates = [r for r, _ in dc.PID_RATES_KHZ] + [0.5, 1.0, 1.25, 2.5, 3.0, 6.4, 12.0, 32.0]
    escs = [e for e, _ in dc.ESCS] + ["autre", ""]
    out_cards = []
    for bf, _ in dc.BF_FAMILIES:
        for size in ("whoop", "micro", "five", "seven", "large"):
            for khz in rates:
                for esc in escs:
                    c = DroneCard("Test", bf=bf, size=size, pid_khz=khz, esc=esc)
                    lr, lc = c.log_rate()
                    pr, pc = c.pid_log_rate()
                    out_cards.append({
                        "card": c.to_dict() | {"pid_khz": hx(khz)}, "bidir": c.bidir, "log_rate": [hx(lr), lc],
                        "pid_log_rate": [hx(pr), pc], "group": c.size_group,
                        "damping": [hx(v) for v in c.damping_series()], "i_range": [hx(v) for v in c.i_range()],
                        "debug": c.debug_mode(), "angle": c.angle_limit_setting(),
                    })
    cases["cards"] = out_cards
    from opentunebox.advisor.profiles import PROFILES
    assert set(PROFILES) == {"whoop", "micro", "five", "seven", "large"}, set(PROFILES)

    dicts = [
        {"name": "A"}, {"name": "A", "bf": "4.4"}, {"name": "A", "size": "seven"}, {"name": "A", "size": "inconnue"},
        {"name": "A", "size": ""}, {"name": "A", "pid_khz": 8.0}, {"name": "A", "pid_khz": 3.2, "esc": "AM32"},
        {"name": "A", "extra": 1, "autre": [1]}, {"name": "A", "done": {"x": True, "y": False}},
        {"name": "A", "done": {}}, {"name": "é ", "bf": "2025.12", "size": "whoop", "pid_khz": 2.0, "esc": "Bluejay",
                                     "done": {"b": True, "a": False}},
        {}, {"bf": "4.4"}, {"name": 3}, {"name": "A", "pid_khz": "4"}, {"name": "A", "done": [1]}, {"name": "A", "bf": None},
    ]
    out_from = []
    for d in dicts:
        try:
            c = DroneCard.from_dict(d)
            r = {k: (hx(v) if k == "pid_khz" else v) for k, v in c.to_dict().items()}
            err = False
        except Exception:  # noqa: BLE001
            r, err = None, True
        out_from.append({"in": d, "out": r, "error": err})
    cases["from_dict"] = out_from

    names = ["Alpha", "alpha", " ALPHA\t", "\u001cAlpha\u001f", "　Alpha ", "Béta", "BÉTA", "ΣΑΣ", "σας", "İ", "i̇",
             "", "   ", "\u001c", "Alpha 2", "alpha2", "ǅ", "ǆ", "ß", "SS", "ẞ"]
    cards = [DroneCard(n) for n in names[:11] + ["ǅ", "ß", "Dup", "dup"]]
    out_log = []
    for n in names + ["Dup", "DUP", " dup "]:
        r = card_for_log(cards, n)
        out_log.append({"craft": n, "found": None if r is None else cards.index(r)})
    cases["card_for_log"] = {"cards": [c.name for c in cards], "queries": out_log}

    out_pitch = []
    for cur in (0.2, 0.4, 0.5, 0.6, 0.8, 1.0, 1.1, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2, 0.0, 0.1, 0.3, 0.7, 0.9, 1.3, 1.5, 1.7, 1.9,
                1.05, 0.15, 0.25, 1.95, 3.0, -1.0):
        for direction in (1, -1, 0, 5, -5):
            for step in (0.2, 0.1, 0.25, 0.05, 0.0):
                out_pitch.append({"cur": hx(cur), "dir": direction, "step": hx(step),
                                  "out": [hx(v) for v in pitch_series(cur, direction, step)]})
    cases["pitch"] = out_pitch
    with open(out, "w", encoding="utf-8") as f:
        json.dump(cases, f, ensure_ascii=True, separators=(",", ":"))
    print(f"{len(out_cards)} fiches, {len(out_from)} from_dict, {len(out_log)} card_for_log, {len(out_pitch)} pitch -> {out}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
