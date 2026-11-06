# AGENTS.md — consignes pour les agents IA

OpenTuneBox analyse les logs Blackbox Betaflight (`.bbl`/`.bfl`) pour régler les drones : décodeur natif,
analyses FFT / Wiener / filtres Betaflight, interface egui, assistant à règles. Objectif : couvrir tous les
outils de PIDtoolbox Pro V0.94, en multiplateforme et hors ligne.

## Le propriétaire

Cédric pilote et règle des drones FPV ; il débute en programmation. **Lui répondre en français, expliquer chaque étape simplement**,
justifier les choix par leur effet concret. Il lance lui-même les commandes `sudo` : les préparer, ne pas les contourner.

## Décisions fixées (ne pas remettre en cause sans lui demander)

Contexte, options écartées et preuves : `docs/decisions.md`.

- **Rust**, interface **egui** + **egui_plot**, noyau autonome, Linux et Windows. D-035. Migration terminée le 2026-11-04 (`docs/migration-rust.md`) :
  plus aucun Python dans le dépôt ; étalon figé dans les tags `python-etalon` et `python-final` (`docs/validation.md`, « Refaire une référence Python »).
- Assistant **100 % hors ligne**, à règles : aucune API, aucune donnée envoyée. D-003
- Dépôt **public**, licence **PolyForm Noncommercial 1.0.0** : code ouvert, tout usage commercial interdit, aucune dépendance copyleft (polices : chacune sous sa licence, D-046). D-043, D-045
- **Aucune copie de code PIDtoolbox** ni d'autres projets : tout est réimplémenté d'après le firmware Betaflight et des méthodes publiées
  (`docs/spec-*.md`). Seule exception : les icônes [Lucide](https://lucide.dev) (ISC) dans `rust/otb-gui/assets/icons/`, avec leur licence. D-004, D-005
- Interface, messages et documentation en français ; code et identifiants en anglais ; commentaires en français. D-001

## Suivi du projet

Trois registres numérotés, un maître par sujet (D-018) ; `cargo test -p otb-dev --test docs` vérifie que chaque numéro est unique et existe.

- `docs/decisions.md` (`D-xxx`) : choix en vigueur et options écartées (tranché par Cédric, ou contrat entre deux parties du code) ;
  le remplacer = nouvelle décision, avec son accord.
- `docs/lecons.md` (`L-xxx`) : histoire des pièges. Début de session : son index rapide, les arrivées récentes et la section du domaine touché ; fin de session :
  y ajouter tout piège qui a coûté du temps. Ici, « Pièges connus » garde la règle courte et son numéro.
- `docs/travaux-reportes.md` (`W-xxx`) : ce qui est vu et laissé de côté, avec la condition de reprise (défaut hors sujet : l'y noter).
- Dans le code : `# ⚠️ <ce qui casse si on l'ignore> (L-xxx)` ; la PR liste les numéros touchés ; pas de numéro dans le sujet des commits.

## Organisation

`rust/` est l'espace de travail Cargo (D-031) ; détail de chaque crate et fichier : `docs/architecture.md`.

- `otb-decoder` (BBL/BFL → valeurs brutes), `otb-log` (FlightLog en unités physiques, float32 : D-029), `otb-dsp` (noyaux de calcul, D-033),
  `otb-analysis` (calculs purs), `otb-advisor` (assistant et guide intégré), `otb-format` (formats de nombres), `otb-cli` (`opentunebox info` / `report`),
  `otb-gui` (egui), `otb-parity` (comparateur avec les références Python), `otb-dev` (outils de développement, jamais livrés).
- `tests/data/jeno5_5s.bbl` : 5 s d'un vrai log, lu par 11 fichiers de tests ; les autres tests sont dans `rust/<crate>/tests/` ;
  `rust/otb-gui/tests/support/mod.rs:synthetic_log` fabrique un vol sans corpus. `packaging/` : installateurs. `docs/` : index dans `docs/README.md`.

Règles :
- `otb-analysis` et `otb-advisor` ne dépendent **jamais** d'`otb-gui` : tout calcul reste utilisable en ligne de commande et testable.
- `otb-gui` affiche seulement ce que `otb-analysis` calcule. Un outil est une page de la fenêtre principale (`rust/otb-gui/src/app.rs`, jamais de
  fenêtre séparée), implémente le trait `Tool` de `rust/otb-gui/src/tool.rs` et s'enregistre dans `rust/otb-gui/src/tools/mod.rs:all_pages`.
- `Tool::compute` n'a pas de `self` : elle reçoit une copie des réglages (`Options`), jamais l'interface.
- Un constat de l'assistant (`rust/otb-advisor/src/rules.rs:Finding`) se reconnaît par sa clé `key` (littéral anglais), jamais par son titre ;
  un réglage déjà conseillé se déclare dans `settings` (D-028).

## Commandes

Depuis `rust/` (`cd rust`) ; postes Linux et Windows 11 (commandes `cargo` identiques, chemins et variables à adapter).

```sh
cargo run -p otb-dev -- verify   # fmt, clippy, tous les tests, mentions de licences, cargo deny ; ≈ 70 s à chaud ; 0 seulement si tout passe (--quick sans deny ; --strict : un test sauté faute de corpus = échec)
cargo test -p otb-dev --test docs   # cohérence de la documentation
cargo run --release -p otb-gui -- [LOG…]   # interface
cargo run --release -p otb-cli -- report LOG.bbl   # diagnostic en ligne de commande
cargo run -p otb-dev -- icons | licenses [--check] | release prepare X.Y.Z   # icônes, licences livrées (--check : compare sans écrire, lancé par verify et la CI), version
cargo run --release -p otb-dev -- corpus rates|calibrate|labels|validate   # calculs sur le corpus : compétence `calculs-corpus`
cargo run --release -p otb-parity -- check "$OTB_GOLDEN"   # concordance avec les références figées
cargo run --release -p otb-gui --example capture -- LOG.bbl DOSSIER [AUTRE_LOG…]   # captures sans écran (wgpu) : regarder l'image produite
```

CI : `tests.yml` (fmt et clippy sur Ubuntu ; tests sur Ubuntu et Windows), `audit.yml` (`cargo deny`), `gui-build.yml` (interface à essayer à la main),
`construire.yml` (installateurs). Elle ne voit pas le corpus.

Variables : `OTB_LOGS` (vrais logs, défaut `~/Documents/Drones/Logs`, avec `corpus.csv`), `OTB_GOLDEN` (références figées, en général `$OTB_LOGS/golden`),
`OTB_REF_DECODER` (`blackbox_decode`), `OTB_DOCS_ROOT` et `OTB_DEV_ROOT` (racines pour `otb-dev`), `OTB_BITEXACT=1` (au bit près, `rust/otb-advisor/tests/smooth.rs`),
`OTB_FORMAT_CASES` (gros fichier de cas, `rust/otb-format/tests/cas.rs`).

## Interdits

- `git push`, `git tag`, `gh pr merge`, `gh release` : seulement avec la confirmation de Cédric **à chaque fois** (« demander » dans `.claude/settings.json`) ;
  l'étiquette `v*` est posée par lui.
- Ne jamais modifier `$OTB_LOGS`, `golden/` ni `corpus.csv` (hors `corpus labels`) ; jamais affaiblir ou sauter un test, ni `--no-verify`.
- Fichiers temporaires (captures, essais) : hors du dépôt.

## Pièges connus

Règle courte de chaque piège ; histoire et preuve dans `docs/lecons.md`. Les pièges d'un domaine sont dans
[docs/pieges-par-domaine.md](docs/pieges-par-domaine.md) (décodeur, analyses, filtres Betaflight, assistant, interface, outillage, exécutables) : le lire avant d'y travailler.

- Couleurs : jamais en dur hors de `rust/otb-gui/src/theme.rs` ; `rust/otb-gui/tests/contracts.rs` refuse `Color32::…` et `#` + 6 chiffres hexadécimaux
  (`#777` passe : ne pas s'y fier). Vérifier les deux thèmes. L-016
- `verify` sans tube (`| tail` masque son échec) ; le crochet `.claude/hooks/check-docs.sh` ne voit que `git commit` (avec `-c` ou `-C`). L-022
- Test d'une clé de cache ou d'une règle d'affichage : casser la protection visée pour le voir échouer ; règle sans effet testable = fonction pure. L-071
- La CI n'a pas le corpus : tout outil ou analyse a d'abord un test sur un log fabriqué ou sur `tests/data/jeno5_5s.bbl` ; un test qui saute faute
  de corpus s'annonce par `[OTB-SAUT]`. L-023, L-025

## Méthode de travail

1. Lire la doc liée à la tâche (`docs/spec-betaflight.md` pour une formule du firmware, `docs/spec-methodes.md` pour les analyses,
   `docs/architecture.md`), l'index rapide de `docs/lecons.md`, ses arrivées récentes et la section du domaine ; vérifier qu'aucune décision (`docs/decisions.md`) ni
   aucun travail reporté (`docs/travaux-reportes.md`) ne porte déjà sur le sujet.
2. Valider sur **de vrais logs** en plus des tests synthétiques ; recouper avec les captures PIDtoolbox (`PTB_FIGS_*`) quand elles existent (`docs/validation.md`).
3. **Documentation à jour dans le même commit que le code** : compétence `docs-sync` ; `verify` vert avant chaque commit ; `CHANGELOG.md` (*Non publié*).
4. Commits en français, un sujet par commit. Auteur `Cedric` avec l'adresse noreply de GitHub (D-043), jamais l'adresse personnelle.
5. Une branche par sujet, `type/description-courte` (`feat`, `fix`, `docs`, `refactor`, `test`, `chore` ; minuscules, tirets, anglais) ; la PR vers `main`
   a pour titre **exactement le nom de la branche** (`CONTRIBUTING.md`).
