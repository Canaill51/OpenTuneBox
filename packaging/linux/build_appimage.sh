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
# Licence du projet, puis mentions des composants embarqués (polices, icônes, bibliothèques). La section « Notices » de PolyForm
# Noncommercial oblige à remettre les termes du projet avec toute copie (D-045) ; les licences tierces l'exigent aussi (W-043).
# `THIRD-PARTY-LICENSES.txt` est produit par `otb-dev licenses`.
mkdir -p "$APPDIR/usr/share/doc/opentunebox"
cp "$ROOT/LICENSE" "$APPDIR/usr/share/doc/opentunebox/LICENSE"
cp "$ROOT/THIRD-PARTY-LICENSES.txt" "$APPDIR/usr/share/doc/opentunebox/THIRD-PARTY-LICENSES.txt"

# Point d'entrée : l'AppImage monte son contenu puis lance ce script, qui passe les fichiers à ouvrir.
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
# Intégration au bureau, au premier lancement seulement (W-045) : sans fichier `.desktop` portant l'app_id de la fenêtre, le bureau
# ne connaît pas l'application et affiche une icône générique (le « W » de Wayland, L-076). On en écrit donc un, pointant vers
# l'AppImage elle-même (`$APPIMAGE`, donné par son lanceur), avec son icône. Jamais écrasé ensuite : si le fichier existe déjà
# (AppImage déplacée, ou `install_linux.sh` déjà passé), on n'y touche pas. Échec silencieux : l'application doit démarrer quand même.
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP="$DATA/applications/OpenTuneBox.desktop"
if [ -n "${APPIMAGE:-}" ] && [ ! -e "$DESKTOP" ]; then
    (
        set -e
        mkdir -p "$DATA/applications" "$DATA/icons/hicolor/256x256/apps"
        cp -f "$HERE/opentunebox.png" "$DATA/icons/hicolor/256x256/apps/opentunebox.png"
        # `Exec` doit lancer l'AppImage là où elle est ; les guillemets protègent un chemin à espaces (spécification Desktop Entry).
        awk -v appimage="$APPIMAGE" '/^Exec=/ { print "Exec=\"" appimage "\" %F"; next } { print }' \
            "$HERE/OpenTuneBox.desktop" > "$DESKTOP.tmp"
        mv -f "$DESKTOP.tmp" "$DESKTOP"
    ) || rm -f "$DESKTOP.tmp"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA/applications" >/dev/null 2>&1 || true
fi
exec "$HERE/usr/bin/OpenTuneBox" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Même nom que l'app_id de la fenêtre (L-076, voir install_linux.sh). Pas de MimeType : il n'existe pas de type standard pour les
# logs Blackbox. ⚠️ Ce fichier n'est qu'un modèle à l'intérieur de l'AppImage : le bureau ne le voit pas. C'est `AppRun` qui en
# recopie une version dans le dossier de l'utilisateur au premier lancement (W-045).
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
