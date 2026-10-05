# Recette PyInstaller — à lancer sur chaque système cible (Linux, Windows, macOS) :
#   pip install pyinstaller
#   pyinstaller packaging/opentunebox.spec --noconfirm
# Résultat : dist/OpenTuneBox/ (dossier autonome, sans Python à installer) ; sous macOS, aussi dist/OpenTuneBox.app.
import re
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
    excludes=["tkinter", "matplotlib", "IPython", "pytest",
              "PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtPdf", "PySide6.QtVirtualKeyboard"],
)

# Parties de Qt jamais utilisées, ajoutées d'office par PyInstaller : le clavier virtuel pour écran tactile
# (il entraîne tout le moteur QML/Quick), le lecteur PDF et les traductions de Qt (aucune n'est chargée).
# Les noms couvrent les trois systèmes : libQt6Qml.so, Qt6Qml.dll, QtQml.framework…
UNUSED = re.compile(r"Qt6?(Qml|Quick|VirtualKeyboard|Pdf)|virtualkeyboard|qpdf|[/\\]translations[/\\]", re.I)
a.binaries = [entry for entry in a.binaries if not UNUSED.search(entry[0])]
a.datas = [entry for entry in a.datas if not UNUSED.search(entry[0])]

# Linux : retire les informations de débogage des bibliothèques (−30 Mo environ). Pas sous macOS, où cela
# invaliderait leurs signatures, ni sous Windows, où l'outil n'existe pas.
strip = sys.platform.startswith("linux")

pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, [], exclude_binaries=True, name="OpenTuneBox", console=False, icon=icon, strip=strip)
coll = COLLECT(exe, a.binaries, a.datas, name="OpenTuneBox", strip=strip)
if sys.platform == "darwin":
    app = BUNDLE(coll, name="OpenTuneBox.app", icon=icon, bundle_identifier="org.opentunebox.app")
