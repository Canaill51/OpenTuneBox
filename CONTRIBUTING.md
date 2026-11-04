# Contribuer à OpenTuneBox

Projet public sous licence GNU GPL 3 (voir [LICENSE](LICENSE)) : en proposant une modification, vous acceptez qu'elle soit publiée sous cette licence.
Les consignes techniques détaillées (organisation
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
   cd rust
   cargo fmt --check && cargo clippy --all-targets -- -D warnings
   cargo test --no-fail-fast -p otb-decoder -p otb-dsp -p otb-format -p otb-log -p otb-analysis -p otb-advisor -p otb-cli -p otb-gui -p otb-parity -p otb-dev
   ```
   [rustup](https://rustup.rs) est le seul prérequis.
3. Mettre la documentation à jour **dans le même commit** : le tableau de `.claude/skills/docs-sync/SKILL.md`
   indique quels documents relire selon le changement ; `cargo test -p otb-dev --test docs` (inclus dans `cargo test`) vérifie
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
fusionnée, `main` reste en Python et le travail Rust passe par `feat/rust-migration`. Chaque étape a sa branche,
créée depuis `feat/rust-migration`, et sa PR **vers `feat/rust-migration`** (titre = nom de la branche) :
```sh
git switch feat/rust-migration && git switch -c feat/rust-decoder
gh pr create --base feat/rust-migration --title "feat/rust-decoder"
```
Rapatrier les correctifs de `main` : `git switch feat/rust-migration && git merge main`. Plan et preuves
d'équivalence : [docs/migration-rust.md](docs/migration-rust.md).

## Contrôles et mises à jour automatiques des dépendances

Dependabot ([`.github/dependabot.yml`](.github/dependabot.yml)) ouvre chaque lundi une demande de fusion
pour les actions GitHub des workflows et pour les crates Rust (mises à jour mineures et correctives groupées, majeure à part).
Ces demandes portent le titre choisi par Dependabot (`chore(deps): …`),
pas le nom d'une branche : c'est normal. On les fusionne à la main, une fois la CI verte et après avoir lu
les notes de la nouvelle version.

Le workflow [`audit.yml`](.github/workflows/audit.yml) cherche les failles connues dans les bibliothèques
livrées (`cargo-deny`, réglages dans `rust/deny.toml`) à chaque envoi, chaque demande de fusion et chaque lundi. S'il échoue, une dépendance
a une faille publiée : la mettre à jour. En local : `(cd rust && cargo deny check)` (failles RustSec, licences, origine des crates ; la base d'avis est téléchargée à
chaque passage). Un avis « non maintenu » sans correctif ne se retire pas du contrôle : il entre dans `ignore` de
`rust/deny.toml` avec sa raison et un numéro `W-xxx`. Une licence nouvelle (surtout un copyleft : GPL, LGPL) se discute avec
Cédric avant d'entrer dans la liste `allow`. Dependabot propose aussi les mises à jour des crates (écosystème `cargo`).

## Publier une version

1. Sur une branche (`chore/release-0.2.0`) : `(cd rust && cargo run -p otb-dev -- licenses)` (refait `THIRD-PARTY-LICENSES.txt`, que les
   deux installateurs livrent : à relancer si une dépendance a changé), puis `(cd rust && cargo run -p otb-dev -- release prepare 0.2.0)`. Cet outil met le
   numéro dans `rust/Cargo.toml`, transforme la section *Non publié* de
   `CHANGELOG.md` en `[0.2.0] — date du jour` et en rouvre une vide. Relire le CHANGELOG (c'est le texte
   de la page de téléchargement), puis demande de fusion vers `main` comme d'habitude.
2. Une fois fusionnée, sur `main` à jour :
   ```sh
   git switch main && git pull
   git tag v0.2.0 && git push origin v0.2.0
   ```
3. Le workflow `construire.yml` vérifie que l'étiquette correspond au numéro du projet (sinon il s'arrête
   tout de suite) et crée un **brouillon** de la page **Releases** avec la section du CHANGELOG. Il
   fabrique et essaie les installateurs Windows (`Setup.exe`) et Linux (AppImage) à partir de l'interface Rust, envoie
   chacun dans le brouillon, puis publie la page quand les deux fichiers y sont. Les fichiers
   ne passent pas par les *Artifacts* de GitHub. Si un
   système échoue : corriger, puis relancer les tâches échouées (`gh run rerun ID --failed`) ; le
   brouillon est réutilisé.

**Essayer la publication sans rien publier** (avant la première version d'une nouvelle chaîne de construction) : pousser une
étiquette `essai-v` suivie du numéro **déjà** présent dans `rust/Cargo.toml` et le CHANGELOG, par exemple
`git tag essai-v0.8.0 && git push origin essai-v0.8.0` sur la branche à essayer. Le workflow fait tout le parcours
(notes, brouillon, construction, envoi des deux fichiers, contrôle de la liste) mais laisse la page en **brouillon**,
invisible, et ne la déclare jamais « dernière version ». Regarder le brouillon, puis le supprimer avec son étiquette :
`gh release delete essai-v0.8.0 --cleanup-tag --yes`.
