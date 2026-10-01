"""Axes de graphique à grille discrète."""

from __future__ import annotations

import pyqtgraph as pg


class MajorGridAxis(pg.AxisItem):
    """Axe qui ne garde que les graduations principales : la grille n'a plus de petites lignes denses."""

    def tickSpacing(self, minVal, maxVal, size):  # noqa: N802, N803
        return super().tickSpacing(minVal, maxVal, size)[:1]


def major_axes(*names: str) -> dict[str, MajorGridAxis]:
    """Dictionnaire `axisItems` pour `addPlot` (par défaut : gauche, bas, droite)."""
    return {n: MajorGridAxis(n) for n in (names or ("left", "bottom", "right"))}
