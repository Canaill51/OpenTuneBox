"""Sortie de l'application : hard_exit ne doit pas planter sans console."""

import sys

import pytest

from opentunebox.gui import app


def test_hard_exit_without_console(monkeypatch):
    # Exécutable Windows sans console (PyInstaller console=False) : pas de sys.stdout ni sys.stderr.
    monkeypatch.setattr(sys, "stdout", None)
    monkeypatch.setattr(sys, "stderr", None)
    codes = []

    def fake_exit(code):
        codes.append(code)
        raise SystemExit(code)

    monkeypatch.setattr(app.os, "_exit", fake_exit)
    with pytest.raises(SystemExit):
        app.hard_exit(3)
    assert codes == [3]
