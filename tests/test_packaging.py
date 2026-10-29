"""Recette PyInstaller et lanceur, sans construire : chemins cités existants, filtres des parties de Qt et de
pyqtgraph écartées (le français de base est gardé), point d'entrée importable."""

import ast
import re
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
SPEC = ROOT / "packaging" / "opentunebox.spec"


def _spec_namespace():
    """Exécute les seules parties de la recette qui ne dépendent pas de PyInstaller : constantes, regex `UNUSED`
    et fonction `used_by_app` (la recette elle-même attend `Analysis`, `SPECPATH`…)."""
    tree = ast.parse(SPEC.read_text(encoding="utf-8"), str(SPEC))
    keep = [n for n in tree.body if isinstance(n, ast.FunctionDef)
            or (isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name)
                and n.targets[0].id in ("PYQTGRAPH_UNUSED", "UNUSED"))]
    ns = {"re": re, "sys": sys}
    exec(compile(ast.Module(keep, type_ignores=[]), str(SPEC), "exec"), ns)
    return tree, ns


def test_spec_paths_exist():
    """Chaque chemin `root / "…" / "…"` de la recette existe dans le dépôt (lanceur, assets, icônes, version)."""
    tree, _ = _spec_namespace()
    paths = []
    for node in ast.walk(tree):
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div) and isinstance(node.right, ast.Constant):
            parts, cur = [node.right.value], node.left
            while isinstance(cur, ast.BinOp) and isinstance(cur.right, ast.Constant):
                parts.insert(0, cur.right.value)
                cur = cur.left
            if isinstance(cur, ast.Name) and cur.id == "root":
                paths.append(Path(*parts))
    assert {"packaging/launcher.py", "src/opentunebox/gui/assets", "packaging/opentunebox.ico",
            "packaging/opentunebox.icns", "src/opentunebox/__init__.py", "src"} <= {p.as_posix() for p in paths}
    missing = [p.as_posix() for p in paths if not (ROOT / p).exists()]
    assert not missing, f"chemins cités par la recette absents du dépôt : {missing}"


def test_spec_filters_unused_qt_and_pyqtgraph_parts():
    _, ns = _spec_namespace()
    unused, used_by_app = ns["UNUSED"], ns["used_by_app"]
    # écartés sur les trois systèmes : QML / Quick, clavier virtuel, PDF, traductions
    for name in ("PySide6/Qt/lib/libQt6Qml.so.6", "PySide6\\Qt6Quick.dll", "QtVirtualKeyboard.framework/Versions/A",
                 "PySide6/Qt/plugins/platforminputcontexts/libqtvirtualkeyboardplugin.so", "PySide6/Qt/lib/libQt6Pdf.so.6",
                 "PySide6/Qt/translations/qtbase_de.qm"):
        assert unused.search(name), name
    for name in ("PySide6/Qt/lib/libQt6Core.so.6", "PySide6/Qt6Widgets.dll", "PySide6/Qt/plugins/platforms/libqxcb.so",
                 "numpy/_core/_multiarray_umath.so"):
        assert not unused.search(name), name
    # même filtre que la recette sur les données : seule la traduction française de base reste
    datas = ["PySide6/Qt/translations/qtbase_fr.qm", "PySide6/Qt/translations/qtbase_de.qm", "opentunebox/gui/assets/x.svg"]
    kept = [d for d in datas if d.endswith("qtbase_fr.qm") or not unused.search(d)]
    assert kept == ["PySide6/Qt/translations/qtbase_fr.qm", "opentunebox/gui/assets/x.svg"]
    assert ns["PYQTGRAPH_UNUSED"] == ["pyqtgraph.examples", "pyqtgraph.opengl", "pyqtgraph.jupyter"]
    assert used_by_app("pyqtgraph.graphicsItems.PlotItem") and used_by_app("pyqtgraph.examplesx")
    assert not used_by_app("pyqtgraph.examples") and not used_by_app("pyqtgraph.opengl.GLViewWidget")


def test_spec_excludes_and_hidden_imports_name_real_packages():
    """Les modules écartés ou forcés sont écrits en clair : vérifier qu'ils sont bien ceux du projet."""
    text = SPEC.read_text(encoding="utf-8")
    assert 'collect_submodules("opentunebox", filter=' in text and 'collect_submodules("pyqtgraph", filter=used_by_app)' in text
    # scipy, numba et llvmlite (≈ 350 Mo) restent hors de l'exécutable, ancien décodeur numba compris (D-032)
    excludes = text.split("excludes=[")[1].split("]")[0]
    for name in ("scipy", "numba", "llvmlite", "opentunebox.decoder.core"):
        assert f'"{name}"' in excludes, name
    assert 'n != "opentunebox.decoder.core"' in text  # sinon la recette force ce que `excludes` écarte
    assert "console=False" in text  # exécutable Windows sans console : sys.stdout vaut None (L-021)
    assert "strip" not in text.split("UNUSED = ")[1].split("\n")[0]  # pas d'option strip (voir en-tête de la recette)


def test_launcher_entry_point():
    import importlib.util

    from opentunebox.gui.app import main

    spec = importlib.util.spec_from_file_location("launcher", ROOT / "packaging" / "launcher.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)  # importe opentunebox.gui.app.main sans lancer l'application
    assert mod.main is main
