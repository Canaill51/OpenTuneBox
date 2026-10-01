#!/bin/sh
# Ajoute OpenTuneBox au menu des applications (KDE, GNOME…) pour l'utilisateur courant.
# Rien n'est installé hors de ~/.local ; pour retirer : supprimer le fichier .desktop créé.
set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$HOME/.local/share/applications"
HICOLOR="$HOME/.local/share/icons/hicolor"
SVG="$ROOT/src/opentunebox/gui/assets/opentunebox.svg"
mkdir -p "$APPS" "$HICOLOR/scalable/apps"
cp "$SVG" "$HICOLOR/scalable/apps/opentunebox.svg"
# Versions PNG en plus du SVG : certains menus et barres des tâches n'utilisent que des PNG.
QT_QPA_PLATFORM=offscreen "$ROOT/.venv/bin/python" - "$SVG" "$HICOLOR" <<'PY'
import os, sys
from PySide6.QtCore import Qt
from PySide6.QtGui import QGuiApplication, QImage, QPainter
from PySide6.QtSvg import QSvgRenderer
app = QGuiApplication([])
svg, hicolor = sys.argv[1], sys.argv[2]
r = QSvgRenderer(svg)
for n in (16, 24, 32, 48, 64, 128, 256):
    d = os.path.join(hicolor, f"{n}x{n}", "apps")
    os.makedirs(d, exist_ok=True)
    img = QImage(n, n, QImage.Format_ARGB32)
    img.fill(Qt.transparent)
    p = QPainter(img)
    r.render(p)
    p.end()
    img.save(os.path.join(d, "opentunebox.png"))
os._exit(0)
PY
cat > "$APPS/opentunebox.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=OpenTuneBox
GenericName=Analyse de logs Blackbox
Comment=Réglage des drones Betaflight : FFT, réponse indicielle, filtres, assistant
Exec="$ROOT/.venv/bin/opentunebox-gui" %F
Icon=opentunebox
Terminal=false
Categories=Science;Engineering;
StartupWMClass=OpenTuneBox
Keywords=betaflight;blackbox;pid;drone;fpv;
DESK
update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HICOLOR" 2>/dev/null || true
kbuildsycoca6 2>/dev/null || kbuildsycoca5 2>/dev/null || true
echo "OpenTuneBox ajouté au menu : $APPS/opentunebox.desktop"
