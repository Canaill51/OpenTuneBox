# AGENTS.md — consignes pour les agents IA

OpenTuneBox analyse les logs Blackbox Betaflight (`.bbl`/`.bfl`) pour régler les drones : décodeur natif,
analyses FFT / Wiener / filtres Betaflight, interface Qt, assistant à règles. Objectif : couvrir tous les
outils de PIDtoolbox Pro V0.94, en multiplateforme et hors ligne.

## Le propriétaire

Cédric pilote et règle des drones FPV ; il débute en programmation. **Lui répondre en français, expliquer
chaque étape simplement**, et justifier les choix techniques par leur effet concret. Il lance lui-même
les commandes qui demandent `sudo` : préparer la commande ou un script, ne pas la contourner.

## Décisions fixées (ne pas remettre en cause sans lui demander)

Résumé ; contexte, options écartées et preuves dans `docs/decisions.md`.

- Multiplateforme (Linux, Windows) : **Rust**, interface **egui** + **egui_plot**, noyau Rust autonome. D-035. (La version 0.8.0 était en Python + PySide6 + numba : D-001, D-002, remplacées par D-035.)
- Assistant **100 % hors ligne**, à règles : aucune API, aucune donnée envoyée. D-003
- Dépôt **public**, licence **GNU GPL 3** (`LICENSE`). D-043
- **Aucune copie de code PIDtoolbox** (binaire chiffré) ni d'autres projets : tout est réimplémenté à partir
  des formules du firmware Betaflight et de méthodes publiées (voir `docs/spec-*.md`).
  Seule exception, validée par Cédric : les icônes [Lucide](https://lucide.dev) (licence ISC) dans
  `rust/otb-gui/assets/icons/`, à garder avec leur fichier de licence. D-004, D-005
- Migration vers Rust : le travail passe par `feat/rust-migration` et ses sous-branches (`docs/migration-rust.md`) ; chaque module a été
  prouvé équivalent avant de supprimer l'ancien. D-031
- **Plus aucun Python** dans le dépôt (application, tests, scripts) depuis la phase 7-5 ; Python figé comme étalon (tags `python-etalon`
  pour les références, `python-final` pour le dernier état de `main`). Pour refaire une référence : « Refaire une référence Python » dans
  `docs/validation.md`. D-035
- Interface, messages et documentation en français ; code et identifiants en anglais ; commentaires en français. D-001

## Suivi du projet

Trois registres numérotés, un maître par sujet (D-018) ; `cargo test -p otb-dev --test docs` vérifie que chaque numéro
est unique et que chaque numéro cité existe.

- `docs/decisions.md` (`D-xxx`) : choix en vigueur, options écartées, preuves. Un choix qui écarte une
  option, tranché par Cédric ou qui fixe un contrat entre deux parties du code y entre ; le remplacer =
  nouvelle décision, avec son accord.
- `docs/lecons.md` (`L-xxx`) : histoire complète des pièges. **En début de session**, lire son index rapide
  et la section du domaine touché ; **en fin de session**, y ajouter tout piège qui a coûté du temps. Ici,
  « Pièges connus » ne garde que la règle courte et son numéro.
- `docs/travaux-reportes.md` (`W-xxx`) : ce qui est vu et laissé de côté, avec la condition de reprise.
  Y noter tout défaut repéré hors du sujet de la branche, plutôt que de l'oublier ou de le corriger en douce.
- Renvoi dans le code : `# ⚠️ <ce qui casse si on l'ignore> (L-xxx)` ; la demande de fusion liste les
  numéros créés ou modifiés. Pas de numéro dans le sujet des commits.

## Organisation

```
rust/         espace de travail Cargo (D-031) ; détail de chaque crate et de chaque fichier : docs/architecture.md
  otb-decoder   décodeur BBL/BFL → session décodée (valeurs brutes) : header, frame, reader, file
  otb-log       FlightLog (unités physiques, courbes en float32 : D-029), settings (en-tête normalisé par version), debug_tables, modes
  otb-dsp       noyaux de calcul (filtres Betaflight, step_response, primitives numériques ; rayon : D-033)
  otb-analysis  calculs purs : spectral, step (Wiener), step_series, open_loop (marges de stabilité), delay, bf_filters, sliders, motors,
                stats, period, datasets (jeux de données analysables), npstat
  otb-advisor   profils de taille (profiles.rs), règles (rules.rs), diagnostic du bruit de Spectre (noise.rs), diagnostic du réglage de la
                Réponse indicielle (step_tuning.rs), guide intégré : vérification des logs (log_check.rs), fiche drone (drone_card.rs),
                parcours à cocher (tuning_path.rs)
  otb-format    formats de nombres (D-035)
  otb-cli       commandes `opentunebox info` et `opentunebox report`
  otb-gui       interface egui + egui_plot : app.rs, viewer.rs / viewer_page.rs (visualiseur), guide.rs / guide_page.rs (guide intégré),
                state.rs, worker.rs, tool.rs, theme.rs, icons.rs, plots.rs, export.rs, tools/ (une page par outil), assets/
  otb-parity    comparateur avec les références figées de l'ancienne version Python (tag `python-etalon`)
  otb-dev       outils de développement, jamais livrés : vérification de la documentation, release, icons, licenses, corpus (rates, calibrate, labels, validate)
tests/        data/jeno5_5s.bbl : 5 s d'un vrai log de Cédric, lu par 15 tests Rust ; les autres tests sont dans chaque crate
              (`rust/<crate>/tests/`) ; `rust/otb-gui/tests/support/mod.rs` : `synthetic_log` fabrique un vol sans corpus
packaging/    icônes (`opentunebox.ico`, `linux/opentunebox.png`, refaites par `otb-dev icons`), installateurs : windows/ (Inno Setup),
              linux/ (AppImage et `install_linux.sh`)
docs/         guide utilisateur, architecture, validation, assistant, spécifications (index : docs/README.md)
```

Règles :
- `otb-analysis` et `otb-advisor` ne dépendent **jamais** d'`otb-gui` : tout calcul doit rester utilisable en ligne de commande et testable.
- `otb-gui` affiche seulement ce que `otb-analysis` calcule. Un outil s'affiche dans une page de la fenêtre principale (`rust/otb-gui/src/app.rs`,
  boutons de la barre = onglets, jamais de fenêtre séparée) ; il implémente le trait `Tool` de `rust/otb-gui/src/tool.rs` et s'enregistre
  dans `rust/otb-gui/src/tools/mod.rs:all_pages`.
- `Tool::compute` est une fonction sans `self` : elle ne lit pas les réglages de l'interface, elle reçoit une copie (`Options`), ce qui évite
  de lire un réglage depuis un autre thread (le compilateur le vérifie).
- Un constat de l'assistant (`rust/otb-advisor/src/rules.rs:Finding`) se reconnaît par sa clé `key` (littéral anglais),
  jamais par son titre ou ses actions : les textes se reformulent librement ; un réglage déjà conseillé se
  déclare dans `settings` (D-028).

## Commandes

Toutes depuis `rust/` (`cd rust`).

```sh
cargo test --no-fail-fast -p otb-decoder -p otb-dsp -p otb-format -p otb-log -p otb-analysis -p otb-advisor -p otb-cli -p otb-gui -p otb-parity -p otb-dev   # tous les tests (sans le corpus)
cargo test -p otb-dev --test docs   # cohérence de la documentation
cargo fmt --check && cargo clippy --all-targets -- -D warnings   # formatage et lint (doivent être vides)
cargo run --release -p otb-gui -- [LOG…]   # interface
cargo run --release -p otb-cli -- report LOG.bbl   # diagnostic en ligne de commande (binaire `opentunebox`)
cargo run -p otb-dev -- icons   # refait packaging/linux/opentunebox.png et packaging/opentunebox.ico depuis le SVG
cargo run -p otb-dev -- licenses   # refait THIRD-PARTY-LICENSES.txt (mentions de licences livrées par les installateurs) ; à relancer quand une dépendance change
cargo run -p otb-dev -- release prepare X.Y.Z   # préparer une version ; `release notes vX.Y.Z` : notes de la page Releases
cargo run --release -p otb-dev -- corpus rates SORTIE.json   # taux de chaque constat sur le corpus ; `--compare AVANT.json APRÈS.json` : mêmes conseils ?
cargo run --release -p otb-dev -- corpus calibrate 600 SORTIE.json   # mesures brutes du corpus pour calibrer les seuils ; `corpus calibrate --resume SORTIE.json` : p10 / p50 / p90 par taille
cargo run --release -p otb-dev -- corpus labels   # tableau d'étiquettes $OTB_LOGS/corpus.csv ; ne réécrit jamais `etat`, `taille_reelle` ni `remarque` ; `--csv COPIE` pour essayer sur une copie
cargo run --release -p otb-dev -- corpus validate [--limit N] [--start N] [-j FILS]   # décodeur vs `blackbox_decode` de référence ; code 1 si une session n'est pas conforme
cargo run --release -p otb-parity -- check "$OTB_GOLDEN"   # concordance avec les références de l'ancienne version (couches : check-header, check-decoder…)
cargo deny check   # failles des dépendances, licences (réglages : rust/deny.toml)
```

CI : `tests.yml` (formatage, lint, tests, sur Ubuntu et Windows) et `audit.yml` (`cargo deny check`, voir `CONTRIBUTING.md`) ;
`construire.yml` fabrique les installateurs. La CI ne voit pas le corpus.

Variables d'environnement : `OTB_LOGS` (dossier de vrais logs, défaut `~/Documents/Drones/Logs` ; contient
aussi `corpus.csv`, les étiquettes des drones remplies par Cédric, à ne jamais écraser), `OTB_GOLDEN` (références figées, en général
`$OTB_LOGS/golden`), `OTB_REF_DECODER` (`blackbox_decode` de référence pour `otb-dev corpus validate`), `OTB_DOCS_ROOT` (autre racine pour la
vérification de la documentation).

Captures sans écran : `cargo run --release -p otb-gui --example capture` (rendu wgpu), puis regarder l'image produite.

## Pièges connus

Règle courte de chaque piège ; histoire, cause et preuve dans `docs/lecons.md` sous le numéro indiqué. Ci-dessous, ceux qui valent pour
presque toute tâche ; **ceux d'un domaine sont dans [docs/pieges-par-domaine.md](docs/pieges-par-domaine.md) : le lire avant de travailler dessus.**

- Couleurs : jamais de couleur en dur (`#777`, `Color32::GRAY`…) hors de `rust/otb-gui/src/theme.rs` ; les textes et les courbes passent
  par le thème (`rust/otb-gui/tests/contracts.rs` refuse tout code couleur ailleurs). Vérifier les deux thèmes. L-016
- Pas de compilateur C sur le poste de développement : pas de dépendance à compiler. D-006
- Vérifier avant un commit : lancer `cargo test` sans tube (`| tail` masque son échec) ; le crochet de Claude Code
  (`.claude/hooks/check-docs.sh`) ne voit que `git commit` et `git -c … commit`. L-022
- Test d'une clé de cache ou d'une règle d'affichage : casser la protection visée pour voir le test échouer ; `AppState::set_period` vide déjà le cache, donc ne prouve pas la clé (changer `period` à la main) ; une règle d'affichage sans effet testable devient une fonction pure. L-071
- Noyau parallèle (rayon, D-033) : L-038. Tableau 2D passé à plat : L-036
- Tests : la CI n'a pas le corpus ; tout outil ou analyse a d'abord un test sur un log fabriqué (`rust/otb-gui/tests/support/mod.rs:synthetic_log`)
  ou sur `tests/data/jeno5_5s.bbl`. Le test de `otb-parity` qui lit `$OTB_GOLDEN` s'ignore avec un message quand le dossier manque. L-023, L-025

Sections de `docs/pieges-par-domaine.md` : décodeur et journaux de vol ; analyses ; filtres Betaflight et notch dynamique ; assistant de réglage ;
interface egui et Rust ; construction des exécutables.

## Méthode de travail

1. Lire la doc liée à la tâche (`docs/spec-betaflight.md` pour toute formule firmware, `docs/spec-methodes.md`
   pour les analyses, `docs/architecture.md`) et l'index de `docs/lecons.md` ; vérifier qu'aucune décision
   (`docs/decisions.md`) ni aucun travail reporté (`docs/travaux-reportes.md`) ne porte déjà sur le sujet.
2. Valider sur **de vrais logs** en plus des tests synthétiques ; recouper avec les captures PIDtoolbox
   (`PTB_FIGS_*` du corpus) quand elles existent (résultats de référence dans `docs/validation.md`).
3. **Documentation à jour à chaque modification**, dans le même commit que le code : suivre la compétence
   `.claude/skills/docs-sync` (quels documents relire selon le changement) ; `cargo test -p otb-dev --test docs` vérifie
   automatiquement liens, chemins, outils, modules, fichiers publiés et langue.
   `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` et `cargo test` propres avant chaque commit ; mettre à jour `CHANGELOG.md` (section *Non publié*).
4. Commits en français, à l'impératif ou descriptifs, un sujet par commit. Auteur `Cedric` avec l'adresse noreply de GitHub (D-043), jamais l'adresse personnelle.
   Ne jamais pousser sur GitHub ni publier quoi que ce soit sans l'accord de Cédric.
5. Une branche par sujet, nommée `type/description-courte` (convention GitHub : `feat`, `fix`, `docs`,
   `refactor`, `test`, `chore` ; minuscules, tirets, anglais). La PR vers `main` a pour titre **exactement
   le nom de la branche** (détails dans `CONTRIBUTING.md`).
