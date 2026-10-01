"""Exécution de calculs en arrière-plan pour ne pas figer l'interface."""

from __future__ import annotations

import traceback
from typing import Any, Callable

from PySide6.QtCore import QObject, QRunnable, QThreadPool, Signal

# Tâches en cours : on garde une référence Python jusqu'à la fin, sinon le ramasse-miettes
# pourrait détruire l'objet qui émet le signal de fin pendant que le calcul tourne encore.
_active: set["Task"] = set()


class _Signals(QObject):
    done = Signal(object)
    failed = Signal(str)


class Task(QRunnable):
    def __init__(self, fn: Callable[..., Any], *args: Any, **kwargs: Any) -> None:
        super().__init__()
        self.setAutoDelete(False)
        self.fn, self.args, self.kwargs = fn, args, kwargs
        self.signals = _Signals()
        self.signals.done.connect(self._finish)
        self.signals.failed.connect(self._finish)

    def run(self) -> None:  # exécuté dans un thread du pool
        try:
            res = self.fn(*self.args, **self.kwargs)
        except Exception:  # noqa: BLE001
            self.signals.failed.emit(traceback.format_exc())
            return
        self.signals.done.emit(res)

    def _finish(self, *_args: Any) -> None:
        _active.discard(self)


def run_async(fn: Callable[..., Any], *args: Any, on_done: Callable[[Any], None] | None = None,
              on_error: Callable[[str], None] | None = None, **kwargs: Any) -> Task:
    task = Task(fn, *args, **kwargs)
    if on_done:
        task.signals.done.connect(on_done)
    if on_error:
        task.signals.failed.connect(on_error)
    _active.add(task)
    QThreadPool.globalInstance().start(task)
    return task


def wait_all(timeout_ms: int = 60000) -> None:
    """Attend la fin des calculs (utile pour les tests et à la fermeture)."""
    QThreadPool.globalInstance().waitForDone(timeout_ms)
