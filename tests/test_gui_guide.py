"""Guide intégré : affiché au démarrage, fiche drone, cases, vérification des logs (Qt hors écran)."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402
from PySide6.QtCore import QSettings, QStandardPaths, QUrl  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from corpus_paths import LOGS  # noqa: E402

FINAL = LOGS / "Cedric/JeNo 5/FINAL.bbl"


@pytest.fixture(scope="module")
def app():
    QStandardPaths.setTestModeEnabled(True)  # fiches et réglages hors du dossier de l'utilisateur
    return QApplication.instance() or QApplication([])


@pytest.fixture
def guide_home():
    QSettings("OpenTuneBox", "OpenTuneBox").setValue("guide/phase", "home")
    QSettings("OpenTuneBox", "OpenTuneBox").setValue("guide/card", "")
    QSettings("OpenTuneBox", "OpenTuneBox").setValue("guide/log_roles", "{}")


def _wait(app, cond, n=600):
    import time

    for _ in range(n):
        app.processEvents()
        if cond():
            return True
        time.sleep(0.02)
    return False


def test_guide_cards_and_steps(app, tmp_path, guide_home):
    from opentunebox.advisor.drone_card import DroneCard
    from opentunebox.gui.guide import GuidePage
    from opentunebox.gui.state import AppState

    g = GuidePage(AppState(), None, tmp_path / "drones.json")
    assert g.current_key() == "home" and "boucle de réglage" in g.home.toPlainText()
    g.show_phase("prep")
    assert not g._widgets  # pas de fiche : on demande d'en créer une
    g.add_card(DroneCard("Test 7", size="seven", pid_khz=3.2))
    assert g.card.name == "Test 7" and len(g._widgets) == 11
    rec = next(w for w in g._widgets if w.step.key == "recording")
    assert "1,6 kHz" in rec.box.text() and "pour les séries PID" in rec.box.text()
    rec.box.setChecked(True)
    assert g.progress()["prep"] == (1, 11)
    again = GuidePage(AppState(), None, tmp_path / "drones.json")  # avancement retrouvé
    assert again.card.done == {"recording": True} and again.current_key() == "prep"
    again.reset_progress(confirm=False)  # bouton « Recommencer »
    assert again.card.done == {} and again.progress()["prep"] == (0, 11)
    assert GuidePage(AppState(), None, tmp_path / "drones.json").card.done == {}
    again.add_card(DroneCard("Autre"))
    again.delete_card(confirm=False)  # bouton « Supprimer » : la fiche choisie disparaît du fichier
    assert [c.name for c in again.cards] == ["Test 7"] and again.card.name == "Test 7"
    assert [c.name for c in GuidePage(AppState(), None, tmp_path / "drones.json").cards] == ["Test 7"]
    g._on_link(QUrl("otb:phase/pid"))
    keys = [w.step.key for w in g._widgets]
    assert g.current_key() == "pid" and keys[2] == "Damping_1.0"  # Damping 7" : liste à partir de 1,0
    assert "pid_logs" in keys and "Ouvrir les logs de la série" in g.logs_view.toPlainText()
    g.refresh()


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_guide_checks_card_logs(app, tmp_path, guide_home):
    from opentunebox.advisor.drone_card import DroneCard
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.log.flightlog import load

    win = MainWindow()
    win.show()
    g = win.guide_page
    g.cards_file = tmp_path / "drones.json"
    g.cards = []
    g.add_card(DroneCard("JeNo Ced", pid_khz=3.2))  # nom du drone dans Betaflight
    g.show_phase("prep")
    win.state.add(load(FINAL)[0])
    assert win.current_page() == "guide"  # depuis une phase, on reste dans le guide
    assert _wait(app, lambda: not g._pending)  # vérification complète en arrière-plan
    status = {w.step.key: w.auto for w in g._widgets}
    # vol avec stationnaire : log des filtres, les cases des séries PID ne le lisent pas
    assert status["recording"] == "ok" and status["log_rates"] is None
    text = g.logs_view.toPlainText()
    assert "Identifier le bruit" in text and "Vol d'une série PID" not in text
    assert g.copy_btn.isVisible() and "set rates_type = ACTUAL" in g._cli
    g._on_link(QUrl(f"otb:role/0/{'pid'}"))  # reclassé à la main : lu comme vol de série
    status = {w.step.key: w.auto for w in g._widgets}
    assert status["recording"] is None and status["log_rates"] == "bad"
    assert "Vol d'une série PID" in g.logs_view.toPlainText()
    g._on_link(QUrl("otb:role/0/filters"))
    g._copy_cli()
    assert QApplication.clipboard().text().endswith("save")
    g.show_phase("filters")
    mech = next(w for w in g._widgets if w.step.key == "mechanics_read")
    g.open_tool(mech.step.tool, mech.step.part)
    assert win.current_page() == "freq_throttle"
    hover = g._check(0)[0].parts["hover"]
    assert win.state.logs[0].period == hover  # période réglée sur la partie stationnaire
    win.close()
    win.deleteLater()
    app.processEvents()


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_first_log_from_home_switches_to_viewer(app, guide_home):
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.log.flightlog import load

    win = MainWindow()
    assert win.current_page() == "guide"
    win.state.add(load(FINAL)[0])
    assert win.current_page() == "viewer"
    win.close()
    win.deleteLater()
    app.processEvents()
