#!/usr/bin/env python3
"""Couche « journaux de vol » des références (D-035, phase 4a-5) : `golden.py record-flightlog DOSSIER`.

`flightlog_record(lg)` résume un `FlightLog` sans rien de propre à Python : empreinte (`dtype|forme|sha256`, comme `golden.py`)
de chaque tableau (`t`, `gyro`, …, `events`), `fs` en bits, `stats`, `pid_columns`, puis les propriétés dérivées : `pid_sum`,
`pid_error`, `armed` (empreintes), `cells`, `segments`, et `d_term_prefilter` (un échantillon tous les 1009 points, en bits : sa dérivée et ses
moindres carrés passent par BLAS, donc comparés avec une tolérance, pas par empreinte, voir `docs/migration-rust.md`).
`record_flightlog` écrit `DOSSIER/flightlog.json` (`{fichier: [session, …]}`) après deux passages qui doivent concorder (le poste
inverse parfois un bit sous pleine charge : compétence `calculs-corpus`). `otb-parity check-flightlog DOSSIER` le relit.
"""
from __future__ import annotations

import json
import os
import struct
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from corpus_labels import map_files  # noqa: E402
from golden import _digest  # noqa: E402

ARRAYS = ("t", "gyro", "gyro_unfilt", "setpoint", "rc_command", "p_term", "i_term", "d_term", "f_term", "throttle", "motors",
          "motor_hz", "debug", "vbat", "current", "acc", "mode_flags", "events")
STRIDE = 1009


def f64(x: float) -> str:
    return "f64:" + struct.pack(">d", x).hex()


def f32_hex(a: np.ndarray) -> list[str]:
    return [struct.pack(">f", float(v)).hex() for v in a]


def flightlog_record(lg) -> dict:
    d: dict = {"session": lg.session, "n": lg.n, "fs": f64(lg.fs)}
    for name in ARRAYS:
        a = getattr(lg, name)
        d[name] = None if a is None else _digest(a)
    d["stats"] = json.loads(json.dumps(lg.stats, default=str, sort_keys=True))
    d["pid_columns"] = sorted(lg.pid_columns)
    d["pid_sum"], d["pid_error"], d["armed"] = _digest(lg.pid_sum), _digest(lg.pid_error), _digest(lg.armed)
    d["cells"] = lg.cells
    d["segments"] = [[s.start, s.stop] for s in lg.segments()]
    d["d_term_prefilter"] = {"stride": STRIDE, "shape": list(lg.d_term_prefilter.shape),
                             "samples": [f32_hex(lg.d_term_prefilter[ax, ::STRIDE]) for ax in range(3)]}
    return d


def flightlog_file(path: str) -> list[dict]:
    from opentunebox.log.flightlog import load
    return [flightlog_record(lg) for lg in load(path)]


def record_flightlog(files, root, out_dir, jobs=None) -> None:
    os.makedirs(out_dir, exist_ok=True)
    a = {os.path.relpath(f, root): r for f, r in map_files(flightlog_file, files, jobs, root)}
    b = {os.path.relpath(f, root): r for f, r in map_files(flightlog_file, files, jobs, root)}
    for _ in range(3):  # fichiers où les deux passages divergent : refaits seuls jusqu'à concordance
        bad = [k for k in a if a[k] != b.get(k)]
        if not bad:
            break
        print(f"{len(bad)} fichier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for k in bad:
            a[k], b[k] = flightlog_file(os.path.join(root, k)), flightlog_file(os.path.join(root, k))
    json.dump(a, open(os.path.join(out_dir, "flightlog.json"), "w", encoding="utf-8"), sort_keys=True, separators=(",", ":"))
    print(f"[journaux] {len(a)} fichiers, {sum(len(v) for v in a.values())} sessions", file=sys.stderr)
