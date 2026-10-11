"""Guide intégré : affiché au démarrage, navigation, liens, vérification des logs (Qt hors écran)."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402
from PySide6.QtCore import QSettings, QUrl  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from corpus_paths import LOGS  # noqa: E402

FINAL = LOGS / "Cedric/JeNo 5/FINAL.bbl"


@pytest.fixture(scope="module")
def app():
    return QApplication.instance() or QApplication([])


def test_guide_shown_at_startup_and_links(app):
    from opentunebox.gui.guide import STEPS
    from opentunebox.gui.main_window import MainWindow

    QSettings("OpenTuneBox", "OpenTuneBox").setValue("guide/step", "bienvenue")
    win = MainWindow()
    g = win.guide_page
    assert win.current_page() == "guide" and win._tool_actions["guide"].isChecked()
    assert "Bienvenue" in g.text.toPlainText()
    for row, (key, _title, _f) in enumerate(STEPS):  # chaque étape s'affiche
        g.steps.setCurrentRow(row)
        assert g.current_key() == key and g.text.toPlainText().strip()
    assert not g.next_btn.isEnabled()
    g._on_link(QUrl("otb:step/filtres"))
    assert g.current_key() == "filtres"
    g._on_link(QUrl("otb:step/verifier"))
    assert "Aucun log ouvert" in g.text.toPlainText() and not g.copy_btn.isVisible()
    win._change_theme("sombre")  # le guide se réaffiche avec les couleurs du thème
    win._change_theme("clair")
    win.close()
    win.deleteLater()
    app.processEvents()


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_guide_checks_open_logs(app):
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.log.flightlog import load

    win = MainWindow()
    win.show()
    win.guide_page.show_step("verifier")
    win.state.add(load(FINAL)[0])
    app.processEvents()
    assert win.current_page() == "guide"  # l'étape « Vérifier mes logs » reste affichée et se remplit
    text = win.guide_page.text.toPlainText()
    assert "Vol d'une série PID" in text and "Feedforward" in text
    assert win.guide_page.copy_btn.isVisible() and "set simplified_feedforward_gain = 0" in win.guide_page._cli
    win.guide_page._copy_cli()
    assert QApplication.clipboard().text().endswith("save")
    win.guide_page._on_link(QUrl("otb:tool/step"))
    assert win.current_page() == "step"
    win.close()
    win.deleteLater()
    app.processEvents()


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_first_log_switches_to_viewer(app):
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.log.flightlog import load

    win = MainWindow()
    win.guide_page.show_step("bienvenue")
    win.state.add(load(FINAL)[0])
    assert win.current_page() == "viewer"
    win.close()
    win.deleteLater()
    app.processEvents()
