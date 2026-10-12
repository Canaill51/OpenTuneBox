"""Séries de vols pour le réglage par la réponse indicielle (méthode de Brian White, docs/spec-methodes.md).

Une série = des vols du même drone (mêmes filtres) où un seul curseur change : Damping (équilibre P/D),
Master (niveau des gains), Pitch T. / Pitch D. (équilibre roll / pitch), Stick Resp. (feedforward, en Acro
seulement : le firmware le coupe en mode Angle ; pas de 0,5, choix de Cédric du 2026-10-12). Ce module reconnaît les séries
parmi les vols chargés et relève, pour chaque vol, les conditions du protocole : feedforward à 0 en Acro
ou vol en mode Angle, curseur I bas. Les conseils sont dans `advisor/step_tuning.py`.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

import numpy as np

from ..log.flightlog import ANGLE_MODE_MASK, FlightLog
from ..log.settings import Settings
from .loop import LoopMetrics
from .sliders import PTB_CATEGORIES, slider_view
from .delay import xcorr_delay_ms
from .step import StepMetrics, step_response

FF_KEY = "Stick Resp."  # curseur du feedforward
SERIES_KEYS = ("Damping", "Master", "Pitch T.", "Pitch D.", FF_KEY)  # curseurs dont on lit les séries
ANGLE_SHARE_MIN = 0.9  # vol « en mode Angle » : au moins 90 % de la période analysée


@dataclass
class FlightConditions:
    """Conditions du protocole pour un vol."""
    ff: tuple[int, int]  # gain F roll, pitch
    angle_share: float  # part de la période en mode Angle (nan : modes non enregistrés)
    i_slider: float  # curseur I (« Drift-Wobble »), 1,0 = 100

    @property
    def angle(self) -> bool:
        return math.isfinite(self.angle_share) and self.angle_share >= ANGLE_SHARE_MIN

    @property
    def ff_active(self) -> bool:
        """Feedforward présent dans la réponse : F > 0 hors mode Angle (le firmware l'y met à 0)."""
        return max(self.ff) > 0 and not self.angle


@dataclass
class FlightStep:
    """Un vol vu par le diagnostic : curseurs, filtres, conditions et réponse indicielle roll / pitch."""
    label: str
    sliders: dict[str, float]  # catégorie PTB → fraction (1,0 = 100)
    filters: tuple  # signature des filtres (deux vols comparables ont la même)
    conditions: FlightConditions
    metrics: list[StepMetrics] = field(default_factory=list)  # roll, pitch
    n: list[int] = field(default_factory=list)  # fenêtres retenues
    sp_delay_ms: list[float] = field(default_factory=list)  # retard consigne → gyro
    loops: list[LoopMetrics] = field(default_factory=list)


@dataclass
class Series:
    key: str  # catégorie PTB du curseur qui change
    flights: list[int]  # indices dans la liste des vols, triés par valeur du curseur
    values: list[float]

    @property
    def steps(self) -> list[float]:
        v = sorted(set(self.values))
        return [round(b - a, 3) for a, b in zip(v, v[1:])]


def flight_conditions(log: FlightLog, sel: slice = slice(None)) -> FlightConditions:
    s = log.settings
    ff = (s.pids[0].F, s.pids[1].F)
    share = float("nan")
    if log.mode_flags is not None:
        f = log.mode_flags[sel]
        if f.size:
            share = float(np.mean((f & ANGLE_MODE_MASK) != 0))
    return FlightConditions(ff, share, slider_view(s).values["Drift-Wobble"])


def filter_signature(s: Settings) -> tuple:
    """Réglages de filtres qui changent la réponse : deux vols d'une série doivent les partager."""
    lp = lambda f: (f.static_hz, f.dyn_min_hz, f.dyn_max_hz)  # noqa: E731
    return (lp(s.gyro_lpf1), lp(s.gyro_lpf2), lp(s.dterm_lpf1), lp(s.dterm_lpf2), tuple(s.gyro_notches),
            s.dterm_notch, s.dyn_notch_count, s.dyn_notch_q, s.dyn_notch_min_hz, s.rpm_harmonics, s.rpm_q,
            s.rpm_min_hz, tuple(s.rpm_weights))


def find_series(flights: list[FlightStep]) -> list[Series]:
    """Séries à un seul curseur parmi `flights` : mêmes filtres, mêmes autres curseurs (à 0,01 près)."""
    out = []
    for key in SERIES_KEYS:
        groups: dict[tuple, list[int]] = {}
        for i, f in enumerate(flights):
            others = tuple(round(f.sliders[k], 2) for k in PTB_CATEGORIES if k != key)
            groups.setdefault((f.filters, others), []).append(i)
        for idx in groups.values():
            vals = [round(flights[i].sliders[key], 2) for i in idx]
            if len(set(vals)) < 2:
                continue
            order = sorted(range(len(idx)), key=lambda j: vals[j])
            out.append(Series(key, [idx[j] for j in order], [vals[j] for j in order]))
    return out


def flight_step(log: FlightLog, sel: slice = slice(None), label: str = "") -> FlightStep:
    """Réponse indicielle roll / pitch d'un vol sur `sel`, avec ses curseurs et ses conditions."""
    s = log.settings
    f = FlightStep(label or log.name, slider_view(s).values, filter_signature(s), flight_conditions(log, sel))
    for ax in range(2):
        sp, gy = log.setpoint[ax, sel], log.gyro[ax, sel]
        r = step_response(sp, gy, log.fs, terms=log.pid_terms(ax, sel), breaks=log.breaks(sel))
        f.metrics.append(r.metrics)
        f.n.append(r.n)
        f.sp_delay_ms.append(xcorr_delay_ms(sp, gy, log.fs, 100.0) if np.std(sp) >= 5.0 else float("nan"))
        f.loops.append(r.loop)
    return f
