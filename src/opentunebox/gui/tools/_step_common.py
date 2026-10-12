"""Outils communs à la réponse indicielle et aux curseurs PID.

- calcul de la réponse indicielle d'un log (par axe), avec les métriques fenêtre par fenêtre
  qui servent aux barres d'erreur (dispersion entre segments) ;
- tracé « Curseurs Betaflight » (8 catégories, position 0–2) partagé par les deux fenêtres.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
import pyqtgraph as pg

from ...analysis.delay import xcorr_delay_ms
from ...analysis.sliders import PTB_CATEGORIES, SliderView, slider_view
from ...analysis.step import StepResult, step_response
from ...analysis.step_series import FlightStep, filter_signature, flight_conditions
from ...log.settings import AxisPID
from .. import theme

# Noms des 8 curseurs dans le configurateur Betaflight (locales/en/messages.json, pidTuning*Slider) :
# nom affiché et, dessous, ce que le curseur règle (même ordre que PTB_CATEGORIES)
CATEGORY_BF = {
    "Damping": ("Damping", "D Gains"),
    "Tracking": ("Tracking", "P & I Gains"),
    "Stick Resp.": ("Stick Response", "FF Gains"),
    "Dynamic D.": ("Dynamic Damping", "D Max"),
    "Drift-Wobble": ("Drift - Wobble", "I Gains"),
    "Pitch D.": ("Pitch Damping", "Pitch:Roll D"),
    "Pitch T.": ("Pitch Tracking", "Pitch:Roll P, I & FF"),
    "Master": ("Master Multiplier", ""),
}

# Séparation petits / grands mouvements : (min_input, max_input)
SPLITS = {"tous": (20.0, None), "< 500 °/s": (20.0, 500.0), "≥ 500 °/s": (500.0, None)}


@dataclass
class AxisStep:
    res: StepResult
    seg_peak: np.ndarray  # pic de chaque réponse individuelle
    seg_latency: np.ndarray  # latence 50 % de chaque réponse individuelle (ms)
    sp_delay_ms: float  # retard consigne → gyro (intercorrélation)


@dataclass
class FileStep:
    label: str
    color: str
    family: str
    pids: list[AxisPID]
    sliders: SliderView
    mode: int  # simplified_pids_mode (0 = réglage simplifié désactivé ou absent)
    axes: list[AxisStep | None] = field(default_factory=list)
    flight: FlightStep | None = None  # roll / pitch pour le « Diagnostic du réglage »


def segment_metrics(t_ms: np.ndarray, R: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Pic et latence (50 %) de chaque réponse (vectorisé, même définition que step_metrics)."""
    if R.size == 0:
        return np.zeros(0), np.zeros(0)
    above = R >= 0.5
    has = above.any(axis=1)
    k = np.argmax(above, axis=1)
    rows = np.arange(R.shape[0])
    km1 = np.maximum(k - 1, 0)
    r0, r1 = R[rows, km1], R[rows, k]
    with np.errstate(invalid="ignore", divide="ignore"):
        frac = np.where(r1 != r0, (0.5 - r0) / (r1 - r0), 0.0)
    dt = t_ms[1] - t_ms[0] if t_ms.size > 1 else 1.0
    lat = np.where(k == 0, t_ms[0], t_ms[km1] + frac * dt)
    lat = np.where(has, lat, np.nan)
    # pic dans l'horizon de montée : max(150 ms, 3 × latence)
    horizon = np.where(np.isfinite(lat), np.maximum(150.0, 3.0 * lat), t_ms[-1])
    mask = t_ms[None, :] <= horizon[:, None]
    peak = np.where(mask, R, -np.inf).max(axis=1)
    return peak, lat


def compute_file_step(entry, opts: dict) -> FileStep:
    """Réponse indicielle des 3 axes d'un log sur sa période d'analyse."""
    log, sel = entry.log, entry.sel
    s = log.settings
    sl = s.sliders()
    fs = FileStep(entry.label, entry.color, s.family, s.pids, slider_view(s), int(sl.get("mode", 0) or 0))
    lo, hi = SPLITS.get(opts.get("split", "tous"), (20.0, None))
    for ax in range(3):
        sp = log.setpoint[ax, sel]
        gy = log.gyro[ax, sel]
        res = step_response(sp, gy, log.fs, seg_s=opts.get("seg_s", 2.0), min_input=lo, max_input=hi,
                            smooth=opts.get("smooth", 0), y_correction=opts.get("y_corr", False),
                            terms=log.pid_terms(ax, sel), breaks=log.breaks(sel))
        peak, lat = segment_metrics(res.t_ms, res.responses)
        delay = xcorr_delay_ms(sp, gy, log.fs, 100.0) if np.std(sp) >= 5.0 else float("nan")
        fs.axes.append(AxisStep(res, peak, lat, delay))
    fs.flight = FlightStep(entry.label, fs.sliders.values, filter_signature(s), flight_conditions(log, sel),
                           [a.res.metrics for a in fs.axes[:2]], [a.res.n for a in fs.axes[:2]],
                           [a.sp_delay_ms for a in fs.axes[:2]], [a.res.loop for a in fs.axes[:2]])
    return fs


# ---------------------------------------------------------------- vue « curseurs Betaflight »


def style_plot(p: pg.PlotItem) -> None:
    th = theme.current()
    p.showGrid(x=True, y=True, alpha=th.grid)
    for name in ("left", "bottom"):
        p.getAxis(name).setTextPen(th.fg)
        p.getAxis(name).setPen(th.fg)
    p.setMenuEnabled(False)


def draw_slider_plot(p: pg.PlotItem, files: list[tuple[str, SliderView]],
                     extra: dict[str, float] | None = None, extra_label: str = "") -> None:
    """Positions des 8 curseurs (0–2) : un point par fichier, décalé verticalement s'il y a plusieurs
    fichiers. Point plein = curseurs de l'en-tête ; point creux = estimation depuis les PID.
    `extra` : positions supplémentaires (fractions) dessinées en losange noir (simulation)."""
    p.clear()
    style_plot(p)
    cats = list(PTB_CATEGORIES)
    n = len(cats)
    ticks = [(n - 1 - i, "\n".join(t for t in CATEGORY_BF[c] if t)) for i, c in enumerate(cats)]
    p.getAxis("left").setTicks([ticks])
    p.getAxis("bottom").setTicks([[(x / 10, f"{x / 10:g}") for x in range(0, 21, 2)]])
    p.setLabel("bottom", "Position du curseur")
    p.setXRange(-0.1, 2.1, padding=0)
    p.setYRange(-0.6, n - 0.4, padding=0)
    p.setMouseEnabled(x=False, y=False)
    p.addItem(pg.InfiniteLine(1.0, angle=90, pen=pg.mkPen(theme.current().fg, style=pg.QtCore.Qt.DashLine)))
    nf = max(1, len(files))
    for k, (color, view) in enumerate(files):
        off = 0.0 if nf == 1 else 0.36 * (0.5 - k / (nf - 1))
        xs = [view.values[c] for c in cats]
        ys = [n - 1 - i + off for i in range(n)]
        if view.from_header:
            sc = pg.ScatterPlotItem(xs, ys, size=12, brush=pg.mkBrush(color), pen=pg.mkPen(theme.current().fg, width=0.8))
        else:
            sc = pg.ScatterPlotItem(xs, ys, size=12, brush=pg.mkBrush(None), pen=pg.mkPen(color, width=2.5),
                                    symbol="o")
        p.addItem(sc)
    if extra:
        xs = [extra[c] for c in cats]
        ys = [n - 1 - i for i in range(n)]
        p.addItem(pg.ScatterPlotItem(xs, ys, size=14, symbol="d", brush=pg.mkBrush(None),
                                     pen=pg.mkPen(theme.current().fg, width=2)))
    title = "Curseurs PID Betaflight"
    if extra_label:
        title += f"  ({extra_label})"
    p.setTitle(title, color=theme.current().fg)


def mode_text(mode: int) -> str:
    return {0: "OFF", 1: "RP", 2: "RPY"}.get(mode, "?")


# ---------------------------------------------------------------- mise en page


def wrap_canvas(win):
    """Insère une barre au-dessus (et une zone sous) le canevas d'une ToolWindow, sans toucher à la
    classe de base : le canevas est déplacé dans un conteneur qui prend sa place dans le séparateur.
    Renvoie (barre_du_haut, zone_du_bas) : deux QHBoxLayout / QVBoxLayout vides."""
    from PySide6.QtWidgets import QHBoxLayout, QVBoxLayout, QWidget

    split = win.canvas.parentWidget()
    idx = split.indexOf(win.canvas)
    container = QWidget()
    split.replaceWidget(idx, container)
    win._canvas_container = container  # garder une référence Python (sinon détruit par PySide)
    lay = QVBoxLayout(container)
    lay.setContentsMargins(0, 0, 0, 0)
    lay.setSpacing(2)
    top = QHBoxLayout()
    lay.addLayout(top)
    lay.addWidget(win.canvas, 1)
    win.canvas.show()
    bottom = QVBoxLayout()
    lay.addLayout(bottom)
    split.setStretchFactor(idx, 1)
    return top, bottom
