"""Couleurs et thèmes de l'interface (palette inspirée de PIDtoolbox)."""

from __future__ import annotations

import tempfile
from pathlib import Path

import pyqtgraph as pg
from PySide6.QtCore import QEvent, QObject, Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPalette, QPixmap
from PySide6.QtWidgets import QApplication, QComboBox, QStyledItemDelegate

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
    "clair": Theme("clair", "#ffffff", "#1b1b1b", 0.22, (128, 128, 128, 90), {
        "window": "#f3f3f3", "base": "#ffffff", "alt": "#f5f7fa", "button": "#fbfbfb",
        "text": "#1b1b1b", "muted": "#5c5c5c", "disabled": "#9a9a9a", "highlight": "#1f57c3",
        "tooltip": "#ffffe8", "link": "#2b6be0", "hover": "#e4e7ec", "light": "#ffffff", "mid": "#c4c4c4", "dark": "#a0a0a0",
    }),
    "sombre": Theme("sombre", "#1e1f22", "#e2e2e2", 0.18, (0, 0, 0, 110), {
        "window": "#2b2d31", "base": "#1e1f22", "alt": "#26282c", "button": "#3a3d43",
        "text": "#e8e8e8", "muted": "#a8a8a8", "disabled": "#6e7177", "highlight": "#3d7be0",
        "tooltip": "#3a3d43", "link": "#5a92f0", "hover": "#3a3d43", "light": "#4a4d54", "mid": "#55585f", "dark": "#18191c",
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


FONT_SCALE = 1.15  # texte un peu plus grand que la taille du système


class _ComboDelegateFix(QObject):
    """Donne à chaque liste déroulante le délégué standard.

    Le délégué propre aux QComboBox ignore la feuille de style des éléments : en thème clair,
    l'élément survolé s'écrivait en blanc sur fond blanc.
    """

    def eventFilter(self, obj, event):  # noqa: N802
        if event.type() == QEvent.Polish and isinstance(obj, QComboBox) and not obj.property("otb_delegate"):
            obj.setItemDelegate(QStyledItemDelegate(obj))
            obj.setProperty("otb_delegate", True)
        return False


_COMBO_FIX = _ComboDelegateFix()


def apply(name: str) -> Theme:
    """Applique un thème à pyqtgraph et à l'interface Qt."""
    global _current
    _current = THEMES.get(name, THEMES["clair"])
    # antialias : courbes lissées (le sous-échantillonnage « peak » garde l'affichage rapide)
    pg.setConfigOptions(background=_current.bg, foreground=_current.fg, antialias=True)
    app = QApplication.instance()
    if app is not None:
        # Style « Fusion » sur tous les systèmes : il suit fidèlement la palette ci-dessous. Les styles
        # natifs (Windows 11 surtout) ignorent une partie des couleurs et grisent les textes des boutons.
        if app.style().name().lower() != "fusion":
            app.setStyle("Fusion")
        if not app.property("otb_font_scaled"):
            font = app.font()
            font.setPointSizeF(font.pointSizeF() * FONT_SCALE)
            app.setFont(font)
            app.setProperty("otb_font_scaled", True)
        app.setPalette(_palette(_current))
        app.setStyleSheet(_stylesheet(_current))
        if not app.property("otb_combo_fix"):
            app.installEventFilter(_COMBO_FIX)
            app.setProperty("otb_combo_fix", True)
    return _current


def _colored_svg(name: str, color: str) -> str:
    """Copie colorée d'une icône, enregistrée dans le dossier temporaire (les feuilles de style
    Qt ne savent charger une image que depuis un fichier)."""
    out = Path(tempfile.gettempdir()) / "opentunebox-icons" / f"{name}-{color.lstrip('#')}.svg"
    if not out.exists():
        out.parent.mkdir(parents=True, exist_ok=True)
        src = Path(__file__).parent / "assets" / "icons" / f"{name}.svg"
        out.write_text(src.read_text(encoding="utf-8").replace("currentColor", color), encoding="utf-8")
    return out.as_posix()


def _stylesheet(t: Theme) -> str:
    """Apparence « moderne et sobre » : coins arrondis, bordures fines, un seul bouton mis en avant.

    Les cases à cocher et boutons radio sont redessinés : Fusion (Qt 6.11) les dessine avec un
    contour presque invisible, surtout sous Windows.
    """
    u = t.ui
    check = (Path(__file__).parent / "assets" / "coche.svg").as_posix()
    down = _colored_svg("chevron-down", u["muted"])
    up = _colored_svg("chevron-up", u["muted"])
    return f"""
QPushButton {{
    background: {u['button']}; border: 1px solid {u['mid']}; border-radius: 6px; padding: 4px 8px;
}}
QPushButton:hover {{ border-color: {u['highlight']}; }}
QPushButton:pressed {{ background: {u['hover']}; }}
QPushButton:disabled {{ color: {u['disabled']}; border-color: {u['hover']}; }}

QComboBox, QSpinBox, QDoubleSpinBox, QLineEdit {{
    background: {u['base']}; border: 1px solid {u['mid']}; border-radius: 5px; padding: 3px 6px;
}}
QComboBox:hover, QSpinBox:hover, QDoubleSpinBox:hover, QLineEdit:hover,
QComboBox:focus, QSpinBox:focus, QDoubleSpinBox:focus, QLineEdit:focus {{ border-color: {u['highlight']}; }}
QComboBox:disabled, QSpinBox:disabled, QDoubleSpinBox:disabled {{ color: {u['disabled']}; }}
QComboBox::drop-down {{ border: none; width: 22px; }}
QComboBox::down-arrow {{ image: url("{down}"); width: 14px; height: 14px; }}
QComboBox QAbstractItemView {{
    background: {u['base']}; border: 1px solid {u['mid']}; selection-background-color: {u['highlight']};
    selection-color: #ffffff; outline: none;
}}
QComboBox QAbstractItemView::item {{ min-height: 1.7em; padding: 0 6px; color: {u['text']}; border: none; }}
QComboBox QAbstractItemView::item:hover, QComboBox QAbstractItemView::item:selected {{
    background: {u['highlight']}; color: #ffffff;
}}
QSpinBox::up-button, QDoubleSpinBox::up-button, QSpinBox::down-button, QDoubleSpinBox::down-button {{
    border: none; width: 18px;
}}
QSpinBox::up-arrow, QDoubleSpinBox::up-arrow {{ image: url("{up}"); width: 12px; height: 12px; }}
QSpinBox::down-arrow, QDoubleSpinBox::down-arrow {{ image: url("{down}"); width: 12px; height: 12px; }}

QListWidget, QTreeWidget, QTableWidget, QTextBrowser, QTextEdit, QPlainTextEdit {{
    background: {u['base']}; border: 1px solid {u['mid']}; border-radius: 6px;
}}
QGroupBox {{
    border: 1px solid {u['mid']}; border-radius: 8px; margin-top: 1.3em; padding: 6px 4px 4px 4px;
}}
QGroupBox::title {{ subcontrol-origin: margin; left: 10px; padding: 0 4px; color: {u['muted']}; }}

QToolBar {{ background: {u['window']}; border: none; border-bottom: 1px solid {u['mid']}; padding: 4px 6px; spacing: 2px; }}
QToolBar::separator {{ background: {u['mid']}; width: 1px; margin: 4px 6px; }}
QToolBar QToolButton {{ border: none; border-radius: 6px; padding: 4px 8px; }}
QToolBar QToolButton:hover {{ background: {u['hover']}; }}
QToolBar QToolButton:pressed {{ background: {u['mid']}; }}
QToolBar QToolButton#primary {{ background: {u['highlight']}; color: #ffffff; font-weight: 600; }}
QToolBar QToolButton#primary:hover {{ background: {u['link']}; }}

QToolButton#sectionHeader {{
    border: none; border-bottom: 1px solid {u['mid']}; border-radius: 0; padding: 4px 0;
    font-weight: 600; color: {u['text']}; text-align: left;
}}
QLabel#heading {{ color: {u['muted']}; font-weight: 600; padding-top: 6px; }}
QStatusBar {{ color: {u['muted']}; }}
QSplitter::handle {{ background: {u['window']}; }}
QToolTip {{ background: {u['tooltip']}; color: {u['text']}; border: 1px solid {u['mid']}; padding: 4px; }}

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
