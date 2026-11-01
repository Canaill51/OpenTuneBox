#!/usr/bin/env python3
"""Couche « ligne de commande » des références (D-035, phase 5c) : `golden.py record-cli DOSSIER`.

Écrit `DOSSIER/cli.json` : pour chaque fichier du corpus, le texte EXACT que Python écrit sur la sortie standard pour
`opentunebox info FICHIER`, `opentunebox report FICHIER` et `opentunebox report FICHIER --profil whoop` (`cli.main`). Si Python plante,
le texte déjà écrit est gardé, suivi de `@@ERREUR <type de l'exception>` : Rust doit écrire le même texte puis échouer. Le dossier du corpus est remplacé par `@` dans les textes
(les lignes « illisible » et « aucune session » citent le chemin). `otb-parity check-cli DOSSIER` rejoue les deux commandes en Rust
(`otb_cli::info` / `otb_cli::report`) et compare **caractère par caractère** ; seul le message d'exception entre parenthèses d'une
ligne « illisible » n'est pas comparé (texte propre à Python).
Deux passages qui doivent concorder (compétence `calculs-corpus`).
"""
from __future__ import annotations

import contextlib
import io
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from corpus_labels import map_files  # noqa: E402


def cli_file(path: str) -> dict[str, str]:
    from opentunebox.cli import main
    out = {}
    for name, argv in (("info", ["info", path]), ("report", ["report", path]),
                       ("report_whoop", ["report", path, "--profil", "whoop"])):
        buf = io.StringIO()
        try:
            with contextlib.redirect_stdout(buf):
                main(argv)
            out[name] = buf.getvalue()
        except Exception as e:  # noqa: BLE001 (Python plante : texte déjà écrit gardé, Rust doit échouer aussi)
            out[name] = buf.getvalue() + f"@@ERREUR {type(e).__name__}"
    return out


def record_cli(files, root, out_dir, jobs=None) -> None:
    a = dict(map_files(cli_file, files, jobs, root))
    b = dict(map_files(cli_file, files, jobs, root))
    for _ in range(3):  # fichiers où les deux passages divergent : refaits seuls jusqu'à concordance
        bad = [k for k in a if a[k] != b.get(k)]
        if not bad:
            break
        print(f"{len(bad)} fichier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for k in bad:
            a[k], b[k] = cli_file(k), cli_file(k)
    prefix = root.rstrip("/") + "/"
    ref = {os.path.relpath(f, root): {c: t.replace(prefix, "@/") for c, t in r.items()} for f, r in sorted(a.items())}
    with open(os.path.join(out_dir, "cli.json"), "w", encoding="utf-8") as fh:
        json.dump(ref, fh, ensure_ascii=False)
    n_err = sum("@@ERREUR" in t for r in ref.values() for t in r.values())
    print(f"[cli] {len(ref)} fichiers, {n_err} sortie(s) en erreur", file=sys.stderr)
