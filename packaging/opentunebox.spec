# Recette PyInstaller — à lancer sur chaque système cible (Linux, Windows, macOS) :
#   pip install pyinstaller
#   pyinstaller packaging/opentunebox.spec --noconfirm
# Résultat : dist/OpenTuneBox/ (dossier autonome, sans Python à installer).
import sys
from pathlib import Path

from PyInstaller.utils.hooks import collect_submodules

root = Path(SPECPATH).parent
icon = None
if sys.platform == "win32" and (root / "packaging" / "opentunebox.ico").exists():
    icon = str(root / "packaging" / "opentunebox.ico")
elif sys.platform == "darwin" and (root / "packaging" / "opentunebox.icns").exists():
    icon = str(root / "packaging" / "opentunebox.icns")

a = Analysis(
    [str(root / "packaging" / "launcher.py")],
    pathex=[str(root / "src")],
    datas=[(str(root / "src" / "opentunebox" / "gui" / "assets"), "opentunebox/gui/assets")],
    hiddenimports=collect_submodules("opentunebox") + collect_submodules("pyqtgraph"),
    excludes=["tkinter", "matplotlib", "IPython", "pytest"],
)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, [], exclude_binaries=True, name="OpenTuneBox", console=False, icon=icon)
coll = COLLECT(exe, a.binaries, a.datas, name="OpenTuneBox")
if sys.platform == "darwin":
    app = BUNDLE(coll, name="OpenTuneBox.app", icon=icon, bundle_identifier="org.opentunebox.app")
