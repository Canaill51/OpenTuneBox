"""Exécution de calculs en arrière-plan pour ne pas figer l'interface."""

from __future__ import annotations

import traceback
from typing import Any, Callable

from PySide6.QtCore import QObject, QRunnable, QThreadPool, Signal


class _Signals(QObject):
    done = Signal(object)
    failed = Signal(str)


class Task(QRunnable):
    def __init__(self, fn: Callable[..., Any], *args: Any, **kwargs: Any) -> None:
        super().__init__()
        self.fn, self.args, self.kwargs = fn, args, kwargs
        self.signals = _Signals()

    def run(self) -> None:  # exécuté dans un thread du pool
        try:
            res = self.fn(*self.args, **self.kwargs)
        except Exception:  # noqa: BLE001
            self.signals.failed.emit(traceback.format_exc())
            return
        self.signals.done.emit(res)


def run_async(fn: Callable[..., Any], *args: Any, on_done: Callable[[Any], None] | None = None,
              on_error: Callable[[str], None] | None = None, **kwargs: Any) -> Task:
    task = Task(fn, *args, **kwargs)
    if on_done:
        task.signals.done.connect(on_done)
    if on_error:
        task.signals.failed.connect(on_error)
    QThreadPool.globalInstance().start(task)
    return task
