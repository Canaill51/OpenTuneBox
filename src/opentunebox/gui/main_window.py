"""Fenêtre principale : visualiseur de log (équivalent du « Log Viewer » de PIDtoolbox)."""

from __future__ import annotations

import datetime as _dt
from pathlib import Path

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import QSettings, Qt, QTimer
from PySide6.QtGui import QAction, QFontDatabase, QKeySequence
from PySide6.QtWidgets import (
    QApplication,
    QButtonGroup,
    QCheckBox,
    QComboBox,
    QFileDialog,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QRadioButton,
    QSizePolicy,
    QSpinBox,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from .. import __version__
from ..log.flightlog import load
from ..log.modes import mode_changes
from . import icons, theme
from .geometry import restore_geometry, save_geometry
from .state import MAX_LOGS, AppState
from .widgets.axis import major_axes
from .widgets.section import Section
from .widgets.sticks import StickWidget
from .worker import run_async

# (clé, libellé, source) — source : attribut de FlightLog ou fonction
TRACES = [
    ("gyro_prefilt", "Gyro (préfiltre)"),
    ("gyro", "Gyro"),
    ("pterm", "Terme P"),
    ("iterm", "Terme I"),
    ("dterm_prefilt", "Terme D (préfiltre)"),
    ("dterm", "Terme D"),
    ("fterm", "Terme F"),
    ("setpoint", "Consigne"),
    ("pid_sum", "Somme PID"),
    ("pid_error", "Erreur PID"),
    ("debug", "Debug"),
]
TRACE_GROUPS = [
    ("Gyro", ["gyro", "gyro_prefilt"]),
    ("Consigne et PID", ["setpoint", "pterm", "iterm", "dterm", "dterm_prefilt", "fterm", "pid_sum", "pid_error"]),
    ("Autres", ["debug"]),
]
DEFAULT_TRACES = {"gyro", "setpoint"}
SMOOTHING = {"aucun": 1, "léger": 5, "moyen": 11, "fort": 25}

# Barre d'outils : (clé, libellé court, icône Lucide, info-bulle)
TOOLS = [
    ("spectral", "Spectre", "audio-waveform", "Analyse spectrale"),
    ("freq_throttle", "Fréq. × gaz", "gauge", "Spectre en fonction des gaz"),
    ("freq_time", "Fréq. × temps", "timer", "Spectrogramme : spectre au cours du vol"),
    ("step", "Indicielle", "chart-line", "Réponse indicielle (réactivité, dépassement)"),
    ("sliders", "Curseurs PID", "sliders-horizontal", "Curseurs PID du configurateur Betaflight"),
    ("filter_sim", "Filtres", "funnel", "Simulateur de filtres"),
    ("setup", "Configuration", "cpu", "Infos de configuration (en-tête du log)"),
    ("stats", "Statistiques", "chart-column", "Statistiques de vol"),
    ("advisor", "Assistant", "lightbulb", "Assistant de réglage"),
]


def _trace_data(log, key: str) -> np.ndarray | None:
    return {
        "gyro_prefilt": lambda: log.gyro_unfilt,
        "gyro": lambda: log.gyro,
        "pterm": lambda: log.p_term,
        "iterm": lambda: log.i_term,
        "dterm_prefilt": lambda: log.d_term_prefilter,
        "dterm": lambda: log.d_term,
        "fterm": lambda: log.f_term,
        "setpoint": lambda: log.setpoint,
        "pid_sum": lambda: log.pid_sum,
        "pid_error": lambda: log.pid_error,
        "debug": lambda: log.debug[:3] if log.debug.shape[0] >= 1 else None,
    }[key]()



def _has_trace(log, key: str) -> bool:
    data = _trace_data(log, key)
    return data is not None and data.size > 0 and bool(np.any(data))


def _smooth(y: np.ndarray, w: int) -> np.ndarray:
    if w <= 1:
        return y
    return np.convolve(y, np.ones(w) / w, mode="same")


class MainWindow(QMainWindow):
    def __init__(self, state: AppState | None = None) -> None:
        super().__init__()
        self.state = state or AppState()
        self.settings = QSettings("OpenTuneBox", "OpenTuneBox")
        theme.apply(self.settings.value("theme", "clair"))
        self.setWindowTitle(f"OpenTuneBox {__version__} — visualiseur de log")
        restore_geometry(self, "main", 0.95)
        self.setAcceptDrops(True)
        self.tool_windows: dict[str, QWidget] = {}
        self._loading = 0
        self._play_timer = QTimer(self)
        self._play_timer.setInterval(40)
        self._play_timer.timeout.connect(self._play_step)

        self._build_ui()
        self.state.logs_changed.connect(self._on_logs_changed)
        self.state.current_changed.connect(lambda _i: self._redraw())
        self.state.period_changed.connect(self._on_period_changed)
        self.state.cursor_changed.connect(self._on_cursor_changed)
        self._redraw()

    # ================================================================ construction
    def _build_ui(self) -> None:
        self._build_toolbar()
        split = QSplitter(Qt.Horizontal)
        split.addWidget(self._build_selection_panel())
        self.plots_widget = pg.GraphicsLayoutWidget()
        split.addWidget(self.plots_widget)
        split.addWidget(self._build_control_panel())
        split.setStretchFactor(1, 1)
        split.setSizes([200, 1110, 290])
        self.setCentralWidget(split)
        self._apply_icons()
        self._build_plots()
        self.statusBar().showMessage("Ouvrez un log Blackbox (.bbl, .bfl) — ou glissez-le dans la fenêtre.")

    def _build_toolbar(self) -> None:
        tb = self.addToolBar("Outils")
        tb.setObjectName("mainToolbar")
        tb.setMovable(False)
        tb.setToolButtonStyle(Qt.ToolButtonTextBesideIcon)
        self._icon_actions: list[tuple[QAction, str, bool]] = []  # (action, icône, bouton principal)

        def add(text: str, icon: str, slot, tip: str = "", primary: bool = False, icon_only: bool = False) -> QAction:
            act = QAction(text, self)
            act.setToolTip(tip or text)
            act.triggered.connect(slot)
            tb.addAction(act)
            if primary:
                tb.widgetForAction(act).setObjectName("primary")
            if icon_only:
                tb.widgetForAction(act).setToolButtonStyle(Qt.ToolButtonIconOnly)
            self._icon_actions.append((act, icon, primary))
            return act

        act_open = add("Ouvrir…", "folder-open", self.open_files, "Ouvrir des logs Blackbox (Ctrl+O)", primary=True)
        act_open.setShortcut(QKeySequence.Open)
        add("Fermer", "x", lambda: self.state.remove(self.state.current), "Fermer le log affiché", icon_only=True)
        add("Tout fermer", "trash-2", self.state.clear, "Fermer tous les logs", icon_only=True)
        tb.addSeparator()
        for key, text, icon, tip in TOOLS:
            add(text, icon, lambda _c=False, k=key: self.open_tool(k), tip)
        spacer = QWidget()
        spacer.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
        tb.addWidget(spacer)
        add("Exporter image", "image-down", lambda: self.save_figure(self), "Exporter la fenêtre en image PNG", icon_only=True)

    def _apply_icons(self) -> None:
        for act, name, primary in self._icon_actions:
            act.setIcon(icons.icon(name, "#ffffff" if primary else theme.current().ui["text"]))
        for b, name in self._player_buttons:
            if b is self.play_btn and self._play_timer.isActive():
                name = "pause"
            b.setIcon(icons.icon(name, theme.current().ui["text"], 16))

    def _build_selection_panel(self) -> QWidget:
        panel = QWidget()
        lay = QVBoxLayout(panel)
        lay.setContentsMargins(10, 8, 6, 8)
        lay.setSpacing(3)
        row = QHBoxLayout()
        row.addWidget(QLabel("Échelle Y (°/s)"))
        self.yscale = QSpinBox()
        self.yscale.setRange(10, 3000)
        self.yscale.setSingleStep(50)
        self.yscale.setValue(int(self.settings.value("viewer/yscale", 500)))
        self.yscale.valueChanged.connect(self._apply_yrange)
        row.addWidget(self.yscale)
        lay.addLayout(row)

        self.trace_boxes: dict[str, QCheckBox] = {}
        saved = self.settings.value("viewer/traces", None)
        enabled = set(saved.split(",")) if isinstance(saved, str) and saved else DEFAULT_TRACES
        labels = dict(TRACES)
        self._group_headings: list[tuple[QLabel, list[str]]] = []
        for heading, keys in TRACE_GROUPS:
            h = QLabel(heading)
            h.setObjectName("heading")
            lay.addWidget(h)
            self._group_headings.append((h, keys))
            for key in keys:
                cb = QCheckBox(labels[key])
                cb.setIcon(theme.swatch_icon(theme.trace_color(key)))
                cb.setChecked(key in enabled)
                cb.toggled.connect(self._on_traces_changed)
                lay.addWidget(cb)
                self.trace_boxes[key] = cb
        self.debug_label = QLabel("")
        self.debug_label.setWordWrap(True)
        self.debug_label.setStyleSheet(theme.muted_style())
        lay.addWidget(self.debug_label)

        h = QLabel("Moteurs")
        h.setObjectName("heading")
        lay.addWidget(h)
        self.motor_box = QCheckBox("Moteurs (%)")
        self.motor_box.setChecked(True)
        self.rps_box = QCheckBox("Rotation moteurs (Hz)")
        self.thr_box = QCheckBox("Gaz (%)")
        self.thr_box.setChecked(True)
        for cb in (self.motor_box, self.rps_box, self.thr_box):
            cb.toggled.connect(self._on_traces_changed)
            lay.addWidget(cb)
        lay.addStretch(1)
        panel.setMinimumWidth(180)
        return panel

    def _build_control_panel(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(6, 8, 10, 8)
        self.file_combo = QComboBox()
        self.file_combo.setPlaceholderText("Aucun log ouvert")
        self.file_combo.currentIndexChanged.connect(self._on_combo)
        lay.addWidget(self.file_combo)
        self.info_label = QLabel("")
        self.info_label.setWordWrap(True)
        self.info_label.setStyleSheet(theme.muted_style())
        lay.addWidget(self.info_label)

        axes = QHBoxLayout()
        self.focus_group = QButtonGroup(self)
        for i, name in enumerate(("Tous", "Roll", "Pitch", "Yaw")):
            rb = QRadioButton(name)
            rb.setChecked(i == 0)
            self.focus_group.addButton(rb, i)
            axes.addWidget(rb)
        self.focus_group.idToggled.connect(lambda _i, on: on and self._apply_focus())
        lay.addLayout(axes)

        row = QHBoxLayout()
        b_period = QPushButton("Période auto")
        b_period.setToolTip("Recalcule la période d'analyse : du décollage à l'atterrissage, avant un éventuel crash.\n"
                            "Vous pouvez aussi déplacer les bords de la zone claire sur le graphique des gaz.")
        b_period.clicked.connect(lambda: self.state.reset_period(self.state.current))
        b_full = QPushButton("Vue entière")
        b_full.clicked.connect(self._view_all)
        row.addWidget(b_period)
        row.addWidget(b_full)
        lay.addLayout(row)

        display = Section("Affichage", "viewer_display", expanded=False)
        gl = QGridLayout()
        display.content_layout.addLayout(gl)
        gl.addWidget(QLabel("Lissage"), 0, 0)
        self.smooth_combo = QComboBox()
        self.smooth_combo.addItems(list(SMOOTHING))
        self.smooth_combo.currentIndexChanged.connect(self._redraw)
        gl.addWidget(self.smooth_combo, 0, 1)
        gl.addWidget(QLabel("Épaisseur"), 1, 0)
        self.width_spin = QSpinBox()
        self.width_spin.setRange(1, 5)
        self.width_spin.setValue(int(self.settings.value("viewer/linewidth", 2)))
        self.width_spin.valueChanged.connect(self._redraw)
        gl.addWidget(self.width_spin, 1, 1)
        gl.addWidget(QLabel("Thème"), 2, 0)
        self.theme_combo = QComboBox()
        self.theme_combo.addItems(list(theme.THEMES))
        self.theme_combo.setCurrentText(theme.current().name)
        self.theme_combo.currentTextChanged.connect(self._change_theme)
        gl.addWidget(self.theme_combo, 2, 1)
        gl.setColumnStretch(1, 1)
        lay.addWidget(display)

        player = Section("Lecteur", "viewer_player", expanded=True)
        pl = player.content_layout
        row = QHBoxLayout()
        self._player_buttons: list[tuple[QPushButton, str]] = []
        for name, tip, fn in (("skip-back", "Début de la période", lambda: self._seek_rel(None, -1)),
                              ("chevron-left", "Reculer de 0,1 s", lambda: self._seek_rel(-0.1)),
                              ("play", "Lecture / pause", self._toggle_play),
                              ("chevron-right", "Avancer de 0,1 s", lambda: self._seek_rel(0.1)),
                              ("skip-forward", "Fin de la période", lambda: self._seek_rel(None, 1))):
            b = QPushButton()
            b.setToolTip(tip)
            b.setFixedWidth(40)
            b.clicked.connect(fn)
            row.addWidget(b)
            self._player_buttons.append((b, name))
            if name == "play":
                self.play_btn = b
        pl.addLayout(row)
        self.time_label = QLabel("temps : —")
        self.time_label.setAlignment(Qt.AlignCenter)
        pl.addWidget(self.time_label)
        sticks = QHBoxLayout()
        self.stick_left = StickWidget("Y", "G")
        self.stick_right = StickWidget("R", "P")
        sticks.addWidget(self.stick_left)
        sticks.addWidget(self.stick_right)
        pl.addLayout(sticks)
        self.readout = QLabel("")
        mono = QFontDatabase.systemFont(QFontDatabase.FixedFont)
        mono.setFamilies(["Cascadia Mono", "Consolas", "Menlo", "DejaVu Sans Mono", mono.family()])
        mono.setPointSizeF(QApplication.font().pointSizeF() * 0.95)
        self.readout.setFont(mono)
        pl.addWidget(self.readout)
        lay.addWidget(player)
        lay.addStretch(1)
        w.setMinimumWidth(270)
        return w

    def _build_plots(self) -> None:
        pw = self.plots_widget
        pw.clear()
        self.axis_plots: list[pg.PlotItem] = []
        for i, name in enumerate(theme.AXIS_NAMES):
            p = pw.addPlot(row=i, col=0, axisItems=major_axes())
            p.setLabel("left", f"{name} (°/s)")
            p.showGrid(x=True, y=True, alpha=theme.current().grid)
            p.setDownsampling(auto=True, mode="peak")
            p.setClipToView(True)
            p.setMenuEnabled(False)
            if i > 0:
                p.setXLink(self.axis_plots[0])
            self.axis_plots.append(p)
        self.motor_plot = pw.addPlot(row=3, col=0, axisItems=major_axes())
        self.motor_plot.setLabel("left", "Gaz | moteurs (%)")
        self.motor_plot.setLabel("bottom", "Temps (s)")
        self.motor_plot.showGrid(x=True, y=True, alpha=theme.current().grid)
        self.motor_plot.setDownsampling(auto=True, mode="peak")
        self.motor_plot.setClipToView(True)
        self.motor_plot.setXLink(self.axis_plots[0])
        self.motor_plot.setYRange(0, 100, padding=0.02)
        self.motor_plot.setMenuEnabled(False)
        # axe de droite pour la rotation des moteurs (Hz)
        self.rps_view = pg.ViewBox()
        self.motor_plot.showAxis("right")
        self.motor_plot.scene().addItem(self.rps_view)
        self.motor_plot.getAxis("right").linkToView(self.rps_view)
        self.motor_plot.getAxis("right").setLabel("Hz")
        self.rps_view.setXLink(self.motor_plot)
        self.motor_plot.vb.sigResized.connect(lambda: self.rps_view.setGeometry(self.motor_plot.vb.sceneBoundingRect()))
        # même largeur d'axes partout pour que les échelles de temps soient alignées
        for p in [*self.axis_plots, self.motor_plot]:
            p.showAxis("right")
            p.getAxis("left").setWidth(62)
            p.getAxis("right").setWidth(48)
            if p is not self.motor_plot:
                p.getAxis("right").setStyle(showValues=False)
        pw.ci.layout.setRowStretchFactor(3, 1)
        for i in range(3):
            pw.ci.layout.setRowStretchFactor(i, 2)

        # période d'analyse : zone claire déplaçable + zones grisées hors période
        self.region = pg.LinearRegionItem(brush=(80, 160, 255, 30))
        self.region.setZValue(-10)
        self.region.sigRegionChangeFinished.connect(self._on_region_moved)
        self.motor_plot.addItem(self.region)
        self.shades: list[tuple[pg.LinearRegionItem, pg.LinearRegionItem]] = []
        for p in [*self.axis_plots, self.motor_plot]:
            a = pg.LinearRegionItem(movable=False, brush=theme.current().shade, pen=pg.mkPen(None))
            b = pg.LinearRegionItem(movable=False, brush=theme.current().shade, pen=pg.mkPen(None))
            a.setZValue(-20)
            b.setZValue(-20)
            p.addItem(a)
            p.addItem(b)
            self.shades.append((a, b))
        # curseur (lecteur)
        self.cursors: list[pg.InfiniteLine] = []
        for p in [*self.axis_plots, self.motor_plot]:
            c = pg.InfiniteLine(angle=90, movable=True, pen=pg.mkPen(theme.current().fg, style=Qt.DashLine))
            c.sigPositionChanged.connect(self._on_cursor_dragged)
            p.addItem(c)
            self.cursors.append(c)
        self._curves: list = []
        self._mode_labels: list = []

    # ================================================================ fichiers
    def open_files(self) -> None:
        start = str(self.settings.value("last_dir", str(Path.home())))
        files, _ = QFileDialog.getOpenFileNames(self, "Ouvrir des logs Blackbox", start,
                                                "Logs Blackbox (*.bbl *.bfl *.BBL *.BFL *.txt *.TXT);;Tous (*)")
        self.load_paths(files)

    def load_paths(self, paths: list[str]) -> None:
        for p in paths:
            self.settings.setValue("last_dir", str(Path(p).parent))
            self._loading += 1
            self.statusBar().showMessage(f"Lecture de {Path(p).name}…")
            run_async(load, p, on_done=lambda logs, p=p: self._loaded(p, logs),
                      on_error=lambda err, p=p: self._load_failed(p, err))

    def _loaded(self, path: str, logs: list) -> None:
        self._loading -= 1
        if not logs:
            self.statusBar().showMessage(f"{Path(path).name} : aucune session exploitable.")
            return
        added = 0
        for lg in logs:
            if self.state.add(lg):
                added += 1
            else:
                QMessageBox.information(self, "Limite atteinte", f"{MAX_LOGS} logs au maximum peuvent être ouverts.")
                break
        self.statusBar().showMessage(f"{Path(path).name} : {added} session(s) chargée(s).")

    def _load_failed(self, path: str, err: str) -> None:
        self._loading -= 1
        QMessageBox.warning(self, "Lecture impossible", f"{Path(path).name} n'a pas pu être lu.\n\n{err[-800:]}")

    def dragEnterEvent(self, e) -> None:  # noqa: N802
        if e.mimeData().hasUrls():
            e.acceptProposedAction()

    def dropEvent(self, e) -> None:  # noqa: N802
        self.load_paths([u.toLocalFile() for u in e.mimeData().urls() if u.isLocalFile()])

    # ================================================================ état → affichage
    def _on_logs_changed(self) -> None:
        self.file_combo.blockSignals(True)
        self.file_combo.clear()
        for ll in self.state.logs:
            self.file_combo.addItem(ll.label)
        self.file_combo.setCurrentIndex(self.state.current)
        self.file_combo.blockSignals(False)

    def _on_combo(self, i: int) -> None:
        if i != self.state.current:
            self.state.set_current(i)

    def _on_traces_changed(self) -> None:
        self.settings.setValue("viewer/traces", ",".join(k for k, cb in self.trace_boxes.items() if cb.isChecked()))
        self._redraw()

    def _redraw(self) -> None:
        for p in [*self.axis_plots, self.motor_plot]:
            for c in [it for it in p.listDataItems()]:
                p.removeItem(c)
        self.rps_view.clear()
        for t in self._mode_labels:
            self.motor_plot.removeItem(t)
        self._mode_labels = []
        ll = self.state.current_log()
        if self.file_combo.currentIndex() != self.state.current:
            self.file_combo.blockSignals(True)
            self.file_combo.setCurrentIndex(self.state.current)
            self.file_combo.blockSignals(False)
        if ll is None:
            self.info_label.setText("")
            self.debug_label.setText("")
            self._set_period_items_visible(False)
            for cb in self.trace_boxes.values():
                cb.setVisible(True)
            for h, _keys in self._group_headings:
                h.setVisible(True)
            return
        self._set_period_items_visible(True)
        log = ll.log
        s = log.settings
        # traces absentes de ce log (non enregistrées, ou nulles) : masquées
        for key, cb in self.trace_boxes.items():
            cb.setVisible(_has_trace(log, key))
        for h, keys in self._group_headings:
            h.setVisible(any(_has_trace(log, k) for k in keys))
        self.rps_box.setVisible(log.motor_hz is not None)
        self.info_label.setText(f"{s.firmware}\n{s.craft_name or ''}  {log.fs:.0f} Hz · {log.duration:.1f} s")
        self.debug_label.setText(f"Debug : {s.debug_mode}")
        w = self.width_spin.value()
        sm = SMOOTHING[self.smooth_combo.currentText()]
        t = log.t
        for key, cb in self.trace_boxes.items():
            if not cb.isChecked():
                continue
            data = _trace_data(log, key)
            if data is None:
                continue
            pen = pg.mkPen(theme.trace_color(key), width=w)
            for ax in range(min(3, data.shape[0])):
                y = data[ax]
                if key in ("dterm", "dterm_prefilt") and not np.any(y):
                    continue
                self.axis_plots[ax].plot(t, _smooth(y, sm), pen=pen)
        if self.motor_box.isChecked():
            for m in range(log.n_motors):
                self.motor_plot.plot(t, _smooth(log.motors[m], sm), pen=pg.mkPen(theme.motor_color(m), width=w))
        if self.thr_box.isChecked():
            self.motor_plot.plot(t, log.throttle, pen=pg.mkPen(theme.trace_color("throttle"), width=w + 1))
        if self.rps_box.isChecked() and log.motor_hz is not None:
            for m in range(log.motor_hz.shape[0]):
                c = pg.PlotCurveItem(t, _smooth(log.motor_hz[m], sm), pen=pg.mkPen(theme.motor_color(m), width=w,
                                                                                    style=Qt.DotLine))
                self.rps_view.addItem(c)
            self.rps_view.setYRange(0, float(np.nanmax(log.motor_hz)) * 1.05)
        for tm, label in mode_changes(log.mode_flags, t, s.version):
            ti = pg.TextItem(label, anchor=(0, 1), color=theme.current().fg)
            ti.setPos(tm, 0)
            self.motor_plot.addItem(ti)
            self._mode_labels.append(ti)
        self._apply_yrange()
        self._apply_focus()
        self._update_period_items()
        self.axis_plots[0].setXRange(float(t[0]), float(t[-1]), padding=0.01)
        self._on_cursor_changed(self.state.cursor if t[0] <= self.state.cursor <= t[-1] else float(t[0]))

    def _apply_yrange(self) -> None:
        v = self.yscale.value()
        self.settings.setValue("viewer/yscale", v)
        for p in self.axis_plots:
            p.setYRange(-v, v, padding=0)

    def _apply_focus(self) -> None:
        """Affiche les trois axes, ou un seul axe sur toute la hauteur (au-dessus des moteurs)."""
        f = self.focus_group.checkedId()
        ci = self.plots_widget.ci
        for p in [*self.axis_plots, self.motor_plot]:
            if p in ci.items:
                ci.removeItem(p)
        shown = [p for i, p in enumerate(self.axis_plots) if f == 0 or f == i + 1]
        for i, p in enumerate(self.axis_plots):
            p.setVisible(p in shown)
        for r, p in enumerate(shown):
            ci.addItem(p, row=r, col=0)
            ci.layout.setRowStretchFactor(r, 2 if f == 0 else 5)
        ci.addItem(self.motor_plot, row=len(shown), col=0)
        ci.layout.setRowStretchFactor(len(shown), 1 if f == 0 else 2)
        for r in range(len(shown) + 1, 4):
            ci.layout.setRowStretchFactor(r, 0)

    def _view_all(self) -> None:
        ll = self.state.current_log()
        if ll:
            self.axis_plots[0].setXRange(float(ll.log.t[0]), float(ll.log.t[-1]), padding=0.01)

    def _change_theme(self, name: str) -> None:
        self.settings.setValue("theme", name)
        theme.apply(name)
        self.plots_widget.setBackground(theme.current().bg)
        self._build_plots()
        for key, cb in self.trace_boxes.items():
            cb.setIcon(theme.swatch_icon(theme.trace_color(key)))
        for lbl in (self.info_label, self.debug_label):
            lbl.setStyleSheet(theme.muted_style())
        self._apply_icons()
        self._redraw()

    # ================================================================ période
    def _set_period_items_visible(self, on: bool) -> None:
        """Zone de période, zones grisées et curseur : seulement quand un log est affiché."""
        self.region.setVisible(on)
        for a, b in self.shades:
            a.setVisible(on)
            b.setVisible(on)
        for c in self.cursors:
            c.setVisible(on)

    def _update_period_items(self) -> None:
        ll = self.state.current_log()
        if ll is None:
            return
        t0, t1 = ll.period
        lo, hi = float(ll.log.t[0]), float(ll.log.t[-1])
        self.region.blockSignals(True)
        self.region.setRegion((t0, t1))
        self.region.blockSignals(False)
        for a, b in self.shades:
            a.setRegion((lo, t0))
            b.setRegion((t1, hi))

    def _on_region_moved(self) -> None:
        t0, t1 = self.region.getRegion()
        self.state.set_period(self.state.current, t0, t1)
        self._update_period_items()

    def _on_period_changed(self, index: int) -> None:
        if index == self.state.current:
            self._update_period_items()

    # ================================================================ curseur / lecteur
    def _on_cursor_dragged(self, line: pg.InfiniteLine) -> None:
        self.state.set_cursor(float(line.value()))

    def _on_cursor_changed(self, tc: float) -> None:
        for c in self.cursors:
            if abs(c.value() - tc) > 1e-9:
                c.blockSignals(True)
                c.setValue(tc)
                c.blockSignals(False)
        ll = self.state.current_log()
        if ll is None:
            return
        log = ll.log
        i = log.index_of(tc)
        self.time_label.setText(f"temps : {log.t[i]:.3f} s")
        rc = log.rc_command
        self.stick_left.set_position(rc[2, i] / 500.0, (rc[3, i] - 1500.0) / 500.0)
        self.stick_right.set_position(rc[0, i] / 500.0, rc[1, i] / 500.0)
        lines = []
        for k in range(0, min(8, log.n_motors), 2):
            lines.append("   ".join(f"M{j + 1} : {log.motors[j, i]:3.0f} %" for j in range(k, min(k + 2, log.n_motors))))
        for a, n in enumerate(theme.AXIS_NAMES):
            lines.append(f"Gyro {n:<5s} : {log.gyro[a, i]:6.0f} °/s")
        lines.append(f"Gaz : {log.throttle[i]:.0f} %")
        if log.vbat is not None:
            lines.append(f"Batterie : {log.vbat[i]:.2f} V")
        self.readout.setText("\n".join(lines))

    def _seek_rel(self, dt: float | None, end: int = 0) -> None:
        ll = self.state.current_log()
        if ll is None:
            return
        if dt is None:
            t = ll.period[0] if end < 0 else ll.period[1]
        else:
            t = self.state.cursor + dt
        self.state.set_cursor(float(np.clip(t, ll.log.t[0], ll.log.t[-1])))

    def _toggle_play(self) -> None:
        if self._play_timer.isActive():
            self._play_timer.stop()
        else:
            self._play_timer.start()
        self._apply_icons()

    def _play_step(self) -> None:
        ll = self.state.current_log()
        if ll is None or self.state.cursor >= ll.log.t[-1]:
            self._toggle_play()
            return
        self._seek_rel(self._play_timer.interval() / 1000.0)

    # ================================================================ outils
    def open_tool(self, key: str) -> None:
        if not self.state.logs:
            QMessageBox.information(self, "Aucun log", "Ouvrez d'abord un log Blackbox.")
            return
        win = self.tool_windows.get(key)
        if win is None:
            from .tools import create_tool  # import différé : démarrage plus rapide

            win = create_tool(key, self.state, self)
            if win is None:
                QMessageBox.information(self, "Bientôt", "Cet outil arrive dans une prochaine étape.")
                return
            self.tool_windows[key] = win
        win.show()
        win.raise_()
        win.activateWindow()

    @staticmethod
    def save_figure(widget: QWidget, base_dir: str | None = None) -> str | None:
        """Exporte une capture du widget en PNG (nom OTB_FIGS_AAAA-MM-JJ-N.png, comme PTB)."""
        settings = QSettings("OpenTuneBox", "OpenTuneBox")
        folder = Path(base_dir or settings.value("last_dir", str(Path.home())))
        day = _dt.date.today().isoformat()
        out = folder / f"OTB_FIGS_{day}"
        n = 1
        while (out.with_name(f"{out.name}-{n}.png")).exists():
            n += 1
        default = str(out.with_name(f"{out.name}-{n}.png"))
        path, _ = QFileDialog.getSaveFileName(widget, "Exporter l'image", default, "Image PNG (*.png)")
        if not path:
            return None
        widget.grab().save(path, "PNG")
        return path

    def closeEvent(self, e) -> None:  # noqa: N802
        self.settings.setValue("viewer/linewidth", self.width_spin.value())
        save_geometry(self, "main")
        for w in self.tool_windows.values():
            w.close()
        super().closeEvent(e)
