#!/bin/sh
# AppImage Linux d'OpenTuneBox : un seul fichier, lancé par double-clic, sans rien installer.
# À lancer après `cargo build --release -p otb-gui` (dans rust/), avec appimagetool (https://github.com/AppImage/appimagetool) :
#   APPIMAGETOOL=chemin/appimagetool-x86_64.AppImage sh packaging/linux/build_appimage.sh rust/target/release/opentunebox-gui dist/OpenTuneBox-Linux.AppImage
# Le programme est un seul fichier : il ne dépend que de la bibliothèque C du système ; le rendu (Vulkan ou OpenGL) et les
# bibliothèques d'affichage (X11, Wayland) sont cherchés sur le système à l'exécution, rien à embarquer.
set -e
if [ $# -ne 2 ]; then
    echo "usage : build_appimage.sh BINAIRE SORTIE.AppImage" >&2
    exit 2
fi
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$1"
OUT="$2"
TOOL="${APPIMAGETOOL:-appimagetool}"
if [ ! -x "$BIN" ]; then
    echo "programme introuvable ou non exécutable : $BIN" >&2
    exit 1
fi
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
APPDIR="$WORK/OpenTuneBox.AppDir"
mkdir -p "$APPDIR/usr/bin"
cp "$BIN" "$APPDIR/usr/bin/OpenTuneBox"

# Point d'entrée : l'AppImage monte son contenu puis lance ce script, qui passe les fichiers à ouvrir.
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/OpenTuneBox" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Pas de MimeType : il n'existe pas de type standard pour les logs Blackbox (voir install_linux.sh).
cat > "$APPDIR/opentunebox.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=OpenTuneBox
GenericName=Analyse de logs Blackbox
Comment=Réglage des drones Betaflight : FFT, réponse indicielle, filtres, assistant
Exec=OpenTuneBox %F
Icon=opentunebox
Terminal=false
Categories=Science;Engineering;
StartupWMClass=OpenTuneBox
EOF
cp "$ROOT/packaging/linux/opentunebox.png" "$APPDIR/opentunebox.png"
cp "$ROOT/packaging/linux/opentunebox.png" "$APPDIR/.DirIcon"

# APPIMAGE_EXTRACT_AND_RUN : appimagetool est lui-même une AppImage, lancée ici sans FUSE (serveurs GitHub).
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT"
