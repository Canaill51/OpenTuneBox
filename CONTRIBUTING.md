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
   .venv/bin/ruff check src tests scripts packaging/make_icons.py
   (cd rust && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p otb-decoder -p otb-dsp -p otb-log)   # si rust/ change
   ```
   Sur `feat/rust-migration`, `pip install -e .` compile l'extension Rust : [rustup](https://rustup.rs) est requis ;
   après un changement dans `rust/`, relancer `.venv/bin/pip install -e .`.
3. Mettre la documentation à jour **dans le même commit** : le tableau de `.claude/skills/docs-sync/SKILL.md`
   indique quels documents relire selon le changement ; `tests/test_docs.py` (lancé par `pytest`) vérifie
   automatiquement liens, chemins, outils, modules et fichiers publiés. Ajouter une ligne dans
   `CHANGELOG.md` (section *Non publié*). Selon le cas, compléter les registres numérotés : un choix qui
   écarte une option → [docs/decisions.md](docs/decisions.md) (`D-xxx`), un piège qui a coûté du temps →
   [docs/lecons.md](docs/lecons.md) (`L-xxx`), un défaut vu hors du sujet de la branche →
   [docs/travaux-reportes.md](docs/travaux-reportes.md) (`W-xxx`). Pas de numéro dans le sujet du commit.
4. Commit en français, un sujet par commit, puis demande de fusion (*Pull request*) vers `main`.
   **Le titre de la PR est exactement le nom de la branche** (ex. `fix/windows-close-crash`) ; la
   description, en français, suit le modèle de PR.
   ```sh
   gh pr create --base main --title "fix/windows-close-crash"
   ```

## Migration vers Rust (branche d'intégration)

Exception temporaire à la règle « créer une branche depuis `main` » (D-031) : tant que la migration n'est pas
validée, `main` reste en Python et le travail Rust passe par `feat/rust-migration`. Chaque étape a sa branche,
créée depuis `feat/rust-migration`, et sa PR **vers `feat/rust-migration`** (titre = nom de la branche) :
```sh
git switch feat/rust-migration && git switch -c feat/rust-decoder
gh pr create --base feat/rust-migration --title "feat/rust-decoder"
```
Rapatrier les correctifs de `main` : `git switch feat/rust-migration && git merge main`. Plan et preuves
d'équivalence : [docs/migration-rust.md](docs/migration-rust.md).

## Contrôles et mises à jour automatiques des dépendances

Dependabot ([`.github/dependabot.yml`](.github/dependabot.yml)) ouvre chaque lundi une demande de fusion
pour les actions GitHub des workflows (mises à jour mineures et correctives groupées, majeure à part).
Pour les bibliothèques Python, `pyproject.toml` ne fixe que des versions minimales (`>=`) : Dependabot n'y
touche pas tant que la borne admet la dernière version, et c'est voulu (les planchers disent ce qui est
compatible, pas ce qui est à jour). Ces demandes portent le titre choisi par Dependabot (`chore(deps): …`),
pas le nom d'une branche : c'est normal. On les fusionne à la main, une fois la CI verte et après avoir lu
les notes de la nouvelle version.

Le workflow [`audit.yml`](.github/workflows/audit.yml) cherche les failles connues dans les bibliothèques
livrées (`pip-audit`) à chaque envoi, chaque demande de fusion et chaque lundi. S'il échoue, une dépendance
a une faille publiée : relever sa version minimale dans `pyproject.toml`. Rien à lancer en local ; pour un
essai, dans un environnement vierge (`python3 -m venv /tmp/projet /tmp/outil`) :
```sh
/tmp/projet/bin/pip install --upgrade pip && /tmp/projet/bin/pip install -e .
/tmp/outil/bin/pip install pip-audit==2.9.0
/tmp/outil/bin/pip-audit --skip-editable --path /tmp/projet/lib/python3.*/site-packages
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
   tout de suite) et crée un **brouillon** de la page **Releases** avec la section du CHANGELOG. Il
   fabrique et essaie les installateurs Windows (`Setup.exe`), macOS (`.dmg`) et Linux (AppImage), envoie
   chacun dans le brouillon, puis publie la page quand les trois fichiers y sont (≈ 10 min). Les fichiers
   ne passent pas par les *Artifacts* de GitHub (stockage limité à 500 Mo pour un dépôt privé). Si un
   système échoue : corriger, puis relancer les tâches échouées (`gh run rerun ID --failed`) ; le
   brouillon est réutilisé. Le dépôt étant privé, seules les personnes ayant accès au dépôt peuvent
   télécharger.
