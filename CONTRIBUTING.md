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

1. Sur une branche (`chore/release-0.2.0`) : `python scripts/release.py prepare 0.2.0`. Le script met le
   numéro dans `pyproject.toml` et `src/opentunebox/__init__.py`, transforme la section *Non publié* de
   `CHANGELOG.md` en `[0.2.0] — date du jour` et en rouvre une vide. Relire le CHANGELOG (c'est le texte
   de la page de téléchargement), puis demande de fusion vers `main` comme d'habitude.
2. Une fois fusionnée, sur `main` à jour :
   ```sh
   git switch main && git pull
   git tag v0.2.0 && git push origin v0.2.0
   ```
3. Le workflow `construire.yml` vérifie que l'étiquette correspond au numéro du projet (sinon il s'arrête
   tout de suite), fabrique les exécutables Windows, macOS et Linux, puis crée la page **Releases** de la
   version avec les trois archives et la section du CHANGELOG (≈ 10 min). Le dépôt étant privé, seules les
   personnes ayant accès au dépôt peuvent télécharger.
