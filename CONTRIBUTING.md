# Contribuer à OpenTuneBox

Projet public sous licence [PolyForm Noncommercial 1.0.0](LICENSE) : le code est ouvert, tout usage commercial est interdit (D-045).
En proposant une modification, vous acceptez qu'elle soit publiée sous cette seule licence — ni CLA à signer, ni cession supplémentaire,
et l'auteur ne se réserve aucun droit commercial.
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
   cargo run -p otb-dev -- verify   # fmt, clippy, tous les tests, cargo deny ; code 0 seulement si tout passe (--quick : sans deny)
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

## Migration vers Rust (terminée)

La migration est terminée depuis le 2026-11-04 (PR 96) : toute branche part de `main`, et la branche d'intégration `feat/rust-migration` est
supprimée sur GitHub. Son histoire et ses preuves d'équivalence : [docs/migration-rust.md](docs/migration-rust.md).

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
`rust/deny.toml` avec sa raison et un numéro `W-xxx`. Une licence nouvelle se discute avec Cédric avant d'entrer dans la liste
`allow`, et une licence à **copyleft** (GPL, LGPL, AGPL, MPL) est un **refus**, pas une discussion : du code sous copyleft ne peut pas être
redistribué sous une licence qui interdit l'usage commercial (D-045). Dependabot propose aussi les mises à jour des crates (écosystème `cargo`).

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
