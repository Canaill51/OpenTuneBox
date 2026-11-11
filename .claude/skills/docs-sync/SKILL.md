---
name: docs-sync
description: Mise à jour de la documentation d'OpenTuneBox après TOUTE modification (code, interface, analyses, assistant, construction, publication, procédures). À charger avant chaque commit, et dès qu'un changement touche ce que voit l'utilisateur ou ce que décrit un .md (README, guide, AGENTS, CONTRIBUTING, docs/*, CHANGELOG, compétences). Dit quels documents relire selon le type de changement et comment le vérifier.
---

# Documentation toujours à jour

Règle de Cédric : **la documentation suit chaque modification, dans le même commit**. Rien n'est terminé
tant que les documents concernés n'ont pas été relus et corrigés.

## 1. Vérification automatique

```sh
(cd rust && cargo test -p otb-dev --test docs)   # un nouveau contrôle s'ajoute dans rust/otb-dev/src/docs.rs, avec sa preuve dans rust/otb-dev/tests/checks.rs
```

Elle contrôle : liens et ancres entre documents, chemins et fonctions cités (`rust/otb-gui/src/app.rs:App`),
chaque bouton d'outil dans le README et le guide, chaque module dans `AGENTS.md` / `docs/architecture.md`,
l'index `docs/README.md`, les fichiers publiés (`construire.yml`) dans le README et le guide, la concordance des seuils que la documentation recopie avec leur constante Rust, les commandes
de la CLI (`rust/otb-cli`) dans le README, la langue (français), les numéros des registres (`D-`, `L-`, `W-` : uniques,
et chaque numéro cité existe). Elle tourne à chaque commit (crochet
de Claude Code) et sur chaque demande de fusion (GitHub). Un échec = documentation à corriger, **pas** test
à affaiblir. Si un nouveau type d'incohérence apparaît, ajouter le contrôle dans `rust/otb-dev/src/docs.rs`.
Seuls les registres d'histoire (`HISTORICAL_DOCS`) peuvent citer des fichiers de l'ancienne version Python.

## 2. Relecture selon le changement

Les tests ne jugent pas le sens du texte. Selon ce qui a changé, relire **en entier** les sections indiquées
et les comparer au code :

| Changement | Documents à relire |
|---|---|
| Interface : bouton, outil, réglage, texte, raccourci, fenêtre | `docs/guide-utilisateur.md` (visualiseur, outil concerné), tableau des outils du `README.md`, `docs/architecture.md` (Interface), captures `docs/images/` si l'aspect change |
| Nouvel outil | tout ce qui précède + étapes de la compétence `opentunebox-dev` (« Ajouter un outil ») |
| Analyse (paramètre, fenêtre, seuil, méthode) | `docs/spec-methodes.md`, `docs/validation.md` (refaire la mesure si besoin), `AGENTS.md` (Pièges connus) |
| Formule ou réglage du firmware | `docs/spec-betaflight.md`, `rust/otb-log/src/settings.rs` / `debug_tables.rs` cohérents |
| Assistant (règle, seuil, profil) | `docs/assistant.md` (Taille du drone, Calibration, Règles, Origine des seuils), `rust/otb-advisor/src/profiles.rs` (commentaires), compétence `assistant-seuils`, textes du guide intégré `rust/otb-gui/assets/guide/` et `rust/otb-advisor/src/log_check.rs` s'ils citent la valeur |
| Procédure de réglage, enregistrement, outil cité par le guide | guide intégré `rust/otb-gui/assets/guide/` (compétence `guide-integre`), résumé dans `docs/guide-utilisateur.md` §3 et §6 |
| Décodeur | `docs/validation.md` (relancer `otb-dev corpus validate`), `docs/pieges-par-domaine.md` |
| Nouveau module ou fichier déplacé | `AGENTS.md` (Organisation), `docs/architecture.md` |
| Construction, installateurs, workflow | `README.md` (Installation, Développement), guide (1. Installer), `CONTRIBUTING.md` (Publier une version), `docs/architecture.md` (Distribution), compétence `opentunebox-dev` (Fabriquer les exécutables) |
| Commande, script, variable d'environnement | `AGENTS.md` (Commandes), `README.md`, `CONTRIBUTING.md` |
| Procédure (branches, commits, publication) | `CONTRIBUTING.md`, `AGENTS.md` (Méthode de travail), `.github/PULL_REQUEST_TEMPLATE.md` |
| Décision de Cédric, choix qui écarte une option, contrat entre deux parties du code | `docs/decisions.md` (nouvelle entrée, ou marque REMPLACÉE / AMENDÉE sur l'ancienne), résumé dans `AGENTS.md` (Décisions fixées) s'il est structurant |
| Piège qui a coûté du temps (échec silencieux, cause contre-intuitive, récidive) | `docs/lecons.md` (entrée complète + index rapide) ; règle courte avec son numéro dans `AGENTS.md` (Pièges connus) si elle vaut pour toute tâche, sinon dans `docs/pieges-par-domaine.md` (section du domaine) |
| Défaut vu hors du sujet de la branche, limite assumée, idée repoussée ; travail repris ou abandonné | `docs/travaux-reportes.md` (nouvelle entrée ou changement de statut, jamais de suppression) |
| Tout changement | Titre de la PR en phrase (il devient la ligne des notes de version) et une étiquette `RN: …` (`CONTRIBUTING.md`, D-056) : plus de CHANGELOG |

Pour une relecture large (avant une version, ou si beaucoup de choses ont changé) : relire **tous** les `.md`
(`git ls-files '*.md'`) et vérifier soi-même chaque valeur chiffrée contre le code (`grep`), sans lancer
d'agents ; au plus un seul agent pour tout le lot si la relecture est vraiment volumineuse.

## 3. Règles d'écriture

- Français partout ; noms de code, réglages Betaflight et formules tels quels.
- Le README et le guide s'adressent aux pilotes : pas de détails de développement hors de la partie
  *Développement* du README. Les noms d'outils sont ceux des boutons (« Spectre », « Filtres »…).
- Un seul fichier de téléchargement par système : ne pas documenter de variantes.
- Décrire ce que fait **notre** code ; citer PIDtoolbox seulement comme référence.
- L'ancien `CHANGELOG.md` (supprimé, D-056) ne se recrée pas : l'histoire des versions vit dans les pages Releases et dans git.
