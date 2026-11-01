#!/usr/bin/env python3
"""Textes de référence de la ligne de commande pour `rust/otb-cli/tests/cli.rs` (phase 5c).

Usage : cli_cases.py   (écrit rust/otb-cli/tests/data/jeno5_5s.{info,report,report_whoop}.txt)

Le texte EXACT que `opentunebox info|report` de Python écrit pour `tests/data/jeno5_5s.bbl` ; le test Rust le compare caractère par
caractère, sans le corpus (la CI n'en a pas).
"""
import contextlib
import io
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "src"))
from opentunebox.cli import main  # noqa: E402


def main_cases() -> None:
    log = os.path.join(ROOT, "tests", "data", "jeno5_5s.bbl")
    out_dir = os.path.join(ROOT, "rust", "otb-cli", "tests", "data")
    os.makedirs(out_dir, exist_ok=True)
    for name, argv in (("info", ["info", log]), ("report", ["report", log]),
                       ("report_whoop", ["report", log, "--profil", "whoop"])):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            main(argv)
        with open(os.path.join(out_dir, f"jeno5_5s.{name}.txt"), "w", encoding="utf-8", newline="") as f:
            f.write(buf.getvalue())
        print(name, len(buf.getvalue().splitlines()), "lignes")


if __name__ == "__main__":
    main_cases()
