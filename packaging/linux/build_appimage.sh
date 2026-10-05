#!/bin/sh
# AppImage Linux d'OpenTuneBox : un seul fichier, lancé par double-clic, sans rien installer.
# À lancer après PyInstaller, avec appimagetool (https://github.com/AppImage/appimagetool) :
#   APPIMAGETOOL=chemin/appimagetool-x86_64.AppImage sh packaging/linux/build_appimage.sh dist/OpenTuneBox dist/OpenTuneBox-Linux.AppImage
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="$1"
OUT="$2"
TOOL="${APPIMAGETOOL:-appimagetool}"
WORK="$(mktemp -d)"
APPDIR="$WORK/OpenTuneBox.AppDir"
mkdir -p "$APPDIR/usr/lib"
cp -a "$DIST" "$APPDIR/usr/lib/opentunebox"

# Point d'entrée : l'AppImage monte son contenu puis lance ce script, qui passe les fichiers à ouvrir.
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/lib/opentunebox/OpenTuneBox" "$@"
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
rm -rf "$WORK"
