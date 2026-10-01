"""Lancement de l'interface graphique."""

from __future__ import annotations

import sys

import pyqtgraph as pg
from PySide6.QtWidgets import QApplication


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    app = QApplication(argv)
    app.setApplicationName("OpenTuneBox")
    app.setOrganizationName("OpenTuneBox")
    pg.setConfigOptions(useOpenGL=False, antialias=False)

    from .main_window import MainWindow

    win = MainWindow()
    win.show()
    files = [a for a in argv[1:] if not a.startswith("-")]
    if files:
        win.load_paths(files)
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
