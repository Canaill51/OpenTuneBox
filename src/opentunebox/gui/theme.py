"""Couleurs et thèmes de l'interface (palette inspirée de PIDtoolbox)."""

from __future__ import annotations

import pyqtgraph as pg
from PySide6.QtGui import QColor, QPalette
from PySide6.QtWidgets import QApplication

# Couleurs des fichiers comparés (jusqu'à 10), dans l'ordre de PIDtoolbox
FILE_COLORS = [
    "#e6194b", "#f58231", "#e6c619", "#7ccd3a", "#2ec4c4",
    "#2f6fd6", "#1c2f8c", "#9b4fd1", "#d65fa6", "#7f7f7f",
]

# Couleurs des traces du visualiseur
TRACE_COLORS = {
    "gyro_prefilt": "#9a9a9a",
    "gyro": "#111111",
    "pterm": "#2ca02c",
    "iterm": "#1f5fd6",
    "dterm_prefilt": "#8c8c1a",
    "dterm": "#e0b000",
    "fterm": "#a0522d",
    "setpoint": "#e6194b",
    "pid_sum": "#8e44ad",
    "pid_error": "#17becf",
    "throttle": "#111111",
    "debug": "#ff7f0e",
}

# Moteurs 1/5, 2/6, 3/7, 4/8
MOTOR_COLORS = ["#e6194b", "#f58231", "#2f6fd6", "#2ec4c4"]

AXIS_NAMES = ("Roll", "Pitch", "Yaw")


def file_color(i: int) -> str:
    return FILE_COLORS[i % len(FILE_COLORS)]


def motor_color(i: int) -> str:
    return MOTOR_COLORS[i % len(MOTOR_COLORS)]


class Theme:
    def __init__(self, name: str, bg: str, fg: str, grid: float, panel: str, shade: str):
        self.name = name
        self.bg = bg
        self.fg = fg
        self.grid = grid
        self.panel = panel
        self.shade = shade  # zones hors période


THEMES = {
    "clair": Theme("clair", "#ffffff", "#202020", 0.25, "#e8e8e8", (128, 128, 128, 90)),
    "sombre": Theme("sombre", "#1e1f22", "#dcdcdc", 0.2, "#2b2d31", (0, 0, 0, 110)),
}

_current = THEMES["clair"]


def current() -> Theme:
    return _current


def apply(name: str) -> Theme:
    """Applique un thème à pyqtgraph et à la palette Qt."""
    global _current
    _current = THEMES.get(name, THEMES["clair"])
    pg.setConfigOptions(background=_current.bg, foreground=_current.fg, antialias=False)
    app = QApplication.instance()
    if app is not None:
        pal = QPalette()
        if _current.name == "sombre":
            pal.setColor(QPalette.Window, QColor(_current.panel))
            pal.setColor(QPalette.WindowText, QColor(_current.fg))
            pal.setColor(QPalette.Base, QColor("#25272b"))
            pal.setColor(QPalette.AlternateBase, QColor("#2f3136"))
            pal.setColor(QPalette.Text, QColor(_current.fg))
            pal.setColor(QPalette.Button, QColor("#34363b"))
            pal.setColor(QPalette.ButtonText, QColor(_current.fg))
            pal.setColor(QPalette.Highlight, QColor("#2f6fd6"))
            pal.setColor(QPalette.HighlightedText, QColor("#ffffff"))
            app.setPalette(pal)
        else:
            app.setPalette(app.style().standardPalette())
    return _current


def trace_color(key: str) -> str:
    c = TRACE_COLORS.get(key, "#444444")
    if _current.name == "sombre" and c in ("#111111",):
        return "#f0f0f0"
    return c
