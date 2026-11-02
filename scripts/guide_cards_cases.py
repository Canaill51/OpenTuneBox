#!/usr/bin/env python3
"""Cas de test du fichier des fiches drone pour `rust/otb-gui/src/guide.rs` (6-5a).

Usage : guide_cards_cases.py SORTIE.json

Chaque cas est un texte de fichier `drones.json` (`input`, écrit comme le ferait Python ou un autre outil) et le texte que la version
Python réécrit après l'avoir lu : `gui/guide.py:load_cards` puis `save_cards`, vraiment exécutées (`output`). La version Rust doit
réécrire exactement les mêmes octets. `"lost": true` = Python ignore le fichier (liste vide, il n'écrit rien de plus ici).

Le dossier de données de Qt (`cards_path`) est mesuré à part, sur le poste : voir `docs/lecons.md`.
"""
import json
import sys
import tempfile
from pathlib import Path

from opentunebox.gui.guide import load_cards, save_cards

CARDS = [
    [],
    [{"name": "A"}],
    [{"name": "JeNo Ced", "bf": "4.5", "size": "five", "pid_khz": 3.2, "esc": "BLHeli_32", "done": {"recording": True}}],
    # entier lu, entier réécrit (Python écrit ce qu'il a lu)
    [{"name": "A", "pid_khz": 4}, {"name": "B", "pid_khz": 8, "done": {}}, {"name": "C", "pid_khz": 4.0}],
    # accents, guillemets, antislash, contrôles, caractères hors du plan de base
    [{"name": "Éé \"q\" \\ \t \u001f \u007f   \U0001F681", "bf": "2025.12", "size": "whoop", "pid_khz": 1.6}],
    # ordre des cases, cases décochées gardées telles qu'elles sont dans le fichier
    [{"name": "A", "done": {"b": True, "a": False, "c": True}}],
    # clés inconnues ignorées, clés dans un autre ordre
    [{"extra": 1, "esc": "AM32", "name": "A", "done": {"x": True}, "autre": [1, 2]}],
    # taille inconnue -> five
    [{"name": "A", "size": "inconnue"}, {"name": "B", "size": ""}],
    # deux fiches de même nom : le fichier les garde toutes les deux
    [{"name": "A", "bf": "4.4"}, {"name": "A", "bf": "4.5"}],
]


def main(out: str) -> None:
    cases = []
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "drones.json"
        for cards in CARDS:
            for indent in (None, 1, 4):
                text = json.dumps(cards, ensure_ascii=False, indent=indent)
                path.write_text(text, encoding="utf-8")
                loaded = load_cards(path)
                save_cards(path, loaded)
                cases.append({"input": text, "output": path.read_text(encoding="utf-8"), "n": len(loaded)})
    # fichiers que Python ne lit pas (il rend une liste vide) : le Rust doit le dire
    unreadable = ["", "pas du json", "{}", "[1]", '[{"bf": "4.4"}]', "[null]", '{"name": "A"}']
    lost = []
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "drones.json"
        for text in unreadable:
            path.write_text(text, encoding="utf-8")
            assert load_cards(path) == [], text
            lost.append(text)
    Path(out).write_text(json.dumps({"roundtrip": cases, "unreadable": lost}, ensure_ascii=False), encoding="utf-8")
    print(f"{len(cases)} aller-retour, {len(lost)} fichiers illisibles -> {out}")


if __name__ == "__main__":
    main(sys.argv[1])
