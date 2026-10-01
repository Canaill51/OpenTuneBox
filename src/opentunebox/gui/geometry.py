"""Taille et position des fenêtres : grande au premier lancement, puis mémorisée."""

from __future__ import annotations

from PySide6.QtCore import QByteArray, QSettings
from PySide6.QtWidgets import QApplication, QWidget


def fit_to_screen(w: QWidget, fraction: float) -> None:
    """Fenêtre centrée occupant `fraction` de l'espace utile de l'écran (hors barre des tâches)."""
    screen = w.screen() or QApplication.primaryScreen()
    if screen is None:
        return
    avail = screen.availableGeometry()
    w.resize(int(avail.width() * fraction), int(avail.height() * fraction))
    frame = w.frameGeometry()
    frame.moveCenter(avail.center())
    w.move(frame.topLeft())


def restore_geometry(w: QWidget, key: str, fraction: float) -> None:
    """Reprend la taille et la position enregistrées, sinon `fit_to_screen`."""
    saved = QSettings("OpenTuneBox", "OpenTuneBox").value(f"geometry/{key}")
    if isinstance(saved, (QByteArray, bytes)) and w.restoreGeometry(QByteArray(saved)):
        return
    fit_to_screen(w, fraction)


def save_geometry(w: QWidget, key: str) -> None:
    QSettings("OpenTuneBox", "OpenTuneBox").setValue(f"geometry/{key}", w.saveGeometry())
