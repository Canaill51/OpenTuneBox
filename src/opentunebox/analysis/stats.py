"""Statistiques de vol et analyse de l'erreur PID."""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from ..log.flightlog import FlightLog


@dataclass
class MeanSd:
    mean: float
    sd: float
    max: float


def _ms(x: np.ndarray) -> MeanSd:
    if x.size == 0:
        return MeanSd(float("nan"), float("nan"), float("nan"))
    return MeanSd(float(np.mean(x)), float(np.std(x)), float(np.max(x)))


@dataclass
class FlightStats:
    duration_s: float
    stick_hist: dict[str, tuple[np.ndarray, np.ndarray]]  # nom → (centres %, % du vol)
    terms: dict[str, list[MeanSd]]  # "gyro", "P", "I", "D", "F" → par axe, valeurs absolues
    motors: list[MeanSd]  # % par moteur
    motor_saturation_pct: float  # % du temps où au moins un moteur est ≥ 98 %
    motor_imbalance_pct: float  # écart du moteur le plus chargé à la moyenne (en vol)
    throttle: MeanSd
    vbat: MeanSd | None
    vbat_min: float | None
    current: MeanSd | None
    mah: float | None
    extras: dict = field(default_factory=dict)


def flight_stats(log: FlightLog, sel: slice | None = None) -> FlightStats:
    sel = sel or slice(None)
    rc = log.rc_command[:, sel]
    thr = log.throttle[sel]
    hist = {}
    edges = np.linspace(-100, 100, 41)
    for i, name in enumerate(("Roll", "Pitch", "Yaw")):
        h, _ = np.histogram(np.clip(rc[i] / 5.0, -100, 100), bins=edges)
        hist[name] = (0.5 * (edges[:-1] + edges[1:]), 100.0 * h / max(1, h.sum()))
    te = np.linspace(0, 100, 41)
    h, _ = np.histogram(np.clip(thr, 0, 100), bins=te)
    hist["Gaz"] = (0.5 * (te[:-1] + te[1:]), 100.0 * h / max(1, h.sum()))

    terms = {
        "gyro": [_ms(np.abs(log.gyro[a, sel])) for a in range(3)],
        "P": [_ms(np.abs(log.p_term[a, sel])) for a in range(3)],
        "I": [_ms(np.abs(log.i_term[a, sel])) for a in range(3)],
        "D": [_ms(np.abs(log.d_term[a, sel])) for a in range(3)],
        "F": [_ms(np.abs(log.f_term[a, sel])) for a in range(3)],
    }
    m = log.motors[:, sel]
    motors = [_ms(m[k]) for k in range(m.shape[0])]
    sat = float(100.0 * np.mean(np.any(m >= 98.0, axis=0))) if m.size else float("nan")
    flying = (log.armed[sel]) & (thr > 5)
    if m.size and flying.sum() > 10:
        means = m[:, flying].mean(axis=1)
        imb = float(100.0 * (means.max() - means.mean()) / max(1e-6, means.mean()))
    else:
        imb = float("nan")

    vb = cur = mah = None
    vmin = None
    if log.vbat is not None and np.any(log.vbat[sel] > 0):
        v = log.vbat[sel]
        vb = _ms(v[v > 0])
        vmin = float(np.min(v[v > 0]))
    if log.current is not None and np.any(log.current[sel] != 0):
        c = log.current[sel]
        cur = _ms(c)
        dt = np.diff(log.t[sel], prepend=log.t[sel][0])
        mah = float(np.sum(c * dt) / 3.6)
    t = log.t[sel]
    return FlightStats(
        duration_s=float(t[-1] - t[0]) if t.size else 0.0,
        stick_hist=hist,
        terms=terms,
        motors=motors,
        motor_saturation_pct=sat,
        motor_imbalance_pct=imb,
        throttle=_ms(thr),
        vbat=vb,
        vbat_min=vmin,
        current=cur,
        mah=mah,
    )


@dataclass
class PidErrorStats:
    edges: np.ndarray  # bornes des histogrammes (°/s)
    hist: list[np.ndarray]  # par axe, % des échantillons
    sd: list[float]  # écart-type robuste de l'erreur par axe (°/s)
    mean_abs: list[float]
    stick_bins: np.ndarray  # centres de déflexion du manche (%)
    err_vs_stick: list[np.ndarray]  # |erreur| moyenne par tranche de déflexion, par axe


def pid_error_stats(log: FlightLog, sel: slice | None = None, max_setpoint: float = 100.0,
                    err_range: float = 50.0) -> PidErrorStats:
    """Distribution de l'erreur PID (gyro − consigne).

    Comme PIDtoolbox, l'histogramme se limite aux instants où |consigne| ≤ 100 °/s (vol
    « calme ») : on y voit l'aptitude à tenir l'attitude (vent, propwash, dérive).
    L'erreur moyenne en fonction de la déflexion du manche montre si le suivi se dégrade
    pendant les grands mouvements.
    """
    sel = sel or slice(None)
    flying = log.armed[sel] & (log.throttle[sel] > 2)
    err = log.pid_error[:, sel][:, flying]
    sp = log.setpoint[:, sel][:, flying]
    rc = np.abs(log.rc_command[:3, sel][:, flying]) / 5.0
    edges = np.linspace(-err_range, err_range, 101)
    hist, sd, ma = [], [], []
    for a in range(3):
        m = np.abs(sp[a]) <= max_setpoint
        e = err[a, m]
        h, _ = np.histogram(e, bins=edges)
        hist.append(100.0 * h / max(1, e.size))
        # σ robuste (1,4826 × écart absolu médian) : insensible aux transitions des figures
        sd.append(float(1.4826 * np.median(np.abs(e - np.median(e)))) if e.size else float("nan"))
        ma.append(float(np.mean(np.abs(e))) if e.size else float("nan"))
    sb = np.linspace(0, 100, 11)
    centers = 0.5 * (sb[:-1] + sb[1:])
    evs = []
    for a in range(3):
        k = np.clip(np.digitize(rc[a], sb) - 1, 0, 9)
        vals = np.full(10, np.nan)
        for i in range(10):
            m = k == i
            if m.sum() > 20:
                vals[i] = float(np.mean(np.abs(err[a, m])))
        evs.append(vals)
    return PidErrorStats(edges, hist, sd, ma, centers, evs)
