"""État partagé de l'application : logs chargés, période d'analyse, curseur temporel.

Toutes les fenêtres d'outils lisent cet état et réagissent à ses signaux, comme dans PIDtoolbox
où la période choisie dans le visualiseur s'applique à tous les outils.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from PySide6.QtCore import QObject, Signal

from ..analysis.period import auto_period
from ..log.flightlog import FlightLog
from .theme import file_color

MAX_LOGS = 10


@dataclass
class LoadedLog:
    log: FlightLog
    period: tuple[float, float]
    color: str
    label: str
    extras: dict = field(default_factory=dict)  # caches de calculs par outil

    @property
    def sel(self) -> slice:
        return self.log.slice_time(*self.period)


class AppState(QObject):
    logs_changed = Signal()
    current_changed = Signal(int)
    period_changed = Signal(int)
    cursor_changed = Signal(float)

    def __init__(self) -> None:
        super().__init__()
        self.logs: list[LoadedLog] = []
        self.current: int = -1
        self.cursor: float = 0.0

    # ------------------------------------------------------------ gestion des logs
    def add(self, log: FlightLog) -> bool:
        if len(self.logs) >= MAX_LOGS:
            return False
        used = {ll.color for ll in self.logs}
        color = next((file_color(i) for i in range(MAX_LOGS) if file_color(i) not in used), file_color(0))
        label = log.name
        names = {ll.label for ll in self.logs}
        k = 2
        while label in names:
            label = f"{log.name} ({k})"
            k += 1
        self.logs.append(LoadedLog(log, auto_period(log), color, label))
        self.logs_changed.emit()
        self.set_current(len(self.logs) - 1)
        return True

    def remove(self, index: int) -> None:
        if 0 <= index < len(self.logs):
            del self.logs[index]
            self.logs_changed.emit()
            self.set_current(min(self.current, len(self.logs) - 1))

    def clear(self) -> None:
        self.logs.clear()
        self.logs_changed.emit()
        self.set_current(-1)

    def set_current(self, index: int) -> None:
        self.current = index
        self.current_changed.emit(index)

    def current_log(self) -> LoadedLog | None:
        if 0 <= self.current < len(self.logs):
            return self.logs[self.current]
        return None

    # ------------------------------------------------------------ période et curseur
    def set_period(self, index: int, t0: float, t1: float) -> None:
        if not (0 <= index < len(self.logs)):
            return
        ll = self.logs[index]
        lo, hi = float(ll.log.t[0]), float(ll.log.t[-1])
        t0, t1 = sorted((max(lo, t0), min(hi, t1)))
        if (t0, t1) != ll.period:
            ll.period = (t0, t1)
            ll.extras.clear()
            self.period_changed.emit(index)

    def reset_period(self, index: int) -> None:
        if 0 <= index < len(self.logs):
            p = auto_period(self.logs[index].log)
            self.set_period(index, *p)

    def set_cursor(self, t: float) -> None:
        self.cursor = t
        self.cursor_changed.emit(t)
