"""Icônes Lucide (SVG) colorées selon le thème."""

from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QByteArray, QRectF, Qt
from PySide6.QtGui import QIcon, QPainter, QPixmap
from PySide6.QtSvg import QSvgRenderer

ICON_DIR = Path(__file__).parent / "assets" / "icons"


def icon(name: str, color: str, size: int = 20) -> QIcon:
    """Icône `name` (fichier assets/icons/<name>.svg) dessinée avec la couleur `color`."""
    svg = (ICON_DIR / f"{name}.svg").read_text(encoding="utf-8").replace("currentColor", color)
    renderer = QSvgRenderer(QByteArray(svg.encode("utf-8")))
    ic = QIcon()
    for scale in (1, 2):  # nette aussi sur les écrans haute densité
        pm = QPixmap(size * scale, size * scale)
        pm.fill(Qt.transparent)
        p = QPainter(pm)
        renderer.render(p, QRectF(0, 0, size * scale, size * scale))
        p.end()
        pm.setDevicePixelRatio(scale)
        ic.addPixmap(pm)
    return ic
