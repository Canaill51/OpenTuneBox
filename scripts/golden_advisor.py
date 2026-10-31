#!/usr/bin/env python3
"""Couche « assistant » des références (D-035, phase 5a-0) : `golden.py record-advisor DOSSIER`.

Pour chaque session que l'assistant analyse (mêmes critères que `advisor_rates.py` : au moins 8 s, période d'au moins
5 s), écrit `DOSSIER/advisor/<fichier>.<session>.npz` avec TOUT ce que produisent `rules.analyze` et
`noise.noise_report` :
- `report`  : profil (clé, raison), période, chaque constat (clé, catégorie, gravité, titre, détail, actions, axe,
  réglages cités) dans l'ordre de `findings` ET dans celui de `sorted()`, mesures (`measures`, noms de `golden.flatten`) ;
- `noise`   : bande, vol avec mouvements, bruit de chaque axe, hauteur des harmoniques, constats rangés par partie,
  blocs de filtres (titre, réglages, observations, constats, verdict) ;
Une autre couche, `record_tuning` (`golden.py record-tuning DOSSIER`), écrit la réponse indicielle par séries de vols
(`step_tuning.step_tuning`, un fichier par dossier : `DOSSIER/advisor_tuning/<dossier>.npz`, clés `tuning|…`) : voir sa docstring.
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
from golden_analysis import _same, digest, stem  # noqa: E402


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


# ------------------------------------------------------------------ réponse indicielle par séries (5a-5)
def tuning_file(path: str) -> list[tuple[int, object]]:
    """[(session, FlightStep ou texte d'erreur)] des sessions que l'assistant analyse (mêmes critères que `advisor_record`).
    `loops` est vidé : `step_tuning` ne le lit pas, et les courbes alourdiraient le passage entre processus."""
    from opentunebox.analysis.period import auto_period
    from opentunebox.analysis.step_series import flight_step
    from opentunebox.log.flightlog import load
    out: list[tuple[int, object]] = []
    for lg in load(path):
        if lg.duration < 8:
            continue
        sel = lg.slice_time(*auto_period(lg))
        if sel.stop - sel.start < lg.fs * 5:
            continue
        try:
            f = flight_step(lg, sel, lg.name)
            f.loops = []
            out.append((lg.session, f))
        except Exception as e:  # noqa: BLE001 (Python plante : noté, Rust doit planter aussi)
            out.append((lg.session, f"{type(e).__name__}: {e}"))
    return out


def tuning_groups(rel: str) -> list[str]:
    """Groupes d'un log : son dossier du premier niveau (`Cedric`), et, plus bas, son dossier de drone (`Cedric/Air65`)."""
    parts = rel.split(os.sep)
    out = [parts[0]] if len(parts) > 1 and parts[0] != "golden" else []
    if len(parts) > 2 and out:
        out.append(os.sep.join(parts[:2]))
    return out


def tuning_dict(items, rep) -> dict:
    """`items` : [(fichier, session, FlightStep)] dans l'ordre des vols."""
    return {"n_flights": len(items), "files": sorted({r for r, _, _ in items}),
            "flights": [{"file": r, "label": f.label, "session": s, "n": list(f.n)} for r, s, f in items],
            "conditions": findings_list(rep.conditions), "n_blocks": len(rep.blocks),
            "blocks": [{"title": b.title, "series": None if b.series is None else
                        {"key": b.series.key, "flights": list(b.series.flights), "values": list(b.series.values)},
                        "lines": list(b.lines), "n_lines": len(b.lines),
                        "finding": None if b.finding is None else finding_dict(b.finding), "best": b.best} for b in rep.blocks]}


def tuning_records(by_file: dict) -> tuple[dict, dict]:
    """({groupe: références aplaties}, {groupe: fichiers}). Vols dans l'ordre alphabétique des chemins, puis des sessions.
    Chaque référence liste ses fichiers (`tuning|files[i]`, même en erreur) : Rust relit exactement ceux-là."""
    from opentunebox.advisor.step_tuning import step_tuning
    members: dict[str, list[str]] = {}
    for rel in sorted(by_file):
        for g in tuning_groups(rel):
            members.setdefault(g, []).append(rel)
    out = {}
    for g, rels in members.items():
        items = [(rel, s, f) for rel in rels for s, f in by_file[rel]]
        if not items:
            continue
        errors = [f"{r} session {s} : {f}" for r, s, f in items if isinstance(f, str)]
        if errors:
            out[g] = flatten({"files": sorted({r for r, _, _ in items})}, "tuning") | {
                "tuning/_error": np.array("flight_step : " + "; ".join(errors))}
            continue
        rec: dict[str, np.ndarray] = {}
        _section(rec, "tuning", lambda: tuning_dict(items, step_tuning([f for _, _, f in items])))
        if "tuning/_error" in rec:
            rec |= flatten({"files": sorted({r for r, _, _ in items})}, "tuning")
        out[g] = rec
    return out, members


def record_tuning(files, root, out_dir, jobs=None) -> None:
    """Un fichier par dossier : `DOSSIER/advisor_tuning/<dossier>.npz` (`Cedric.npz`, `Cedric__Air65.npz`), clés `tuning|…`.

    Pour chaque dossier du premier niveau sous `root` et chaque dossier de drone en dessous : la liste des `FlightStep` de toutes
    les sessions analysées par l'assistant (chemins triés, sessions dans l'ordre du log, nom = `FlightLog.name`) passe par `step_tuning` ;
    on garde les conditions, les blocs (titre, série, lignes, constat entier, vol conseillé). Python qui plante : `tuning|_error`.
    Les vols sont calculés en parallèle (`map_files`), deux passages qui doivent concorder (compétence `calculs-corpus`)."""
    dest = os.path.join(out_dir, "advisor_tuning")
    os.makedirs(dest, exist_ok=True)
    a = {os.path.relpath(f, root): r for f, r in map_files(tuning_file, files, jobs, root)}
    b = {os.path.relpath(f, root): r for f, r in map_files(tuning_file, files, jobs, root)}
    ra, members = tuning_records(a)
    rb, _ = tuning_records(b)
    for _ in range(3):  # dossiers où les deux passages divergent : fichiers refaits seuls jusqu'à concordance
        bad = [g for g in ra if digest(ra[g]) != digest(rb.get(g, {}))]
        if not bad:
            break
        print(f"{len(bad)} dossier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for rel in sorted({rel for g in bad for rel in members[g]}):
            a[rel], b[rel] = tuning_file(os.path.join(root, rel)), tuning_file(os.path.join(root, rel))
        ra, members = tuning_records(a)
        rb, _ = tuning_records(b)
    n_err = 0
    for g, rec in ra.items():
        buf = io.BytesIO()
        np.savez_compressed(buf, **{k.replace("/", "|"): v for k, v in rec.items()})
        with open(os.path.join(dest, g.replace(os.sep, "__") + ".npz"), "wb") as fh:
            fh.write(buf.getvalue())
        n_err += "tuning/_error" in rec
    print(f"[réponse indicielle] {len(ra)} dossiers, {n_err} en erreur", file=sys.stderr)
