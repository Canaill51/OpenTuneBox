#!/usr/bin/env python3
"""Couche « en-têtes » des références (D-035, phase 4a-2) : `golden.py record-header DOSSIER`.

Écrit `DOSSIER/header.jsonl` : une ligne JSON par session de chaque fichier du corpus (toutes les sessions, même celles que
le décodeur écarte), `{"file", "session", "start", "end", "raw", "frames", "data_start", "data_end", "props"}`, sans rien
de propre à Python (pas de `repr`). `otb-parity check-header DOSSIER` relit ce fichier et refait l'analyse en Rust.
"""
from __future__ import annotations

import json
import os
import sys

from opentunebox.decoder.header import find_log_starts, parse_header


def session_record(data: bytes, k: int, start: int, end: int) -> dict:
    h = parse_header(data, start, end)
    return {
        "session": k + 1, "start": start, "end": end, "raw": h.raw,
        "frames": {t: {"names": f.names, "signed": f.signed, "predictor": f.predictor, "encoding": f.encoding}
                   for t, f in h.frames.items()},
        "data_start": h.data_start, "data_end": h.data_end,
        "props": {"data_version": h.data_version, "i_interval": h.i_interval, "p_interval": list(h.p_interval),
                  "minthrottle": h.minthrottle, "motor_output": list(h.motor_output), "vbatref": h.vbatref},
    }


def record_header(files, root, out_dir) -> None:
    os.makedirs(out_dir, exist_ok=True)
    n = 0
    with open(os.path.join(out_dir, "header.jsonl"), "w", encoding="utf-8") as out:
        for i, f in enumerate(files):
            data = open(f, "rb").read()
            starts = find_log_starts(data)
            for k, s in enumerate(starts):
                rec = {"file": os.path.relpath(f, root)}
                try:
                    rec |= session_record(data, k, s, starts[k + 1] if k + 1 < len(starts) else len(data))
                except Exception as e:  # noqa: BLE001 (Python plante sur ce fichier : noté, non comparé)
                    rec |= {"session": k + 1, "error": f"{type(e).__name__}: {e}"}
                out.write(json.dumps(rec, ensure_ascii=False, separators=(",", ":")) + "\n")
                n += 1
            if (i + 1) % 100 == 0:
                print(f"[en-têtes] {i + 1}/{len(files)} fichiers, {n} sessions", file=sys.stderr)
    print(f"[en-têtes] {len(files)} fichiers, {n} sessions", file=sys.stderr)
