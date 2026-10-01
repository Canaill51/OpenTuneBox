"""Section repliable : un titre cliquable qui montre ou cache son contenu."""

from __future__ import annotations

from PySide6.QtCore import QSettings, Qt
from PySide6.QtWidgets import QSizePolicy, QToolButton, QVBoxLayout, QWidget


class Section(QWidget):
    """Titre ▸/▾ + contenu. L'état ouvert/fermé est mémorisé sous la clé `key`."""

    def __init__(self, title: str, key: str, expanded: bool = True, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._settings = QSettings("OpenTuneBox", "OpenTuneBox")
        self._key = f"sections/{key}"
        expanded = str(self._settings.value(self._key, expanded)).lower() in ("true", "1")

        self.header = QToolButton()
        self.header.setObjectName("sectionHeader")
        self.header.setText(title)
        self.header.setCheckable(True)
        self.header.setChecked(expanded)
        self.header.setToolButtonStyle(Qt.ToolButtonTextBesideIcon)
        self.header.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.header.toggled.connect(self._toggle)

        self.body = QWidget()
        self.content_layout = QVBoxLayout(self.body)
        self.content_layout.setContentsMargins(4, 2, 0, 6)

        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(2)
        lay.addWidget(self.header)
        lay.addWidget(self.body)
        self._toggle(expanded, save=False)

    def _toggle(self, on: bool, save: bool = True) -> None:
        self.header.setArrowType(Qt.DownArrow if on else Qt.RightArrow)
        self.body.setVisible(on)
        if save:
            self._settings.setValue(self._key, on)
