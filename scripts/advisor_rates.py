#!/usr/bin/env python3
"""Taux de déclenchement des constats de l'assistant sur tout le corpus (docs/assistant.md, « Calibration »).

Usage : advisor_rates.py SORTIE.json [--limit N] [-j PROCESSUS]   (dossier de logs : variable OTB_LOGS ;
        défaut : tous les cœurs, -j 1 = un fichier à la fois)
        advisor_rates.py --resume SORTIE.json                     (tableau seul, sans recalcul)

Mêmes sessions que la calibration (au moins 8 s, période analysée d'au moins 5 s). Pour chaque session :
taille détectée, vol avec ou sans mouvements, et constats (clé, titre, gravité, avec ou sans action). Le
tableau donne, par constat d'attention ou de conseil (regroupé par clé, affiché par un de ses titres), la
part des sessions signalées : toutes, vols avec mouvements, vols stationnaires.
"""
import json, os, sys
from collections import Counter

from opentunebox.advisor.rules import ATTENTION, CONSEIL, analyze
from opentunebox.analysis.period import auto_period
from opentunebox.log.flightlog import load
from corpus_labels import log_files, logs_root, map_files


def analyze_file(f, root):
    out = []
    for lg in load(f):
        if lg.duration < 8:
            continue
        sel = lg.slice_time(*auto_period(lg))
        if sel.stop - sel.start < lg.fs * 5:
            continue
        rep = analyze(lg, sel)
        out.append({"file": os.path.relpath(f, root), "sess": lg.session, "profile": rep.profile.key,
                    "moving": bool(rep.measures["delays"].moving),
                    "findings": [(x.key, x.title, x.severity, bool(x.actions)) for x in rep.findings]})
    return out


def summary(path):
    rows = json.load(open(path))
    groups = {"tous": rows, "mouvements": [r for r in rows if r["moving"]],
              "stationnaires": [r for r in rows if not r["moving"]]}
    print(f"{len(rows)} sessions : " + ", ".join(f"{k} {len(v)}" for k, v in groups.items()))
    counts = {k: Counter(key for r in v for key in {key for key, _t, sev, _ in r["findings"] if sev in (ATTENTION, CONSEIL)})
              for k, v in groups.items()}
    titles = {key: t for r in rows for key, t, _s, _ in r["findings"]}  # un titre par clé, pour la lecture
    print(f"{'constat':70s} " + " ".join(f"{k:>13s}" for k in groups))
    for key, _ in counts["tous"].most_common():
        label = f"{key} ({titles[key]})"
        print(f"{label[:70]:70s} " + " ".join(f"{100 * counts[k][key] / max(1, len(v)):12.0f}%" for k, v in groups.items()))


if __name__ == "__main__":
    if sys.argv[1] == "--resume":
        summary(sys.argv[2])
        sys.exit()
    from functools import partial
    limit = int(sys.argv[sys.argv.index("--limit") + 1]) if "--limit" in sys.argv else None
    jobs = int(sys.argv[sys.argv.index("-j") + 1]) if "-j" in sys.argv else None
    root = logs_root()
    res = map_files(partial(analyze_file, root=root), log_files(root)[:limit], jobs, root)
    json.dump([r for _, rows in res for r in rows], open(sys.argv[1], "w"), ensure_ascii=False)
    summary(sys.argv[1])
