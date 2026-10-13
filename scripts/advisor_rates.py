#!/usr/bin/env python3
"""Taux de déclenchement des constats de l'assistant sur tout le corpus (docs/assistant.md, « Calibration »).

Usage : advisor_rates.py SORTIE.json [--limit N] [-j PROCESSUS]   (dossier de logs : variable OTB_LOGS ;
        défaut : tous les cœurs, -j 1 = un fichier à la fois)
        advisor_rates.py --resume SORTIE.json                     (tableau seul, sans recalcul)
        advisor_rates.py --compare AVANT.json APRÈS.json [N]      (constats différents, N sessions détaillées)

Mêmes sessions que la calibration (au moins 8 s, période analysée d'au moins 5 s). Pour chaque session :
taille détectée, vol avec ou sans mouvements, et constats (clé, titre, gravité, avec ou sans action). Le
tableau donne, par constat d'attention ou de conseil (regroupé par clé, affiché par un de ses titres), la
part des sessions signalées : toutes, vols avec mouvements, vols stationnaires.

Chaque session garde aussi le texte complet de ses constats (`details` : clé, axe, gravité, titre, détail,
actions) et la durée de l'analyse : `--compare` prouve qu'une modification (optimisation, refactorisation)
ne change aucun conseil, chiffres affichés compris, et mesure le temps gagné. Code de sortie 1 si une
session diffère.
"""
import json, os, sys, time
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
        t0 = time.perf_counter()
        rep = analyze(lg, sel)
        out.append({"file": os.path.relpath(f, root), "sess": lg.session, "profile": rep.profile.key,
                    "moving": bool(rep.measures["delays"].moving), "t_analyze": time.perf_counter() - t0,
                    "findings": [(x.key, x.title, x.severity, bool(x.actions)) for x in rep.findings],
                    "details": [(x.key, x.axis, x.severity, x.title, x.detail, list(x.actions)) for x in rep.findings]})
    return out


def compare(old_path, new_path, show=10):
    """Sessions dont les constats (texte complet) diffèrent entre deux passages ; renvoie leur nombre."""
    load = lambda p: {(r["file"], r["sess"]): r for r in json.load(open(p))}  # noqa: E731
    a, b = load(old_path), load(new_path)
    print(f"sessions : {len(a)} avant, {len(b)} après ; présentes d'un seul côté : {len(a.keys() ^ b.keys())}")
    if any("details" not in r for r in (*a.values(), *b.values())):
        print("passage sans texte complet (ancienne version du script) : comparaison des clés et titres seulement")
    field = lambda r: r.get("details", r["findings"])  # noqa: E731
    diff = sorted(k for k in a.keys() & b.keys() if field(a[k]) != field(b[k]) or a[k]["profile"] != b[k]["profile"])
    print(f"sessions aux constats différents : {len(diff)}")
    for k in diff[:show]:
        print(f"== {k[0]} (session {k[1]})")
        fa, fb = [list(x) for x in field(a[k])], [list(x) for x in field(b[k])]
        for x in fa:
            if x not in fb:
                print("  -", x)
        for x in fb:
            if x not in fa:
                print("  +", x)
    if all("t_analyze" in r for r in (*a.values(), *b.values())) and a and b:
        ta, tb = (sum(r["t_analyze"] for r in d.values()) for d in (a, b))
        ma, mb = (max(r["t_analyze"] for r in d.values()) for d in (a, b))
        print(f"durée cumulée de l'assistant : {ta:.0f} s → {tb:.0f} s ; session la plus lente : {ma:.1f} s → {mb:.1f} s")
    return len(diff) + len(a.keys() ^ b.keys())


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
    if sys.argv[1] == "--compare":
        sys.exit(1 if compare(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 10) else 0)
    from functools import partial
    limit = int(sys.argv[sys.argv.index("--limit") + 1]) if "--limit" in sys.argv else None
    jobs = int(sys.argv[sys.argv.index("-j") + 1]) if "-j" in sys.argv else None
    root = logs_root()
    res = map_files(partial(analyze_file, root=root), log_files(root)[:limit], jobs, root)
    json.dump([r for _, rows in res for r in rows], open(sys.argv[1], "w"), ensure_ascii=False)
    summary(sys.argv[1])
