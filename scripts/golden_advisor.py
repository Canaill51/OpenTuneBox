#!/usr/bin/env python3
"""Couche « assistant » des références (D-035, phase 5a-0) : `golden.py record-advisor DOSSIER`.

Pour chaque session que l'assistant analyse (mêmes critères que `advisor_rates.py` : au moins 8 s, période d'au moins
5 s), écrit `DOSSIER/advisor/<fichier>.<session>.npz` avec TOUT ce que produisent `rules.analyze` et
`noise.noise_report` :
- `report`  : profil (clé, raison), période, chaque constat (clé, catégorie, gravité, titre, détail, actions, axe,
  réglages cités) dans l'ordre de `findings` ET dans celui de `sorted()`, mesures (`measures`, noms de `golden.flatten`) ;
- `noise`   : bande, vol avec mouvements, bruit de chaque axe, hauteur des harmoniques, constats rangés par partie,
  blocs de filtres (titre, réglages, observations, constats, verdict) ;
- `tuning`  : voir `record_tuning` (réponse indicielle par séries, un fichier par dossier de drone).
Les textes sont comparés caractère par caractère. Une section qui plante est notée (`<section>/_error`) : Rust doit
alors planter aussi. `record_advisor` fait deux passages qui doivent concorder (le poste inverse parfois un bit sous
pleine charge : compétence `calculs-corpus`). `otb-parity check-advisor DOSSIER` relit ces fichiers.
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


def finding_dict(f) -> dict:
    return {"key": f.key, "category": f.category, "severity": f.severity, "title": f.title, "detail": f.detail,
            "actions": list(f.actions), "n_actions": len(f.actions), "axis": f.axis, "settings": list(f.settings),
            "n_settings": len(f.settings)}


def findings_list(fs) -> dict:
    return {"n": len(fs), "items": [finding_dict(f) for f in fs]}


def report_dict(rep) -> dict:
    return {"profile": rep.profile.key, "reason": rep.profile_reason, "period": rep.period,
            "findings": findings_list(rep.findings), "sorted": findings_list(rep.sorted()),
            "steps": [s.axis for s in rep.steps], "measures": rep.measures}


def block_dict(b) -> dict | None:
    if b is None:
        return None
    return {"title": b.title, "settings": b.settings, "observations": list(b.observations),
            "n_observations": len(b.observations), "findings": findings_list(b.findings),
            "verdict": None if b.verdict is None else finding_dict(b.verdict)}


def noise_dict(nr) -> dict:
    return {"band": nr.band, "moving": nr.moving, "has_unfiltered": nr.has_unfiltered,
            "gyro": [vars(x) for x in nr.gyro], "n_gyro": len(nr.gyro),
            "dterm": [vars(x) for x in nr.dterm], "n_dterm": len(nr.dterm), "harmonics": nr.harmonics,
            "conditions": findings_list(nr.conditions), "gyro_findings": findings_list(nr.gyro_findings),
            "dterm_findings": findings_list(nr.dterm_findings), "strategy": findings_list(nr.strategy),
            "gyro_blocks": [block_dict(b) for b in nr.gyro_blocks], "n_blocks": len(nr.gyro_blocks),
            "gyro_other": findings_list(nr.gyro_other), "dterm_block": block_dict(nr.dterm_block)}


def _section(out: dict, name: str, fn) -> None:
    try:
        out |= flatten(fn(), name)
    except Exception as e:  # noqa: BLE001 (Python plante : noté, Rust doit planter aussi)
        out[f"{name}/_error"] = np.array(f"{type(e).__name__}: {e}")


def advisor_record(lg) -> dict[str, np.ndarray] | None:
    """Références d'une session ; None si l'assistant ne l'analyse pas (trop courte)."""
    from opentunebox.advisor.noise import noise_report
    from opentunebox.advisor.rules import analyze
    from opentunebox.analysis.period import auto_period
    if lg.duration < 8:
        return None
    sel = lg.slice_time(*auto_period(lg))
    if sel.stop - sel.start < lg.fs * 5:
        return None
    out: dict[str, np.ndarray] = {"_session": np.array(lg.session), "_n": np.array(lg.n),
                                  "_sel": np.array([sel.start, sel.stop])}
    _section(out, "report", lambda: report_dict(analyze(lg, sel)))
    _section(out, "noise", lambda: noise_dict(noise_report(lg, sel)))
    return out


def advisor_file(path: str) -> list[dict[str, np.ndarray]]:
    from opentunebox.log.flightlog import load
    return [r for lg in load(path) if (r := advisor_record(lg)) is not None]


def record_advisor(files, root, out_dir, jobs=None) -> None:
    dest = os.path.join(out_dir, "advisor")
    os.makedirs(dest, exist_ok=True)
    a = {os.path.relpath(f, root): r for f, r in map_files(advisor_file, files, jobs, root)}
    b = {os.path.relpath(f, root): r for f, r in map_files(advisor_file, files, jobs, root)}
    for _ in range(3):  # fichiers où les deux passages divergent : refaits seuls jusqu'à concordance
        bad = [k for k in a if not _same(a[k], b.get(k, []))]
        if not bad:
            break
        print(f"{len(bad)} fichier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for k in bad:
            a[k], b[k] = advisor_file(os.path.join(root, k)), advisor_file(os.path.join(root, k))
    n = 0
    for rel, recs in a.items():
        for rec in recs:
            buf = io.BytesIO()
            np.savez_compressed(buf, **{k.replace("/", "|"): v for k, v in rec.items()})  # « / » interdit dans un nom npz
            with open(os.path.join(dest, stem(rel, int(rec["_session"]))), "wb") as f:
                f.write(buf.getvalue())
            n += 1
    print(f"[assistant] {len(a)} fichiers, {n} sessions", file=sys.stderr)
