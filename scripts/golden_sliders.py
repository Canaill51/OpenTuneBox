#!/usr/bin/env python3
"""Couche « curseurs » des références (D-035, phase 4b-5) : `golden.py record-sliders DOSSIER`.

Écrit `DOSSIER/sliders.jsonl` (une ligne par session du corpus : `{"file", "session", "view", "estimate", "calc"}`) :
`slider_view(settings)` (curseurs, origine, cohérence), l'estimation `sliders_from_pids(settings.pids)` et, quand l'en-tête
porte un réglage simplifié, les PID recalculés par `pids_from_sliders` à partir de ses curseurs. Les flottants s'écrivent en
bits (`"f64:<16 chiffres hexa>"`). `otb-parity check-sliders DOSSIER` relit ce fichier et refait les trois calculs en Rust à
partir de l'en-tête lu en Rust : les appels enregistrés par `record-modules` sont presque tous aux valeurs par défaut, ceux-ci
couvrent tous les vrais réglages du corpus.
"""
from __future__ import annotations

import json
import os
import sys

from golden_settings import f64

from opentunebox.analysis import sliders
from opentunebox.decoder.header import find_log_starts, parse_header
from opentunebox.log.settings import parse_settings


def record_sliders(files, root, out_dir) -> None:
    os.makedirs(out_dir, exist_ok=True)
    n = 0
    with open(os.path.join(out_dir, "sliders.jsonl"), "w", encoding="utf-8") as out:
        for i, f in enumerate(files):
            data = open(f, "rb").read()
            starts = find_log_starts(data)
            for k, s in enumerate(starts):
                rec = {"file": os.path.relpath(f, root), "session": k + 1}
                try:
                    st = parse_settings(parse_header(data, s, starts[k + 1] if k + 1 < len(starts) else len(data)).raw)
                    v = sliders.slider_view(st)
                    rec["view"] = {"values": [[c, f64(x)] for c, x in v.values.items()], "from_header": v.from_header,
                                   "consistent": v.consistent}
                    rec["estimate"] = [[c, f64(x)] for c, x in sliders.sliders_from_pids(st.pids, st.family).items()]
                    sl = st.sliders()
                    rec["calc"] = ([[p.P, p.I, p.D, p.Dmax, p.F] for p in sliders.pids_from_sliders(sl, st.family, base=st.pids)]
                                   if sl.get("mode", 0) > 0 else None)
                except Exception as e:  # noqa: BLE001 (Python plante : noté, non comparé)
                    rec["error"] = f"{type(e).__name__}: {e}"
                out.write(json.dumps(rec, ensure_ascii=False, separators=(",", ":")) + "\n")
                n += 1
            if (i + 1) % 100 == 0:
                print(f"[curseurs] {i + 1}/{len(files)} fichiers, {n} sessions", file=sys.stderr)
    print(f"[curseurs] {len(files)} fichiers, {n} sessions", file=sys.stderr)
