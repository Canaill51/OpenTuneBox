"""Fréquence × temps (équivalent du « Freq x Time » de PIDtoolbox).

Un spectrogramme par axe (Roll, Pitch, Yaw) sur la période analysée, avec en surimpression les
harmoniques moteur (eRPM) et les centres des notchs dynamiques. Une ligne verticale suit le
curseur du lecteur du visualiseur ; un clic sur un spectrogramme déplace ce curseur.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import QRectF, Qt
from PySide6.QtWidgets import QCheckBox, QGridLayout, QHBoxLayout, QLabel, QVBoxLayout

from ...analysis import datasets
from ...analysis.motors import dyn_notch_centres
from ...analysis.spectral import Spectrogram, freq_time, to_ptb_db
from .. import theme
from ..state import LoadedLog
from . import _spectral_common as sc
from .base import ToolWindow

# lissage en fréquence (largeur en points de spectre → niveau de smooth_freq : largeur = 1 + 2 × niveau)
FREQ_SMOOTHING = [(0, "aucun"), (2, "5 points"), (5, "10 points"), (10, "20 points")]
# pas entre deux fenêtres de 300 ms
TIME_STEPS = [(1, "300 ms"), (2, "150 ms"), (5, "60 ms"), (10, "30 ms")]
HARMONIC_CHOICES = [((), "Harmoniques : aucune"), ((1,), "1re harmonique"), ((2,), "2e harmonique"),
                    ((3,), "3e harmonique"), ((1, 2, 3), "1re, 2e, 3e")]
MAX_TRACE_POINTS = 3000


@dataclass
class FreqTimeResult:
    entry: LoadedLog
    key: str
    maps: dict[int, Spectrogram]
    motor_t: np.ndarray | None = None  # temps des traces moteur (réduites)
    motor_hz: list[np.ndarray] = field(default_factory=list)  # fréquence de chaque moteur
    notches: list[tuple[np.ndarray, np.ndarray]] = field(default_factory=list)  # (t, Hz) par notch
    nyquist: float = 0.0


class FreqTimeWindow(sc.OptionsSnapshot, ToolWindow):
    title = "Fréquence × temps"
    multi_file = False

    # ------------------------------------------------------------ panneau de réglages
    def build_controls(self, layout: QVBoxLayout) -> None:
        self.plots: list[pg.PlotItem] = []
        self.cursor_lines: list[pg.InfiniteLine] = []
        self._result: FreqTimeResult | None = None

        g = QGridLayout()
        g.addWidget(QLabel("Données"), 0, 0)
        self.dataset = sc.combo(list(datasets.DATASETS.items()), list(datasets.DATASETS).index("gyro_prefilt"),
                                self.run)
        g.addWidget(self.dataset, 0, 1)
        g.addWidget(QLabel("Lissage fréq."), 1, 0)
        self.freq_smooth = sc.combo(FREQ_SMOOTHING, 1, self.run)
        g.addWidget(self.freq_smooth, 1, 1)
        g.addWidget(QLabel("Pas temporel"), 2, 0)
        self.time_step = sc.combo(TIME_STEPS, 1, self.run)
        g.addWidget(self.time_step, 2, 1)
        g.addWidget(QLabel("Couleurs"), 3, 0)
        self.cmap = sc.combo([(n, n) for n in sc.COLORMAP_NAMES], 0, self.on_options_changed)
        g.addWidget(self.cmap, 3, 1)
        g.addWidget(QLabel("Z min (dB)"), 4, 0)
        g.addWidget(QLabel("Z max (dB)"), 4, 1)
        self.zmin = sc.number(-30, -200, 200, 5, 0, self.on_options_changed)
        self.zmax = sc.number(10, -200, 200, 5, 0, self.on_options_changed)
        g.addWidget(self.zmin, 5, 0)
        g.addWidget(self.zmax, 5, 1)
        self.sub100 = QCheckBox("< 100 Hz")
        self.sub100.toggled.connect(self.on_options_changed)
        g.addWidget(self.sub100, 6, 0)
        self.notch_box = QCheckBox("Notchs dyn.")
        self.notch_box.toggled.connect(self.on_options_changed)
        g.addWidget(self.notch_box, 6, 1)
        self.harmonics = sc.combo(HARMONIC_CHOICES, 1, self.on_options_changed)
        g.addWidget(self.harmonics, 7, 0, 1, 2)
        layout.addLayout(g)
        self.motors = sc.MotorToggles(self.on_options_changed)
        layout.addWidget(self.motors)
        row = QHBoxLayout()
        row.addWidget(QLabel("RPM estimé ×"))
        self.multiplier = sc.number(2.1, 0.1, 20, 0.1, 2, self.run)
        self.multiplier.setToolTip("Utilisé seulement si le log n'a pas l'eRPM (télémétrie DShot).")
        row.addWidget(self.multiplier)
        layout.addLayout(row)
        hint = QLabel("Clic sur un spectrogramme : place le curseur du lecteur.")
        hint.setWordWrap(True)
        hint.setStyleSheet(theme.muted_style())
        layout.addWidget(hint)

        self.state.cursor_changed.connect(self._on_cursor_changed)
        self.canvas.scene().sigMouseClicked.connect(self._on_click)

    def collect_options(self) -> dict:
        return {
            "key": self.dataset.currentData(),
            "freq_smoothing": int(self.freq_smooth.currentData()),
            "hop_div": int(self.time_step.currentData()),
            "multiplier": float(self.multiplier.value()),
        }

    # ------------------------------------------------------------ calcul (arrière-plan)
    def compute(self, entries: list[LoadedLog]) -> FreqTimeResult:
        opts = self.take_options()
        ll = entries[0]
        log, sel = ll.log, ll.sel
        key = opts["key"]
        res = FreqTimeResult(ll, key, {}, nyquist=log.fs / 2)
        data = datasets.get(log, key)
        if data is not None:
            for ax in range(3):
                if datasets.has_axis(log, key, ax):
                    m = freq_time(data[ax, sel], log.t[sel], log.fs, 300.0, opts["hop_div"], opts["freq_smoothing"])
                    m.z = to_ptb_db(m.z)
                    res.maps[ax] = m
        t = log.t[sel]
        mhz, _est = sc.motor_freqs(log, opts["multiplier"])
        if mhz is not None:
            for m in mhz[:, sel]:
                tt, hz = sc.decimate(t, m, MAX_TRACE_POINTS)
                res.motor_hz.append(hz)
            res.motor_t = tt
        notches = dyn_notch_centres(log)
        if notches is not None:
            for c in notches[:, sel]:
                tt, hz = sc.decimate(t, np.where(c > 0, c, np.nan), MAX_TRACE_POINTS)
                res.notches.append((tt, hz))
        return res

    # ------------------------------------------------------------ affichage
    def render(self, res: FreqTimeResult) -> None:
        self.canvas.clear()
        self.plots = []
        self.cursor_lines = []
        self._result = res
        cmap = sc.colormap(self.cmap.currentData())
        levels = (self.zmin.value(), max(self.zmax.value(), self.zmin.value() + 1e-6))
        f_top = 100.0 if self.sub100.isChecked() else res.nyquist
        title = f"{datasets.DATASETS[res.key]} | {res.entry.label}"
        self.canvas.addLabel(f'<span style="color:{res.entry.color}; font-weight:600">{title}</span>',
                             row=0, col=0, colspan=2).setMaximumHeight(22)
        t0, t1 = res.entry.period
        for r, ax in enumerate(self.axes()):
            sg = res.maps.get(ax)
            if sg is None or sg.times.size == 0:
                continue
            p = sc.add_plot(self.canvas, 1 + r, 0)
            p.vb.setBackgroundColor(sc.MAP_BACKGROUND)
            sc.style_plot(p, f"{theme.AXIS_NAMES[ax]} | fréq. (Hz)", "Temps (s)")
            img = self._draw_map(p, sg, cmap, levels)
            self._draw_overlays(p, res)
            p.setXRange(t0, t1, padding=0)
            p.setYRange(0, f_top, padding=0)
            if self.plots:
                p.setXLink(self.plots[0])
            bar = pg.ColorBarItem(values=levels, colorMap=cmap, interactive=False, width=14,
                                  label="Densité spectrale (dB)")
            bar.setImageItem(img)
            self.canvas.addItem(bar, row=1 + r, col=1)
            line = pg.InfiniteLine(angle=90, movable=True,
                                   pen=pg.mkPen(theme.PLOT_COLORS["overlay"], width=1.5, style=Qt.DashLine),
                                   hoverPen=pg.mkPen(theme.PLOT_COLORS["overlay_hover"], width=2))
            line.setZValue(30)
            line.sigPositionChanged.connect(self._on_line_dragged)
            p.addItem(line, ignoreBounds=True)
            self.plots.append(p)
            self.cursor_lines.append(line)
        lay = self.canvas.ci.layout
        lay.setRowStretchFactor(0, 0)
        for r in range(1, 4):
            lay.setRowStretchFactor(r, 1)
        lay.setColumnStretchFactor(0, 1)
        lay.setColumnStretchFactor(1, 0)
        sc.restore_canvas(self.canvas)
        self._on_cursor_changed(self.state.cursor)

    def _draw_map(self, p: pg.PlotItem, sg: Spectrogram, cmap: pg.ColorMap,
                  levels: tuple[float, float]) -> pg.ImageItem:
        f, t = sg.freqs, sg.times
        df = f[1] - f[0] if f.size > 1 else 1.0
        dt = float(np.median(np.diff(t))) if t.size > 1 else 0.3
        img = pg.ImageItem(sg.z, axisOrder="row-major")
        img.setColorMap(cmap)
        img.setLevels(levels)
        img.setRect(QRectF(t[0] - dt / 2, f[0] - df / 2, dt * t.size, df * f.size))
        p.addItem(img)
        return img

    def _draw_overlays(self, p: pg.PlotItem, res: FreqTimeResult) -> None:
        enabled = set(self.motors.enabled())
        if res.motor_t is not None:
            for h in self.harmonics.currentData():
                for m, hz in enumerate(res.motor_hz):
                    if m % 4 not in enabled:
                        continue
                    pen = pg.mkPen(theme.motor_color(m), width=1.5, style=sc.HARM_STYLES.get(h, Qt.DotLine))
                    p.plot(res.motor_t, hz * h, pen=pen, connect="finite")
        if self.notch_box.isChecked():
            for t, hz in res.notches:
                p.plot(t, hz, pen=pg.mkPen(theme.NOTCH_TRACE, width=1), connect="finite")

    # ------------------------------------------------------------ curseur du lecteur
    def _on_cursor_changed(self, tc: float) -> None:
        for line in self.cursor_lines:
            line.blockSignals(True)
            line.setValue(tc)
            line.blockSignals(False)

    def _on_line_dragged(self, line: pg.InfiniteLine) -> None:
        self.state.set_cursor(float(line.value()))

    def _on_click(self, ev) -> None:
        if ev.button() != Qt.LeftButton:
            return
        pos = ev.scenePos()
        for p in self.plots:
            if p.vb.sceneBoundingRect().contains(pos):
                self.state.set_cursor(float(p.vb.mapSceneToView(pos).x()))
                return
