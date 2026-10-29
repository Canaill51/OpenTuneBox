"""Garde de l'installateur (D-031, phase 2a) : l'application ne charge ni scipy, ni numba, ni llvmlite.

Ces trois bibliothèques pèsent ≈ 350 Mo une fois installées. Le test tourne dans un sous-processus neuf (le
processus de pytest a déjà importé scipy et numba pour ses tests de référence) : il décode un vrai log, lance
l'assistant, puis chaque outil de l'interface sur un vol synthétique, et liste les modules lourds chargés.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

TESTS = Path(__file__).resolve().parent
LOG = TESTS / "data" / "jeno5_5s.bbl"
HEAVY = ("scipy", "numba", "llvmlite")

SCRIPT = f"""
import json, os, sys
HEAVY = {HEAVY!r}

class Blocker:
    allow = False

    def find_spec(self, name, path=None, target=None):
        if not Blocker.allow and name.split(".")[0] in HEAVY:
            raise ImportError("import interdit : " + name)

sys.meta_path.insert(0, Blocker())
os.environ["QT_QPA_PLATFORM"] = "offscreen"
from PySide6.QtWidgets import QApplication
app = QApplication([])
from opentunebox.cli import main
from opentunebox.gui.app import hard_exit
from opentunebox.gui.state import AppState
from opentunebox.gui.tools import REGISTRY, create_tool

# le vol synthétique est fabriqué avec scipy (aide de test) : bloqueur levé le temps de le fabriquer, puis
# modules lourds purgés et de nouveau interdits (sinon un import tardif de l'application passerait inaperçu)
Blocker.allow = True
from synthetic import make_flightlog
flight = make_flightlog()
Blocker.allow = False
for m in [m for m in sys.modules if m.split(".")[0] in HEAVY]:
    del sys.modules[m]
code = main(["report", {str(LOG)!r}])
st = AppState()
st.add(flight)
for key in sorted(REGISTRY):
    win = create_tool(key, st, None)
    win.auto_run = False
    if hasattr(win, "compute"):
        if win.multi_file:
            win.files.selectAll()
        win.render(win.compute_sync(win.files.selected_entries()))
    win.close()
loaded = sorted({{m.split(".")[0] for m in sys.modules}} & set({HEAVY!r}))
sys.stderr.write("LOADED=" + json.dumps(loaded) + "\\n")
sys.stderr.flush()
hard_exit(code)
"""


def test_application_loads_no_scipy_numba_llvmlite():
    env = {**os.environ, "OTB_BACKEND": "rust", "PYTHONPATH": str(TESTS)}
    env.pop("NUMBA_DISABLE_JIT", None)
    pytest.importorskip("opentunebox._native")
    r = subprocess.run([sys.executable, "-c", SCRIPT], env=env, capture_output=True, text=True, timeout=600)
    lines = [ln for ln in r.stderr.splitlines() if ln.startswith("LOADED=")]
    assert lines, f"le script n'a pas abouti (code {r.returncode}) :\n{r.stderr[-1500:]}"
    assert json.loads(lines[-1][len("LOADED="):]) == [], "module lourd chargé : " + lines[-1]
