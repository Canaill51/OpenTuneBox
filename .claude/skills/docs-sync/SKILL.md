---
name: docs-sync
description: Mise à jour de la documentation d'OpenTuneBox après TOUTE modification (code, interface, analyses, assistant, construction, publication, procédures). À charger avant chaque commit, et dès qu'un changement touche ce que voit l'utilisateur ou ce que décrit un .md (README, guide, AGENTS, CONTRIBUTING, docs/*, CHANGELOG, compétences). Dit quels documents relire selon le type de changement et comment le vérifier.
---

# Documentation toujours à jour

Règle de Cédric : **la documentation suit chaque modification, dans le même commit**. Rien n'est terminé
tant que les documents concernés n'ont pas été relus et corrigés.

## 1. Vérification automatique

```sh
.venv/bin/pytest -q tests/test_docs.py
```

Elle contrôle : liens et ancres entre documents, chemins et fonctions cités (`gui/app.py:hard_exit`),
chaque bouton d'outil dans le README et le guide, chaque module dans `AGENTS.md` / `docs/architecture.md`,
l'index `docs/README.md`, les fichiers publiés (`construire.yml`) dans le README et le guide, les commandes
de `cli.py` dans le README, la langue (français), les numéros des registres (`D-`, `L-`, `W-` : uniques,
et chaque numéro cité existe). Elle tourne aussi avec `pytest`, à chaque commit (crochet
de Claude Code) et sur chaque demande de fusion (GitHub). Un échec = documentation à corriger, **pas** test
à affaiblir. Si un nouveau type d'incohérence apparaît, ajouter le contrôle dans `tests/test_docs.py`.

## 2. Relecture selon le changement

Les tests ne jugent pas le sens du texte. Selon ce qui a changé, relire **en entier** les sections indiquées
et les comparer au code :

| Changement | Documents à relire |
|---|---|
| Interface : bouton, outil, réglage, texte, raccourci, fenêtre | `docs/guide-utilisateur.md` (visualiseur, outil concerné), tableau des outils du `README.md`, `docs/architecture.md` (Interface), captures `docs/images/` si l'aspect change |
| Nouvel outil | tout ce qui précède + étapes de la compétence `opentunebox-dev` (« Ajouter un outil ») |
| Analyse (paramètre, fenêtre, seuil, méthode) | `docs/spec-methodes.md`, `docs/validation.md` (refaire la mesure si besoin), `AGENTS.md` (Pièges connus) |
| Formule ou réglage du firmware | `docs/spec-betaflight.md`, `log/settings.py` / `log/debug_tables.py` cohérents |
| Assistant (règle, seuil, profil) | `docs/assistant.md` (Taille du drone, Calibration, Règles, Origine des seuils), `advisor/profiles.py` (commentaires), compétence `assistant-seuils`, textes du guide intégré `gui/assets/guide/` et `advisor/log_check.py` s'ils citent la valeur |
| Procédure de réglage, enregistrement, outil cité par le guide | guide intégré `gui/assets/guide/` (compétence `guide-integre`), résumé dans `docs/guide-utilisateur.md` §3 et §6 |
| Décodeur | `docs/validation.md` (relancer `scripts/validate_corpus.py`), `docs/pieges-par-domaine.md` |
| Nouveau module ou fichier déplacé | `AGENTS.md` (Organisation), `docs/architecture.md` |
| Construction, installateurs, workflow | `README.md` (Installation, Développement), guide (1. Installer), `CONTRIBUTING.md` (Publier une version), `docs/architecture.md` (Distribution), compétence `opentunebox-dev` (Fabriquer les exécutables) |
| Commande, script, variable d'environnement | `AGENTS.md` (Commandes), `README.md`, `CONTRIBUTING.md` |
| Procédure (branches, commits, publication) | `CONTRIBUTING.md`, `AGENTS.md` (Méthode de travail), `.github/PULL_REQUEST_TEMPLATE.md` |
| Décision de Cédric, choix qui écarte une option, contrat entre deux parties du code | `docs/decisions.md` (nouvelle entrée, ou marque REMPLACÉE / AMENDÉE sur l'ancienne), résumé dans `AGENTS.md` (Décisions fixées) s'il est structurant |
| Piège qui a coûté du temps (échec silencieux, cause contre-intuitive, récidive) | `docs/lecons.md` (entrée complète + index rapide), règle courte avec son numéro dans `AGENTS.md` (Pièges connus) |
| Défaut vu hors du sujet de la branche, limite assumée, idée repoussée ; travail repris ou abandonné | `docs/travaux-reportes.md` (nouvelle entrée ou changement de statut, jamais de suppression) |
| Tout changement visible | `CHANGELOG.md`, section *Non publié* (Ajouté / Modifié / Corrigé) |

Pour une relecture large (avant une version, ou si beaucoup de choses ont changé) : relire **tous** les `.md`
(`git ls-files '*.md'`) et vérifier soi-même chaque valeur chiffrée contre le code (`grep`), sans lancer
d'agents ; au plus un seul agent pour tout le lot si la relecture est vraiment volumineuse.

## 3. Règles d'écriture

- Français partout ; noms de code, réglages Betaflight et formules tels quels.
- Le README et le guide s'adressent aux pilotes : pas de détails de développement hors de la partie
  *Développement* du README. Les noms d'outils sont ceux des boutons (« Spectre », « Filtres »…).
- Un seul fichier de téléchargement par système : ne pas documenter de variantes.
- Décrire ce que fait **notre** code ; citer PIDtoolbox seulement comme référence.
- Ne pas réécrire l'historique des versions publiées du `CHANGELOG.md`.
