"""Couleurs et thèmes de l'interface (palette inspirée de PIDtoolbox)."""

from __future__ import annotations

from pathlib import Path

import pyqtgraph as pg
from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPalette, QPixmap
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
    def __init__(self, name: str, bg: str, fg: str, grid: float, shade: tuple, ui: dict[str, str]):
        self.name = name
        self.bg = bg  # fond des graphiques
        self.fg = fg  # texte et axes
        self.grid = grid
        self.shade = shade  # zones hors période
        self.ui = ui  # couleurs des panneaux Qt
        self.panel = ui["window"]
        self.muted = ui["muted"]  # textes d'aide, secondaires


THEMES = {
    "clair": Theme("clair", "#ffffff", "#1b1b1b", 0.25, (128, 128, 128, 90), {
        "window": "#f3f3f3", "base": "#ffffff", "alt": "#f5f7fa", "button": "#fbfbfb",
        "text": "#1b1b1b", "muted": "#5c5c5c", "disabled": "#9a9a9a", "highlight": "#1f57c3",
        "tooltip": "#ffffe8", "link": "#1f57c3", "light": "#ffffff", "mid": "#c4c4c4", "dark": "#a0a0a0",
    }),
    "sombre": Theme("sombre", "#1e1f22", "#e2e2e2", 0.2, (0, 0, 0, 110), {
        "window": "#2b2d31", "base": "#1e1f22", "alt": "#26282c", "button": "#3a3d43",
        "text": "#e8e8e8", "muted": "#a8a8a8", "disabled": "#6e7177", "highlight": "#3d7be0",
        "tooltip": "#3a3d43", "link": "#7aa7ff", "light": "#4a4d54", "mid": "#55585f", "dark": "#18191c",
    }),
}

# Couleurs des textes mis en valeur (boutons d'outils…), choisies pour rester lisibles sur
# chaque fond (contraste ≥ 4,5:1) : (thème clair, thème sombre).
ACCENTS = {
    "green": ("#137a2c", "#5fd47c"),
    "red": ("#b3261e", "#ff7b70"),
    "blue": ("#1f57c3", "#82acff"),
    "orange": ("#9a5300", "#ffb54d"),
    "purple": ("#7432a0", "#cf9df5"),
}

_current = THEMES["clair"]


def current() -> Theme:
    return _current


def accent(name: str) -> str:
    light, dark = ACCENTS[name]
    return dark if _current.name == "sombre" else light


def accent_style(name: str | None) -> str:
    """Feuille de style d'un bouton mis en valeur (vide si aucun accent)."""
    return f"color:{accent(name)}; font-weight:600" if name else ""


def muted_style(size_pt: float | None = None) -> str:
    """Feuille de style d'un texte d'aide (couleur discrète mais lisible)."""
    size = f"; font-size:{size_pt}pt" if size_pt else ""
    return f"color:{_current.muted}{size}"


def _palette(t: Theme) -> QPalette:
    c = {k: QColor(v) for k, v in t.ui.items()}
    pal = QPalette()
    for role, key in ((QPalette.Window, "window"), (QPalette.WindowText, "text"), (QPalette.Base, "base"),
                      (QPalette.AlternateBase, "alt"), (QPalette.Text, "text"), (QPalette.Button, "button"),
                      (QPalette.ButtonText, "text"), (QPalette.ToolTipBase, "tooltip"),
                      (QPalette.ToolTipText, "text"), (QPalette.PlaceholderText, "muted"),
                      (QPalette.Highlight, "highlight"), (QPalette.Link, "link"), (QPalette.Light, "light"),
                      (QPalette.Midlight, "mid"), (QPalette.Mid, "mid"), (QPalette.Dark, "dark"),
                      (QPalette.Shadow, "dark")):
        pal.setColor(role, c[key])
    pal.setColor(QPalette.HighlightedText, QColor("#ffffff"))
    pal.setColor(QPalette.BrightText, QColor("#ff5050"))
    for role in (QPalette.WindowText, QPalette.Text, QPalette.ButtonText):
        pal.setColor(QPalette.Disabled, role, c["disabled"])
    return pal


def apply(name: str) -> Theme:
    """Applique un thème à pyqtgraph et à l'interface Qt."""
    global _current
    _current = THEMES.get(name, THEMES["clair"])
    pg.setConfigOptions(background=_current.bg, foreground=_current.fg, antialias=False)
    app = QApplication.instance()
    if app is not None:
        # Style « Fusion » sur tous les systèmes : il suit fidèlement la palette ci-dessous. Les styles
        # natifs (Windows 11 surtout) ignorent une partie des couleurs et grisent les textes des boutons.
        if app.style().name().lower() != "fusion":
            app.setStyle("Fusion")
        app.setPalette(_palette(_current))
        app.setStyleSheet(_indicator_style(_current))
    return _current


def _indicator_style(t: Theme) -> str:
    """Cases à cocher et boutons radio bien contrastés.

    Fusion (Qt 6.11) les dessine avec un contour presque invisible, surtout sous Windows.
    """
    u = t.ui
    check = (Path(__file__).parent / "assets" / "coche.svg").as_posix()
    return f"""
QCheckBox::indicator, QRadioButton::indicator {{
    width: 13px; height: 13px; border: 1px solid {u['muted']}; background: {u['base']};
}}
QCheckBox::indicator {{ border-radius: 3px; }}
QRadioButton::indicator {{ border-radius: 7px; }}
QCheckBox::indicator:hover, QRadioButton::indicator:hover {{ border-color: {u['highlight']}; }}
QCheckBox::indicator:checked {{
    background: {u['highlight']}; border-color: {u['highlight']}; image: url("{check}");
}}
QRadioButton::indicator:checked {{ background: #ffffff; border: 4px solid {u['highlight']}; width: 7px; height: 7px; }}
QCheckBox::indicator:disabled, QRadioButton::indicator:disabled {{
    border-color: {u['disabled']}; background: {u['window']};
}}
QCheckBox::indicator:checked:disabled {{ background: {u['disabled']}; }}
"""


def swatch_icon(color: str) -> QIcon:
    """Petit trait de couleur placé devant un libellé (légende des traces)."""
    pm = QPixmap(18, 12)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.fillRect(1, 4, 16, 4, QColor(color))
    p.end()
    return QIcon(pm)


def trace_color(key: str) -> str:
    c = TRACE_COLORS.get(key, "#444444")
    if _current.name == "sombre" and c in ("#111111",):
        return "#f0f0f0"
    return c
