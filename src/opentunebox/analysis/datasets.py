"""Jeux de données analysables (équivalent des listes « Gyro, Gyro prefilt, Dterm… » de PIDtoolbox)."""

from __future__ import annotations

import numpy as np

from ..log.flightlog import FlightLog

# clé → libellé affiché
DATASETS: dict[str, str] = {
    "gyro": "Gyro",
    "gyro_prefilt": "Gyro préfiltre",
    "dterm": "Terme D",
    "dterm_prefilt": "Terme D préfiltre",
    "pterm": "Terme P",
    "pid_error": "Erreur PID",
    "setpoint": "Consigne",
    "fterm": "Terme F",
    "pid_sum": "Somme PID",
    "motors": "Moteurs",
    "debug": "Debug",
}

# paires (filtré, préfiltré) pour comparer avant / après filtres
PREFILT_OF = {"gyro": "gyro_prefilt", "dterm": "dterm_prefilt"}


def get(log: FlightLog, key: str) -> np.ndarray | None:
    """Tableau (3, n) — ou (moteurs, n) / (champs debug, n) — pour le jeu de données demandé."""
    if key == "gyro":
        return log.gyro
    if key == "gyro_prefilt":
        return log.gyro_unfilt
    if key == "dterm":
        return log.d_term
    if key == "dterm_prefilt":
        return log.d_term_prefilter
    if key == "pterm":
        return log.p_term
    if key == "pid_error":
        return log.pid_error
    if key == "setpoint":
        return log.setpoint
    if key == "fterm":
        return log.f_term
    if key == "pid_sum":
        return log.pid_sum
    if key == "motors":
        return log.motors if log.n_motors else None
    if key == "debug":
        return log.debug if log.debug.shape[0] else None
    raise KeyError(key)


def available(log: FlightLog) -> list[str]:
    return [k for k in DATASETS if get(log, k) is not None]


def has_axis(log: FlightLog, key: str, axis: int) -> bool:
    """Le terme D n'existe pas sur le yaw (D yaw = 0 par défaut) : on ne l'affiche pas."""
    x = get(log, key)
    if x is None or axis >= x.shape[0]:
        return False
    return bool(np.any(x[axis] != 0))
