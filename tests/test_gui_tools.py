"""Contrat commun des pages d'outils (ToolWindow), vérifié sur chaque outil du registre, sans log ni corpus."""

import ast
import inspect
import os
import textwrap

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import pytest  # noqa: E402
from PySide6.QtCore import QObject  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from opentunebox.gui.state import AppState  # noqa: E402
from opentunebox.gui.tools import REGISTRY, create_tool  # noqa: E402


@pytest.fixture(scope="module")
def app():
    return QApplication.instance() or QApplication([])


def _widget_attrs(win) -> set[str]:
    """Attributs de l'instance qui sont (ou contiennent) des objets Qt : interdits depuis le thread de calcul."""
    out = set()
    for k, v in vars(win).items():
        if k in ("state", "main_window"):
            continue
        items = list(v.values()) if isinstance(v, dict) else list(v) if isinstance(v, (list, tuple)) else [v]
        if any(isinstance(x, QObject) for x in items):
            out.add(k)
    return out


def _self_attrs_reached(cls, method: str, seen: set[str]) -> set[str]:
    """Attributs `self.x` lus par `method` et, récursivement, par les méthodes `self.m()` qu'elle appelle."""
    fn = getattr(cls, method, None)
    if fn is None or method in seen or not inspect.isfunction(fn):
        return set()
    seen.add(method)
    tree = ast.parse(textwrap.dedent(inspect.getsource(fn)))
    attrs = set()
    for n in ast.walk(tree):
        if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id == "self":
            attrs.add(n.attr)
    for n in ast.walk(tree):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and isinstance(n.func.value, ast.Name) \
                and n.func.value.id == "self":
            attrs |= _self_attrs_reached(cls, n.func.attr, seen)
    return attrs


@pytest.mark.parametrize("key", sorted(REGISTRY))
def test_compute_reads_no_widget(app, key):
    """`compute` tourne dans un autre thread : il ne lit aucun widget, directement ou par une méthode appelée ;
    les valeurs passent par `snapshot_options` → `run_options` (AGENTS.md)."""
    win = create_tool(key, AppState(), None)
    assert win is not None
    if not hasattr(win, "compute"):
        pytest.skip(f"{key} : page sans calcul en arrière-plan")
    widgets = _widget_attrs(win)
    reached = _self_attrs_reached(type(win), "compute", set())
    assert not (reached & widgets), f"{key}.compute lit des widgets : {sorted(reached & widgets)}"
    win.deleteLater()
    app.processEvents()
