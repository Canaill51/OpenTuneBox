#!/usr/bin/env python3
"""Couche « analyses » des références (D-035, phase 4c-0) : `golden.py record-analysis DOSSIER`.

Pour chaque session exploitable du corpus (celles que `load` rend), écrit `DOSSIER/analysis/<fichier>.<session>.npz` : tous
les nombres, textes et tableaux (noms stables de `golden.flatten`, flottants en float64 exact) de la période de vol
(`slice_time(*auto_period)`, comme l'assistant) pour :
- `period`    : `auto_period`, `crash_time`, `flight_parts` ;
- `datasets`  : `available`, `has_axis` de chaque jeu et de chaque axe ;
- `stats`     : `flight_stats`, `pid_error_stats` ;
- `delay`     : `compute_delays` ;
- `motors`    : `motor_hz` (tableau et indicateur « estimée »), `hover_motor_hz`, `dyn_notch_centres` et, avec eRPM et gyro non
  filtré, les appels de l'assistant : bruit moteur, hauteur des harmoniques, cibles du notch dynamique, forme et hauteur des raies,
  raies du vol stationnaire, spectre après filtre RPM ;
- `step`      : `flight_step` (métriques, retards, conditions, boucle ouverte de roll et de pitch, `LoopMetrics` entier :
  marges, écarts-types jackknife, bande, masque `valid`) ;
- `series`    : `find_series` de la session seule (sa signature).
Une section qui plante est notée (`<section>/_error`) au lieu d'être omise : Rust doit alors planter aussi (ou l'écart est
assumé et documenté). Les pièges de comparaison sont dans `docs/migration-rust.md` (phase 4c) : décisions discrètes
(indices, masques, nombres de fenêtres, NaN) à égalité exacte, valeurs continues à tolérance mesurée.

`record_analysis` fait deux passages qui doivent concorder (le poste inverse parfois un bit sous pleine charge : compétence
`calculs-corpus`) ; les fichiers discordants sont refaits seuls. `otb-parity check-analysis DOSSIER` relit ces fichiers.
"""
from __future__ import annotations

import hashlib
import io
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from corpus_labels import map_files  # noqa: E402
from golden import flatten  # noqa: E402

SECTIONS = ("period", "datasets", "stats", "delay", "motors", "step", "series")


def _section(out: dict, name: str, fn) -> None:
    try:
        out |= flatten(fn(), name)
    except Exception as e:  # noqa: BLE001 (Python plante : noté, Rust doit planter aussi)
        out[f"{name}/_error"] = np.array(f"{type(e).__name__}: {e}")


def analysis_record(lg) -> dict[str, np.ndarray]:
    from opentunebox.analysis import datasets, delay, motors, period, stats, step_series
    out: dict[str, np.ndarray] = {"_session": np.array(lg.session), "_n": np.array(lg.n)}
    box: dict = {}

    def _period():
        t0, t1 = period.auto_period(lg)
        box["sel"] = lg.slice_time(t0, t1)
        return {"auto_period": (t0, t1), "crash_time": period.crash_time(lg), "flight_parts": period.flight_parts(lg, t0, t1),
                "sel": (box["sel"].start, box["sel"].stop)}

    _section(out, "period", _period)
    sel = box.get("sel", slice(None))
    _section(out, "datasets", lambda: {"available": datasets.available(lg),
                                      "has_axis": [[datasets.has_axis(lg, k, a) for a in range(3)] for k in datasets.DATASETS]})
    _section(out, "stats", lambda: {"flight": stats.flight_stats(lg, sel), "pid_error": stats.pid_error_stats(lg, sel)})
    _section(out, "delay", lambda: delay.compute_delays(lg, sel))

    def _motors():
        mhz, est = motors.motor_hz(lg)
        res = {"motor_hz": mhz, "estimated": est, "hover": motors.hover_motor_hz(lg), "dyn": motors.dyn_notch_centres(lg)}
        if lg.motor_hz is None or lg.gyro_unfilt is None:  # les règles de l'assistant n'en font rien sans eRPM ni gyro non filtré
            return res
        st, fs, br = lg.settings, lg.fs, lg.breaks(sel)
        mh, raw, flt = lg.motor_hz[:, sel], lg.gyro_unfilt[:, sel], lg.gyro[:, sel]
        res["noise"] = [[motors.motor_noise(g[a], mh, fs, breaks=br) for g in (flt, raw)] for a in range(2)]
        res["prominence"] = [[motors.harmonic_prominence(g[a], mh, fs, breaks=br) for g in (raw, flt)] for a in range(2)]
        band = (max(st.dyn_notch_min_hz - 37, 20), st.dyn_notch_max_hz + 37)
        res["targets"] = [motors.dyn_notch_targets(raw[a], mh, fs, band, st, breaks=br) for a in range(2)]
        peaks = [float(f) for f in max(res["targets"], key=lambda x: x.freqs.size).freqs] + [133.0, 421.5]
        res["peaks"] = peaks
        res["shapes"] = [motors.line_shapes(raw[a], mh, fs, peaks, st.rpm_harmonics, breaks=br, rpm_settings=st) for a in range(2)]
        res["heights"] = [motors.line_heights(flt[a], fs, peaks, breaks=br) for a in range(2)]
        res["lines"] = [motors.spectrum_lines(raw[a], flt[a], mh, fs, (20.0, 900.0), min_db=6.0, breaks=br) for a in range(2)]
        res["filtered"] = [motors.rpm_filtered_spectrum(raw[a], mh, fs, st, breaks=br) for a in range(2)]
        return res

    _section(out, "motors", _motors)
    fs_box: dict = {}

    def _step():
        fs_box["f"] = step_series.flight_step(lg, sel, "essai")
        return fs_box["f"]

    _section(out, "step", _step)
    if "f" in fs_box:
        _section(out, "series", lambda: step_series.find_series([fs_box["f"]]))
    return out


def analysis_file(path: str) -> list[dict[str, np.ndarray]]:
    from opentunebox.log.flightlog import load
    return [analysis_record(lg) for lg in load(path)]


def digest(rec: dict[str, np.ndarray]) -> str:
    h = hashlib.sha256()
    for k in sorted(rec):
        a = np.asarray(rec[k])
        h.update(k.encode() + str(a.dtype).encode() + str(a.shape).encode() + a.tobytes())
    return h.hexdigest()


def _same(a: list[dict], b: list[dict]) -> bool:
    return [digest(x) for x in a] == [digest(x) for x in b]


def stem(rel: str, session: int) -> str:
    return f"{rel.replace(os.sep, '__')}.{session}.npz"


def record_analysis(files, root, out_dir, jobs=None) -> None:
    dest = os.path.join(out_dir, "analysis")
    os.makedirs(dest, exist_ok=True)
    a = {os.path.relpath(f, root): r for f, r in map_files(analysis_file, files, jobs, root)}
    b = {os.path.relpath(f, root): r for f, r in map_files(analysis_file, files, jobs, root)}
    for _ in range(3):  # fichiers où les deux passages divergent : refaits seuls jusqu'à concordance
        bad = [k for k in a if not _same(a[k], b.get(k, []))]
        if not bad:
            break
        print(f"{len(bad)} fichier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for k in bad:
            a[k], b[k] = analysis_file(os.path.join(root, k)), analysis_file(os.path.join(root, k))
    n = 0
    for rel, recs in a.items():
        for rec in recs:
            buf = io.BytesIO()
            np.savez_compressed(buf, **{k.replace("/", "|"): v for k, v in rec.items()})  # « / » interdit dans un nom npz
            with open(os.path.join(dest, stem(rel, int(rec["_session"]))), "wb") as f:
                f.write(buf.getvalue())
            n += 1
    print(f"[analyses] {len(a)} fichiers, {n} sessions", file=sys.stderr)
