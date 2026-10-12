"""Fenêtre « Réponse indicielle » (équivalent du Step Response Tool de PIDtoolbox).

À gauche, une ligne par axe (yaw masqué au départ) : réponse indicielle moyenne de chaque fichier (couleur
du fichier), bande ± écart-type et réponses brutes en option, point creux à l'arrêt de la montée quand il y
a une cassure, et dans le graphique le bloc « P, I, D, Dm, F (n=…) · pic · latence · cassure ».
Au milieu, deux colonnes de métriques au choix (une valeur par fichier = « test » 1…N, barres d'erreur =
dispersion entre fenêtres pour le pic et la latence, écart-type jackknife pour les marges et la cassure ;
vols conseillés entourés). Marges de stabilité : point plein = boucle ouverte mesurée, point creux =
méthode approchée, triangle = minorant (pic de sensibilité au bord de la bande mesurable).
À droite, le « Diagnostic du réglage » (advisor/step_tuning.py) et, dessous, les curseurs PID Betaflight.
"""

from __future__ import annotations

import html

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QFontMetrics
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDoubleSpinBox,
    QFormLayout,
    QGroupBox,
    QLabel,
    QSplitter,
    QTextBrowser,
    QVBoxLayout,
)

from ...advisor.step_tuning import RIPPLE_EARLY, TuningReport, step_tuning
from .. import theme
from ._step_common import (
    SPLITS,
    FileStep,
    compute_file_step,
    draw_slider_plot,
    mode_text,
    style_plot,
    wrap_canvas,
)
from .advisor_panel import finding_html
from .base import ToolWindow

# clé → (titre de la liste, libellé d'axe)
METRICS = {
    "peak": ("Pic", "Pic"),
    "latency": ("Latence (50 %)", "Latence (ms)"),
    "sp_delay": ("Retard consigne→gyro", "Retard consigne→gyro (ms)"),
    "rise": ("Temps de montée", "Montée 10–90 % (ms)"),
    "ripple": ("Cassure (ripple)", "Cassure"),
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
REPORT_WIDTH = 400  # largeur du rapport « Diagnostic du réglage » à son ouverture (px)
SIDE_REPORT_SHARE = 0.6  # part de la hauteur pour le rapport, le reste pour les curseurs Betaflight
X_RANGES = (500, 250, 150)  # durées affichées (ms) : 150 pour bien voir la montée et la cassure
X_DEFAULT = 250  # la montée (pic, cassure) occupe assez de largeur, le palier reste visible


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
    if key == "ripple":
        return m.ripple, m.ripple_std if np.isfinite(m.ripple_std) else 0.0
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
        # colonne entre les courbes et le panneau de réglages : rapport du diagnostic, puis curseurs Betaflight
        self.report = QTextBrowser()
        self.report.setOpenExternalLinks(False)
        self.slider_canvas = pg.GraphicsLayoutWidget()
        self.slider_canvas.setBackground(theme.current().bg)
        self.side = QSplitter(Qt.Vertical)
        self.side.addWidget(self.report)
        self.side.addWidget(self.slider_canvas)
        self.side.setStretchFactor(0, 3)
        self.side.setStretchFactor(1, 2)
        self.side.setMinimumWidth(320)
        self.side.hide()
        self._canvas_container.parentWidget().insertWidget(1, self.side)
        self._best: set[int] = set()
        self.axis_boxes[2].setChecked(False)  # yaw : ni jugé par le diagnostic, ni souvent mesurable

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
        self.xmax = QComboBox()
        for ms in X_RANGES:
            self.xmax.addItem(f"{ms} ms", ms)
        self.xmax.setToolTip("150 ms : la montée en grand, pour voir la cassure (« ripple »).")
        self.xmax.setCurrentIndex(X_RANGES.index(X_DEFAULT))
        self.xmax.currentIndexChanged.connect(self.on_options_changed)
        form.addRow("Temps affiché", self.xmax)
        self.raw = QCheckBox("Brutes (toutes les fenêtres)")
        self.sd = QCheckBox("± écart-type")
        for cb in (self.raw, self.sd):
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
            "Courbe idéale : montée rapide, pic proche de 1 puis plat (sans feedforward, environ 1,03 ; "
            "le feedforward fait monter le pic). Pic marqué : manque de D (ou trop de P) ; "
            "montée molle sans pic : trop de D ou P faible. "
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

    def snapshot_options(self) -> dict:
        return dict(self._opts)

    # ------------------------------------------------------------ calcul (thread de travail)
    def compute(self, entries) -> list[FileStep]:
        opts = dict(self.run_options)
        key = ("step", tuple(sorted(opts.items())))
        out = []
        for e in entries:
            fs = e.extras.get(key)
            if fs is None:
                fs = compute_file_step(e, opts)
                e.extras[key] = fs
            # couleur / nom à jour (le fichier a pu changer de rang)
            fs.color, fs.label = e.color, e.label
            if fs.flight is not None:
                fs.flight.label = e.label
            out.append(fs)
        return out

    # ------------------------------------------------------------ affichage
    def render(self, result: list[FileStep]) -> None:
        self._render_diagnosis(result)
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
        grid.setColumnStretchFactor(1, 2)
        grid.setColumnStretchFactor(2, 2)
        if not result or not axes:
            return
        for row, ax in enumerate(axes):
            p = c.addPlot(row=row, col=0)
            self._draw_step(p, result, ax, first=row == 0, last=row == len(axes) - 1)
            for col, combo in ((1, self.metric1), (2, self.metric2)):
                mp = c.addPlot(row=row, col=col)
                self._draw_metric(mp, result, ax, combo.currentData(), last=row == len(axes) - 1)
        if self._pending:
            self._pending = False
            QTimer.singleShot(0, self.run)

    # ------------------------------------------------------------ diagnostic du réglage
    def _render_diagnosis(self, result: list[FileStep]) -> None:
        flights = [f.flight for f in result if f.flight is not None]
        if not flights:
            self.side.hide()
            self._best = set()
            return
        if self.side.isHidden():
            self.show_beside_canvas(self.side, REPORT_WIDTH)
            h = sum(self.side.sizes()) or self.height()
            # 8 curseurs sur deux lignes (nom, ce qu'il règle) : assez de hauteur pour que les noms ne se
            # chevauchent pas, plus le titre, l'axe et la légende (≈ 7 lignes) ; le rapport défile
            need = int((8 * 2 * 1.15 + 7) * QFontMetrics(self.font()).lineSpacing())
            sliders = min(max(h - int(h * SIDE_REPORT_SHARE), need), int(h * 0.75))
            self.side.setSizes([h - sliders, sliders])
        sc = self.slider_canvas
        sc.clear()
        sc.setBackground(theme.current().bg)
        draw_slider_plot(sc.addPlot(row=0, col=0), [(f.color, f.sliders) for f in result])
        sc.addLabel(_modes_legend(result), row=1, col=0)
        rep = step_tuning(flights)
        self._best = {b.best for b in rep.blocks if b.best is not None}  # vols conseillés, entourés
        self.report.setHtml(_tuning_html(rep, {f.label: f.color for f in result}))

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
            m = r.metrics
            if np.isfinite(m.ripple) and m.ripple >= RIPPLE_EARLY:  # arrêt de la montée : la cassure
                y = float(np.interp(m.ripple_ms, r.t_ms, r.mean))
                p.addItem(pg.ScatterPlotItem([m.ripple_ms], [y], size=10, symbol="o", brush=pg.mkBrush(th.bg),
                                             pen=pg.mkPen(f.color, width=2.5)))
        xmax = float(self.xmax.currentData() or X_DEFAULT)
        p.setXRange(0, xmax, padding=0.025)
        p.setYRange(0, ymax, padding=0.04)
        major = 100 if xmax > 300 else 50 if xmax > 160 else 25
        p.getAxis("bottom").setTicks([[(v, f"{v:g}") for v in np.arange(0, xmax + 1, major)],
                                      [(v, "") for v in np.arange(major / 2, xmax, major)]])
        text = pg.TextItem(html=self._text_block(result, ax), anchor=(1, 1))
        text.setPos(xmax, 0.02 * ymax)
        p.addItem(text)
        step = 0.25 if ymax <= 2.5 else 0.5
        p.getAxis("left").setTicks([[(v, f"{v:g}") for v in np.arange(0, ymax + 1e-9, step)]])

    def _text_block(self, result: list[FileStep], ax: int) -> str:
        """Bloc par fichier, en bas à droite du graphique (zone sans courbe) : gains, pic, latence, cassure."""
        rows = [f"<b style='color:{theme.current().fg}'>P, I, D, Dm, F · pic · latence · cassure</b>"]
        for i, f in enumerate(result):
            a = f.axes[ax]
            if a is None or a.res.n == 0:
                txt = "données insuffisantes"
            else:
                m = a.res.metrics
                rip = f" · {m.ripple:+.3f}".replace(".", ",") if np.isfinite(m.ripple) else ""
                txt = f"{f.pids[ax].text()} (n={a.res.n}) · {m.peak:.2f} · {m.latency_ms:.1f} ms".replace(".", ",") + rip
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
            if i in self._best:  # vol conseillé par le diagnostic
                p.addItem(pg.ScatterPlotItem(xs[i:i + 1], ys[i:i + 1], size=24, symbol="o",
                                             brush=pg.mkBrush(None), pen=pg.mkPen(th.fg, width=2)))
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


def _tuning_html(rep: TuningReport, colors: dict[str, str]) -> str:
    parts = ["<h2>Diagnostic du réglage</h2>", "<h3>Conditions des vols</h3>"]
    parts += [finding_html(f) for f in rep.conditions] or [
        "<p>Feedforward à 0 ou mode Angle, curseur I bas, assez de mouvements : vols exploitables.</p>"]
    if not rep.blocks:
        parts.append("<p>Aucune série reconnue : chargez plusieurs vols du même drone, mêmes filtres, où un seul "
                     "curseur change (Damping, Master, Pitch Tracking ou Pitch Damping).</p>")
    for b in rep.blocks:
        parts.append(f"<h3>{html.escape(b.title)}</h3>")
        if b.lines:
            parts.append("<ul>" + "".join(f"<li>{html.escape(line)}</li>" for line in b.lines) + "</ul>")
        if b.finding is not None:
            parts.append(finding_html(b.finding))
    legend = " · ".join(f"<span style='color:{c}'>■</span> {html.escape(lab)}" for lab, c in colors.items())
    parts.append(f"<hr><p style='{theme.muted_style()}'>Vols : {legend}<br>Dans les colonnes de mesures, un "
                 "cercle entoure les vols conseillés ; sur les courbes, un point creux marque l'arrêt de la montée "
                 "quand il y a une cassure.<br>Ordre de la méthode : Damping avec "
                 "Master 1,0, puis Master, puis roll / pitch, puis affinage. Cassure (« ripple ») : la montée "
                 "s'arrête avant la consigne puis reprend lentement ; vagues : oscillation sur le palier, "
                 "comparée au bruit de mesure (≈ 1 : bruit).</p>")
    return "".join(parts)


def _modes_legend(result: list[FileStep]) -> str:
    """Légende courte sous les curseurs (la colonne fait 400 px) : mode du réglage simplifié par fichier,
    cinq par ligne ; ⚠ si les PID ne correspondent pas aux curseurs."""
    items = [f"<span style='color:{f.color}'>{i + 1}) {mode_text(f.mode)}{'' if f.sliders.consistent else ' ⚠'}</span>"
             for i, f in enumerate(result)]
    lines = [" &nbsp;".join(items[k:k + 5]) for k in range(0, len(items), 5)]
    return ("<b>MODE</b> &nbsp;" + "<br>".join(lines) +
            "<br><span style='font-size:8pt'>● curseurs de l'en-tête &nbsp; ○ estimés depuis les PID</span>")
