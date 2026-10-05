"""Fenêtre « Réponse indicielle » (équivalent du Step Response Tool de PIDtoolbox).

À gauche, une ligne par axe : réponse indicielle moyenne de chaque fichier (couleur du fichier),
bande ± écart-type et réponses brutes en option, bloc de texte « P, I, D, Dm, F (n=…) ».
À droite, deux colonnes de métriques au choix (une valeur par fichier = « test » 1…N, barres
d'erreur = dispersion entre fenêtres pour le pic et la latence, écart-type jackknife pour les marges),
ou la vue « Curseurs Betaflight ». Marges de stabilité : point plein = boucle ouverte mesurée,
point creux = méthode approchée, triangle = minorant (pic de sensibilité au bord de la bande mesurable).
"""

from __future__ import annotations

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import Qt, QTimer
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDoubleSpinBox,
    QFormLayout,
    QGroupBox,
    QLabel,
    QVBoxLayout,
)

from .. import theme
from ._step_common import (
    SPLITS,
    FileStep,
    compute_file_step,
    draw_slider_plot,
    sliders_legend_html,
    style_plot,
    wrap_canvas,
)
from .base import ToolWindow

# clé → (titre de la liste, libellé d'axe)
METRICS = {
    "peak": ("Pic", "Pic"),
    "latency": ("Latence (50 %)", "Latence (ms)"),
    "sp_delay": ("Retard consigne→gyro", "Retard consigne→gyro (ms)"),
    "rise": ("Temps de montée", "Montée 10–90 % (ms)"),
    "bandwidth": ("Bande passante", "Bande passante (Hz)"),
    "sens_peak": ("Pic de sensibilité", "Pic de sensibilité (dB)"),
    "sens_hz": ("Fréq. du pic de sensibilité", "Fréq. pic sensibilité (Hz)"),
    "phase_margin": ("Marge de phase", "Marge de phase (°)"),
    "gain_margin": ("Marge de gain", "Marge de gain (dB)"),
    "crossover": ("Fréquence de croisement", "Fréq. de croisement (Hz)"),
}
LOOP_KEYS = {"sens_peak": "sensitivity_peak_db", "sens_hz": "sensitivity_peak_hz", "phase_margin": "phase_margin_deg",
             "gain_margin": "gain_margin_db", "crossover": "crossover_hz"}
MAX_RAW = 300  # réponses brutes dessinées au plus par fichier et par axe


def metric_value(f: FileStep, ax: int, key: str) -> tuple[float, float]:
    """(valeur, demi-barre d'erreur) d'une métrique pour un fichier et un axe."""
    a = f.axes[ax] if ax < len(f.axes) else None
    if a is None or a.res.n == 0:
        return float("nan"), 0.0
    m, lp = a.res.metrics, a.res.loop
    if key == "peak":
        return m.peak, _spread(a.seg_peak)
    if key == "latency":
        return m.latency_ms, _spread(a.seg_latency)
    if key == "sp_delay":
        return a.sp_delay_ms, 0.0
    if key == "rise":
        return m.rise_ms, 0.0
    if key == "bandwidth":
        return lp.bandwidth_hz, 0.0
    field = LOOP_KEYS[key]
    err = lp.std.get(field, 0.0)
    return float(getattr(lp, field)), float(err) if np.isfinite(err) else 0.0


def loop_marker(f: FileStep, ax: int, key: str) -> str:
    """Forme du point d'une marge : « mesurée », « approchée » ou « minorant » (pic au bord de la bande)."""
    a = f.axes[ax] if ax < len(f.axes) else None
    if key not in LOOP_KEYS or a is None or a.res.n == 0:
        return "mesurée"
    lp = a.res.loop
    if not lp.measured:
        return "approchée"
    if key == "sens_peak" and lp.sensitivity_peak_at_edge:
        return "minorant"
    return "mesurée"


def _spread(v) -> float:
    v = np.asarray(v, dtype=float)
    v = v[np.isfinite(v)]
    return float(np.std(v)) if v.size > 1 else 0.0


class StepWindow(ToolWindow):
    title = "Réponse indicielle"
    multi_file = True

    def __init__(self, state, main_window=None) -> None:
        self._opts = {"seg_s": 2.0, "smooth": 0, "split": "tous", "y_corr": False}
        self._pending = False
        self._last_result = None
        super().__init__(state, main_window)
        top, _bottom = wrap_canvas(self)
        top.addStretch(1)
        top.addWidget(QLabel("Colonne 1 :"))
        top.addWidget(self.metric1)
        top.addSpacing(30)
        top.addWidget(QLabel("Colonne 2 :"))
        top.addWidget(self.metric2)
        top.addSpacing(20)

    # ------------------------------------------------------------ panneau de réglages
    def build_controls(self, layout: QVBoxLayout) -> None:
        self.metric1 = QComboBox()
        self.metric2 = QComboBox()
        for cb in (self.metric1, self.metric2):
            for key, (name, _unit) in METRICS.items():
                cb.addItem(name, key)
            cb.currentIndexChanged.connect(self.on_options_changed)
        self.metric1.setCurrentIndex(list(METRICS).index("latency"))
        self.metric2.setCurrentIndex(list(METRICS).index("sp_delay"))

        box = QGroupBox("Affichage")
        form = QFormLayout(box)
        self.ymax = QDoubleSpinBox()
        self.ymax.setRange(0.5, 5.0)
        self.ymax.setSingleStep(0.1)
        self.ymax.setValue(1.5)
        self.ymax.valueChanged.connect(self.on_options_changed)
        form.addRow("Y max", self.ymax)
        self.expand = QComboBox()
        self.expand.addItems(["non", "Roll", "Pitch", "Yaw"])
        self.expand.currentIndexChanged.connect(self.on_options_changed)
        form.addRow("Agrandir", self.expand)
        self.raw = QCheckBox("Brutes (toutes les fenêtres)")
        self.sd = QCheckBox("± écart-type")
        self.bf_view = QCheckBox("Vue curseurs Betaflight")
        for cb in (self.raw, self.sd, self.bf_view):
            cb.toggled.connect(self.on_options_changed)
            form.addRow(cb)
        layout.addWidget(box)

        box2 = QGroupBox("Calcul")
        form2 = QFormLayout(box2)
        self.smooth = QComboBox()
        for n in (0, 20, 40, 60):
            self.smooth.addItem("aucun" if n == 0 else f"{n} échantillons", n)
        form2.addRow("Lissage gyro", self.smooth)
        self.seg = QComboBox()
        for s in (1.0, 2.0, 3.0):
            self.seg.addItem(f"{s:g} s", s)
        self.seg.setCurrentIndex(1)
        form2.addRow("Fenêtre déconv.", self.seg)
        self.split = QComboBox()
        self.split.addItems(list(SPLITS))
        form2.addRow("Consigne max", self.split)
        self.ycorr = QCheckBox("Y corr. (recentrer sur 1)")
        form2.addRow(self.ycorr)
        for w in (self.smooth, self.seg, self.split):
            w.currentIndexChanged.connect(self._calc_options_changed)
        self.ycorr.toggled.connect(self._calc_options_changed)
        layout.addWidget(box2)
        help_ = QLabel(
            "Courbe idéale : montée rapide, pic 1,02–1,08 puis plat à 1. "
            "Pic > 1,1 : manque de D (ou trop de P) ; montée molle sans pic : trop de D ou P faible. "
            "Barres d'erreur : dispersion entre fenêtres de 2 s. "
            "Marges de stabilité : ● mesurées sur les termes P, I, D, F du log ; "
            "○ approchées (termes absents) ; ▲ au moins cette valeur (pic hors de la bande mesurable).")
        help_.setWordWrap(True)
        help_.setStyleSheet(theme.muted_style())
        layout.addWidget(help_)

    def _calc_options_changed(self, *_a) -> None:
        self._opts = {
            "seg_s": float(self.seg.currentData()),
            "smooth": int(self.smooth.currentData()),
            "split": self.split.currentText(),
            "y_corr": self.ycorr.isChecked(),
        }
        if self.files.selected_entries():
            self.run()

    def run(self) -> None:
        if self._busy:
            self._pending = True
            return
        self._run_opts = dict(self._opts)
        super().run()

    # ------------------------------------------------------------ calcul (thread de travail)
    def compute(self, entries) -> list[FileStep]:
        opts = dict(getattr(self, "_run_opts", self._opts))
        key = ("step", tuple(sorted(opts.items())))
        out = []
        for e in entries:
            fs = e.extras.get(key)
            if fs is None:
                fs = compute_file_step(e, opts)
                e.extras[key] = fs
            # couleur / nom à jour (le fichier a pu changer de rang)
            fs.color, fs.label = e.color, e.label
            out.append(fs)
        return out

    # ------------------------------------------------------------ affichage
    def render(self, result: list[FileStep]) -> None:
        c = self.canvas
        c.clear()
        c.setBackground(theme.current().bg)
        axes = self.axes()
        if self.expand.currentIndex() > 0:
            axes = [self.expand.currentIndex() - 1]
        grid = c.ci.layout
        for col in range(5):
            grid.setColumnStretchFactor(col, 0)
        grid.setColumnStretchFactor(0, 5)
        grid.setColumnStretchFactor(2, 2)
        grid.setColumnStretchFactor(3, 2)
        if not result or not axes:
            return
        bf = self.bf_view.isChecked()
        for row, ax in enumerate(axes):
            p = c.addPlot(row=row, col=0)
            self._draw_step(p, result, ax, first=row == 0, last=row == len(axes) - 1)
            lab = c.addLabel(self._text_block(result, ax), row=row, col=1, justify="left")
            lab.setMinimumWidth(190)
            grid.setAlignment(lab, Qt.AlignTop | Qt.AlignLeft)
            if bf:
                continue
            for col, combo in ((2, self.metric1), (3, self.metric2)):
                mp = c.addPlot(row=row, col=col)
                self._draw_metric(mp, result, ax, combo.currentData(), last=row == len(axes) - 1)
        self.metric1.setEnabled(not bf)
        self.metric2.setEnabled(not bf)
        if bf:
            sp = c.addPlot(row=0, col=2, colspan=2)
            draw_slider_plot(sp, [(f.color, f.sliders) for f in result])
            leg = c.addLabel(sliders_legend_html(result), row=0, col=4, justify="left")
            grid.setAlignment(leg, Qt.AlignTop | Qt.AlignLeft)
        if self._pending:
            self._pending = False
            QTimer.singleShot(0, self.run)

    def _draw_step(self, p: pg.PlotItem, result: list[FileStep], ax: int, first: bool, last: bool) -> None:
        style_plot(p)
        th = theme.current()
        ymax = self.ymax.value()
        if first:
            p.setTitle("Réponse indicielle", color=th.fg)
        p.setLabel("left", f"{theme.AXIS_NAMES[ax]} — réponse")
        p.setLabel("bottom", "Temps (ms)")
        p.addItem(pg.InfiniteLine(1.0, angle=0, pen=pg.mkPen(th.fg, width=1, style=Qt.DashLine)))
        valid = [(f, f.axes[ax].res) for f in result if f.axes[ax] is not None and f.axes[ax].res.n > 0]
        # 1) réponses brutes, 2) bandes ± écart-type, 3) moyennes : les moyennes restent lisibles
        if self.raw.isChecked():
            for f, r in valid:
                R = r.responses
                if R.shape[0] > MAX_RAW:
                    R = R[np.linspace(0, R.shape[0] - 1, MAX_RAW).astype(int)]
                x = np.tile(np.append(r.t_ms, np.nan), R.shape[0])
                y = np.hstack([R, np.full((R.shape[0], 1), np.nan)]).ravel()
                rc = pg.mkColor(f.color)
                rc.setAlpha(25)
                p.addItem(pg.PlotCurveItem(x, y, pen=pg.mkPen(rc, width=1), connect="finite"))
        if self.sd.isChecked():
            for f, r in valid:
                bc = pg.mkColor(f.color)
                bc.setAlpha(50)
                hi = pg.PlotCurveItem(r.t_ms, r.mean + r.std, pen=pg.mkPen(None))
                lo = pg.PlotCurveItem(r.t_ms, r.mean - r.std, pen=pg.mkPen(None))
                p.addItem(hi)
                p.addItem(lo)
                p.addItem(pg.FillBetweenItem(hi, lo, brush=pg.mkBrush(bc)))
        for f, r in valid:
            if self.raw.isChecked():  # liseré sombre pour détacher la moyenne du nuage
                p.plot(r.t_ms, r.mean, pen=pg.mkPen(th.fg, width=4.5))
            p.plot(r.t_ms, r.mean, pen=pg.mkPen(f.color, width=2.5))
        p.setXRange(0, 500, padding=0.025)
        p.setYRange(0, ymax, padding=0.04)
        p.getAxis("bottom").setTicks([[(v, f"{v:g}") for v in range(0, 501, 100)],
                                      [(v, "") for v in range(50, 500, 100)]])
        step = 0.25 if ymax <= 2.5 else 0.5
        p.getAxis("left").setTicks([[(v, f"{v:g}") for v in np.arange(0, ymax + 1e-9, step)]])

    def _text_block(self, result: list[FileStep], ax: int) -> str:
        rows = [f"<b style='color:{theme.current().fg}'>P, I, D, Dm, F</b>"]
        for i, f in enumerate(result):
            a = f.axes[ax]
            if a is None or a.res.n == 0:
                txt = "données insuffisantes"
            else:
                txt = f"{f.pids[ax].text()}&nbsp;&nbsp;(n={a.res.n})"
            rows.append(f"<span style='color:{f.color}; font-weight:600'>{i + 1}) {txt}</span>")
        return "<br>".join(rows)

    def _draw_metric(self, p: pg.PlotItem, result: list[FileStep], ax: int, key: str, last: bool) -> None:
        style_plot(p)
        th = theme.current()
        _name, unit = METRICS[key]
        p.setLabel("left", f"{theme.AXIS_NAMES[ax]} — {unit}")
        p.setLabel("bottom", "Test")
        n = len(result)
        xs = np.arange(1, n + 1, dtype=float)
        vals = np.array([metric_value(f, ax, key) for f in result], dtype=float).reshape(n, 2)
        ys, err = vals[:, 0], vals[:, 1]
        ok = np.isfinite(ys)
        if key == "sens_peak":
            p.addItem(pg.InfiniteLine(4.0, angle=0, pen=pg.mkPen("#e6c619", width=2)))
            p.addItem(pg.InfiniteLine(6.0, angle=0, pen=pg.mkPen("#e6194b", width=2)))
        if key == "peak":
            p.addItem(pg.InfiniteLine(1.0, angle=0, pen=pg.mkPen(th.fg, style=Qt.DashLine)))
        for i, f in enumerate(result):
            if not ok[i]:
                continue
            if err[i] > 0:
                p.addItem(pg.ErrorBarItem(x=xs[i:i + 1], y=ys[i:i + 1], height=2 * err[i:i + 1], beam=0.12,
                                          pen=pg.mkPen(f.color, width=1.5)))
            kind = loop_marker(f, ax, key)
            symbol = "t1" if kind == "minorant" else "o"
            brush = pg.mkBrush(None) if kind == "approchée" else pg.mkBrush(f.color)
            p.addItem(pg.ScatterPlotItem(xs[i:i + 1], ys[i:i + 1], size=11, symbol=symbol, brush=brush,
                                         pen=pg.mkPen(f.color, width=2)))
        p.getAxis("bottom").setTicks([[(i, str(i)) for i in range(1, n + 1)]])
        p.setXRange(0.5, n + 0.5, padding=0)
        p.setMouseEnabled(x=False)
        if ok.any():
            lo = np.nanmin(ys[ok] - err[ok])
            hi = np.nanmax(ys[ok] + err[ok])
            if key == "sens_peak":
                lo, hi = min(lo, 0.0), max(hi, 7.0)
            span = max(hi - lo, 0.1 * max(abs(hi), 1e-3), 0.02)
            p.setYRange(lo - 0.25 * span, hi + 0.25 * span, padding=0)
