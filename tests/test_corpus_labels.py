"""Tests du tableau d'étiquettes du corpus (scripts/corpus_labels.py), sans vrais logs."""

import csv
import importlib.util
from pathlib import Path

import numpy as np

from opentunebox.log.settings import parse_settings

_spec = importlib.util.spec_from_file_location("corpus_labels",
                                               Path(__file__).parent.parent / "scripts" / "corpus_labels.py")
cl = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(cl)


def test_drone_id_matches_calibration():
    assert cl.drone_id(" JeNo Ced ", "Cedric/JeNo 5/FINAL.bbl") == "JeNo Ced"
    assert cl.drone_id("", "Cedric/JeNo 5/FINAL.bbl") == "?Cedric/JeNo 5"
    # nom porté par plusieurs machines : nom + dossier
    assert cl.drone_id("Jax", "Guigui/HORUS/a.bbl") != cl.drone_id("Jax", "Guigui/Whisper25/b.bbl")


def test_shared_name_candidates():
    rows = {"A": {"dossiers": "Cedric/Air65 | Cedric/Air65/16012025"},  # sous-dossiers : même drone
            "B": {"dossiers": "Guigui | Guigui/HORUS | Guigui/Whisper25"},
            "?Leo": {"dossiers": "Leo"}}
    assert [x.split(" : ")[0] for x in cl.shared_name_candidates(rows)] == ["B"]


def test_guess_state():
    assert cl.guess_state({"1. PTB"}, ["tune final.bbl"]) == "aide"  # demande d'aide avant tout
    assert cl.guess_state({"3. BBLR FB"}, ["x.bbl"]) == "aide"
    assert cl.guess_state({"Cedric"}, ["FINAL.bbl"]) == "regle"
    assert cl.guess_state({"Cedric"}, ["LAST.bbl", "Stock.bbl"]) == "regle"
    assert cl.guess_state({"Cedric"}, ["Stock_Air65.bbl"]) == "origine"
    assert cl.guess_state({"Cedric"}, ["blast.bbl", "LOG00001.BFL"]) == "inconnu"


def test_guess_size():
    assert cl.guess_size(["Apex 3.5 inch"]) == "3,5"
    assert cl.guess_size(['Source One 5"']) == "5"
    assert cl.guess_size(["mon 7in"]) == "7"
    assert cl.guess_size(["F1_Air65_2026.6RC2.bbl"]) == "whoop"
    assert cl.guess_size(["LOG00065.BFL", "JeNo 5"]) == ""


def test_cell_count_firmware_formula():
    s = parse_settings({"vbatref": "2520", "vbatcellvoltage": "330,350,430"})
    assert cl.cell_count(s, None) == 6
    s = parse_settings({"vbatref": "420", "vbatcellvoltage": "330,350,440"})
    assert cl.cell_count(s, None) == 1
    # sans vbatref : première tension loggée, tension max par cellule par défaut (4,30 V)
    assert cl.cell_count(parse_settings({}), np.array([16.6, 15.0])) == 4
    assert cl.cell_count(parse_settings({}), None) is None


def test_merge_keeps_user_columns(tmp_path):
    path = tmp_path / "corpus.csv"
    auto = {c: "" for c in cl.COLUMNS}
    old = {"A": {**auto, "drone": "A", "sessions": "3", "etat": "origine", "taille_reelle": "3,5",
                 "remarque": "note"},
           "B": {**auto, "drone": "B", "sessions": "1", "etat": "inconnu"}}
    # un tableur peut réenregistrer le fichier avec des virgules (valeurs « 3,5 » entre guillemets)
    with open(path, "w", encoding="utf-8", newline="") as fh:
        w = csv.DictWriter(fh, cl.COLUMNS)
        w.writeheader()
        w.writerows(old.values())
    assert cl.read_labels(path)["A"]["taille_reelle"] == "3,5"
    assert cl.read_labels(tmp_path / "absent.csv") == {}
    cl.write_labels(path, cl.merge({}, old))
    new = {"A": {**auto, "drone": "A", "sessions": 5, "etat": "aide", "taille_reelle": "5"},
           "C": {**auto, "drone": "C", "sessions": 2, "etat": "regle"}}
    rows = {r["drone"]: r for r in cl.merge(cl.read_labels(path), new)}
    assert rows["A"]["sessions"] == 5  # colonne automatique mise à jour
    assert (rows["A"]["etat"], rows["A"]["taille_reelle"], rows["A"]["remarque"]) == ("origine", "3,5", "note")
    assert rows["B"]["etat"] == "inconnu"  # drone absent du passage : ligne gardée
    full = {r["drone"]: r for r in cl.merge(cl.read_labels(path), new, full=True)}
    assert full["B"]["sessions"] == 0 and full["B"]["etat"] == "inconnu"
    assert rows["C"]["etat"] == "regle"  # nouveau drone : pré-rempli
