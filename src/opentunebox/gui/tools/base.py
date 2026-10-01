"""Base commune des fenêtres d'outils : sélection des fichiers (×10), axes, boutons standard.

Chaque outil hérite de `ToolWindow` et implémente :
- `build_controls(layout)` : ses réglages propres dans le panneau de droite ;
- `compute(entries)` : calcul (appelé en arrière-plan) à partir des logs sélectionnés ;
- `render(result)` : affichage du résultat (dans le thread de l'interface).
"""

from __future__ import annotations

from typing import Any

import pyqtgraph as pg
from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QAbstractItemView,
    QCheckBox,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from .. import theme
from ..geometry import restore_geometry, save_geometry
from ..state import AppState, LoadedLog
from ..worker import run_async


class FileSelector(QListWidget):
    """Liste des logs ouverts, sélection multiple (couleurs des fichiers)."""

    def __init__(self, state: AppState, multi: bool = True) -> None:
        super().__init__()
        self.state = state
        self.setSelectionMode(QAbstractItemView.ExtendedSelection if multi else QAbstractItemView.SingleSelection)
        self.setMinimumHeight(120)
        state.logs_changed.connect(self.refresh)
        self.refresh()

    def refresh(self) -> None:
        keep = {it.text() for it in self.selectedItems()}
        self.blockSignals(True)
        self.clear()
        for ll in self.state.logs:
            it = QListWidgetItem(ll.label)
            it.setForeground(pg.mkColor(ll.color))
            self.addItem(it)
            if ll.label in keep:
                it.setSelected(True)
        if not self.selectedItems() and self.count():
            cur = self.state.current if 0 <= self.state.current < self.count() else 0
            self.item(cur).setSelected(True)
        self.blockSignals(False)

    def selected_entries(self) -> list[LoadedLog]:
        rows = sorted(self.row(it) for it in self.selectedItems())
        return [self.state.logs[r] for r in rows if r < len(self.state.logs)]


class ToolWindow(QWidget):
    title = "Outil"
    multi_file = True
    auto_run = True
    _last_result: Any = None
    run_options: dict = {}

    def __init__(self, state: AppState, main_window=None) -> None:
        super().__init__(None, Qt.Window)
        self.state = state
        self.main_window = main_window
        self.setWindowTitle(f"OpenTuneBox — {self.title}")
        restore_geometry(self, f"tool/{type(self).__name__}", 0.9)
        self._busy = False
        self._pending = False

        split = QSplitter(Qt.Horizontal, self)
        self.canvas = pg.GraphicsLayoutWidget()
        self.canvas.setBackground(theme.current().bg)
        split.addWidget(self.canvas)
        panel = QWidget()
        self.panel_layout = QVBoxLayout(panel)
        box = QGroupBox("Fichiers (max 10)")
        bl = QVBoxLayout(box)
        self.files = FileSelector(state, self.multi_file)
        bl.addWidget(self.files)
        self.panel_layout.addWidget(box)

        row = QHBoxLayout()
        self.run_btn = QPushButton("Calculer")
        self.run_btn.setStyleSheet(theme.accent_style("green"))
        self.run_btn.clicked.connect(self.run)
        save = QPushButton("Exporter image")
        save.clicked.connect(self._save)
        row.addWidget(self.run_btn)
        row.addWidget(save)
        self.panel_layout.addLayout(row)

        self.axis_boxes = []
        axes = QHBoxLayout()
        for name in theme.AXIS_NAMES:
            cb = QCheckBox(name[0])
            cb.setChecked(True)
            cb.toggled.connect(self.on_options_changed)
            axes.addWidget(cb)
            self.axis_boxes.append(cb)
        self.panel_layout.addLayout(axes)

        self.build_controls(self.panel_layout)
        self.status = QLabel("")
        self.status.setWordWrap(True)
        self.panel_layout.addWidget(self.status)
        self.panel_layout.addStretch(1)
        panel.setFixedWidth(280)
        split.addWidget(panel)
        split.setStretchFactor(0, 1)
        lay = QVBoxLayout(self)
        lay.setContentsMargins(4, 4, 4, 4)
        lay.addWidget(split)

        state.period_changed.connect(lambda _i: self.auto_run and self.isVisible() and self.run())

    # ------------------------------------------------------------ à surcharger
    def build_controls(self, layout: QVBoxLayout) -> None:
        pass

    def snapshot_options(self) -> dict:
        """Lu dans le thread de l'interface juste avant le calcul : copier ici les valeurs des
        widgets dont `compute` a besoin (accessibles ensuite via `self.run_options`)."""
        return {}

    def compute(self, entries: list[LoadedLog]) -> Any:
        raise NotImplementedError

    def render(self, result: Any) -> None:
        raise NotImplementedError

    # ------------------------------------------------------------ mécanique commune
    def axes(self) -> list[int]:
        return [i for i, cb in enumerate(self.axis_boxes) if cb.isChecked()]

    def on_options_changed(self, *_args) -> None:
        """Options d'affichage modifiées : on réaffiche sans recalculer si possible."""
        if self._last_result is not None:
            self.render(self._last_result)

    def closeEvent(self, e) -> None:  # noqa: N802
        save_geometry(self, f"tool/{type(self).__name__}")
        super().closeEvent(e)

    def showEvent(self, e) -> None:  # noqa: N802
        super().showEvent(e)
        if self.auto_run and self._last_result is None:
            self.run()

    def run(self) -> None:
        entries = self.files.selected_entries()
        if not entries:
            self.status.setText("Sélectionnez au moins un log.")
            return
        if self._busy:
            self._pending = True  # recalcul dès la fin du calcul en cours
            return
        self.run_options = self.snapshot_options()
        self._busy = True
        self.run_btn.setEnabled(False)
        self.status.setText("Calcul en cours…")
        run_async(self.compute, entries, on_done=self._done, on_error=self._failed)

    def _done(self, result: Any) -> None:
        self._busy = False
        self.run_btn.setEnabled(True)
        self.status.setText("")
        self._last_result = result
        self.render(result)
        if self._pending:
            self._pending = False
            self.run()

    def _failed(self, err: str) -> None:
        self._busy = False
        self.run_btn.setEnabled(True)
        self._pending = False
        self.status.setText("Erreur de calcul :\n" + err[-600:])

    def _save(self) -> None:
        from ..main_window import MainWindow

        MainWindow.save_figure(self)
