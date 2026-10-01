#!/bin/sh
# Ajoute OpenTuneBox au menu des applications (KDE, GNOME…) pour l'utilisateur courant.
# Rien n'est installé hors de ~/.local ; pour retirer : supprimer le fichier .desktop créé.
set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
mkdir -p "$APPS" "$ICONS"
cp "$ROOT/src/opentunebox/gui/assets/opentunebox.svg" "$ICONS/opentunebox.svg"
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
MimeType=application/octet-stream;
Keywords=betaflight;blackbox;pid;drone;fpv;
DESK
update-desktop-database "$APPS" 2>/dev/null || true
echo "OpenTuneBox ajouté au menu : $APPS/opentunebox.desktop"
