#!/bin/sh
# Crochet Claude Code (PreToolUse sur Bash) : avant chaque « git commit », vérifie la documentation
# (tests/test_docs.py). En cas d'échec, le commit est bloqué (code 2) et le message est renvoyé à Claude.
cmd=$(jq -r '.tool_input.command // empty')
case "$cmd" in
    *"git commit"*) ;;
    *) exit 0 ;;
esac
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
[ -x .venv/bin/python ] || exit 0
out=$(.venv/bin/python -m pytest -q tests/test_docs.py 2>&1) && exit 0
echo "$out" | tail -25 >&2
echo "Commit bloqué : la documentation n'est plus cohérente avec le code. Charger la compétence docs-sync," >&2
echo "corriger les documents concernés (pas le test), puis recommencer le commit." >&2
exit 2
