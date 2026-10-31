#!/usr/bin/env python3
"""Cas de test de `advisor/tuning_path.py` pour `rust/otb-advisor/src/tuning_path.rs` (phase 5b-3).

Usage : tuning_path_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python -I`)

- `cards`   : `phases(card)` pour chaque version × taille × boucle PID × ESC du menu (toutes les cases : clé, titre et aide résolus pour
  la fiche, vérifications, usages, vol de série, outil, partie, rôle). Les textes sont rangés dans une table (`strings`) : ils se répètent
  d'une fiche à l'autre ;
- `status`  : `auto_status(step, checks, targets)` de chaque case du parcours pour des listes de `LogCheck` fabriqués directement (points
  et usages dans tous leurs états, rôle, curseurs au voisinage des valeurs de série), avec et sans repères de fin de la phase Filtres ;
- tables : textes d'aide partagés, état « ok » / « bad ».

Un flottant s'écrit en hexadécimal.
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))  # -I retire le dossier du script de sys.path
from bf_filters_cases import hx  # noqa: E402

from opentunebox.advisor import log_check as LC  # noqa: E402
from opentunebox.advisor import tuning_path as TP  # noqa: E402
from opentunebox.advisor.drone_card import BF_FAMILIES, DroneCard, ESCS, PID_RATES_KHZ  # noqa: E402
from opentunebox.advisor.log_check import CheckItem, Fix, FilterTargets, LogCheck, Usage  # noqa: E402
from opentunebox.advisor.profiles import PROFILES  # noqa: E402

STRINGS: dict[str, int] = {}


def sid(s: str) -> int:
    return STRINGS.setdefault(s, len(STRINGS))


def enc_step(step, card) -> list:
    return [step.key, sid(step.text(card)), sid(step.help_text(card)), list(step.checks), list(step.usages), bool(step.any_log),
            None if step.flight is None else [step.flight[0], hx(step.flight[1])], step.flight_usage, step.tool, step.part,
            bool(step.targets), step.logs, bool(step.open_logs)]


def enc_phases(card) -> list:
    return [[p.key, sid(p.title), sid(p.intro), [enc_step(s, card) for s in p.steps]] for p in TP.phases(card)]


def enc_check(ck: LogCheck) -> dict:
    return {"label": ck.label, "role": ck.role, "sliders": [[k, hx(v)] for k, v in ck.sliders.items()],
            "items": [[i.key, i.title, i.status, i.detail] for i in ck.items],
            "usages": [[u.key, u.label, bool(u.ok), u.reason, bool(u.shown)] for u in ck.usages]}


def enc_targets(t):
    if t is None:
        return None
    return {"vals": [hx(t.gyro_ptb_db), hx(t.dterm_ptb_db), hx(t.gyro_delay_ms), hx(t.dterm_delay_ms)],
            "goal": None if t.delay_goal_ms is None else [hx(t.delay_goal_ms[0]), hx(t.delay_goal_ms[1])]}


ITEM_KEYS = ("rpm", "simplified", "log_rate", "gyro_unfilt", "debug_mode", "rates", "angle_limit", "flight_mode", "i_slider", "dyn_damping",
             "pid_log_rate")
STATUSES = (LC.OK, LC.WARN, LC.BLOCK, LC.INFO)
USAGE_KEYS = ("noise", "delay", "series", "ff_series")
SERIES = {"Damping": (0.6, 0.8, 1.0, 1.2, 1.4, 1.6), "Master": (1.0, 1.2, 1.4, 1.6, 1.8, 2.0), "Stick Resp.": (0.0, 0.5, 1.0)}


def fabricate(rng, idx: int) -> LogCheck:
    items = []
    for k in ITEM_KEYS:
        r = rng.random()
        if r < 0.25:
            continue  # point absent
        st = STATUSES[int(rng.integers(4))]
        items.append(CheckItem(k, f"Titre {k}", st, f"Détail {k} {st}", [Fix("où", "cli")] if st in (LC.WARN, LC.BLOCK) else []))
    usages = []
    for k in USAGE_KEYS:  # `check_log` rend toujours les quatre usages
        usages.append(Usage(k, f"Usage {k}", bool(rng.random() < 0.5), f"raison {k}", True))
    sliders = {}
    if rng.random() < 0.8:
        cat = list(SERIES)[int(rng.integers(3))]
        base = SERIES[cat][int(rng.integers(len(SERIES[cat])))]
        sliders[cat] = base + [0.0, 0.005, -0.005, 0.0099, -0.0099, 0.01, -0.01, 0.0101, 0.02, 0.2][int(rng.integers(10))]
    role = (LC.FILTERS, LC.PID)[int(rng.integers(2))]
    return LogCheck(f"log{idx}", True, {"hover": None, "moving": None}, items, usages, [], False, sliders, role)


def main():
    out_path = sys.argv[1]
    cards = []
    n_steps = 0
    for bf, _ in BF_FAMILIES:
        for size in PROFILES:
            for khz, _l in PID_RATES_KHZ:
                for esc, _l2 in ESCS:
                    c = DroneCard("Test", bf=bf, size=size, pid_khz=khz, esc=esc)
                    ph = enc_phases(c)
                    n_steps = sum(len(p[3]) for p in ph)
                    cards.append({"card": {"bf": bf, "size": size, "pid_khz": hx(khz), "esc": esc}, "phases": ph})

    base = TP.phases(DroneCard("Test"))
    steps = [s for p in base for s in p.steps]
    rng = np.random.default_rng(2026)
    pool = [fabricate(rng, i) for i in range(60)]
    # cas dirigés : un seul log, du rôle voulu, avec chaque curseur de chaque série pile sur sa valeur
    directed = []
    for cat, vals in SERIES.items():
        for v in vals:
            for ok in (True, False):
                for role in (LC.FILTERS, LC.PID):
                    usages = [Usage(k, f"Usage {k}", ok, f"raison {k}", True) for k in USAGE_KEYS]
                    directed.append(LogCheck("dirigé", True, {"hover": None, "moving": None}, [], usages, [], False, {cat: v}, role))
    pool += directed
    scenarios = []
    for i in range(240):
        k = int(rng.integers(0, 5))
        scenarios.append([int(rng.integers(len(pool))) for _ in range(k)])
    scenarios += [[], [0], [1], [0, 1], [len(pool) - 1]]
    targets_list = [None, FilterTargets(-35.0, -15.0, 0.5, 1.0, (1.0, 2.0)), FilterTargets(-25.0, -15.0, 0.5, 1.0, (1.0, 2.0)),
                    FilterTargets(float("nan"), -15.0, 0.5, 1.0, (1.0, 2.0)), FilterTargets(-35.0, -5.0, 3.0, 1.0, (1.0, 2.0)),
                    FilterTargets(-35.0, -15.0, float("nan"), float("nan"), None)]
    status = []
    for sc in scenarios:
        checks = [pool[i] for i in sc]
        for t_i, t in enumerate(targets_list if len(sc) % 2 == 0 else targets_list[:2]):
            res = [TP.auto_status(s, checks, t) for s in steps]
            status.append({"checks": sc, "targets": t_i, "out": [[a, b] for a, b in res]})
    seen = {}
    for e in status:
        for (a, _b) in e["out"]:
            seen[str(a)] = seen.get(str(a), 0) + 1
    print("états vus :", seen, file=sys.stderr)
    result = {"strings": list(STRINGS), "cards": cards, "pool": [enc_check(c) for c in pool],
              "targets": [enc_targets(t) for t in targets_list], "status": status, "steps": [s.key for s in steps],
              "auto": [TP.AUTO_OK, TP.AUTO_BAD], "n_steps": n_steps}
    # `strings` est une table : les identifiants sont dans l'ordre d'insertion
    text = json.dumps(result, separators=(",", ":"))
    Path(out_path).write_text(text)
    print(len(cards), "fiches,", len(STRINGS), "textes,", len(status), "listes de logs,", f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)


if __name__ == "__main__":
    main()
