# Plan par lots : cinq chantiers après la migration Rust

Plan de travail issu de l'audit du dépôt du 2026-11-04 (architecture, vérification, consignes, CI, robustesse), à mener
**sans régression** : chaque lot garde les 579 tests verts, `clippy` propre, et prouve ce qu'il change par un test qui
échouait avant (contre-preuve), ou par une mesure consignée.

## Comment s'en servir

- **Une session = un lot = une PR.** Faire `/clear`, puis demander « Lot N de `docs/plan-chantiers.md` ». La branche porte le
  nom indiqué dans le lot ; le titre de la PR est exactement ce nom (`CONTRIBUTING.md`).
- Chaque lot suit les phases **explorer → planifier → implémenter → vérifier → livrer → consigner**. Explorer = lire les
  fichiers listés dans « À lire ». Planifier = présenter le découpage et attendre l'accord de Cédric. Consigner = cocher les
  cases ci-dessous, remplir « Avancement », mettre à jour `CHANGELOG.md` (section *Non publié*) et les registres
  (`docs/decisions.md`, `docs/lecons.md`, `docs/travaux-reportes.md`) selon la compétence `docs-sync`.
- **Vérification minimale de tout lot**, depuis `rust/` : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --no-fail-fast` ; à partir du lot 1, la commande unique `cargo run -p otb-dev -- verify` remplace ces trois lignes.
- **Un nouveau test se prouve par contre-preuve** : réintroduire le défaut (ou le casser à la main), voir le test rougir, le
  noter dans la PR.
- **Critère d'arrêt** : si une hypothèse d'un lot tombe, s'arrêter, consigner ce qui a été vu, et revenir vers Cédric ; ne pas
  improviser un autre découpage.
- Modèle conseillé par lot : **Sonnet** pour le mécanique, la documentation, les tests ; **Opus** pour le décodeur, la
  mémoire et le débogage difficile.

## Les cinq chantiers et leurs preuves

| # | Chantier | Constat (preuve) | Valeur | Risque |
|---|---|---|---|---|
| C1 | Décodeur robuste face à un en-tête hostile | `I interval` et dénominateur de `P interval` à 9·10¹⁸ : boucle sans fin dans `rust/otb-decoder/src/lib.rs:decode_stream` (recherche de l'itération attendue) ; 20 000 champs déclarés : `memory allocation of 67107840000 bytes failed`, abandon du processus (une allocation ratée n'est pas une panique, `catch_unwind` ne la rattrape pas) ; tout à zéro : « PID 1000000 Hz » et profil absurde | haute | faible |
| C2 | Résultat périmé affiché par le socle des outils | `rust/otb-gui/src/tool.rs:update` : un résultat qui arrive après « Tout fermer » ou un changement de sélection écrase l'état « Sélectionnez au moins un log » et reste affiché ; le test existant vide l'état après l'attente, donc ne le couvre pas | haute | faible |
| C3 | Commande unique de vérification et crochet réparé | Aucune commande pass/fail ; la liste des dix `-p` est recopiée à quatre endroits alors que `cargo test` seul est équivalent ; `.claude/hooks/check-docs.sh` dépend de `jq`, absent du poste Windows : il sort avec le code 0 sans rien vérifier ; les tests `golden.rs` et `smooth.rs` sautés par environnement comptent comme réussis | haute | nul |
| C4 | Mémoire du décodeur | Fichier entier lu en mémoire (`rust/otb-decoder/src/file.rs:decode_file`), toutes les sessions décodées en `i64` avant conversion (`rust/otb-log/src/flightlog.rs:load`) : 8 octets × 52 champs par trame pour un log qui en pèse 40 ; W-016 mesurait 39 Mo → 364 Mo ; estimation ≈ 8 Go pour un log de 500 Mo | haute sur les gros logs | moyen |
| C5 | Consignes et CI remises à jour | `AGENTS.md` : « interface Qt », branche `feat/rust-migration` encore présentée comme active (fusionnée, PR 96), « pas de compilateur C » (contredit par D-035), commande de capture incomplète, « 15 tests » faux, `gui-build.yml` non cité ; `tests.yml` et `audit.yml` écoutent encore `feat/rust-migration` | moyenne | nul |

## Branches

```
main
 ├─ chore/corpus-probe          lot 0 : sonde, aucun code livré (peut ne produire qu'un compte rendu)
 ├─ feat/dev-verify             lot 1 : C3
 ├─ docs/agents-refresh         lot 2 : C5
 ├─ fix/tool-page-stale-result  lot 3 : C2
 ├─ fix/decoder-hostile-header  lot 4 : C1
 ├─ perf/decoder-memory         lot 5 : C4 (deux sous-lots, deux PR)
 ├─ docs/chantiers-bilan        lot 6 : bilan et mesures finales
 └─ (à nommer)                  lot 7 : première publication Rust, après décision sur W-046
```

Chaque branche part de `main` à jour. Dépendances : le lot 1 d'abord (il donne la commande de vérification à tous les
autres) ; le lot 0 avant les lots 4 et 5 (ils ont besoin du corpus et des références pour prouver l'absence de régression) ;
le lot 2 après le lot 1 (il documente la nouvelle commande) ; les lots 3 et 4 sont indépendants entre eux.

## Ce qui revient à Cédric (à trancher dès le début)

1. **Références figées.** Sur ce poste Windows, `OTB_GOLDEN` n'est pas défini et aucun dossier `golden/` n'existe sous
   `Documents\Drones` ; `OTB_REF_DECODER` n'est pas défini non plus. Les lots 4 et 5 touchent le décodeur : sans ces
   références, la seule preuve d'équivalence est les 70 fichiers de `rust/otb-parity/tests/data/decode/` et les tests.
   **Réponse de Cédric (2026-11-04)** : le travail a surtout été fait sur le poste Linux, les références y sont très
   probablement. Conséquence : le lot 0 se fait sur le poste Linux (ou commence par vérifier que `golden/` y est), et les
   lots 4 et 5 se mènent sur le poste qui a les références.
2. **Bornes du décodeur (lot 4).** L'en-tête d'un log annonce trois nombres que le décodeur croit sur parole : le nombre de
   champs par trame (la ligne `Field I name`), `I interval` (une trame complète toutes les N boucles) et `P interval` (une
   trame de différences toutes les M boucles). Aujourd'hui le décodeur les lit sans limite haute (`rust/otb-decoder/src/header.rs`
   ramène seulement 0 à 1) : des valeurs absurdes font soit réserver une table gigantesque (20 000 champs → 67 Go demandés,
   abandon du programme), soit chercher pendant des heures la prochaine boucle attendue. Une borne, c'est un plafond au-delà
   duquel le fichier est refusé avec un message, au lieu d'être cru. **Valeurs réelles relevées le 2026-11-04 sur les 525 logs
   du corpus** : `I interval` ∈ {102, 128, 256} ; `P interval` ∈ {1, 2, 4, 8} ; nombre de champs de 35 à 60 (4 fichiers à
   4 champs, en-tête probablement incomplet). Dans le firmware, ces deux intervalles sont, de mémoire, des entiers 16 bits
   (plafond naturel 65 535, à confirmer dans les sources Betaflight au lot 4). **Bornes retenues, accord de Cédric le 2026-11-04** : 512 champs au
   plus, intervalles entre 1 et 65 535 ; c'est dix à cent fois au-dessus de tout vrai log, donc aucun risque d'en rejeter un,
   et assez bas pour que le pire cas reste instantané. Le lot 4 écrira la décision `D-xxx` correspondante.
3. **Contenu d'`AGENTS.md` (lot 2).** Le plan propose de retirer l'historique Python et de ramener le bloc « Organisation » à
   une ligne par crate (le détail reste dans `docs/architecture.md`, que le test de documentation accepte).
   **Accord de Cédric (2026-11-04).**
4. **Protection de la branche `main` (action manuelle, hors code).** Le ruleset exige la CI mais zéro approbation : avec le
   `gh` du propriétaire, un agent peut fusionner seul. **Fait par Cédric le 2026-11-04** (vérifié par `gh api` : une
   approbation requise, approbations périmées rejetées à chaque nouveau commit, contournement du rôle administrateur
   gardé « pour les PR seulement »). Comme toute règle GitHub est contournable par le compte du propriétaire, le vrai
   verrou est côté Claude Code : `.claude/settings.json` refusait `git push`, `gh pr merge`, `git tag` et
   `gh release` (accord de Cédric le même jour). **Changé par Cédric le 2026-11-04** : ces quatre commandes sont en « demander »
   (dans ce projet et dans `~/.claude/settings.json`, donc tous ses dépôts) : l'agent ne les lance qu'avec sa confirmation à
   chaque fois. La règle entre aussi dans `AGENTS.md` au lot 2.
5. **Première publication Rust.** W-046 (police Ubuntu Light face à la licence du projet) la bloque et relève d'une décision de
   Cédric ; depuis D-045 (licence PolyForm Noncommercial, qui n'est pas un copyleft), la cause du conflit a disparu — il reste à clore.
   **Cédric (2026-11-04) : à traiter aussi.** Un lot 7 est réservé en fin de plan ; il ne se détaille qu'une fois W-046 tranché.

## Lot 0 : sonde du corpus et des références (Sonnet, une courte session)

**But** : savoir, avant de toucher au décodeur, avec quoi on prouvera l'absence de régression, et relever les mesures de
départ que le lot 5 devra battre.

**À lire** : `docs/validation.md` (procédures), `.claude/skills/calculs-corpus/SKILL.md`, `rust/otb-dev/src/corpus/mod.rs`
(variables d'environnement, dossier par défaut).

- [x] Relever sur ce poste : `OTB_LOGS` (défaut `~/Documents/Drones/Logs`, 525 logs et `corpus.csv` vus le 2026-11-04),
      présence d'un dossier `golden/`, présence d'un `blackbox_decode` de référence ; noter le résultat dans « Avancement ».
- [x] Si les références manquent : poser la question 1 ci-dessus à Cédric et **s'arrêter là** (critère d'arrêt).
- [x] Si elles existent : lancer `cargo run --release -p otb-parity -- check "$OTB_GOLDEN"` et
      `cargo run --release -p otb-dev -- corpus validate` ; les deux doivent être à 0 écart sur `main` : c'est la ligne de base.
- [x] Mesures de départ pour le lot 5, sur le plus gros log du corpus : temps de `opentunebox info` et pic mémoire
      (gestionnaire des tâches ou `Measure-Command` + compteur `PeakWorkingSet`), notées dans `docs/validation.md`.
- [x] Vérification : aucune ; ce lot ne change pas de code. Livrable : la section « Avancement » remplie, et si besoin
      les variables d'environnement à définir, préparées pour Cédric (pas de `sudo` requis).

## Lot 1 : commande unique `otb-dev verify` et crochet réparé (Sonnet) — C3

**But** : qu'un agent puisse vérifier seul son travail par une commande qui rend 0 ou 1, et que le crochet de pré-commit
bloque réellement.

**À lire** : `rust/otb-dev/src/main.rs:run` (aiguillage des sous-commandes), `.claude/hooks/check-docs.sh`,
`.claude/settings.json`, `rust/otb-parity/tests/golden.rs` et `rust/otb-advisor/tests/smooth.rs` (tests sautés en silence),
`docs/lecons.md` L-069 (profil `dev` et disque de la CI), L-023 (tests ignorés sans bruit).

- [x] Sous-commande `verify` dans `otb-dev` : enchaîne `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --no-fail-fast` (sans la liste des `-p`, équivalente : 74 exécutables dans les deux cas), puis
      `cargo deny check` si `cargo-deny` est installé (sinon avertissement visible) ; code de sortie 0 seulement si tout passe ;
      option `--quick` sans `deny`. Afficher la durée de chaque étape.
- [x] Tests sautés rendus visibles : `golden.rs` et `smooth.rs` affichent leur saut sur la sortie standard **et** `verify` le
      compte ; option `--strict` : un saut vaut échec (pour le poste qui a le corpus). Ne pas transformer le saut en
      `#[ignore]` sans vérifier que `cargo test` continue de les lister.
- [x] Crochet : plus de `jq`. Lire la commande dans le JSON avec `sed`/`grep` POSIX, ou échouer bruyamment (code 2 et message)
      si l'outil manque, jamais `exit 0` en silence. Garder le motif `git commit` et ajouter `git -C … commit`.
- [x] Remplacer la liste des `-p` par `cargo test --no-fail-fast` dans `.github/workflows/tests.yml`, `CONTRIBUTING.md`,
      `AGENTS.md`, `README.md` (une seule source : la ligne de `verify`).
- [x] Documentation : `AGENTS.md` (bloc « Commandes »), `CONTRIBUTING.md`, `docs/architecture.md` (module de `otb-dev`),
      `CHANGELOG.md`.
- [x] Vérification : `cargo run -p otb-dev -- verify` rend 0 sur `main`. **Contre-preuves** : (a) casser un test, `verify`
      rend 1 ; (b) ajouter un espace de formatage, 1 ; (c) `verify --strict` sans `OTB_GOLDEN`, 1 ; (d) crochet : avec
      `OTB_DOCS_ROOT` vers une copie sabotée (lien cassé), un `git commit` est bloqué avec le code 2 ; sans `jq` installé, il
      bloque quand même. Mesurer et noter la durée totale (≈ 100 s à chaud attendu).
- [x] Critère d'arrêt : si `cargo test` sans `-p` ne donne pas les mêmes 74 exécutables, chercher pourquoi avant de changer
      la CI.

## Lot 2 : consignes et CI remises à jour (Sonnet) — C5

**But** : qu'`AGENTS.md` ne contienne plus rien de faux, coûte moins à chaque session, et dise à un agent ce qu'il ne doit
jamais faire.

**À lire** : `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `docs/architecture.md`, `.github/workflows/tests.yml`,
`.github/workflows/audit.yml`, `rust/otb-dev/src/docs.rs` (ce que le test de documentation exige : chemins, modules, index).

- [x] Corriger les huit points faux : « interface Qt » (ligne 4), branche `feat/rust-migration` (lignes 24-25, et la section
      « Migration vers Rust » de `CONTRIBUTING.md`), « pas de compilateur C » (D-035 dit le contraire), « 15 tests », fmt et
      clippy « sur Ubuntu et Windows » (Ubuntu seulement), `gui-build.yml` non cité, commande de capture sans
      `-- LOG.bbl DOSSIER`, détecteur de couleurs (6 chiffres hexadécimaux seulement, `#777` passe).
- [x] Retirer `feat/rust-migration` de `branches:` dans `tests.yml` et `audit.yml` ; relire `docs/migration-rust.md` : ajouter
      en tête « Terminé le 2026-11-04 (PR 96) », sans réécrire l'histoire.
- [x] Alléger (après accord, question 3) : bloc « Organisation » à une ligne par crate ; historique Python réduit à une phrase
      avec renvoi vers `docs/architecture.md` ; renvois L-036 et L-038 (époque PyO3) retirés des pièges génériques ; liste des
      compétences retirée de `CLAUDE.md` (elles se chargent d'après leur description).
- [x] Ajouter : la commande `verify` et sa durée ; les règles explicites (`git push`, `git tag`, `gh pr merge` et `gh release` :
      seulement avec la confirmation de Cédric à chaque fois ; l'étiquette `v*` reste posée par lui ; ne pas modifier `$OTB_LOGS`, `golden/` ni `corpus.csv`, ne jamais affaiblir un
      test pour le faire passer) ; la consigne « fichiers temporaires hors du dépôt » ; le poste de développement (Windows 11 et
      Linux : commandes données pour les deux ou en POSIX avec la mention) ; les variables `OTB_DEV_ROOT`, `OTB_BITEXACT`,
      `OTB_FORMAT_CASES`.
- [x] Mettre d'accord `AGENTS.md` et `docs/lecons.md` sur ce qu'il faut lire en début de session (une seule consigne) ; mettre
      à jour le tableau de `.claude/skills/docs-sync/SKILL.md` (les pièges d'un domaine vont dans `docs/pieges-par-domaine.md`).
- [x] Vérification : `cargo run -p otb-dev -- verify` vert (le test `docs` y est inclus) ;
      `grep -rn "feat/rust-migration" AGENTS.md CONTRIBUTING.md .github/` ne rend que des mentions historiques explicites, ou
      rien ; `AGENTS.md` réduit d'au moins un tiers (147 lignes, 12,6 Ko le 2026-11-04). **Contre-preuve** : retirer un
      module de `docs/architecture.md`, le test `every_module_documented` doit rougir (preuve que l'allègement
      d'`AGENTS.md` n'a pas supprimé le filet).

**Avancement (2026-11-04, branche « agents-refresh »)** : `verify` vert en 66-69 s à chaud. `AGENTS.md` : 12 608 → 8 390 octets (−33 %),
147 → 102 lignes (−31 %). `grep feat/rust-migration` : une seule mention, historique, dans `CONTRIBUTING.md`. Contre-preuve : un module retiré
d'`architecture.md` et d'`AGENTS.md` fait rougir `every_module_documented`. Détecteur de couleurs : il refuse `#` + 6 chiffres hexadécimaux seulement
(`#777` passe), documenté mais non modifié. La branche locale `feat/rust-migration` existe encore (supprimée sur GitHub) : à effacer par Cédric s'il le souhaite.

## Lot 3 : résultat périmé du socle des outils (Sonnet) — C2

**But** : qu'une page d'outil n'affiche jamais un résultat qui ne correspond pas aux logs et réglages courants.

**À lire** : `rust/otb-gui/src/tool.rs` (`ToolPage`, `update`, `run`), `rust/otb-gui/src/worker.rs:Runner`,
`rust/otb-gui/tests/tool.rs` (barrière de test déjà utilisée pour bloquer un calcul, compteur d'appels),
`docs/lecons.md` L-071 (prouver une protection en la cassant), W-036 point (8) (correction locale faite dans le Simulateur
par un identifiant, à généraliser).

- [x] **Test d'abord** (rouge) dans `rust/otb-gui/tests/tool.rs` : bloquer le calcul par la barrière, vider l'état
      (`clear`), avancer d'une image, libérer la barrière, avancer d'une image → le résultat doit être `None` et le statut
      « Sélectionnez au moins un log. ». Second test : changer la sélection pendant un calcul → le résultat de l'ancienne
      sélection ne s'affiche jamais (vérifier le contenu, pas seulement l'absence).
- [x] Correction : numéroter chaque demande (`request`) et faire porter le numéro par le résultat ; `update` ignore tout
      résultat dont le numéro n'est pas celui de la dernière demande (`asked`). Ne pas toucher au cache (`state.rs`) : L-071.
- [x] Vérifier que le Simulateur de filtres (correction locale W-036 (8)) continue de passer ses tests, puis retirer son
      mécanisme propre s'il devient redondant (sinon le laisser et le dire).
- [x] Documentation : `docs/architecture.md` (socle des outils), W-036 (8) mis à jour, `CHANGELOG.md` (*Corrigé*).
- [x] Vérification : `verify` vert ; **contre-preuve** : revenir au `update` d'origine, les deux nouveaux tests rougissent ;
      `cargo run --release -p otb-gui --example check_tools` sans écart.
- [x] Critère d'arrêt : si `egui_kittest` ne permet pas d'avancer d'une seule image entre le `clear` et la libération,
      le dire et proposer un test de `Runner` seul plutôt que d'affaiblir le test.

## Lot 4 : décodeur robuste face à un en-tête hostile (Opus) — C1

**But** : qu'un fichier `.bbl` malformé ou hostile donne toujours une erreur propre, en moins d'une seconde, sans jamais
emporter le programme.

**À lire** : `rust/otb-decoder/src/header.rs`, `rust/otb-decoder/src/lib.rs` (`decode_stream`, `should_have_frame`,
`Table`), `rust/otb-decoder/src/file.rs`, `rust/otb-gui/src/app.rs:poll_opening` (ouverture dans un thread, un fichier à la
fois), `rust/otb-parity/tests/decode_fixtures.rs` (70 fichiers abîmés déjà couverts), `docs/pieges-par-domaine.md` section
décodeur, `docs/validation.md`.

Sous-lot 4a, tests rouges (même PR) :

- [x] Nouveau fichier de tests d'intégration *robustness.rs* dans `rust/otb-decoder/tests/` : fabriquer les variantes **en
      mémoire** à partir de `tests/data/jeno5_5s.bbl` (remplacer les lignes `H I interval:` et `H P interval:`, dupliquer la
      ligne des noms de champs pour en déclarer 20 000, mettre tous les réglages à zéro) ; aucun nouveau fichier binaire dans
      le dépôt. Chaque cas : `decode_bytes` rend une erreur ou zéro session, en moins d'une seconde (chronomètre dans le
      test), et le test ne doit jamais abandonner le processus (lancer d'abord un à un).
- [x] Rejouer les variantes gardées dans le bac à sable de la session du 2026-11-04 si elles existent encore ; sinon les
      refaire : tronqué 50 %, en-tête coupé, vide, corps à 0xFF, dix mutations à 1 % (déjà propres : servent de non-régression).

Sous-lot 4b, correction :

- [x] Bornes à la lecture de l'en-tête (question 2) : nombre de champs, `I interval`, `P interval` ; hors bornes → erreur
      typée, message en français, session refusée.
- [x] Croissance de `Table` : réserver par `try_reserve` **avant chaque ajout** qui dépasse la capacité (ou borner les lignes
      par la taille du fichier : jamais plus de trames que d'octets), et transformer l'échec en erreur « fichier trop grand pour
      la mémoire disponible » remontée jusqu'à la barre d'état de l'interface.
- [x] Boucle de recherche de l'itération attendue : borne dure (la constante `MAX_ITERATION_JUMP` existe déjà pour un autre
      cas) ; au-delà, trame déclarée corrompue, comme aujourd'hui pour une trame trop grande.
- [x] Réglages à zéro : `otb-log` garde ses `.max(1)`, mais l'interface et la CLI doivent signaler « réglages de l'en-tête
      invraisemblables » plutôt qu'afficher 1 000 000 Hz (constat, pas d'invention de valeur).
- [x] Documentation : `docs/architecture.md`, `docs/validation.md` (section robustesse : cas et résultats),
      `docs/pieges-par-domaine.md`, `docs/lecons.md` (nouvelle leçon : une allocation ratée n'est pas rattrapable par
      `catch_unwind`), `CHANGELOG.md`.
- [x] Vérification : `verify` vert ; `cargo run --release -p otb-dev -- corpus validate` à 0 non conforme et
      `otb-parity check` à 0 écart (lot 0) : **aucun vrai log ne doit changer de résultat**. **Contre-preuve** : retirer une
      borne, le test correspondant rougit (ou bloque : le lancer avec `timeout`).
- [x] Critère d'arrêt : si une borne proposée rejette un vrai log du corpus, s'arrêter et consigner le fichier et la valeur.

## Lot 5 : mémoire du décodeur (Opus) — C4

**But** : ouvrir un gros log sans multiplier son poids par neuf en mémoire, sans changer un seul bit des résultats.

**À lire** : `rust/otb-decoder/src/file.rs` (`decode_bytes`, `list_sessions`, `decode_file`, `forward_fill`),
`rust/otb-decoder/src/lib.rs:Table`, `rust/otb-log/src/flightlog.rs` (`load`, `from_session`, colonnes gardées),
`docs/travaux-reportes.md` W-014 et W-016, `docs/decisions.md` D-029 (courbes en `f32`), `docs/lecons.md` L-051 (ordre des
opérations flottantes et parité).

Sous-lot 5a, banc et conversion session par session (PR `perf/decoder-memory`) :

- [ ] Banc reproductible : exemple ou sous-commande `otb-dev` qui ouvre un log donné, affiche la durée et le pic mémoire
      (compteur du système, pas d'estimation) ; l'exécuter sur `tests/data/jeno5_5s.bbl` et sur le plus gros log du corpus ;
      consigner les valeurs de départ (lot 0) dans `docs/validation.md`.
- [ ] `load` : lister les sessions puis décoder et convertir **une session à la fois** (le paramètre `only` de `decode_file`
      existe déjà), en libérant la table `i64` avant la suivante.
- [ ] Vérification : `verify` vert ; `corpus validate` 0 non conforme ; `otb-parity check` 0 écart ; pic mémoire du plus gros
      log **divisé par au moins 1,5** par rapport au lot 0, mesure consignée. **Contre-preuve** : le banc rejoué sur `main`
      redonne l'ancienne valeur.

Sous-lot 5b, colonnes utiles seulement (PR `perf/decoder-columns`, ouverte seulement si 5a ne suffit pas à l'objectif « ÷ 2 ») :

- [ ] Décoder vers la table `i64` uniquement les colonnes que `from_session` garde, ou convertir en `f32` à la volée par
      blocs de trames ; les trames `S`, `E`, `G`, `H` gardent leur traitement actuel.
- [ ] Vérification : mêmes preuves qu'en 5a ; objectif pic mémoire **÷ 2** et temps d'ouverture non dégradé (W-014).
- [ ] Critère d'arrêt : un seul bit qui change sur le corpus (`otb-parity`) arrête le sous-lot ; on revient à 5a seul.

## Lot 6 : bilan (Sonnet)

- [ ] Relire les cinq chantiers : état, preuves, mesures ; mettre à jour `docs/validation.md` (robustesse, mémoire) et ce plan.
- [ ] Clore ou mettre à jour les W concernés : W-014, W-016, W-035 (9), W-036 (8), W-041 si touché.
- [ ] Leçons à ajouter dans `docs/lecons.md` : allocation ratée non rattrapable ; crochet dépendant d'un outil absent ;
      résultat de thread sans étiquette.
- [ ] Proposer à Cédric la suite : première publication Rust (bloquée par W-046), cache borné (W-035 (9)), factorisation des
      pages spectrales, lints pédantiques ciblés (**jamais** `suboptimal_flops` : L-051).

## Lot 7 : première publication Rust (réservé)

Demandé par Cédric le 2026-11-04 ; ne se détaille qu'une fois W-046 tranché (garder la police Ubuntu Light, ou la retirer au profit
d'une autre police). Depuis D-045, la licence du projet n'est plus un copyleft : la cause du conflit a disparu, la clôture reste à écrire. À prévoir alors : `otb-dev licenses`, `otb-dev release
prepare`, essai complet de la chaîne par une étiquette `essai-v…` (procédure dans `CONTRIBUTING.md`), puis l'étiquette `v…`
posée **par Cédric lui-même**.

- [ ] W-046 tranché (décision `D-xxx` écrite).
- [ ] Lot détaillé ici, avec ses preuves (installateurs essayés sur Windows et Linux).

## Avancement

| Lot | État |
|---|---|
| 0 | Fait le 2026-11-04 sur le poste Linux : références présentes ; `otb-parity` 0 écart ; `corpus validate` 697/712 sessions conformes (les 15 autres sont des écarts connus, la ligne de base n'est donc pas « 0 écart » comme le plan le supposait) ; départ mémoire 491 Mo sur `LOG00008.BFL`. Détail dans `docs/validation.md` (« Ligne de base ») |
| 1 | Fait le 2026-11-04 (branche `feat/dev-verify`) : `otb-dev verify [--quick] [--strict]`, marqueur `[OTB-SAUT]`, crochet sans `jq`, listes de `-p` retirées (tests.yml, CONTRIBUTING, README, AGENTS). 74 exécutables avec et sans `-p` (critère d'arrêt levé). `verify` : 63–67 s à chaud sur ce poste, code 0 sur le code actuel. Contre-preuves : (a) test cassé → 1, (b) espace de formatage → 1, (c) `--strict` sans `OTB_GOLDEN` → 1 (2 sauts comptés), (d) docs sabotées (lien cassé) : le crochet rend 2 sans `jq` dans le PATH pour `git commit`, la forme avec `-C` et celle avec `-c`, et 0 sans sabotage. Non traités : les autres sauts de tests (`otb-gui`…) ; W-047 |
| 2 | À faire |
| 3 | À faire |
| 4 | Fait le 2026-11-04 (branche `fix/decoder-hostile-header`, D-044, L-080) : `DecodeError`, bornes 512 champs / intervalles 1 à 65 535, `try_reserve`, `Settings::implausible`. `robustness.rs` : 10 cas, 6 rouges avant correction (les intervalles à 9·10¹⁸ ne bouclaient pas sur cette source : refusés par la borne seulement) ; contre-preuves : 4 retraits rougissent. `otb-parity` 0 échec, `corpus validate` 697/712 comme la ligne de base |
| 5 | À faire |
| 6 | À faire |
| 7 | Réservé (attend W-046) |
