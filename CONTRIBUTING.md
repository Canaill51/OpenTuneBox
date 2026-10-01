# Contribuer à OpenTuneBox

Projet privé : les contributions se font sur invitation. Les consignes techniques détaillées (organisation
du code, pièges, commandes) sont dans [AGENTS.md](AGENTS.md) ; elles valent pour les humains comme pour les agents IA.

## Signaler un problème

Ouvrir un ticket (*Issues*) avec le modèle « Problème » : version d'OpenTuneBox et du système, outil
concerné, ce qui était attendu, et **si possible le log Blackbox** (ou son nom) et une capture d'écran.
Pour une valeur qui diffère de PIDtoolbox, joindre la capture PIDtoolbox du même log.

## Proposer une modification

1. Créer une branche depuis `main`, nommée selon la convention GitHub `type/description-courte` :
   minuscules, mots séparés par des tirets, en anglais, sans accents. Types : `feat` (nouvelle fonction),
   `fix` (correction), `docs`, `refactor`, `test`, `chore` (maintenance). Une branche par sujet.
   ```sh
   git switch -c fix/windows-close-crash
   ```
2. Modifier, puis vérifier :
   ```sh
   .venv/bin/pytest -q
   .venv/bin/python -m pyflakes src tests scripts
   ```
3. Ajouter une ligne dans `CHANGELOG.md` (section *Non publié*).
4. Commit en français, un sujet par commit, puis demande de fusion (*Pull request*) vers `main`.
   **Le titre de la PR est exactement le nom de la branche** (ex. `fix/windows-close-crash`) ; la
   description, en français, suit le modèle de PR.
   ```sh
   gh pr create --base main --title "fix/windows-close-crash"
   ```

## Publier une version

1. Déplacer la section *Non publié* de `CHANGELOG.md` sous un nouveau numéro, mettre à jour `version`
   dans `pyproject.toml`.
2. `git tag v0.2.0 && git push --tags` : le workflow `construire.yml` fabrique les exécutables Windows,
   macOS et Linux, à joindre à la version (*Releases*).
