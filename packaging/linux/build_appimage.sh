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
# Mentions de licences des composants embarqués (polices, icônes, bibliothèques) : exigées par leurs licences (W-043).
# Fichier produit par `otb-dev licenses`.
mkdir -p "$APPDIR/usr/share/doc/opentunebox"
cp "$ROOT/THIRD-PARTY-LICENSES.txt" "$APPDIR/usr/share/doc/opentunebox/THIRD-PARTY-LICENSES.txt"

# Point d'entrée : l'AppImage monte son contenu puis lance ce script, qui passe les fichiers à ouvrir.
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/OpenTuneBox" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Même nom que l'app_id de la fenêtre (L-076, voir install_linux.sh). Pas de MimeType : il n'existe pas de type standard pour les
# logs Blackbox. ⚠️ L'AppImage n'est connue du bureau que si elle est intégrée (AppImageLauncher…) : lancée telle quelle, la fenêtre
# garde une icône générique sous Wayland.
cat > "$APPDIR/OpenTuneBox.desktop" <<'EOF'
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

# ⚠️ Sans `--runtime-file`, appimagetool 1.9.1 télécharge le « runtime » (la tête exécutable de l'AppImage) depuis la version mouvante
# `continuous` de AppImage/type2-runtime, et ne contrôle que sa taille, aucune empreinte (vérifié dans son code). La CI donne donc un runtime
# de version fixe dont elle a vérifié le SHA-256 : `APPIMAGE_RUNTIME=chemin/runtime-x86_64`. Sans cette variable (essai local), il
# télécharge le sien, comme avant.
set -- --no-appstream
if [ -n "${APPIMAGE_RUNTIME:-}" ]; then
    if [ ! -f "$APPIMAGE_RUNTIME" ]; then
        echo "runtime introuvable : $APPIMAGE_RUNTIME" >&2
        exit 1
    fi
    set -- "$@" --runtime-file "$APPIMAGE_RUNTIME"
fi
# APPIMAGE_EXTRACT_AND_RUN : appimagetool est lui-même une AppImage, lancée ici sans FUSE (serveurs GitHub).
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" "$@" "$APPDIR" "$OUT"
