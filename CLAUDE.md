# CLAUDE.md

@AGENTS.md

## Spécifique à Claude Code

- Compétence du projet : `.claude/skills/opentunebox-dev` (procédures de validation sur le corpus,
  recoupement avec PIDtoolbox, ajout d'un outil, fabrication des exécutables). La charger avant de
  modifier le décodeur, les analyses ou l'interface.
- Assistant de réglage : compétence `.claude/skills/assistant-seuils` (calibrer, choisir et sourcer chaque
  seuil). La charger avant de modifier un seuil, une règle ou un profil de taille.
- Commits : terminer le message par `Co-Authored-By: Claude <noreply@anthropic.com>` (ou la ligne
  d'attribution fournie par Claude Code).
- Fichiers temporaires (captures, scripts d'essai) : dans le répertoire de travail temporaire de la
  session, jamais dans le dépôt.
