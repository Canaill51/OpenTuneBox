"""Petits widgets : position des manches au curseur (comme le lecteur de PIDtoolbox)."""

from __future__ import annotations

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPen
from PySide6.QtWidgets import QWidget


class StickWidget(QWidget):
    """Carré avec un point : x, y dans [−1, 1] (y = +1 en haut)."""

    def __init__(self, x_label: str, y_label: str, parent=None) -> None:
        super().__init__(parent)
        self.x_label, self.y_label = x_label, y_label
        self.x = 0.0
        self.y = 0.0
        self.setMinimumSize(90, 90)

    def set_position(self, x: float, y: float) -> None:
        self.x = max(-1.0, min(1.0, x))
        self.y = max(-1.0, min(1.0, y))
        self.update()

    def paintEvent(self, _event) -> None:  # noqa: N802
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        side = min(self.width(), self.height()) - 18
        ox = (self.width() - side) / 2 + 6
        oy = 4
        fg = self.palette().windowText().color()
        p.setPen(QPen(fg, 1))
        p.drawRect(int(ox), int(oy), int(side), int(side))
        mid = QColor(fg)
        mid.setAlpha(80)
        p.setPen(QPen(mid, 1))
        p.drawLine(QPointF(ox + side / 2, oy), QPointF(ox + side / 2, oy + side))
        p.drawLine(QPointF(ox, oy + side / 2), QPointF(ox + side, oy + side / 2))
        cx = ox + (self.x + 1) / 2 * side
        cy = oy + (1 - (self.y + 1) / 2) * side
        p.setPen(QPen(fg, 2))
        p.drawEllipse(QPointF(cx, cy), 5, 5)
        p.setPen(QPen(fg, 1))
        p.drawText(int(ox - 12), int(oy + side / 2 + 4), self.y_label)
        p.drawText(int(ox + side / 2 - 4), int(oy + side + 14), self.x_label)
        p.end()


def colored_label_html(text: str, color: str) -> str:
    return f'<span style="color:{color}; font-weight:600">{text}</span>'


ALIGN_CENTER = Qt.AlignCenter
