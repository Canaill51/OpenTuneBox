"""Tests de l'assistant de réglage et des fenêtres associées (sur logs réels)."""

import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from corpus_paths import LOGS  # noqa: E402
pytestmark = [pytest.mark.corpus, pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")]


def _load(rel):
    from opentunebox.log.flightlog import load

    return load(LOGS / rel)[0]


def test_profiles_detected_by_motor_speed():
    from opentunebox.advisor.profiles import detect_profile

    assert detect_profile(_load("Cedric/Air65/stock.bbl"))[0].key == "whoop"
    assert detect_profile(_load("Cedric/JeNo 5/FINAL.bbl"))[0].key == "five"
    # Meteor 75 (≈ 355 Hz en vol) : whoop depuis la limite à 345 Hz
    assert detect_profile(_load("Cedric/Meteor 75 Pro O4 HD/03112025/D.bbl"))[0].key == "whoop"


def test_profile_thresholds():
    from dataclasses import replace

    from opentunebox.advisor.profiles import PROFILES, profile_key_from_hz
    from opentunebox.advisor.rules import analyze

    assert [profile_key_from_hz(h) for h in (480, 350, 300, 220, 140, 100)] == \
        ["whoop", "whoop", "micro", "five", "seven", "large"]
    lg = _load("Cedric/JeNo 5/FINAL.bbl")
    titles = lambda rep: {f.title for f in rep.findings}  # noqa: E731
    # un seuil mécanique nul déclenche le constat : la règle lit bien le profil
    strict = replace(PROFILES["five"], imbalance_high_pct=0.0)
    assert "Moteurs déséquilibrés" in titles(analyze(lg, profile=strict))
    assert "Seuils provisoires pour cette taille" in titles(analyze(lg, profile=PROFILES["micro"]))
    assert "Seuils provisoires pour cette taille" not in titles(analyze(lg, profile=PROFILES["five"]))


def test_overshoot_detected_on_low_damping():
    from opentunebox.advisor.rules import ATTENTION, analyze
    from opentunebox.analysis.period import auto_period

    lg = _load("1. PTB/BlackSummer/Damping 0.8.BFL")
    rep = analyze(lg, lg.slice_time(*auto_period(lg)))
    titles = [f.title for f in rep.findings if f.severity == ATTENTION]
    assert any("dépassement" in t and "roll" in t for t in titles)
    # chaque constat explique sa mesure
    assert all(f.detail for f in rep.findings)


def test_imbalance_interpretation():
    from opentunebox.advisor.rules import _imbalance_finding

    front = _imbalance_finding([20, 30, 20, 30], 20)
    assert "avant" in front.detail
    diag = _imbalance_finding([30, 20, 20, 30], 20)
    assert "diagonale" in diag.detail


def test_tool_windows_render():
    from PySide6.QtWidgets import QApplication

    from opentunebox.gui.state import AppState
    from opentunebox.gui.tools.advisor_panel import AdvisorWindow
    from opentunebox.gui.tools.flight_stats import FlightStatsWindow
    from opentunebox.gui.tools.setup_info import SetupInfoWindow

    app = QApplication.instance() or QApplication([])
    st = AppState()
    st.add(_load("1. PTB/BlackSummer/Damping 0.8.BFL"))
    st.add(_load("1. PTB/BlackSummer/Damping 1.4.BFL"))
    wins = []
    for cls in (FlightStatsWindow, AdvisorWindow):
        w = cls(st)
        w.files.selectAll() if w.multi_file else None
        w.render(w.compute(w.files.selected_entries()))
        wins.append(w)
    s = SetupInfoWindow(st)
    s.files.selectAll()
    s.only_diff.setChecked(True)
    assert s.tree.topLevelItemCount() >= 2  # PID et curseurs diffèrent
    wins.append(s)
    for w in wins:
        w.close()
        w.deleteLater()
    app.processEvents()
