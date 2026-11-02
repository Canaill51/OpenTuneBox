#!/bin/sh
# Crochet Claude Code (PreToolUse sur Bash) : avant chaque « git commit », vérifie la documentation
# (tests/test_docs.py, puis `cargo test -p otb-dev --test docs`). En cas d'échec, le commit est bloqué (code 2) et le message est renvoyé à Claude.
cmd=$(jq -r '.tool_input.command // empty')
case "$cmd" in
    *"git commit"* | *git\ -c\ *commit*) ;;
    *) exit 0 ;;
esac
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
# Les deux suites (Python jusqu'à la phase 7, puis Rust seule). Le délai du crochet est de 60 s (.claude/settings.json) : la suite Rust
# est donc coupée à 50 s, avec un avertissement visible (le commit passe alors sans ce contrôle ; le lancer à la main).
failed=0
out=""
if [ -x .venv/bin/python ]; then
    out=$(.venv/bin/python -m pytest -q tests/test_docs.py 2>&1) || failed=1
fi
if [ "$failed" = 0 ] && [ -d rust/otb-dev ]; then
    if command -v cargo >/dev/null 2>&1; then
        out=$(cd rust && timeout 50 cargo test -q -p otb-dev --test docs 2>&1)
        status=$?
        if [ "$status" = 124 ]; then
            echo "Avertissement : la vérification Rust de la documentation a dépassé 50 s (compilation à froid ?) et n'a pas été faite." >&2
            echo "Lancer : (cd rust && cargo test -p otb-dev --test docs)" >&2
        elif [ "$status" != 0 ]; then
            failed=1
        fi
    else
        echo "Avertissement : cargo introuvable, la vérification Rust de la documentation n'a pas été faite." >&2
    fi
fi
[ "$failed" = 0 ] && exit 0
echo "Commit bloqué : la documentation n'est plus cohérente avec le code. Charger la compétence docs-sync," >&2
echo "corriger les documents concernés (pas le test), puis recommencer le commit." >&2
exit 2
