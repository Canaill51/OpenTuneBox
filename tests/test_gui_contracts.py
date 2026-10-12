"""Contrats de l'interface vérifiables sans corpus : couches (analysis / advisor sans gui), ressources (icônes,
licence, guide), deux thèmes, calcul en arrière-plan (file d'attente, erreur)."""

import ast
import subprocess
import sys
from pathlib import Path

import pytest
from PySide6.QtGui import QColor

from opentunebox.gui import theme
from opentunebox.gui.state import AppState

SRC = Path(__file__).resolve().parents[1] / "src" / "opentunebox"
NO_GUI = ("analysis", "advisor", "log", "decoder", "cli.py")
FORBIDDEN = ("opentunebox.gui", ".gui", "PySide6", "pyqtgraph")


def test_analysis_and_advisor_never_import_gui():
    """`analysis/` et `advisor/` (et le décodeur, les logs, la CLI) restent utilisables sans Qt (AGENTS.md)."""
    bad = []
    for part in NO_GUI:
        for p in ([SRC / part] if part.endswith(".py") else (SRC / part).rglob("*.py")):
            tree = ast.parse(p.read_text(encoding="utf-8"))
            for node in ast.walk(tree):
                names = []
                if isinstance(node, ast.Import):
                    names = [a.name for a in node.names]
                elif isinstance(node, ast.ImportFrom):
                    names = [("." * node.level) + (node.module or "")]
                for n in names:
                    if n.startswith(FORBIDDEN) or n.split(".")[0] in ("PySide6", "pyqtgraph") or ".gui" in n:
                        bad.append(f"{p.relative_to(SRC)} : {n}")
    assert not bad, bad
    # à l'exécution aussi : importer ces modules ne charge ni Qt ni pyqtgraph (sous-processus, sys.modules vierge)
    code = ("import opentunebox.analysis.spectral, opentunebox.analysis.step, opentunebox.analysis.loop, "
            "opentunebox.analysis.bf_filters, opentunebox.analysis.sliders, opentunebox.analysis.motors, "
            "opentunebox.advisor.rules, opentunebox.advisor.noise, opentunebox.advisor.step_tuning, "
            "opentunebox.advisor.log_check, opentunebox.advisor.tuning_path, opentunebox.advisor.drone_card, "
            "opentunebox.log.flightlog, opentunebox.cli, sys; "
            "bad = sorted(m for m in sys.modules if m.startswith(('opentunebox.gui', 'PySide6', 'pyqtgraph'))); "
            "print(bad); raise SystemExit(1 if bad else 0)")
    r = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, timeout=120)
    assert r.returncode == 0, r.stdout + r.stderr


def test_assets_icons_and_license_exist(app):
    from opentunebox.gui.main_window import MainWindow

    assets = SRC / "gui" / "assets"
    assert (assets / "opentunebox.svg").exists() and (assets / "coche.svg").exists()
    assert (assets / "guide" / "accueil.md").exists()
    icons = assets / "icons"
    assert any(p.name.lower().startswith("license") for p in icons.iterdir()), "licence ISC de Lucide absente"
    svgs = {p.stem for p in icons.glob("*.svg")}
    assert svgs
    # toutes les icônes citées dans gui/ existent (noms littéraux passés à icon(…) ou _colored_svg(…), et TOOLS)
    import re
    cited = set()
    for p in (SRC / "gui").rglob("*.py"):
        src = p.read_text(encoding="utf-8")
        cited |= set(re.findall(r"(?:\bicon|_colored_svg)\(\s*\"([a-z0-9-]+)\"", src))
        # lignes de TOOLS : (clé, libellé, icône, couleur, info-bulle)
        cited |= set(re.findall(r"^\s*\(\"\w+\", \"[^\"]+\", \"([a-z0-9-]+)\", \"[a-z]+\", ", src, re.M))
    missing = sorted(c for c in cited if c not in svgs)
    assert not missing, missing
    # la fenêtre principale charge toutes ses icônes (un fichier manquant lèverait une exception)
    win = MainWindow(AppState())
    win._apply_icons()
    win.close()
    win.deleteLater()
    app.processEvents()


def test_main_window_both_themes_without_log(app):
    from opentunebox.gui.main_window import MainWindow

    win = MainWindow(AppState())
    for name in ("sombre", "clair", "sombre"):
        win._change_theme(name)
        app.processEvents()
        assert theme.current().name == name
        assert win.plots_widget.backgroundBrush().color() == QColor(theme.current().bg)
    win.close()
    win.deleteLater()
    app.processEvents()


def test_theme_change_updates_open_tools(app, synthetic_state):
    """Un changement de thème repeint aussi les outils déjà ouverts (fond du canevas)."""
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.gui.tools import REGISTRY
    from opentunebox.gui.worker import wait_all

    win = MainWindow(synthetic_state)
    win.show()
    for key in ("spectral", "step"):
        win.open_tool(key)
        wait_all()
        app.processEvents()
    for name in ("sombre", "clair"):
        win._change_theme(name)
        wait_all()
        app.processEvents()
        for key in ("spectral", "step"):
            canvas = win.tool_windows[key].canvas
            assert canvas.backgroundBrush().color() == QColor(theme.current().bg), f"{key} en thème {name}"
    assert set(win.tool_windows) <= set(REGISTRY)
    win.close()
    win.deleteLater()
    app.processEvents()


def test_tool_rerun_when_busy_and_compute_error(app, synthetic_state, monkeypatch):
    """Une demande arrivée pendant un calcul est rejouée à la fin (`_pending`) ; une exception dans `compute`
    finit dans la barre d'état, sans planter."""
    from opentunebox.gui.tools import create_tool
    from opentunebox.gui.worker import wait_all

    win = create_tool("freq_time", synthetic_state, None)
    win.auto_run = False
    win._busy = True
    win.run()
    assert win._pending
    win._busy = False
    win._pending = False
    win.run()
    wait_all()
    app.processEvents()
    assert win._last_result is not None and win.status.text() == ""

    def boom(_entries):
        raise RuntimeError("panne simulée")

    monkeypatch.setattr(win, "compute", boom)
    win.run()
    wait_all()
    app.processEvents()
    assert win.status.text().startswith("Erreur de calcul") and "panne simulée" in win.status.text()
    assert not win._busy and win.run_btn.isEnabled()
    win.close()
    win.deleteLater()
    app.processEvents()


@pytest.mark.parametrize("name", ["clair", "sombre"])
def test_every_tool_renders_in_both_themes(app, synthetic_state, name):
    from opentunebox.gui.tools import REGISTRY, create_tool

    theme.apply(name)
    for key in sorted(REGISTRY):
        win = create_tool(key, synthetic_state, None)
        win.auto_run = False
        if hasattr(win, "compute"):
            if win.multi_file:
                win.files.selectAll()
            win.render(win.compute_sync(win.files.selected_entries()))
            assert win.canvas.backgroundBrush().color() == QColor(theme.current().bg), key
        assert not win.grab().isNull()
        win.close()
        win.deleteLater()
        app.processEvents()
    theme.apply("clair")


def test_every_color_lives_in_theme():
    """Toute couleur de l'interface vit dans `gui/theme.py` (L-016) : ailleurs dans `gui/`, aucun code
    `#rrggbb` ni nom de couleur CSS dans une chaîne, aucune couleur numérique, nommée ou en tuple passée à
    QColor / mkPen / mkBrush. Les clés de `theme.ACCENTS` (« green »…) restent permises."""
    import io
    import re
    import tokenize

    hex_code = re.compile(r"#[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3})?(?:[0-9a-fA-F]{2})?(?![0-9a-zA-Z])")
    css_name = re.compile(r"\b(?:color|background)\s*:\s*(?!\{)[a-zA-Z]", re.I)  # « color:white », pas « color:{…} »
    string_types = {tokenize.STRING, getattr(tokenize, "FSTRING_MIDDLE", -1)}
    color_calls = {"QColor", "mkPen", "mkBrush", "setColor", "setBrush", "setPen", "setForeground", "setBackground"}

    def literal(node) -> bool:
        if isinstance(node, ast.Constant):
            return isinstance(node.value, (str, int)) and not isinstance(node.value, bool)
        return isinstance(node, ast.Tuple) and bool(node.elts) and all(literal(e) for e in node.elts)

    bad = []
    for p in sorted((SRC / "gui").rglob("*.py")):
        if p.name == "theme.py":
            continue
        src, rel = p.read_text(encoding="utf-8"), p.relative_to(SRC)
        for tok in tokenize.generate_tokens(io.StringIO(src).readline):
            if tok.type in string_types and (hex_code.search(tok.string) or css_name.search(tok.string)):
                bad.append(f"{rel}:{tok.start[0]} {tok.string.strip()[:70]}")
        for node in ast.walk(ast.parse(src)):
            if not isinstance(node, ast.Call) or not node.args:
                continue
            name = node.func.id if isinstance(node.func, ast.Name) else getattr(node.func, "attr", "")
            if name in color_calls and literal(node.args[0]):
                bad.append(f"{rel}:{node.lineno} {name}({ast.unparse(node.args[0])[:40]}…)")
    assert not bad, "couleur en dur hors de gui/theme.py (L-016) :\n" + "\n".join(bad)
