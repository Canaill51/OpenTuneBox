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

# Parties de pyqtgraph jamais utilisées : les exemples de démonstration (les parcourir ouvre une fenêtre,
# ce qui plante sur un serveur sans écran), l'affichage 3D (OpenGL) et l'intégration Jupyter.
PYQTGRAPH_UNUSED = ["pyqtgraph.examples", "pyqtgraph.opengl", "pyqtgraph.jupyter"]


def used_by_app(name):
    return not any(name == mod or name.startswith(mod + ".") for mod in PYQTGRAPH_UNUSED)


a = Analysis(
    [str(root / "packaging" / "launcher.py")],
    pathex=[str(root / "src")],
    datas=[(str(root / "src" / "opentunebox" / "gui" / "assets"), "opentunebox/gui/assets")],
    hiddenimports=collect_submodules("opentunebox") + collect_submodules("pyqtgraph", filter=used_by_app),
    excludes=["tkinter", "matplotlib", "IPython", "pytest", *PYQTGRAPH_UNUSED,
              "PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtPdf", "PySide6.QtVirtualKeyboard"],
)

# Parties de Qt jamais utilisées, ajoutées d'office par PyInstaller : le clavier virtuel pour écran tactile
# (il entraîne tout le moteur QML/Quick), le lecteur PDF et les traductions de Qt (aucune n'est chargée).
# Les noms couvrent les trois systèmes : libQt6Qml.so, Qt6Qml.dll, QtQml.framework…
# Pas d'option strip (informations de débogage) : le strip d'Ubuntu 22.04 abîme les bibliothèques de numpy
# (« ELF load command address/offset not page-aligned »).
UNUSED = re.compile(r"Qt6?(Qml|Quick|VirtualKeyboard|Pdf)|virtualkeyboard|qpdf|[/\\]translations[/\\]", re.I)
a.binaries = [entry for entry in a.binaries if not UNUSED.search(entry[0])]
a.datas = [entry for entry in a.datas if not UNUSED.search(entry[0])]

pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, [], exclude_binaries=True, name="OpenTuneBox", console=False, icon=icon)
coll = COLLECT(exe, a.binaries, a.datas, name="OpenTuneBox")
if sys.platform == "darwin":
    version = re.search(r'__version__ = "([^"]+)"', (root / "src" / "opentunebox" / "__init__.py").read_text()).group(1)
    log_type = "org.opentunebox.blackbox-log"
    app = BUNDLE(
        coll, name="OpenTuneBox.app", icon=icon, bundle_identifier="org.opentunebox.app", version=version,
        info_plist={
            "CFBundleDisplayName": "OpenTuneBox",
            "NSHighResolutionCapable": True,
            # Logs Blackbox .bbl / .bfl : type déclaré par l'application, proposé dans « Ouvrir avec ».
            "UTExportedTypeDeclarations": [{
                "UTTypeIdentifier": log_type,
                "UTTypeDescription": "Log Blackbox Betaflight",
                "UTTypeConformsTo": ["public.data"],
                "UTTypeTagSpecification": {"public.filename-extension": ["bbl", "bfl", "BBL", "BFL"]},
            }],
            "CFBundleDocumentTypes": [{
                "CFBundleTypeName": "Log Blackbox Betaflight",
                "CFBundleTypeRole": "Viewer",
                "LSHandlerRank": "Owner",
                "LSItemContentTypes": [log_type],
            }],
        },
    )
