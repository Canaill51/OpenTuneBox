"""Fréquence × gaz (équivalent du « Freq x Throttle » de PIDtoolbox).

Jusqu'à 4 colonnes, chacune avec son jeu de données et son fichier ; une ligne par axe.
Chaque carte montre l'amplitude (ou la PSD) du bruit selon la fréquence (verticale) et le gaz
(horizontale) : une ligne horizontale trahit une résonance du châssis, une diagonale le bruit moteur.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import QRectF
from PySide6.QtWidgets import (
    QButtonGroup,
    QCheckBox,
    QComboBox,
    QDoubleSpinBox,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QRadioButton,
    QVBoxLayout,
)

from ...analysis import datasets
from ...analysis.spectral import FreqThrottle, freq_throttle, smooth_map, to_ptb_db
from .. import theme
from ..state import LoadedLog
from . import _spectral_common as sc
from .base import ToolWindow

N_COLUMNS = 4
X_AXES = [("throttle", "% gaz"), ("motor_pct", "% moteur moyen"), ("motor_hz", "Hz moteur (eRPM)")]
X_LABELS = {"throttle": "% gaz", "motor_pct": "% moteur (moyenne)", "motor_hz": "fréquence moteur (Hz)"}
SMOOTHING = [(0, "aucun"), (1, "1"), (2, "2"), (3, "3"), (4, "4"), (6, "6")]

# préréglages PIDtoolbox : (jeux de données des 4 colonnes, fichiers par colonne : indices dans la
# liste des fichiers sélectionnés — A = 0, B = 1…)
PRESETS = [
    ("1 : Gyro pf | Gyro | D pf | D", ["gyro_prefilt", "gyro", "dterm_prefilt", "dterm"], [0, 0, 0, 0]),
    ("2 : Gyro pf | Gyro | P | D", ["gyro_prefilt", "gyro", "pterm", "dterm"], [0, 0, 0, 0]),
    ("3 : Gyro | D | Consigne | Erreur PID", ["gyro", "dterm", "setpoint", "pid_error"], [0, 0, 0, 0]),
    ("4 : A|A|B|B Gyro pf | Gyro", ["gyro_prefilt", "gyro", "gyro_prefilt", "gyro"], [0, 0, 1, 1]),
    ("5 : A|A|B|B D pf | D", ["dterm_prefilt", "dterm", "dterm_prefilt", "dterm"], [0, 0, 1, 1]),
    ("6 : A|B|C|D Gyro pf", ["gyro_prefilt"] * 4, [0, 1, 2, 3]),
    ("7 : A|B|C|D Gyro", ["gyro"] * 4, [0, 1, 2, 3]),
    ("8 : A|B|C|D D", ["dterm"] * 4, [0, 1, 2, 3]),
    ("9 : A|B|C|D Erreur PID", ["pid_error"] * 4, [0, 1, 2, 3]),
]


@dataclass
class Column:
    key: str
    entry: LoadedLog
    maps: dict[int, FreqThrottle]  # axe → carte
    x_range: tuple[float, float]


@dataclass
class FreqThrottleResult:
    mode: str  # "amp" ou "psd"
    xaxis: str
    columns: list[Column | None]


def _x_values(ll: LoadedLog, xaxis: str, multiplier: float) -> tuple[np.ndarray, tuple[float, float]] | None:
    log = ll.log
    if xaxis == "throttle":
        return log.throttle, (0.0, 100.0)
    if not log.n_motors:
        return None
    if xaxis == "motor_pct":
        return log.motors.mean(axis=0), (0.0, 100.0)
    mhz, _est = sc.motor_freqs(log, multiplier)
    m = mhz.mean(axis=0)
    top = np.nanpercentile(m[ll.sel], 99.5) if m[ll.sel].size else 100.0
    return m, (0.0, float(max(50.0, np.ceil(top / 50.0) * 50.0)))


class FreqThrottleWindow(sc.OptionsSnapshot, ToolWindow):
    title = "Fréquence × gaz"

    # ------------------------------------------------------------ panneau de réglages
    def build_controls(self, layout: QVBoxLayout) -> None:
        box = QGroupBox("Colonnes : données, fichier, échelle")
        g = QGridLayout(box)
        g.setVerticalSpacing(2)
        self.col_ds: list[QComboBox] = []
        self.col_file: list[QComboBox] = []
        self.col_sub: list[QCheckBox] = []
        self.col_scale: list[QDoubleSpinBox] = []
        items = [(None, "(vide)")] + list(datasets.DATASETS.items())
        defaults = PRESETS[0][1]
        for c in range(N_COLUMNS):
            ds = sc.combo(items, 1 + list(datasets.DATASETS).index(defaults[c]), self.run)
            sub = QCheckBox("< 100 Hz")
            sub.toggled.connect(self.on_options_changed)
            fc = QComboBox()
            fc.currentIndexChanged.connect(self.run)
            g.addWidget(QLabel(f"{c + 1}"), 2 * c, 0)
            g.addWidget(ds, 2 * c, 1)
            g.addWidget(sub, 2 * c, 2)
            scale = sc.number(0.5, -200, 1e4, 0.05, 2, self.on_options_changed)
            scale.setToolTip("Échelle : haut de la palette de couleurs de cette colonne")
            g.addWidget(fc, 2 * c + 1, 1)
            g.addWidget(scale, 2 * c + 1, 2)
            self.col_scale.append(scale)
            self.col_ds.append(ds)
            self.col_file.append(fc)
            self.col_sub.append(sub)
        layout.addWidget(box)
        self.state.logs_changed.connect(self._refresh_files)
        self._refresh_files()

        self.presets = sc.combo([(None, "Préréglages…")] + [(i, p[0]) for i, p in enumerate(PRESETS)], 0,
                                self._apply_preset)
        layout.addWidget(self.presets)
        auto = QPushButton("Échelle auto")
        auto.setToolTip("Adapte l'échelle de chaque colonne à ses données (sinon 0,5 comme PIDtoolbox).")
        auto.clicked.connect(self._auto_scale)
        layout.addWidget(auto)

        g = QGridLayout()
        self.amp_radio = QRadioButton("Amplitude")
        self.psd_radio = QRadioButton("PSD (dB)")
        self.amp_radio.setChecked(True)
        grp = QButtonGroup(self)
        grp.addButton(self.amp_radio)
        grp.addButton(self.psd_radio)
        self.psd_radio.toggled.connect(self._on_mode_toggled)
        g.addWidget(self.amp_radio, 0, 0)
        g.addWidget(self.psd_radio, 0, 1)
        g.addWidget(QLabel("Axe X"), 1, 0)
        self.xaxis = sc.combo(X_AXES, 0, self.run)
        g.addWidget(self.xaxis, 1, 1)
        g.addWidget(QLabel("Couleurs"), 2, 0)
        self.cmap = sc.combo([(n, n) for n in sc.COLORMAP_NAMES], 0, self.on_options_changed)
        g.addWidget(self.cmap, 2, 1)
        g.addWidget(QLabel("Lissage"), 3, 0)
        self.smoothing = sc.combo(SMOOTHING, 2, self.on_options_changed)
        g.addWidget(self.smoothing, 3, 1)
        g.addWidget(QLabel("Bas de l'échelle"), 4, 0)
        self.zmin = sc.number(0.0, -200, 1e4, 0.05, 2, self.on_options_changed)
        g.addWidget(self.zmin, 4, 1)
        g.addWidget(QLabel("Bande moy./pic < 100 Hz (Hz)"), 6, 0, 1, 2)
        self.flo = sc.number(20, 0, 100, 5, 0, self.on_options_changed)
        self.fhi = sc.number(60, 0, 100, 5, 0, self.on_options_changed)
        g.addWidget(self.flo, 7, 0)
        g.addWidget(self.fhi, 7, 1)
        layout.addLayout(g)
        row = QHBoxLayout()
        row.addWidget(QLabel("RPM estimé ×"))
        self.multiplier = sc.number(2.1, 0.1, 20, 0.1, 2, self.run)
        self.multiplier.setToolTip("Utilisé seulement si le log n'a pas l'eRPM (télémétrie DShot).")
        row.addWidget(self.multiplier)
        layout.addLayout(row)

    def _refresh_files(self) -> None:
        for fc in self.col_file:
            keep = fc.currentIndex()
            fc.blockSignals(True)
            fc.clear()
            for i, ll in enumerate(self.state.logs):
                fc.addItem(ll.label, i)
            if self.state.logs:
                fc.setCurrentIndex(keep if 0 <= keep < fc.count() else max(0, self.state.current))
            fc.blockSignals(False)

    def _apply_preset(self, idx: int) -> None:
        p = self.presets.currentData()
        if p is None:
            return
        _name, keys, files = PRESETS[p]
        sel = [self.state.logs.index(e) for e in self.files.selected_entries()] or [max(0, self.state.current)]
        widgets = self.col_ds + self.col_file + self.col_sub
        for w in widgets:
            w.blockSignals(True)
        for c in range(N_COLUMNS):
            self.col_ds[c].setCurrentIndex(self.col_ds[c].findData(keys[c]))
            fi = files[c]
            if fi < len(sel):
                self.col_file[c].setCurrentIndex(self.col_file[c].findData(sel[fi]))
            else:  # pas assez de fichiers sélectionnés : colonne vide
                self.col_ds[c].setCurrentIndex(0)
        for w in widgets:
            w.blockSignals(False)
        self.presets.blockSignals(True)
        self.presets.setCurrentIndex(0)
        self.presets.blockSignals(False)
        self.run()

    def _auto_scale(self) -> None:
        """Haut de l'échelle = 99e centile (amplitude) ou maximum (PSD) des cartes de la colonne, dans la
        bande des valeurs « moy./pic » (les très basses fréquences des mouvements de manche écraseraient
        sinon le bruit)."""
        res = self._last_result
        if res is None:
            return
        for c, col in enumerate(res.columns):
            if col is None or not col.maps:
                continue
            lo, hi = (self.flo.value(), self.fhi.value()) if self.col_sub[c].isChecked() else (100.0, 1000.0)
            parts = [m.z[(m.freqs >= lo) & (m.freqs <= hi)] for m in col.maps.values()]
            z = np.concatenate([p[np.isfinite(p)] for p in parts])
            if z.size == 0:
                continue
            if res.mode == "psd":
                top = float(np.ceil(np.max(z) / 5.0) * 5.0)
            else:
                top = sc.nice_ceil(float(np.percentile(z, 99)))
            self.col_scale[c].blockSignals(True)
            self.col_scale[c].setValue(top)
            self.col_scale[c].blockSignals(False)
        self.on_options_changed()

    def _on_mode_toggled(self, psd: bool) -> None:
        for s, v in [(self.zmin, -40 if psd else 0)] + [(z, 10 if psd else 0.5) for z in self.col_scale]:
            s.blockSignals(True)
            s.setValue(v)
            s.blockSignals(False)
        self.run()

    def collect_options(self) -> dict:
        cols = []
        for c in range(N_COLUMNS):
            cols.append((self.col_ds[c].currentData(), self.col_file[c].currentData()))
        return {
            "columns": cols,
            "mode": "psd" if self.psd_radio.isChecked() else "amp",
            "xaxis": self.xaxis.currentData(),
            "multiplier": float(self.multiplier.value()),
        }

    # ------------------------------------------------------------ calcul (arrière-plan)
    def compute(self, entries: list[LoadedLog]) -> FreqThrottleResult:
        opts = self.take_options()
        logs = list(self.state.logs)
        columns: list[Column | None] = []
        for key, fi in opts["columns"]:
            if key is None or fi is None or not (0 <= fi < len(logs)):
                columns.append(None)
                continue
            ll = logs[fi]
            xv = _x_values(ll, opts["xaxis"], opts["multiplier"])
            if xv is None or datasets.get(ll.log, key) is None:
                columns.append(None)
                continue
            xaxis, x_range = xv
            sel = ll.sel
            maps = {}
            for ax in range(3):
                if not datasets.has_axis(ll.log, key, ax):
                    continue
                x = datasets.get(ll.log, key)[ax, sel]
                m = freq_throttle(x, xaxis[sel], ll.log.fs, opts["mode"], x_range=x_range, breaks=ll.log.breaks(sel))
                if m.mode == "psd":
                    m.z = to_ptb_db(m.z)
                maps[ax] = m
            columns.append(Column(key, ll, maps, x_range))
        return FreqThrottleResult(opts["mode"], opts["xaxis"], columns)

    # ------------------------------------------------------------ affichage
    def render(self, res: FreqThrottleResult) -> None:
        self.canvas.clear()
        axes = self.axes()
        cmap = sc.colormap(self.cmap.currentData())
        smooth = int(self.smoothing.currentData())
        xlabel = X_LABELS[res.xaxis]
        for c, col in enumerate(res.columns):
            if col is None:
                continue
            lo = self.zmin.value()
            levels = (lo, max(self.col_scale[c].value(), lo + 1e-6))
            title = f"{datasets.DATASETS[col.key]} | {col.entry.label}"
            lab = self.canvas.addLabel(f'<span style="color:{col.entry.color}; font-weight:600">{title}</span>',
                                       row=0, col=c)
            lab.setMaximumHeight(22)
            bar = pg.ColorBarItem(values=levels, colorMap=cmap, orientation="horizontal", interactive=False,
                                  width=12)
            bar.setMaximumHeight(42)
            self.canvas.addItem(bar, row=1, col=c)
            for r, ax in enumerate(axes):
                ft = col.maps.get(ax)
                if ft is None:
                    continue
                p = sc.add_plot(self.canvas, 2 + r, c)
                p.vb.setBackgroundColor(sc.MAP_BACKGROUND)
                sc.style_plot(p, f"{theme.AXIS_NAMES[ax]} | fréq. (Hz)", xlabel)
                img = self._draw_map(p, ft, col, self.col_sub[c].isChecked(), smooth, cmap, levels)
                if r == 0:
                    bar.setImageItem(img)
        # en-têtes (titre, barre de couleurs) compacts, cartes extensibles
        lay = self.canvas.ci.layout
        for r in range(2 + 3):
            lay.setRowStretchFactor(r, 0 if r < 2 else 1)
        for c in range(N_COLUMNS):
            lay.setColumnStretchFactor(c, 1)
        # ColorBarItem réduit la zone centrale quand on l'ajoute à une fenêtre visible : on la remet
        # à la taille de la vue
        sc.restore_canvas(self.canvas)

    def _draw_map(self, p: pg.PlotItem, ft: FreqThrottle, col: Column, sub100: bool, smooth: int,
                  cmap: pg.ColorMap, levels: tuple[float, float]) -> pg.ImageItem:
        f_top = 100.0 if sub100 else float(ft.freqs[-1])
        keep = ft.freqs <= f_top + 1e-9
        z = smooth_map(ft.z, smooth)[keep]
        f = ft.freqs[keep]
        df = f[1] - f[0] if f.size > 1 else 1.0
        x0, x1 = col.x_range
        img = pg.ImageItem(z, axisOrder="row-major")
        img.setColorMap(cmap)
        img.setLevels(levels)
        # chaque pixel couvre une tranche (x) et un pas de fréquence (y), centré sur sa valeur
        img.setRect(QRectF(x0, f[0] - df / 2, x1 - x0, df * f.size))
        p.addItem(img)
        p.setXRange(x0, x1, padding=0)
        p.setYRange(0, f_top, padding=0)
        lo, hi = (self.flo.value(), self.fhi.value()) if sub100 else (100.0, 1000.0)
        mean, peak = ft.band_stats(lo, hi)
        white = theme.PLOT_COLORS["overlay"]
        sc.CornerText(p, sc.html_lines([(f"moy.={sc.fmt3(mean)}", white), (f"pic={sc.fmt3(peak)}", white)]),
                      fill=pg.mkBrush(theme.OVERLAY_FILL))
        return img
