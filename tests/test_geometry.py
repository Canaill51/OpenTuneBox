"""Taille des fenêtres : grande au premier lancement, puis mémorisée."""

from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QWidget

from opentunebox.gui.geometry import fit_to_screen, restore_geometry, save_geometry

KEY = "test/geometry"


def _forget():
    QSettings("OpenTuneBox", "OpenTuneBox").remove(f"geometry/{KEY}")


def test_fit_to_screen_uses_most_of_the_screen(qtbot):
    w = QWidget()
    qtbot.addWidget(w)
    fit_to_screen(w, 0.95)
    avail = w.screen().availableGeometry()
    assert abs(w.width() - avail.width() * 0.95) <= 1
    assert abs(w.height() - avail.height() * 0.95) <= 1


def test_geometry_is_remembered(qtbot):
    _forget()
    try:
        a = QWidget()
        qtbot.addWidget(a)
        restore_geometry(a, KEY, 0.5)  # rien d'enregistré : moitié de l'écran
        a.resize(640, 480)
        a.show()
        save_geometry(a, KEY)
        b = QWidget()
        qtbot.addWidget(b)
        restore_geometry(b, KEY, 0.95)
        assert (b.width(), b.height()) == (640, 480)
    finally:
        _forget()
