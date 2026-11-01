#!/usr/bin/env python3
"""Analyse des arguments de la ligne de commande : `opentunebox` de Python (argparse) contre le binaire Rust (phase 5c, W-031).

Usage : cli_args_check.py [BINAIRE_RUST]   (défaut : rust/target/debug/opentunebox, à construire par `cargo build -p otb-cli`)

Rejoue des combinaisons d'arguments tirées au hasard (graines fixes) avec des fichiers qui n'existent pas, et compare le CODE DE SORTIE
des deux programmes (0 : accepté ou aide, 2 : usage faux). Le texte d'aide et les messages d'erreur ne sont pas comparés.
Mesuré avec Python 3.14.4 (le comportement d'argparse sur `-5x`, `-hx`, `--help=` dépend de la version). Code 1 si un cas diffère.
"""
import os
import random
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PY = os.path.join(ROOT, ".venv", "bin", "opentunebox")
RS = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "rust", "target", "debug", "opentunebox")
POOL = ["nofile1", "nofile2", "--profil", "five", "zz", "--profil=whoop", "--profil=zz", "--prof", "--pro=micro", "--p", "-h", "--help",
        "--he", "--", "-x", "-", "--profil=", "-5", "--inconnue", "--inconnue=3", "-.5", "-5.", "--prof=five", "--he=3", "-h5", "-hx",
        "micro", "--profil=large", "---", "-1e3", "-٣", "-5 6", "-é", "-é1", "-.x", "-.", "--profil=a b", "a b", "-_1", "--help=", "-x y", "--zz a"]


def code(program, args):
    return subprocess.run([program, *args], capture_output=True, text=True).returncode


def main() -> int:
    cases = [[], ["info"], ["report"], ["autre", "nofile1"], ["-x", "info", "nofile1"], ["nofile1"],
             ["report", "--profil", "--", "five"], ["report", "nofile1", "--profil", "--", "five"],
             ["report", "--profil", "five", "--", "nofile1"], ["info", "-x y"], ["info", "--zz a"], ["report", "nofile1", "-x y"]]
    for seed in (7, 2026, 99):
        random.seed(seed)
        cases += [[random.choice(["info", "report", "report"])] + [random.choice(POOL) for _ in range(random.randint(1, 5))]
                  for _ in range(260)]
    bad = 0
    for c in cases:
        a, b = code(PY, c), code(RS, c)
        if a != b:
            bad += 1
            print("≠", c, "python", a, "rust", b)
    print(f"{len(cases)} cas, {bad} différents")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
