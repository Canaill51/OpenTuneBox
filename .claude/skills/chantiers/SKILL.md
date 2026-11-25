---
name: chantiers
description: Travailler à plusieurs conversations en parallèle sur OpenTuneBox, une par sujet (son worktree, sa branche, sa PR), et les piloter depuis le dossier principal. À charger quand Cédric ouvre une conversation par « Chantier <sujet> », quand il demande de piloter ou coordonner les conversations, et avant de prendre un numéro D-, L- ou W-.
---

# Chantiers en parallèle (D-079)

Une conversation = un sujet = un worktree = une branche = une PR. Le **dossier principal** (`~/GitHub/OpenTuneBox`)
reste sur `main`, propre : il est à la conversation de **pilotage**. Raison : deux conversations dans un même dossier
se partagent la branche et les fichiers non validés (2026-11-25, L-125).

## Conversation de chantier (« Chantier <sujet> »)

1. **Premier geste** : `EnterWorktree` avec `name` = le sujet en minuscules et tirets (`fiche-drone`). Il crée
   `.claude/worktrees/<sujet>/` depuis `origin/main` sur la branche `worktree-<sujet>` : la renommer aussitôt selon la
   convention (`git branch -m worktree-<sujet> feat/…`, `CONTRIBUTING.md`).
2. **Rester dans son worktree** : jamais de `cd` vers le dossier principal ni vers un autre chantier, jamais de
   changement de branche ailleurs, jamais de `git stash` nu (la pile est commune à tous les worktrees).
3. **Numéro de registre** : `cargo run -p otb-dev -- numero D "note"` (ou `L`, `W`) **avant** de l'écrire où que ce soit,
   même dans un commentaire. Il voit les registres et le travail en cours de tous les worktrees, et inscrit la réservation.
4. **Premier `cargo`** : le worktree compile tout (≈ 20 Go dans son `rust/target`, quelques minutes) ; ensuite, comme d'habitude.
5. **Fin** : `verify`, commit, puis push et PR avec l'accord de Cédric (règles d'`AGENTS.md`, inchangées). Si `main` a
   avancé entre-temps (un autre chantier fusionné) : `git fetch origin && git rebase origin/main`, `verify`, puis
   demander à Cédric avant le push (une branche déjà poussée et rebasée se pousse avec `--force-with-lease`).
6. **Après la fusion** : `ExitWorktree` avec `remove` (accord de Cédric ; le `rust/target` part avec).

⚠️ Les crochets (`.claude/hooks/`) et `.claude/settings.json` sont lus **dans le dossier principal**
(`$CLAUDE_PROJECT_DIR`) : un crochet changé dans un chantier ne vaut qu'une fois fusionné et le principal mis à jour.

## Pilotage (dossier principal)

- **Voir** : `git worktree list` (chantiers et branches), `ListAgents` (conversations ouvertes),
  `cargo run -p otb-dev -- numero` (numéros réservés), `git -C <worktree> status --short` et
  `git -C <worktree> diff --stat origin/main` (avancement, fichiers touchés).
- **Tableau** : `.git/otb-chantiers.md` (commun à tous les worktrees, jamais dans le dépôt) : sujet | worktree | branche |
  conversation | numéros | état | fichiers partagés avec un autre chantier. Le relire à chaque reprise, le tenir à jour.
- **Recouvrements** : deux chantiers qui touchent le même fichier (souvent `app.rs`, `profiles.rs`, les registres) : le
  dire à Cédric et proposer l'ordre de fusion.
- **Fusions une à une** : après chaque fusion, `git pull --ff-only` dans le principal, puis prévenir les autres
  conversations (`SendMessage`) de repartir du nouveau `main` (point 5).
- **Prévenir sans déranger** : `SendMessage` avec une première ligne qui se suffit ; `notify_when_idle` pour savoir
  quand une conversation a fini son tour. Jamais de « tu as fini ? » répété, jamais d'agent lancé automatiquement.
- **Ménage** : un worktree dont la PR est fusionnée se supprime (`git worktree remove`, accord de Cédric).
