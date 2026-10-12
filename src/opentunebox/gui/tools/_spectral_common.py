"""Éléments partagés par les fenêtres spectrales (spectre, fréquence × gaz, fréquence × temps).

- palettes de couleurs des cartes (« hot » comme PIDtoolbox, viridis, inferno, turbo, jet) ;
- petits widgets : listes déroulantes, champs numériques, cases moteurs M1…M4 ;
- textes ancrés dans un coin d'un graphique (ne bougent pas quand on zoome) ;
- choix et styles des harmoniques moteur.
"""

from __future__ import annotations

from typing import Callable

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QCheckBox, QComboBox, QDoubleSpinBox, QGridLayout, QWidget

from ...analysis.datasets import PREFILT_OF
from ...analysis.motors import motor_hz
from ...log.flightlog import FlightLog
from .. import theme

# ---------------------------------------------------------------- palettes

_COLORMAP_STOPS = {
    # noir → rouge → jaune → blanc, comme « hot » de MATLAB utilisé par PIDtoolbox
    "hot": [(0.0, (0, 0, 0)), (0.375, (255, 0, 0)), (0.75, (255, 255, 0)), (1.0, (255, 255, 255))],
    "jet": [(0.0, (0, 0, 143)), (0.125, (0, 0, 255)), (0.375, (0, 255, 255)), (0.625, (255, 255, 0)),
            (0.875, (255, 0, 0)), (1.0, (128, 0, 0))],
}
COLORMAP_NAMES = ["hot", "inferno", "viridis", "turbo", "jet"]


def colormap(name: str) -> pg.ColorMap:
    if name in _COLORMAP_STOPS:
        pos, cols = zip(*_COLORMAP_STOPS[name])
        return pg.ColorMap(np.array(pos), np.array(cols, dtype=np.ubyte))
    return pg.colormap.get(name)


# ---------------------------------------------------------------- harmoniques moteur

# style de trait par rang d'harmonique (1re plein, 2e tirets, 3e pointillés), comme PTB
HARM_STYLES = {1: Qt.SolidLine, 2: Qt.DashLine, 3: Qt.DotLine}
MOTOR_LABELS = ["M1/5", "M2/6", "M3/7", "M4/8"]

PREFILT_KEYS = set(PREFILT_OF.values())


def is_prefilt(key: str) -> bool:
    return key in PREFILT_KEYS


def motor_freqs(log: FlightLog, multiplier: float | None) -> tuple[np.ndarray | None, bool]:
    """Fréquence de chaque moteur (Hz). Le multiplicateur ne sert que sans eRPM."""
    return motor_hz(log, multiplier if log.motor_hz is None else None)


def rpm_filter_text(log: FlightLog) -> str:
    """« 100,25,100 % | Q500 » (poids des harmoniques du filtre RPM et Q), ou « RPM inactif »."""
    s = log.settings
    if s.rpm_harmonics <= 0:
        return "filtre RPM inactif"
    w = ",".join(str(v) for v in s.rpm_weights[: s.rpm_harmonics])
    return f"{w} % | Q{s.rpm_q}"


def fmt_ms(v: float) -> str:
    return "—" if not np.isfinite(v) else f"{v:.2f}".rstrip("0").rstrip(".").replace(".", ",") + " ms"


def fmt3(v: float) -> str:
    """Valeur à 3 chiffres significatifs, virgule décimale."""
    if not np.isfinite(v):
        return "—"
    return f"{v:.3g}".replace(".", ",")


def decimate(t: np.ndarray, y: np.ndarray, max_points: int = 4000) -> tuple[np.ndarray, np.ndarray]:
    """Réduit une courbe longue (moyenne par blocs) pour garder un affichage fluide."""
    n = t.shape[0]
    if n <= max_points:
        return t, y
    k = int(np.ceil(n / max_points))
    m = (n // k) * k
    return t[:m:k] + 0.0, y[:m].reshape(-1, k).mean(axis=1)


# ---------------------------------------------------------------- graphiques


class EdgeAxis(pg.AxisItem):
    """Axe qui garde les étiquettes des bornes (0, Nyquist, Y min/max) quand la grille est active.

    Avec la grille, pyqtgraph réduit la zone de l'axe et masque le texte qui en dépasse, si bien que
    les valeurs extrêmes disparaissent ; on élargit simplement cette zone de quelques pixels.
    """

    def boundingRect(self):  # noqa: N802
        r = super().boundingRect()
        if self.orientation in ("left", "right"):
            return r.adjusted(0, -12, 0, 12)
        return r.adjusted(-20, 0, 20, 0)


def add_plot(layout: pg.GraphicsLayout, row: int, col: int, **kw) -> pg.PlotItem:
    axes = {side: EdgeAxis(side) for side in ("left", "bottom")}
    return layout.addPlot(row=row, col=col, axisItems=axes, **kw)


# fond des cartes (zones sans données) : gris neutre, distinct du noir (faible) et du blanc (saturé)
MAP_BACKGROUND = (128, 128, 128)


def nice_ceil(v: float) -> float:
    """Arrondi supérieur « rond » (1, 2, 2,5, 5 × 10^n) pour une échelle de couleurs."""
    if not np.isfinite(v) or v <= 0:
        return 1.0
    e = 10.0 ** np.floor(np.log10(v))
    for m in (1.0, 2.0, 2.5, 5.0, 10.0):
        if v <= m * e * (1 + 1e-9):
            return m * e
    return 10.0 * e


def restore_canvas(canvas: pg.GraphicsLayoutWidget) -> None:
    """Redonne à la grille de graphiques toute la surface du widget (voir freq_throttle/freq_time)."""
    canvas.ci.setGeometry(canvas.range)


def style_plot(p: pg.PlotItem, left: str = "", bottom: str = "") -> None:
    p.showGrid(x=True, y=True, alpha=theme.current().grid + 0.1)
    p.setMenuEnabled(False)
    if left:
        p.setLabel("left", left)
    if bottom:
        p.setLabel("bottom", bottom)
    for side in ("left", "bottom"):
        ax = p.getAxis(side)
        ax.setZValue(10)
        # graduations principales seulement (grille lisible) et étiquettes des bornes visibles
        ax.setStyle(maxTickLevel=1, hideOverlappingLabels=False)


class CornerText(pg.TextItem):
    """Texte ancré dans un coin de la zone de tracé (indépendant du zoom)."""

    def __init__(self, plot: pg.PlotItem, html: str, corner: str = "tr", margin: int = 6,
                 fill=None) -> None:
        anchor = {"tr": (1, 0), "tl": (0, 0), "br": (1, 1), "bl": (0, 1)}[corner]
        super().__init__(html=html, anchor=anchor, fill=fill)
        self._corner = corner
        self._margin = margin
        self._vb = plot.vb
        self.setParentItem(plot.vb)
        self.setZValue(50)
        plot.vb.sigResized.connect(self._place)
        self._place()

    def _place(self, *_a) -> None:
        r = self._vb.boundingRect()
        x = r.right() - self._margin if self._corner[1] == "r" else r.left() + self._margin
        y = r.top() + self._margin if self._corner[0] == "t" else r.bottom() - self._margin
        self.setPos(x, y)


def html_lines(lines: list[tuple[str, str]], bold: bool = True, size: int = 9) -> str:
    """Bloc HTML de lignes colorées [(texte, couleur)]."""
    w = "600" if bold else "400"
    body = "<br>".join(f'<span style="color:{c}">{t}</span>' for t, c in lines)
    return f'<div style="font-size:{size}pt; font-weight:{w}; text-align:right">{body}</div>'


# ---------------------------------------------------------------- widgets


def combo(items: list[tuple[object, str]], current: int = 0, on_change: Callable | None = None) -> QComboBox:
    """Liste déroulante dont chaque entrée porte une donnée (clé)."""
    c = QComboBox()
    for data, label in items:
        c.addItem(label, data)
    c.setCurrentIndex(current)
    if on_change is not None:
        c.currentIndexChanged.connect(on_change)
    return c


def number(value: float, lo: float, hi: float, step: float = 1.0, decimals: int = 0,
           on_change: Callable | None = None) -> QDoubleSpinBox:
    s = QDoubleSpinBox()
    s.setRange(lo, hi)
    s.setDecimals(decimals)
    s.setSingleStep(step)
    s.setValue(value)
    s.setKeyboardTracking(False)
    if on_change is not None:
        s.valueChanged.connect(on_change)
    return s


class MotorToggles(QWidget):
    """Cases M1/5…M4/8 aux couleurs des moteurs."""

    def __init__(self, on_change: Callable) -> None:
        super().__init__()
        lay = QGridLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        self.boxes: list[QCheckBox] = []
        for i, name in enumerate(MOTOR_LABELS):
            cb = QCheckBox(name)
            cb.setChecked(True)
            cb.setStyleSheet(f"QCheckBox {{ color: {theme.motor_color(i)}; font-weight: 600; }}")
            cb.toggled.connect(on_change)
            lay.addWidget(cb, i // 2, i % 2)
            self.boxes.append(cb)

    def enabled(self) -> list[int]:
        return [i for i, cb in enumerate(self.boxes) if cb.isChecked()]


class OptionsSnapshot:
    """À combiner avec `ToolWindow` : les réglages sont relevés dans le thread de l'interface au
    lancement du calcul (`collect_options`, appelé par `snapshot_options`), puis relus par `compute`
    via `take_options`, jamais depuis les widgets (autre thread). Une demande arrivée pendant un
    calcul est rejouée à la fin par `ToolWindow` (`_pending`)."""

    def collect_options(self) -> dict:
        raise NotImplementedError

    def snapshot_options(self) -> dict:
        return self.collect_options()

    def take_options(self) -> dict:
        return self.run_options

    def run(self, *_a) -> None:  # accepte l'argument d'un signal Qt
        super().run()
