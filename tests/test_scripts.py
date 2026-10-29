"""Scripts du corpus sans vrais logs : passages parallèles (`corpus_labels.map_files`) et tableaux récapitulatifs
(`advisor_rates.summary`, `calibrate_advisor.summary`) sur de petits fichiers JSON fabriqués."""

import importlib
import json
import os
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).parent.parent / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))  # calibrate_advisor et advisor_rates importent corpus_labels par son nom


def _load(name):
    """Module d'un script, enregistré sous son nom : les processus du passage parallèle le retrouvent par ce nom
    (`_guarded` est transmis par référence au module `corpus_labels`)."""
    return importlib.import_module(name)


cl = _load("corpus_labels")


def _size(path):
    return os.path.getsize(path)


def _rayon_threads(path):
    return os.environ.get("RAYON_NUM_THREADS")


def _size_or_fail(path):
    if path.endswith("bad.bbl"):
        raise ValueError("fichier abîmé")
    return os.path.getsize(path)


@pytest.fixture
def files(tmp_path):
    out = []
    for name, size in (("a.bbl", 10), ("b.bbl", 300), ("c.bbl", 20), ("bad.bbl", 5)):
        p = tmp_path / name
        p.write_bytes(b"x" * size)
        out.append(str(p))
    return out


@pytest.mark.parametrize("jobs", [1, 2])
def test_map_files_keeps_input_order_and_retries_failures(files, jobs, capsys):
    """Résultats dans l'ordre des fichiers (les gros partent en premier en parallèle) ; un fichier en erreur est
    refait seul puis, s'il échoue encore, signalé ERR et ignoré."""
    res = cl.map_files(_size, files, jobs=jobs)
    assert res == [(f, os.path.getsize(f)) for f in files]
    res = cl.map_files(_size_or_fail, files, jobs=jobs, root=os.path.dirname(files[0]))
    assert [f for f, _ in res] == files[:3] and dict(res)[files[1]] == 300
    err = capsys.readouterr().err
    assert "ERR bad.bbl" in err and "ValueError" in err
    assert cl.map_files(_size, [], jobs=jobs) == [] and cl.map_files(_size, files[:1], jobs=jobs) == [(files[0], 10)]


def test_map_files_gives_each_process_one_rust_thread_and_leaves_the_parent_alone(files, monkeypatch):
    """Un processus par cœur : sans `RAYON_NUM_THREADS=1` dans chacun, cœurs² fils (L-038). La variable est posée
    dans les processus de calcul, pas dans le parent (un forkserver déjà lancé ne la recevrait pas). Limite : une
    variable posée APRÈS le démarrage du forkserver n'atteint pas les processus (non testable dans ce processus)."""
    monkeypatch.delenv("RAYON_NUM_THREADS", raising=False)
    res = cl.map_files(_rayon_threads, files[:3], jobs=2)
    assert [v for _, v in res] == ["1", "1", "1"]
    assert "RAYON_NUM_THREADS" not in os.environ


def test_default_jobs_is_cpu_count():
    assert cl.default_jobs() == (os.cpu_count() or 1)


def test_advisor_rates_summary(tmp_path, capsys):
    ar = _load("advisor_rates")
    rows = [
        {"file": "a.bbl", "sess": 1, "profile": "five", "moving": True,
         "findings": [("gyro_noisy", "Gyro filtré bruité en roll", "attention", True),
                      ("hover_line_residual", "Raie à 330 Hz", "conseil", True),
                      ("hover_line_residual", "Raie à 410 Hz", "conseil", True),
                      ("hover_noise_summary", "Vol stationnaire : bruit identifié", "info", False)]},
        {"file": "b.bbl", "sess": 1, "profile": "whoop", "moving": False,
         "findings": [("hover_line_residual", "Raie à 250 Hz", "conseil", True), ("step_ok", "Dépassement en roll", "ok", False)]},
    ]
    path = tmp_path / "rates.json"
    path.write_text(json.dumps(rows), encoding="utf-8")
    ar.summary(str(path))
    out = capsys.readouterr().out
    assert out.startswith("2 sessions : tous 2, mouvements 1, stationnaires 1")
    lines = {ln.split("  ")[0].strip(): ln for ln in out.splitlines()[2:]}
    line = lines["hover_line_residual (Raie à 250 Hz)"]  # regroupée par clé, comptée une fois par session
    assert "100%" in line
    assert "0%" in lines["gyro_noisy (Gyro filtré bruité en roll)"]
    assert not any(t.startswith(("hover_noise_summary", "step_ok")) for t in lines)  # info et ok ignorés


def test_calibrate_advisor_summary(tmp_path, capsys):
    ca = _load("calibrate_advisor")
    row = {"file": "Cedric/JeNo 5/a.bbl", "sess": 1, "craft": "JeNo", "hover_hz": 200.0, "cells": 6, "sp95": 300.0,
           "gnoise_0": -30.0, "gnoise_1": -31.0, "peak_0": 1.05, "peak_1": 1.12, "imean_0": -20.0, "imean_1": 5.0,
           "mnoise_0": [-40.0, -35.0], "hprom_0": [20.0, 5.0, 1.0], "hleft_0": [2.0, 1.0, 0.5],
           "res_db": 9.0, "res_near": False, "gdelay_0": 1.4, "ddelay_0": 2.2,
           "measured_0": True, "fc_0": 15.0, "smaxedge_0": False, "measured_1": False}
    hover = {**row, "file": "Guigui/b.bbl", "craft": "Jax", "hover_hz": 400.0, "cells": 1, "sp95": 10.0,
             "gdelay_0": 9.9, "res_near": True}
    path = tmp_path / "calib.json"
    path.write_text(json.dumps([row, hover]), encoding="utf-8")
    ca.summary(str(path))
    out = capsys.readouterr().out
    assert "Boucle ouverte : 2/4 axes mesurés (50 %), croisement trouvé 100 %, pic de sensibilité au bord 0 %" in out
    assert "== five : 1 sessions, 1 drones" in out and "== whoop : 1 sessions, 1 drones" in out  # reclassés par eRPM
    assert "== TOUS : 2 sessions, 2 drones" in out
    tous = out[out.index("== TOUS"):]
    assert "peakdiff" in tous and "imean_abs" in tous and "prom1" in tous and "res_free_db" in tous  # mesures dérivées
    assert [ln for ln in tous.splitlines() if ln.startswith("gdelay")][0].split("n=")[1].strip().startswith("1 ")  # stationnaire exclu


def test_advisor_rates_compare(tmp_path, capsys):
    ar = _load("advisor_rates")
    base = {"file": "a.bbl", "sess": 1, "profile": "five", "moving": True, "t_analyze": 2.0,
            "findings": [("rpm_q_raise", "Filtre RPM : Q peut monter", "conseil", True)],
            "details": [("rpm_q_raise", None, "conseil", "Filtre RPM : Q peut monter", "1e -0 dB", ["rpm_filter_q 600"])]}
    same = {**base, "file": "b.bbl", "t_analyze": 1.0}
    old, new = tmp_path / "avant.json", tmp_path / "apres.json"
    old.write_text(json.dumps([base, same]), encoding="utf-8")
    changed = {**base, "t_analyze": 1.0, "details": [(*base["details"][0][:4], "1e -1 dB", ["rpm_filter_q 600"])]}
    new.write_text(json.dumps([changed, same]), encoding="utf-8")
    assert ar.compare(str(old), str(new)) == 1  # seul le chiffre du détail change : la session est signalée
    out = capsys.readouterr().out
    assert "sessions aux constats différents : 1" in out and "== a.bbl (session 1)" in out
    assert "1e -0 dB" in out and "1e -1 dB" in out
    assert "3 s → 2 s" in out
    assert ar.compare(str(old), str(old)) == 0
