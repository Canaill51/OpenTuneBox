"""Lancement de l'interface graphique."""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pyqtgraph as pg
from PySide6.QtCore import QSettings
from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    app = QApplication(argv)
    app.setApplicationName("OpenTuneBox")
    app.setOrganizationName("OpenTuneBox")
    # Icône des fenêtres + lien avec le raccourci opentunebox.desktop (indispensable sous Wayland
    # pour que la barre des tâches affiche la bonne icône).
    app.setWindowIcon(QIcon(str(Path(__file__).parent / "assets" / "opentunebox.svg")))
    app.setDesktopFileName("opentunebox")
    pg.setConfigOptions(useOpenGL=False, antialias=False)

    from .main_window import MainWindow
    from .worker import wait_all

    win = MainWindow()
    win.show()
    files = [a for a in argv[1:] if not a.startswith("-")]
    if files:
        win.load_paths(files)
    code = app.exec()
    hard_exit(code, wait_all)
    return code


def hard_exit(code: int = 0, before=None) -> None:
    """Quitte sans le nettoyage automatique de fin de PySide6.

    Avec PySide6 6.11 + pyqtgraph, la destruction automatique des objets graphiques à la sortie de
    Python peut provoquer une erreur de segmentation (objet Python déjà libéré pendant que Qt
    détruit la scène). Les réglages sont enregistrés juste avant ; il n'y a rien d'autre à sauver.
    """
    if before is not None:
        before()
    QSettings("OpenTuneBox", "OpenTuneBox").sync()
    # Exécutable Windows sans console : sys.stdout et sys.stderr valent None.
    for stream in (sys.stdout, sys.stderr):
        if stream is not None:
            stream.flush()
    os._exit(code)


if __name__ == "__main__":
    sys.exit(main())
