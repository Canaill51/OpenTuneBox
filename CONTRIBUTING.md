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
   automatiquement liens, chemins, outils, modules et fichiers publiés. Selon le cas, compléter les registres numérotés : un choix qui
   écarte une option → [docs/decisions.md](docs/decisions.md) (`D-xxx`), un piège qui a coûté du temps →
   [docs/lecons.md](docs/lecons.md) (`L-xxx`), un défaut vu hors du sujet de la branche →
   [docs/travaux-reportes.md](docs/travaux-reportes.md) (`W-xxx`). Pas de numéro dans le sujet du commit.
4. Commit en français, un sujet par commit, puis demande de fusion (*Pull request*) vers `main`.
   **Le titre de la PR est une phrase en français qui dit l'effet du changement** pour celui qui utilise
   OpenTuneBox (ex. « Le programme ne plante plus à la fermeture sous Windows ») : elle devient telle quelle la ligne
   des notes de version. Pas un nom de branche : le contrôle de la PR le refuse. La description, en français, suit le
   modèle de PR.
5. Poser **une seule** étiquette `RN: …` sur la PR : elle choisit sa rubrique dans les notes de version (D-056).

   | Étiquette | Rubrique des notes | Pour |
   |---|---|---|
   | `RN: nouveauté majeure`, `RN: nouveauté` | Nouveautés | un outil, une fonction, un réglage que le pilote n'avait pas |
   | `RN: amélioration`, `RN: sécurité`, `RN: refonte` | Améliorations | ce qui existait marche mieux, plus vite, plus sûrement |
   | `RN: correction` | Corrections | un défaut corrigé |
   | `RN: ignorer` | absente des notes | CI, outils de développement, plans, registres, documentation interne |

   ```sh
   gh pr create --base main --title "Le programme ne plante plus à la fermeture sous Windows" --label "RN: correction"
   ```
   Le workflow [`etiquette-pr.yml`](.github/workflows/etiquette-pr.yml) échoue tant que la PR n'a pas exactement une
   étiquette `RN: …` ou que son titre ressemble à un nom de branche ; il se relance quand on change l'étiquette ou le titre.

## Migration vers Rust (terminée)

La migration est terminée depuis le 2026-11-04 (PR 96) : toute branche part de `main`, et la branche d'intégration `feat/rust-migration` est
supprimée sur GitHub. Son histoire et ses preuves d'équivalence : [docs/migration-rust.md](docs/migration-rust.md).

## Contrôles et mises à jour automatiques des dépendances

Dependabot ([`.github/dependabot.yml`](.github/dependabot.yml)) ouvre chaque lundi une demande de fusion
pour les actions GitHub des workflows et pour les crates Rust (mises à jour mineures et correctives groupées, majeure à part).
Ces demandes portent le titre choisi par Dependabot (`chore(deps): …`) et l'étiquette `dependencies`, sans
étiquette `RN: …` : c'est normal, elles n'apparaissent pas dans les notes de version. On les fusionne à la main, une fois la CI verte et après avoir lu
les notes de la nouvelle version.

La **version de Rust** est figée par [`rust/rust-toolchain.toml`](rust/rust-toolchain.toml), sur le poste comme en CI.
Sans ce fichier, chaque passage prenait « stable » : une nouvelle version de Rust ou de clippy pouvait rendre la CI
rouge du jour au lendemain, en plein travail sur autre chose. Pour en changer : modifier le numéro, lancer
`(cd rust && cargo run -p otb-dev -- verify)`, corriger ce que le nouveau clippy signale, et le dire dans la description de la PR.

Les **workflows** eux-mêmes sont relus par `actionlint` (job de [`tests.yml`](.github/workflows/tests.yml)) à chaque
demande de fusion : syntaxe, expressions, paramètres des actions, et scripts bash des étapes passés à `shellcheck`. Rien à
installer sur le poste. Sa version est figée et vérifiée par son empreinte ; Dependabot ne la voit pas : pour la monter,
changer le numéro **et** l'empreinte (D-055).

Le workflow [`audit.yml`](.github/workflows/audit.yml) cherche les failles connues dans les bibliothèques
livrées (`cargo-deny`, réglages dans `rust/deny.toml`) à chaque envoi **sur `main`**, chaque demande de fusion et chaque lundi. S'il échoue, une dépendance
a une faille publiée : la mettre à jour. En local : `(cd rust && cargo deny check)` (failles RustSec, licences, origine des crates ; la base d'avis est téléchargée à
chaque passage). Un avis « non maintenu » sans correctif ne se retire pas du contrôle : il entre dans `ignore` de
`rust/deny.toml` avec sa raison et un numéro `W-xxx`. Une licence nouvelle se discute avec Cédric avant d'entrer dans la liste
`allow`, et une licence à **copyleft** (GPL, LGPL, AGPL, MPL) est un **refus**, pas une discussion : du code sous copyleft ne peut pas être
redistribué sous une licence qui interdit l'usage commercial (D-045). Dependabot propose aussi les mises à jour des crates (écosystème `cargo`).

Le workflow [`claude.yml`](.github/workflows/claude.yml) fait intervenir Claude Code dans une issue ou une demande de
fusion. Il ne part **que** si Cédric écrit « @claude » — dans un commentaire, ou dans le titre ou le corps d'une
issue qu'il ouvre. Pas de relecture automatique, parce que
chaque passage consomme l'abonnement Claude et des minutes GitHub Actions. Le dépôt étant public, le workflow refuse
aussi tout commentaire qui ne vient pas du propriétaire, et toute demande de fusion venue d'un **fork** : Claude y lancerait
`cargo`, donc le code d'un inconnu, avec le jeton de l'abonnement (D-054). Pour faire relire une telle contribution, lire
ses changements, les reprendre sur une branche du dépôt, puis écrire « @claude » sur la nouvelle demande de fusion. Il a besoin du secret `CLAUDE_CODE_OAUTH_TOKEN` du dépôt,
fabriqué par `claude setup-token` ; sans ce secret, le passage échoue. Pour une relecture avant d'ouvrir la demande de
fusion, `/code-review` en local ne coûte aucune minute GitHub.

Quand un passage échoue, l'action remplace la sortie de Claude par « full output hidden for security » : le journal ne
dit pas pourquoi. Une dernière étape du workflow rend alors **le seul message d'erreur final**, jamais la conversation —
les journaux d'Actions d'un dépôt public sont lisibles par tout le monde.

## Publier une version

Dans les exemples ci-dessous, `X.Y.Z` est le **prochain** numéro (celui de `rust/Cargo.toml` une fois préparé) :
`0.2.0` servait d'exemple, or cette étiquette existe déjà — la recopier telle quelle fait échouer la publication.

1. Sur une branche (`chore/release-X.Y.Z`) : `(cd rust && cargo run -p otb-dev -- licenses)` (refait `THIRD-PARTY-LICENSES.txt`, que les
   deux installateurs livrent : à relancer si une dépendance a changé ; `otb-dev verify` et la CI le contrôlent par
   `licenses --check`, qui compare sans rien écrire et rend 1 avec le message « relancer `cargo run -p otb-dev -- licenses` »), puis `(cd rust && cargo run -p otb-dev -- release prepare X.Y.Z)`. Cet outil met le
   numéro dans `rust/Cargo.toml`. Relire les titres et les étiquettes des PR fusionnées depuis la dernière version : ce
   sont eux qui font les notes. Pour voir le texte à l'avance, sans rien publier :
   ```sh
   gh api -X POST repos/Canaill51/OpenTuneBox/releases/generate-notes -f tag_name=vX.Y.Z -f target_commitish=main \
     -f previous_tag_name=vA.B.C --jq .body > liste.md   # vA.B.C : la dernière version publiée
   (cd rust && cargo run -p otb-dev -- release notes vX.Y.Z ../liste.md)
   ```
   Puis demande de fusion vers `main` comme d'habitude (titre en phrase, étiquette `RN: ignorer`).
2. Une fois fusionnée, sur `main` à jour :
   ```sh
   git switch main && git pull
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```
3. Le workflow `construire.yml` vérifie que l'étiquette correspond au numéro du projet (sinon il s'arrête
   tout de suite) et crée un **brouillon** de la page **Releases**. Ses notes : la liste que GitHub génère des PR
   fusionnées depuis la dernière version publiée, rangées par étiquette (`.github/release.yml`), traduite par
   `otb-dev release notes` qui ajoute en tête le bloc Téléchargements ; une liste vide arrête tout. Il
   fabrique et essaie les installateurs Windows (`Setup.exe`) et Linux (AppImage) à partir de l'interface Rust, envoie
   chacun dans le brouillon, puis publie la page quand les deux fichiers y sont. Les fichiers
   ne passent pas par les *Artifacts* de GitHub. Si un
   système échoue : corriger, puis relancer les tâches échouées (`gh run rerun ID --failed`) ; le
   brouillon est réutilisé.

**Essayer la publication sans rien publier** (avant la première version d'une nouvelle chaîne de construction) : pousser une
étiquette `essai-v` suivie du numéro **déjà** présent dans `rust/Cargo.toml`, par exemple
`git tag essai-vX.Y.Z && git push origin essai-vX.Y.Z` sur la branche à essayer (avec le numéro du moment : une
étiquette d'une version déjà publiée ferait échouer le contrôle). Le workflow fait tout le parcours
(notes, brouillon, construction, envoi des deux fichiers, contrôle de la liste) mais laisse la page en **brouillon**,
invisible, et ne la déclare jamais « dernière version ». Regarder le brouillon, puis le supprimer avec son étiquette :
`gh release delete essai-v0.8.0 --cleanup-tag --yes`.
