"""Choix automatique de la période d'analyse (équivalent du bouton « Period » de PIDtoolbox).

On garde le vol utile : du décollage à l'atterrissage, et on s'arrête avant un crash.
Un crash se reconnaît à une erreur PID énorme et durable avec des moteurs saturés (le drone
tourne sans que le pilote l'ait demandé), ou à une rotation hors de portée du gyro.
"""

from __future__ import annotations

import numpy as np

from ..log.flightlog import FlightLog


def auto_period(log: FlightLog, takeoff_throttle: float = 8.0, margin_s: float = 0.5) -> tuple[float, float]:
    """(début, fin) en secondes."""
    t = log.t
    if log.n < 10:
        return float(t[0]), float(t[-1])
    flying = log.armed & (log.throttle > takeoff_throttle)
    if not flying.any():
        return float(t[0]), float(t[-1])
    i0 = int(np.argmax(flying))
    i1 = int(len(flying) - 1 - np.argmax(flying[::-1]))
    crash = crash_time(log)
    t0 = t[i0] + margin_s
    t1 = t[i1] - margin_s
    if crash is not None:
        t1 = min(t1, crash - 0.3)
    if t1 - t0 < 1.0:  # vol trop court : on garde tout
        return float(t[0]), float(t[-1])
    return float(t0), float(t1)


MIN_PART_S = 5.0  # partie stationnaire ou avec mouvements retenue : au moins 5 s


def flight_parts(log: FlightLog, t0: float, t1: float, block_s: float = 1.0,
                 moving_dps: float = 50.0) -> dict[str, tuple[float, float] | None]:
    """Découpe un vol « stationnaire puis mouvements » (vol des filtres du guide) : la plus longue partie
    stationnaire et la plus longue partie avec mouvements entre t0 et t1, chacune d'au moins `MIN_PART_S`.

    Blocs de `block_s` ; un bloc est « en mouvement » si le 95e centile de |consigne| roll / pitch y atteint
    `moving_dps` (même critère que `delay.MOVING_SETPOINT_DPS` sur toute une période), puis lissage par
    majorité sur 3 blocs (un coup de manche isolé ne coupe pas le stationnaire)."""
    sel = log.slice_time(t0, t1)
    i0 = sel.start or 0
    sp = np.max(np.abs(log.setpoint[:2, sel]), axis=0)
    L = max(1, int(round(block_s * log.fs)))
    nb = sp.size // L
    out: dict[str, tuple[float, float] | None] = {"hover": None, "moving": None}
    if nb < 3:
        return out
    moving = np.percentile(sp[: nb * L].reshape(nb, L), 95, axis=1) >= moving_dps
    pad = np.concatenate([moving[:1], moving, moving[-1:]]).astype(int)
    moving = (pad[:-2] + pad[1:-1] + pad[2:]) >= 2
    for key, flag in (("hover", False), ("moving", True)):
        best, start = (0, 0), None
        for k in range(nb + 1):
            inside = k < nb and moving[k] == flag
            if inside and start is None:
                start = k
            elif not inside and start is not None:
                if k - start > best[1] - best[0]:
                    best = (start, k)
                start = None
        a, b = best
        if (b - a) * block_s >= MIN_PART_S:
            out[key] = (float(log.t[i0 + a * L]), float(log.t[min(i0 + b * L, log.n) - 1]))
    return out


def crash_time(log: FlightLog, err_dps: float = 300.0, min_ms: float = 60.0) -> float | None:
    """Instant du premier crash détecté, ou None."""
    err = np.max(np.abs(log.pid_error), axis=0)
    sat = np.any(log.motors >= 98.0, axis=0) if log.n_motors else np.zeros(log.n, bool)
    low = np.any(log.motors <= 8.0, axis=0) if log.n_motors else np.zeros(log.n, bool)
    bad = (err > err_dps) & sat & low
    bad |= np.max(np.abs(log.gyro), axis=0) > 1900.0
    w = max(1, int(min_ms * log.fs / 1000.0))
    if bad.sum() < w:
        return None
    run = np.convolve(bad.astype(float), np.ones(w), mode="valid")
    k = np.nonzero(run >= 0.8 * w)[0]
    if k.size == 0:
        return None
    return float(log.t[k[0]])
