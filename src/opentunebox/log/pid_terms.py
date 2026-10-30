"""Termes PID loggés d'un axe (sortis de `analysis/loop.py` : le noyau Rust de `log/` ne dépend pas de `analysis/`, D-035)."""
from __future__ import annotations

from dataclasses import dataclass

import numpy as np


@dataclass
class PidTerms:
    """Termes PID loggés d'un axe sur la période analysée (unités « somme PID »)."""

    p: np.ndarray
    i: np.ndarray
    d: np.ndarray
    f: np.ndarray
    note: str = ""  # non vide : boucle ouverte non mesurable (raison), repli sur la méthode approchée
    instrument: np.ndarray | None = None  # signal extérieur à la boucle pour estimer G ; défaut : la consigne
