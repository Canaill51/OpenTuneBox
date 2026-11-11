#!/bin/sh
# Crochet Claude Code (PreToolUse sur Bash) : demande la confirmation de Cédric pour un envoi, une étiquette ou une action GitHub
# que les règles « ask » de .claude/settings.json laissent passer. Ces règles comparent le DÉBUT de la commande : `git push*`
# ne voit ni `git -C rust push` ni `git -c x=y tag v1`, ni une commande glissée dans `sh -c "…"`. Ce crochet cherche le motif
# partout dans la commande, avec la même expression que check-docs.sh pour les options de git (sans `jq` : absent du poste Windows).
# Il ne bloque rien : il fait seulement poser la question. Faux positif possible (le texte « git push » dans un message de
# commit) : la question est alors posée pour rien, ce qui est le bon côté de l'erreur.
input=$(cat)
if printf '%s' "$input" | grep -Eq 'git( +(-c +[^ ]+|-C +[^ ]+|--[a-z-]+(=[^ ]+)?))* +(push|tag)([^a-z-]|$)|gh +(release|pr +merge|api|workflow +run)([^a-z-]|$)'; then
    printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"Envoi, étiquette ou action GitHub : confirmation de Cédric demandée (crochet guard-git.sh)."}}'
fi
exit 0
