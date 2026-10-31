#!/usr/bin/env python3
"""Couche « guide » des références (D-035, phase 5b-4) : `golden.py record-guide DOSSIER`.

Pour chaque session de chaque log, écrit `DOSSIER/guide/<fichier>.<session>.npz` avec TOUT ce que produit le guide intégré :
- `check`       : `log_check.check_log(lg, label=lg.name, exact=False)` (points avec textes et corrections, usages, parties du vol,
  fenêtres, curseurs, rôle, retard consigne → gyro, vues `items_for` / `usages_for` / `cli_commands` des deux rôles) ;
- `check_exact` : le même avec `exact=True` (nombre réel de fenêtres de la réponse indicielle) ;
- `targets`     : `log_check.filter_targets(lg, parts, inch)` (`inch` = taille d'hélice du profil détecté), repères et textes.
Les textes sont comparés caractère par caractère. Une section qui plante est notée (`<section>/_error`) : Rust doit alors planter aussi.
Deux passages qui doivent concorder (compétence `calculs-corpus`). `otb-parity check-guide DOSSIER` relit ces fichiers.
"""
from __future__ import annotations

import io
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from corpus_labels import map_files  # noqa: E402
from golden import flatten  # noqa: E402
from golden_analysis import _same, stem  # noqa: E402


def check_dict(ck) -> dict:
    def item(it):
        return {"key": it.key, "title": it.title, "status": it.status, "detail": it.detail, "n_fixes": len(it.fixes),
                "fixes": [{"where": f.where, "cli": f.cli} for f in it.fixes]}

    def usage(u):
        return {"key": u.key, "label": u.label, "ok": bool(u.ok), "reason": u.reason, "shown": bool(u.shown)}

    def part(p):
        return None if p is None else [float(p[0]), float(p[1])]

    roles = (None, "filters", "pid")
    return {"label": ck.label, "moving": bool(ck.moving), "hover": part(ck.parts["hover"]), "moving_part": part(ck.parts["moving"]),
            "n_items": len(ck.items), "items": [item(i) for i in ck.items], "n_usages": len(ck.usages),
            "usages": [usage(u) for u in ck.usages], "n_windows": len(ck.windows), "windows": [int(w) for w in ck.windows],
            "exact": bool(ck.exact), "sliders": {k: float(v) for k, v in ck.sliders.items()}, "role": ck.role,
            "n_sp_delay": len(ck.sp_delay_ms), "sp_delay": [float(v) for v in ck.sp_delay_ms],
            "items_for": {str(r): [i.key for i in ck.items_for(r)] for r in roles},
            "usages_for": {str(r): [u.key for u in ck.usages_for(r)] for r in roles},
            "n_cli": len(ck.cli_commands()), "cli": ck.cli_commands()}


def targets_dict(ft) -> dict | None:
    if ft is None:
        return None
    return {"gyro_ptb_db": float(ft.gyro_ptb_db), "dterm_ptb_db": float(ft.dterm_ptb_db), "gyro_delay_ms": float(ft.gyro_delay_ms),
            "dterm_delay_ms": float(ft.dterm_delay_ms),
            "goal": None if ft.delay_goal_ms is None else [float(ft.delay_goal_ms[0]), float(ft.delay_goal_ms[1])],
            "n_checks": len(ft.checks()), "checks": [{"name": a, "ok": b, "text": c} for a, b, c in ft.checks()],
            "reached": ft.reached()}


def _section(out: dict, name: str, fn) -> None:
    try:
        out |= flatten(fn(), name)
    except Exception as e:  # noqa: BLE001 (Python plante : noté, Rust doit planter aussi)
        out[f"{name}/_error"] = np.array(f"{type(e).__name__}: {e}")


def guide_record(lg) -> dict[str, np.ndarray]:
    """Références d'une session (toutes sont lues par le guide)."""
    from opentunebox.advisor.log_check import check_log, filter_targets
    from opentunebox.advisor.profiles import detect_profile
    out: dict[str, np.ndarray] = {"_session": np.array(lg.session), "_n": np.array(lg.n)}
    _section(out, "check", lambda: check_dict(check_log(lg, None, lg.name, False)))
    _section(out, "check_exact", lambda: check_dict(check_log(lg, None, lg.name, True)))

    def targets():
        ck = check_log(lg, None, lg.name, False)
        return targets_dict(filter_targets(lg, ck.parts, detect_profile(lg)[0].nominal_inch))

    _section(out, "targets", targets)
    return out


def guide_file(path: str) -> list[dict[str, np.ndarray]]:
    from opentunebox.log.flightlog import load
    return [guide_record(lg) for lg in load(path)]


def record_guide(files, root, out_dir, jobs=None) -> None:
    dest = os.path.join(out_dir, "guide")
    os.makedirs(dest, exist_ok=True)
    a = {os.path.relpath(f, root): r for f, r in map_files(guide_file, files, jobs, root)}
    b = {os.path.relpath(f, root): r for f, r in map_files(guide_file, files, jobs, root)}
    for _ in range(3):  # fichiers où les deux passages divergent : refaits seuls jusqu'à concordance
        bad = [k for k in a if not _same(a[k], b.get(k, []))]
        if not bad:
            break
        print(f"{len(bad)} fichier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for k in bad:
            a[k], b[k] = guide_file(os.path.join(root, k)), guide_file(os.path.join(root, k))
    n = n_err = 0
    for rel, recs in a.items():
        for rec in recs:
            buf = io.BytesIO()
            np.savez_compressed(buf, **{k.replace("/", "|"): v for k, v in rec.items()})  # « / » interdit dans un nom npz
            with open(os.path.join(dest, stem(rel, int(rec["_session"]))), "wb") as f:
                f.write(buf.getvalue())
            n += 1
            n_err += any(k.endswith("/_error") for k in rec)
    print(f"[guide] {len(a)} fichiers, {n} sessions, {n_err} avec une section en erreur", file=sys.stderr)
