"""Ouverture d'un log depuis le Finder de macOS (QFileOpenEvent), simulée hors écran."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtGui import QFileOpenEvent  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from opentunebox.gui.app import FileOpenFilter  # noqa: E402


def test_file_open_event_loads_log():
    app = QApplication.instance() or QApplication([])
    opened = []
    file_open = FileOpenFilter(opened.extend)
    app.installEventFilter(file_open)
    try:
        QApplication.sendEvent(app, QFileOpenEvent("/logs/vol.bbl"))
    finally:
        app.removeEventFilter(file_open)
    assert opened == ["/logs/vol.bbl"]
