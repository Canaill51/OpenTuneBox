#!/bin/sh
# Image disque macOS d'OpenTuneBox : fenêtre avec l'application et un raccourci vers « Applications »
# (l'utilisateur glisse l'une sur l'autre). À lancer sur un Mac après PyInstaller :
#   sh packaging/macos/build_dmg.sh dist/OpenTuneBox.app dist/OpenTuneBox-macOS.dmg
set -e
APP="$1"
OUT="$2"
STAGE="$(mktemp -d)"
# ditto garde les liens symboliques et les attributs des frameworks Qt (cp peut les abîmer).
ditto "$APP" "$STAGE/OpenTuneBox.app"
ln -s /Applications "$STAGE/Applications"
rm -f "$OUT"
# hdiutil échoue parfois au hasard (« Resource busy ») sur les serveurs de GitHub : quelques essais.
for essai in 1 2 3 4 5; do
    if hdiutil create -volname OpenTuneBox -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$OUT"; then
        rm -rf "$STAGE"
        exit 0
    fi
    echo "hdiutil a échoué (essai $essai), nouvel essai dans 5 s…"
    sleep 5
done
exit 1
