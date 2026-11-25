# CLAUDE.md

@AGENTS.md

## Spécifique à Claude Code

- Compétences du projet (`.claude/skills/`) : elles se chargent d'après leur description ; charger `opentunebox-dev` avant de modifier
  le décodeur, les analyses ou l'interface.
- **Plusieurs conversations à la fois** : une conversation de sujet commence par « Chantier <sujet> » et travaille dans
  son propre worktree (`EnterWorktree`), jamais dans le dossier principal, réservé au pilotage ; compétence `chantiers`.
- Commits : terminer le message par `Co-Authored-By: Claude <noreply@anthropic.com>` (ou la ligne
  d'attribution fournie par Claude Code).
