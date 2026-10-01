"""Analyse spectrale (équivalent du « Spectral Analyzer » de PIDtoolbox).

Grille de graphiques, une ligne par axe (Roll, Pitch, Yaw) :
- à gauche le spectre complet (0 → Nyquist) ;
- à droite, au choix, le détail sous 100 Hz (avec les retards des filtres) ou le « bruit moteur »
  (niveau aux harmoniques 1, 2, 3 des moteurs, filtré contre préfiltre).

Superpositions comme dans PTB : courbes en « S » montrant où se situent les harmoniques moteur
(et les notchs dynamiques) pendant la période analysée.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QAbstractItemView,
    QCheckBox,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QVBoxLayout,
)

from ...analysis import datasets
from ...analysis.delay import compute_delays
from ...analysis.motors import dyn_notch_centres, motor_noise
from ...analysis.spectral import smooth_freq, spectrum
from .. import theme
from ..state import LoadedLog
from . import _spectral_common as sc
from .base import ToolWindow

RESOLUTIONS = [(0.5, "0,5 Hz"), (1.0, "1 Hz"), (2.0, "2 Hz"), (4.0, "4 Hz")]
HARMONIC_CHOICES = [((), "Harmoniques : aucune"), ((1,), "1re harmonique"), ((1, 2, 3), "1re, 2e, 3e"),
                    ("rpm", "Plage du filtre RPM")]
RIGHT_MODES = [("sub100", "< 100 Hz"), ("motor_noise", "Bruit moteur")]
DEFAULT_DATASETS = ("gyro", "gyro_prefilt")


@dataclass
class FileSpectra:
    entry: LoadedLog
    # (clé du jeu de données, axe) → (fréquences, valeurs non lissées)
    spectra: dict[tuple[str, int], tuple[np.ndarray, np.ndarray]] = field(default_factory=dict)
    # (clé, axe) → (moyennes, écarts-types) aux harmoniques 1, 2, 3
    noise: dict[tuple[str, int], tuple[np.ndarray, np.ndarray]] = field(default_factory=dict)
    gyro_ms: float = float("nan")
    dterm_ms: float = float("nan")
    motor_sorted: list[np.ndarray] = field(default_factory=list)  # fréquences moteur triées, par moteur
    notch_sorted: list[np.ndarray] = field(default_factory=list)  # centres des notchs dynamiques triés
    rpm_text: str = ""
    nyquist: float = 0.0


@dataclass
class SpectralResult:
    mode: str  # "psd" ou "amp"
    keys: list[str]
    files: list[FileSpectra]


def _cdf(sorted_vals: np.ndarray, freqs: np.ndarray) -> np.ndarray:
    if sorted_vals.size == 0:
        return np.zeros_like(freqs)
    return np.searchsorted(sorted_vals, freqs, side="right") / sorted_vals.size


class SpectralWindow(sc.OptionsSnapshot, ToolWindow):
    title = "Analyse spectrale"

    # ------------------------------------------------------------ panneau de réglages
    def build_controls(self, layout: QVBoxLayout) -> None:
        box = QGroupBox("Jeux de données")
        bl = QVBoxLayout(box)
        self.ds_list = QListWidget()
        self.ds_list.setSelectionMode(QAbstractItemView.MultiSelection)
        for key, label in datasets.DATASETS.items():
            it = QListWidgetItem(label)
            it.setData(Qt.UserRole, key)
            self.ds_list.addItem(it)
            it.setSelected(key in DEFAULT_DATASETS)
        self.ds_list.setMaximumHeight(150)
        self.ds_list.itemSelectionChanged.connect(self.run)
        bl.addWidget(self.ds_list)
        layout.addWidget(box)

        row = QHBoxLayout()
        for key, label, color in (("freq_throttle", "Fréq. × gaz", "red"),
                                  ("freq_time", "Fréq. × temps", "blue")):
            b = QPushButton(label)
            b.setStyleSheet(theme.accent_style(color))
            b.clicked.connect(lambda _c=False, k=key: self._open(k))
            row.addWidget(b)
        layout.addLayout(row)

        g = QGridLayout()
        self.psd_box = QCheckBox("PSD (dB)")
        self.psd_box.setChecked(True)
        self.psd_box.toggled.connect(self._on_mode_toggled)
        g.addWidget(self.psd_box, 0, 0)
        self.right_mode = sc.combo([(k, v) for k, v in RIGHT_MODES], 0, self.on_options_changed)
        g.addWidget(self.right_mode, 0, 1)
        g.addWidget(QLabel("Y min"), 1, 0)
        g.addWidget(QLabel("Y max"), 1, 1)
        self.ymin = sc.number(-50, -200, 1e4, 5, 2, self.on_options_changed)
        self.ymax = sc.number(20, -200, 1e4, 5, 2, self.on_options_changed)
        g.addWidget(self.ymin, 2, 0)
        g.addWidget(self.ymax, 2, 1)
        g.addWidget(QLabel("Lissage"), 3, 0)
        g.addWidget(QLabel("Résolution"), 3, 1)
        self.smoothing = sc.combo([(i, str(i)) for i in range(6)], 2, self.on_options_changed)
        self.resolution = sc.combo(RESOLUTIONS, 1, self.run)
        g.addWidget(self.smoothing, 4, 0)
        g.addWidget(self.resolution, 4, 1)
        self.harmonics = sc.combo(HARMONIC_CHOICES, 2, self.on_options_changed)
        g.addWidget(self.harmonics, 5, 0, 1, 2)
        self.notch_box = QCheckBox("Notchs dynamiques")
        self.notch_box.setChecked(True)
        self.notch_box.toggled.connect(self.on_options_changed)
        g.addWidget(self.notch_box, 6, 0, 1, 2)
        layout.addLayout(g)

        self.motors = sc.MotorToggles(self.on_options_changed)
        layout.addWidget(self.motors)
        row = QHBoxLayout()
        row.addWidget(QLabel("RPM estimé ×"))
        self.multiplier = sc.number(2.1, 0.1, 20, 0.1, 2, self.run)
        self.multiplier.setToolTip("Utilisé seulement si le log n'a pas l'eRPM (télémétrie DShot).")
        row.addWidget(self.multiplier)
        layout.addLayout(row)

    def _open(self, key: str) -> None:
        if self.main_window is not None:
            self.main_window.open_tool(key)

    def _on_mode_toggled(self, psd: bool) -> None:
        for s, v in ((self.ymin, -50 if psd else 0), (self.ymax, 20 if psd else 1)):
            s.blockSignals(True)
            s.setValue(v)
            s.blockSignals(False)
        self.run()

    def selected_keys(self) -> list[str]:
        return [self.ds_list.item(i).data(Qt.UserRole) for i in range(self.ds_list.count())
                if self.ds_list.item(i).isSelected()]

    def collect_options(self) -> dict:
        return {
            "keys": self.selected_keys(),
            "mode": "psd" if self.psd_box.isChecked() else "amp",
            "resolution": float(self.resolution.currentData()),
            "multiplier": float(self.multiplier.value()),
        }

    # ------------------------------------------------------------ calcul (arrière-plan)
    def compute(self, entries: list[LoadedLog]) -> SpectralResult:
        opts = self.take_options()
        keys, mode = opts["keys"], opts["mode"]
        files = []
        for ll in entries:
            log, sel = ll.log, ll.sel
            fsp = FileSpectra(ll, nyquist=log.fs / 2, rpm_text=sc.rpm_filter_text(log))
            mhz, _est = sc.motor_freqs(log, opts["multiplier"])
            if mhz is not None:
                mhz = mhz[:, sel]
                fsp.motor_sorted = [np.sort(m[np.isfinite(m) & (m > 0)]) for m in mhz]
            notches = dyn_notch_centres(log)
            if notches is not None:
                fsp.notch_sorted = [np.sort(c[sel][c[sel] > 0]) for c in notches]
            for key in keys:
                for ax in range(3):
                    if not datasets.has_axis(log, key, ax):
                        continue
                    x = datasets.get(log, key)[ax, sel]
                    s = spectrum(x, log.fs, mode, opts["resolution"], 0)
                    fsp.spectra[(key, ax)] = (s.freqs, s.values)
                    if mhz is not None:
                        mn = motor_noise(x, mhz, log.fs)
                        fsp.noise[(key, ax)] = (mn.mean_db, mn.std_db)
            d = compute_delays(log, sel)
            fsp.gyro_ms, fsp.dterm_ms = d.gyro_ms, d.dterm_ms
            files.append(fsp)
        return SpectralResult(mode, keys, files)

    # ------------------------------------------------------------ affichage
    def render(self, res: SpectralResult) -> None:
        self.canvas.clear()
        axes = self.axes()
        if not res.files or not axes:
            return
        psd = res.mode == "psd"
        ymin, ymax = self.ymin.value(), self.ymax.value()
        smooth = int(self.smoothing.currentData())
        right = self.right_mode.currentData()
        nyq = max(f.nyquist for f in res.files)
        unit = "PSD (dB)" if psd else "amplitude"
        for row, ax in enumerate(axes):
            name = theme.AXIS_NAMES[ax]
            pl = sc.add_plot(self.canvas, row, 0)
            sc.style_plot(pl, f"{name} | {unit}", "Fréquence (Hz)")
            pr = sc.add_plot(self.canvas, row, 1)
            if row == 0:
                pl.setTitle("Spectre complet")
                pr.setTitle("Sous 100 Hz" if right == "sub100" else "Bruit moteur")
                legend = pl.addLegend(offset=(-10, 10), labelTextSize="8pt")
                legend.setBrush(pg.mkBrush(255, 255, 255, 190) if theme.current().name == "clair"
                                else pg.mkBrush(30, 31, 34, 190))
            self._draw_spectra(pl, res, ax, smooth, legend=row == 0)
            self._draw_overlays(pl, res, ymin, ymax if not psd else min(0.0, ymax), nyq)
            pl.setXRange(0, nyq, padding=0)
            pl.setYRange(ymin, ymax, padding=0)
            if right == "sub100":
                sc.style_plot(pr, f"{name} | {unit}", "Fréquence (Hz)")
                self._draw_spectra(pr, res, ax, smooth, legend=False)
                pr.setXRange(0, 100, padding=0)
                pr.setYRange(ymin, ymax, padding=0)
                self._delay_text(pr, res, ax)
            else:
                sc.style_plot(pr, f"{name} | bruit moteur moyen (dB)", "Harmonique moteur")
                self._draw_motor_noise(pr, res, ax)
                pr.getAxis("bottom").setTicks([[(1, "1re"), (2, "2e"), (3, "3e")]])
                pr.setXRange(0.7, 3.3, padding=0)
                if psd:
                    pr.setYRange(ymin, ymax, padding=0)
                else:
                    pr.setYRange(-50, 20, padding=0)
                if row == 0:
                    lines = [("Poids filtre RPM | Q", theme.current().fg)]
                    lines += [(f.rpm_text, f.entry.color) for f in res.files]
                    sc.CornerText(pr, sc.html_lines(lines))

    def _pen(self, key: str, color: str) -> pg.QtGui.QPen:
        if sc.is_prefilt(key):
            return pg.mkPen(color, width=1.5, style=Qt.DotLine)
        return pg.mkPen(color, width=2.5)

    def _draw_spectra(self, p: pg.PlotItem, res: SpectralResult, ax: int, smooth: int, legend: bool) -> None:
        for fsp in res.files:
            for key in res.keys:
                data = fsp.spectra.get((key, ax))
                if data is None:
                    continue
                f, v = data
                # dernier point (Nyquist) écarté : il n'est pas doublé et crée un crochet
                f, v = f[:-1], smooth_freq(v, smooth)[:-1]
                name = f"{datasets.DATASETS[key]} | {fsp.entry.label}" if legend else None
                p.plot(f, v, pen=self._pen(key, fsp.entry.color), name=name)

    def _draw_overlays(self, p: pg.PlotItem, res: SpectralResult, y0: float, y1: float, nyq: float) -> None:
        """Courbes en « S » : part du temps où chaque harmonique moteur est sous la fréquence f."""
        choice = self.harmonics.currentData()
        enabled = set(self.motors.enabled())
        f = np.linspace(0, nyq, 800)
        span = y1 - y0
        for fsp in res.files:
            if choice == "rpm":
                self._draw_rpm_range(p, fsp)
            elif choice:
                for m, vals in enumerate(fsp.motor_sorted):
                    if m % 4 not in enabled:
                        continue
                    for h in choice:
                        y = y0 + span * _cdf(vals * h, f)
                        p.plot(f, y, pen=pg.mkPen(theme.motor_color(m), width=1.2,
                                                  style=sc.HARM_STYLES.get(h, Qt.DotLine)))
            if self.notch_box.isChecked():
                for vals in fsp.notch_sorted:
                    y = y0 + span * _cdf(vals, f)
                    p.plot(f, y, pen=pg.mkPen("#808080", width=1.5, style=Qt.DashDotLine))

    def _draw_rpm_range(self, p: pg.PlotItem, fsp: FileSpectra) -> None:
        """Plage couverte par les notchs du filtre RPM (5e → 95e centile de chaque harmonique active)."""
        s = fsp.entry.log.settings
        if s.rpm_harmonics <= 0 or not fsp.motor_sorted:
            return
        allv = np.concatenate(fsp.motor_sorted)
        if allv.size == 0:
            return
        lo, hi = np.percentile(allv, [5, 95])
        for h in range(1, s.rpm_harmonics + 1):
            w = s.rpm_weights[h - 1] if h - 1 < len(s.rpm_weights) else 100
            if w <= 0:
                continue
            a, b = max(lo * h, s.rpm_min_hz), hi * h
            if b <= a:
                continue
            c = pg.mkColor(fsp.entry.color)
            c.setAlpha(int(10 + 25 * w / 100))  # plus opaque si le poids de l'harmonique est fort
            edge = pg.mkPen(fsp.entry.color, width=1, style=sc.HARM_STYLES.get(h, Qt.DotLine))
            reg = pg.LinearRegionItem((a, b), movable=False, brush=pg.mkBrush(c), pen=edge)
            reg.setZValue(-10)
            p.addItem(reg)

    def _delay_text(self, p: pg.PlotItem, res: SpectralResult, ax: int) -> None:
        lines = []
        for fsp in res.files:
            txt = f"Gyro : {sc.fmt_ms(fsp.gyro_ms)}"
            if ax < 2 or datasets.has_axis(fsp.entry.log, "dterm", ax):
                txt += f" | D : {sc.fmt_ms(fsp.dterm_ms)}"
            lines.append((txt, fsp.entry.color))
        sc.CornerText(p, sc.html_lines(lines))

    def _draw_motor_noise(self, p: pg.PlotItem, res: SpectralResult, ax: int) -> None:
        x = np.array([1.0, 2.0, 3.0])
        for fsp in res.files:
            for key in res.keys:
                data = fsp.noise.get((key, ax))
                if data is None:
                    continue
                mean, std = data
                ok = np.isfinite(mean)
                if not ok.any():
                    continue
                pen = self._pen(key, fsp.entry.color)
                p.plot(x[ok], mean[ok], pen=pen, symbol="o", symbolSize=7,
                       symbolBrush=fsp.entry.color, symbolPen=fsp.entry.color)
                err = pg.ErrorBarItem(x=x[ok], y=mean[ok], height=2 * np.nan_to_num(std[ok]), beam=0.06,
                                      pen=pg.mkPen(fsp.entry.color, width=1.5))
                p.addItem(err)
