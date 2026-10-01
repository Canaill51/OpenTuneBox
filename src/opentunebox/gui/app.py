"""Lancement de l'interface graphique."""

from __future__ import annotations

import os
import sys

import pyqtgraph as pg
from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QApplication


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    app = QApplication(argv)
    app.setApplicationName("OpenTuneBox")
    app.setOrganizationName("OpenTuneBox")
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
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(code)


if __name__ == "__main__":
    sys.exit(main())
