#!/bin/sh
# Installe OpenTuneBox pour l'utilisateur courant et l'ajoute au menu des applications (KDE, GNOME…).
#   sh packaging/linux/install_linux.sh             compile le programme (cargo build --release) puis l'installe
#   sh packaging/linux/install_linux.sh CHEMIN      installe le programme déjà compilé (ex. rust/target/release/opentunebox-gui)
# Rien n'est installé hors de ~/.local : le programme (~/.local/bin/opentunebox-gui), l'icône et le fichier .desktop.
# Pour retirer : supprimer ces trois éléments.
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN_DIR="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
HICOLOR="$HOME/.local/share/icons/hicolor"
# ⚠️ Le SVG vit encore dans l'arborescence Python ; il passe sous rust/otb-gui/assets/ à la phase 7-5 (mettre ce chemin à jour).
SVG="$ROOT/src/opentunebox/gui/assets/opentunebox.svg"
PNG="$ROOT/packaging/linux/opentunebox.png"

if [ $# -ge 1 ]; then
    BIN="$1"
else
    (cd "$ROOT/rust" && cargo build --release -p otb-gui)
    BIN="$ROOT/rust/target/release/opentunebox-gui"
fi
if [ ! -x "$BIN" ]; then
    echo "programme introuvable ou non exécutable : $BIN" >&2
    exit 1
fi

mkdir -p "$BIN_DIR" "$APPS" "$HICOLOR/scalable/apps" "$HICOLOR/256x256/apps"
cp "$BIN" "$BIN_DIR/opentunebox-gui"
# Icône vectorielle, plus un PNG de 256 px : certains menus et barres des tâches n'utilisent que des PNG.
cp "$SVG" "$HICOLOR/scalable/apps/opentunebox.svg"
cp "$PNG" "$HICOLOR/256x256/apps/opentunebox.png"

# Pas de MimeType : il n'existe pas de type standard pour les logs Blackbox (.bbl, .bfl) ; l'association se fait
# par « Ouvrir avec » dans le gestionnaire de fichiers.
cat > "$APPS/opentunebox.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=OpenTuneBox
GenericName=Analyse de logs Blackbox
Comment=Réglage des drones Betaflight : FFT, réponse indicielle, filtres, assistant
Exec="$BIN_DIR/opentunebox-gui" %F
Icon=opentunebox
Terminal=false
Categories=Science;Engineering;
StartupWMClass=OpenTuneBox
Keywords=betaflight;blackbox;pid;drone;fpv;
DESK
update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HICOLOR" 2>/dev/null || true
kbuildsycoca6 2>/dev/null || kbuildsycoca5 2>/dev/null || true
echo "OpenTuneBox installé : $BIN_DIR/opentunebox-gui"
echo "Ajouté au menu : $APPS/opentunebox.desktop"
