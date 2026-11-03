#!/bin/sh
# Installe OpenTuneBox pour l'utilisateur courant et l'ajoute au menu des applications (KDE, GNOME…).
#   sh packaging/linux/install_linux.sh             compile le programme (cargo build --release) puis l'installe
#   sh packaging/linux/install_linux.sh CHEMIN      installe le programme déjà compilé (ex. rust/target/release/opentunebox-gui)
# Rien n'est installé hors de ~/.local : le programme (~/.local/bin/opentunebox-gui), l'icône et le fichier .desktop.
# Pour retirer : supprimer ces éléments (programme, icônes, OpenTuneBox.desktop).
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
# Copie à côté puis remplacement d'un coup : `cp` directement sur le programme échoue (« Fichier texte occupé ») quand OpenTuneBox est
# ouvert, ce qui est le cas d'une mise à jour ; `mv` remplace le fichier même s'il tourne (l'ancienne fenêtre garde l'ancien programme).
cp "$BIN" "$BIN_DIR/.opentunebox-gui.new"
mv -f "$BIN_DIR/.opentunebox-gui.new" "$BIN_DIR/opentunebox-gui"
# Icône vectorielle, plus un PNG de 256 px : certains menus et barres des tâches n'utilisent que des PNG.
cp "$SVG" "$HICOLOR/scalable/apps/opentunebox.svg"
cp "$PNG" "$HICOLOR/256x256/apps/opentunebox.png"
# Les versions du script d'avant la phase 7-4 posaient aussi des PNG de 16 à 128 px (rendus par Qt) : ils masqueraient l'icône actuelle.
for n in 16 24 32 48 64 128; do rm -f "$HICOLOR/${n}x${n}/apps/opentunebox.png"; done

# ⚠️ Le fichier s'appelle OpenTuneBox.desktop, comme l'app_id de la fenêtre, majuscules comprises : sous Wayland (KDE), c'est ce nom
# qui fait trouver l'icône (avec opentunebox.desktop, la fenêtre garde une icône générique : L-076).
# Pas de MimeType : il n'existe pas de type standard pour les logs Blackbox (.bbl, .bfl) ; l'association se fait
# par « Ouvrir avec » dans le gestionnaire de fichiers.
rm -f "$APPS/opentunebox.desktop" # ancien nom (versions du script d'avant la phase 7-4) : éviter un doublon dans le menu
cat > "$APPS/OpenTuneBox.desktop" <<DESK
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
echo "Ajouté au menu : $APPS/OpenTuneBox.desktop"
