# Contribuer à OpenTuneBox

Projet privé : les contributions se font sur invitation. Les consignes techniques détaillées (organisation
du code, pièges, commandes) sont dans [AGENTS.md](AGENTS.md) ; elles valent pour les humains comme pour les agents IA.

## Signaler un problème

Ouvrir un ticket (*Issues*) avec le modèle « Problème » : version d'OpenTuneBox et du système, outil
concerné, ce qui était attendu, et **si possible le log Blackbox** (ou son nom) et une capture d'écran.
Pour une valeur qui diffère de PIDtoolbox, joindre la capture PIDtoolbox du même log.

## Proposer une modification

1. Créer une branche depuis `main` : `git switch -c correction/nom-court`.
2. Modifier, puis vérifier :
   ```sh
   .venv/bin/pytest -q
   .venv/bin/python -m pyflakes src tests scripts
   ```
3. Ajouter une ligne dans `CHANGELOG.md` (section *Non publié*).
4. Commit en français, un sujet par commit, puis demande de fusion (*Pull request*) vers `main`.

## Publier une version

1. Déplacer la section *Non publié* de `CHANGELOG.md` sous un nouveau numéro, mettre à jour `version`
   dans `pyproject.toml`.
2. `git tag v0.2.0 && git push --tags` : le workflow `construire.yml` fabrique les exécutables Windows,
   macOS et Linux, à joindre à la version (*Releases*).
