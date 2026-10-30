#!/usr/bin/env python3
"""Cas de test de `log/debug_tables.py` et `log/modes.py` pour `rust/otb-log` (D-035, phase 4a-1).

Usage : log_tables_cases.py SORTIE.json

Écrit les tables complètes (4 familles, noms des modes de débogage et des boîtes), l'ensemble SHOWN, puis les résultats de
Python sur une grille de cas : `family` (versions limites), `debug_mode_name` (indices -2 à 120, nom explicite), `mode_names`
(masques dont négatifs et 2^31 - 1, `shown_only` vrai et faux), `mode_changes` (suites de masques). Le test
`rust/otb-log/tests/tables.rs` exige les mêmes résultats.
"""
import json
import sys

import numpy as np

from opentunebox.log import debug_tables as dt
from opentunebox.log import modes

VERSIONS = [(), (4,), (4, 3, 0), (4, 4, 0), (4, 4, 9), (4, 5, 0), (4, 6, 1), (5, 0, 0), (1999, 12), (2000, 1, 0),
            (2025, 11, 9), (2025, 12, 0), (2025, 12, 2), (2026, 5, 9), (2026, 6, 0), (2026, 6, 1), (2027, 1, 0)]
FAMILY_VERSIONS = [v for v in VERSIONS] + [(m, n, 0) for m in (3, 4, 5, 2024, 2025, 2026, 2027) for n in range(0, 14)]
MASKS = [0, 1, 2, 3, 6, 0b110, 0b1011, 2**31 - 1, -1, -2, -2**31, 1 << 40, (1 << 47) | 1, 2**33, 0x7FFF_FFFF_FFFF]
FLAG_SEQUENCES = [[0, 0, 1, 1, 3, 3, 0], [1, 3, 7, 3, 1], [2**31 - 1, 0, -1, -1, 5], [6, 4, 2, 0, 0, 6]]


def main():
    out = {
        "debug_tables": dt.TABLES,
        "mode_tables": modes.TABLES,
        "shown": sorted(modes.SHOWN),
        "family": [[list(v), dt.family(v)] for v in FAMILY_VERSIONS],
        "debug_mode_name": [],
        "mode_names": [],
        "mode_changes": [],
    }
    for v in VERSIONS:
        for i in [None, *range(-2, 121)]:
            for explicit in (None, "", " fft_freq ", "chirp"):
                out["debug_mode_name"].append([i, list(v), explicit, dt.debug_mode_name(i, v, explicit)])
        for mask in MASKS:
            for shown in (True, False):
                out["mode_names"].append([mask, list(v), shown, modes.mode_names(mask, v, shown)])
        for seq in FLAG_SEQUENCES:
            flags = np.array(seq, dtype=np.int32)
            t = np.arange(len(seq)) * 0.25 + 0.125
            out["mode_changes"].append([seq, t.tolist(), list(v), [[x, s] for x, s in modes.mode_changes(flags, t, v)]])
        out["mode_changes"].append([None, [], list(v), []])
    json.dump(out, open(sys.argv[1], "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    print({k: len(v) for k, v in out.items()})


if __name__ == "__main__":
    main()
