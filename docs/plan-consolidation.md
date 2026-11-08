# Plan de consolidation : revue intégrale du projet (lot 16)

Suite de [plan-suite.md](plan-suite.md). Celui-ci traitait **ce qui restait à faire** (travaux reportés, publication,
calibration). Ce plan-ci traite **ce qui existe déjà** : le code, les tests, la documentation et l'outillage, relus
intégralement le 2026-11-08 à la demande de Cédric, pour que le projet soit « propre, complet et à jour ».

## Ce qui a été relu, et comment

Quatre relevés menés en parallèle sur l'état réel du dépôt (branche `chore/registers-cleanup`, après le lot 12), chacun
avec pour consigne de **vérifier dans le fichier** et de dire « non vérifié » sinon :

| Relevé | Périmètre | Volume |
|---|---|---|
| Code | `rust/*/src/` : règles du projet, fragilité, code mort, duplication | ~41 000 lignes, 106 fichiers |
| Tests | `rust/*/tests/`, `rust/*/examples/`, données de test | ~20 500 lignes, 106 fichiers de données |
| Documentation | 30 `.md` : README, AGENTS, CONTRIBUTING, `docs/`, les 5 compétences | ~8 000 lignes |
| Outillage | `.github/`, `.claude/`, `packaging/`, manifestes Cargo, licences | 4 workflows, 11 `Cargo.toml` |

Les constats les plus lourds ont ensuite été **revérifiés à la main**, et deux ont été corrigés à la baisse :

- Les 71 fichiers `.bbl` versionnés ne pèsent que **892 Ko** (70 sont des logs fabriqués ou abîmés pour le décodeur).
  Ce n'était pas un problème : le poids du dépôt vient des JSON de cas.
- Le tableau des outils du README **n'est pas faux** : le contrôle `otb-dev --test docs` exige que le titre réel de
  chaque page figure en colonne 1 **ou** 2, et la colonne 2 le contient. Ce qui est trompeur est le **nom de la
  colonne** (« Bouton »), qui annonce le libellé de la barre du haut alors qu'elle porte celui du guide intégré.

## Ce que Cédric a tranché le 2026-11-08

1. **D-050 ne visait que l'auteur de PIDtoolbox.** Les références bibliographiques d'ouvrages publiés gardent leurs
   auteurs : c'est la façon normale de citer un livre, et D-005 exige que chaque seuil soit justifié par une méthode
   publiée. D-050 est **amendée** pour lever l'ambiguïté de sa formulation (« aucun nom de personne, nulle part »).
2. **Données de test (133 Mo)** : mesurer avant de décider. Essai de compression sur les dix plus gros, chiffres
   présentés, décision ensuite. Rien n'est compressé dans ce lot sans son accord.
3. **Dette de structure** : ne rien remanier sans bénéfice démontrable. Un gros fichier testé qui marche n'est pas un
   défaut ; le découper risquerait d'abîmer des calculs validés au bit près. Seule la **duplication qui crée un risque
   d'oubli** est traitée.
4. **Un seul lot, une seule demande de fusion.** Elle sera découpée en **commits thématiques** (un par étape ci-dessous)
   pour rester relisible. ⚠️ Risque signalé et assumé : une PR de cette taille est difficile à relire d'un bloc et ne
   peut pas être annulée point par point — seulement commit par commit.

## Lot 16 : étapes

Chaque étape est **un commit**. L'ordre va du plus visible par un pilote au moins visible.

### 16a — Ce que le pilote lit et ne retrouve pas dans l'application

Défauts vérifiés dans le code, qui trompent un utilisateur :

- [x] **`Ctrl+O` n'existe pas.** `docs/guide-utilisateur.md` annonce « Bouton bleu **Ouvrir** de la barre d'outils (ou
      Ctrl+O) ». Il n'y a **aucun** raccourci clavier dans `rust/otb-gui/src/` (ni `KeyboardShortcut`, ni `Key::`).
      Deux issues : retirer la mention, **ou** ajouter le raccourci. À décider avec Cédric — ajouter un vrai `Ctrl+O`
      est peu de code et rend service ; c'est le seul point de cette étape qui peut toucher au code.
- [x] **Le bouton « Visualiseur » s'appelle « Courbes ».** `guide-utilisateur.md` parle du bouton « Visualiseur » et dit
      « **Visualiseur** ramène aux courbes » ; la barre affiche « Courbes » (`rust/otb-gui/src/app.rs:App`). Un pilote
      cherche un bouton qui n'existe pas.
- [x] **Trois jeux de noms pour les mêmes outils.** Les titres des pages (« Analyse spectrale », « Simulateur de
      filtres »), les libellés courts du guide intégré (`rust/otb-gui/src/guide.rs:TOOLS` : « Spectre », « Filtres »,
      « Visualiseur ») et ceux de la documentation ne coïncident pas. « Indicielle » (README, colonne « Bouton »)
      n'existe dans aucun des deux jeux. Geste : renommer la colonne du README en « Dans le guide », et faire dire au
      guide utilisateur le **libellé de la barre** quand il parle d'un bouton de la barre.
- [x] **« Python est inclus ».** `docs/guide-utilisateur.md` dit encore « Python et les bibliothèques sont inclus ».
      Faux depuis la 0.9.0 : il n'y a plus de Python du tout.
- [x] **Thème mal situé.** Le guide place « thème clair ou sombre » dans la section Affichage ; c'est la case « Thème
      sombre » de la barre du haut (`rust/otb-gui/src/app.rs:App`). La section Affichage
      (`rust/otb-gui/src/viewer_page.rs:ViewerPage`) ne contient que « Lissage » et « Épaisseur ».
- [x] **Une seule section est repliable.** `guide-utilisateur.md` et `docs/architecture.md` disent « les sections
      Affichage et Lecteur se replient d'un clic » : « Lecteur » est une simple étiquette.
- [x] **Options d'installation présentées comme acquises.** `README.md` annonce « menu Démarrer, Bureau, ouverture des
      `.bbl` » ; dans `packaging/windows/opentunebox.iss`, Bureau et association sont des **cases facultatives** (le
      guide le dit, le README non).
- [x] **Le sélecteur de fichiers accepte `.txt`.** Il propose `txt`/`TXT`
      (`rust/otb-gui/src/app.rs:App`) alors que les logs KISS `.TXT` ne sont **pas** décodés (hors périmètre, W-001).
      Un pilote peut choisir un fichier qui échouera sans comprendre pourquoi. Geste : retirer ces deux extensions du
      filtre, ou expliquer le refus dans la barre d'état. Décision à prendre en lisant le message actuel.
- [x] **`opentunebox info` sans moyen de l'obtenir.** `README.md` documente la ligne de commande « depuis les sources »
      sans donner la commande qui produit le binaire ; l'installateur n'embarque que l'interface.
- [x] **Preuve exigée** : chaque libellé corrigé est relu **dans l'application lancée**
      (`cargo run --release -p otb-gui`), pas seulement dans le code. Capture pour les deux thèmes si l'aspect change.
      **Fait autrement, et c'est volontaire** : aucun libellé de l'interface n'a été changé. Les écarts étaient tous du côté des
      documents, qui annonçaient ce que l'application ne fait pas ; ce sont eux qui ont été alignés sur les libellés réels, relevés
      dans le code puis dans l'application. Rien n'a changé d'aspect, donc aucune capture n'était due.

### 16b — Le filet de tests là où il manque vraiment

- [x] **`otb-parity` : 4 990 lignes, presque aucun test.** Ses onze comparateurs — `check_header`, `check_decoder`,
      `check_settings`, `check_flightlog`, `check_sliders` (`rust/otb-parity/src/layers.rs:check_header`),
      `check_analysis` (`rust/otb-parity/src/analysis_layer.rs:check_analysis`, 822 lignes),
      `check_advisor` (`rust/otb-parity/src/advisor_layer.rs:check_advisor`),
      `check_guide` (`rust/otb-parity/src/guide_layer.rs:check_guide`),
      `check_tuning` (`rust/otb-parity/src/tuning_layer.rs:check_tuning`),
      `check_cli` (`rust/otb-parity/src/cli_layer.rs:check_cli`) — ne sont appelés par **aucun** test, seulement à la
      main. C'est l'outil qui garantit que le Rust donne les mêmes résultats que l'étalon : s'il se trompe, il se
      trompe en silence. Geste : au moins un test par comparateur sur une référence **fabriquée** (pas le corpus), qui
      prouve qu'il détecte un écart introduit exprès. C'est l'étape la plus utile du lot.
- [x] **Fonctions prouvées seulement par le corpus.** `pt1_gain` et `notch_q` (`rust/otb-dsp/src/lib.rs:pt1_gain`),
      `freq_response` (`rust/otb-analysis/src/bf_filters.rs:freq_response`), et dans
      `rust/otb-log/src/flightlog.rs:FlightLog` : `chirp_excitation`, `debug_name`, `term_present`, `pid_terms`,
      `slice_time`, `cell_count`. Leur seule preuve passe par `$OTB_GOLDEN`, donc **la CI ne les vérifie pas**. Or
      `pt1_gain` et `notch_q` sont au cœur des filtres Betaflight. Geste : un test de cas écrit pour chacune.
- [x] **Trois tests disparaissent sous Windows sans rien dire.** `rust/otb-dev/tests/install_linux.rs` est entièrement
      sous `#![cfg(unix)]` : sous Windows, ses trois tests s'évaporent **sans message**, alors que le projet exige
      qu'un test sauté s'annonce par `[OTB-SAUT]`. Geste : afficher le marqueur, ou dire explicitement dans le fichier
      que ces tests sont réservés à Unix.
- [x] **Le garde-fou des sauts est codé en dur.** `rust/otb-dev/tests/verify.rs` ne vérifie la présence de
      `[OTB-SAUT]` que dans **deux fichiers nommés à la main** : un troisième test conditionnel passerait inaperçu.
      Geste : chercher les conditions de saut au lieu de les énumérer.
- [x] **`slice_time` ne protège pas son invariant.** `rust/otb-log/src/flightlog.rs:FlightLog` rend
      `index_of(t0)..index_of(t1)+1` : si `t0 > t1`, la plage est inversée et tout découpage qui s'en sert **panique**.
      Aucun chemin atteignable aujourd'hui (l'interface ordonne la période), mais l'invariant repose sur les appelants.
      Geste : ordonner les bornes dans `slice_time`, avec un test.
- [x] **Tests fragiles, à borner et non à supprimer** : `rust/otb-decoder/tests/robustness.rs` affirme `dt < 1 s` sur du
      temps réel (échoue sur une machine chargée) ; une vingtaine de boucles d'attente d'`otb-gui` ont un plafond de 10
      à 60 s ; `rust/otb-gui/tests/tool.rs` compare des **compteurs globaux absolus** (`== 1`, `== 3`) et ne tient que
      parce qu'aucun autre test du fichier n'utilise le même objet d'essai. W-041 reste traité à part (il attend un
      échec réel en CI).
- [x] **Preuve exigée** : chaque test nouveau est **vu en échec** avant d'être accepté (sabotage du code visé), comme
      le projet l'impose. Pour les comparateurs d'`otb-parity`, le sabotage est un écart glissé dans la référence.

### 16c — Ce qui peut casser sans prévenir (CI, crochets, publication)

- [x] **La chaîne Rust n'est pas figée.** Chaque job fait `rustup toolchain install stable` : une nouvelle version de
      Rust peut rendre la CI rouge du jour au lendemain, sans qu'on ait rien changé. Il n'y a pas de
      `rust-toolchain.toml`. Geste : en ajouter un, et le mettre à jour volontairement.
- [x] **`fmt` et `clippy` ne tournent que sous Ubuntu.** Le code sous `cfg(windows)` n'est donc **jamais** passé au
      lint. Geste : ajouter clippy au job Windows.
- [x] **Le crochet de documentation laisse passer des commits.** `.claude/hooks/check-docs.sh` : au-delà de 50 s
      (compilation à froid) ou si `cargo` est absent, il **avertit et laisse passer** — c'est L-087, déjà connu. Pire,
      si `timeout` manque, il **bloque** avec un message trompeur. Geste : distinguer les trois cas dans le message.
- [x] **`gh workflow run` n'est pas soumis à confirmation.** `.claude/settings.json` met `git push`, `git tag`,
      `gh pr merge` et `gh release` en « demander », mais un `gh workflow run construire.yml` lancerait la chaîne de
      construction sans rien demander. Geste : ajouter le motif.
- [x] **Un installateur « 0.0.0 » peut sortir en silence.** `packaging/windows/opentunebox.iss` prend `AppVersion`
      par défaut à `0.0.0` quand `/DAppVersion=` est oublié, sans erreur. Geste : échouer au lieu de produire une
      version fantaisiste.
- [x] **Workflow fantôme : rien à faire, le relevé se trompait.** GitHub garde bien une entrée « Interface Windows »
      dont le fichier (`gui-windows.yml`) a été supprimé, mais son état est `disabled_manually`, pas `active` : vérifié le
      2026-11-08 par `gh api repos/:owner/:repo/actions/workflows`. Il ne se déclenche pas et ne compte pas parmi les
      workflows actifs. L'API ne sait de toute façon que désactiver un workflow, jamais le supprimer : l'entrée restera
      tant que GitHub gardera l'historique de ses passages. **Sixième constat des relevés corrigé après vérification.**
- [x] **`gui-build.yml` envoie un artefact hors lancement manuel.** Son `upload-artifact` tourne aussi sur les demandes
      de fusion qui modifient ce fichier, alors que la règle de Cédric ne le tolère qu'en lancement manuel.
      `retention-days: 1` est bien respecté. Geste : conditionner l'étape à `workflow_dispatch`.
- [x] **Deux échecs masqués par un tube**, à corriger sans réécrire les workflows :
      `construire.yml` liste les fichiers d'une version avec `gh … | sort | tee` (un échec de `gh` est rattrapé par le
      `diff` suivant, mais par accident), et `gh release view … || gh release create …` traite **toute** erreur de
      `view` — réseau compris — comme « la page n'existe pas ».
- [x] **Deux dépendances déclarées pour rien** : `flate2` et `otb-decoder` en `[dev-dependencies]` d'`otb-advisor`,
      aucune occurrence dans sa crate. Geste : les retirer, vérifier que les tests passent toujours.
- [x] **Pas de `[workspace.dependencies]`.** Les versions externes sont écrites crate par crate, avec des notations
      différentes pour la même dépendance (`rayon` « 1.12.0 » ici, « 1 » là). Aucun conflit aujourd'hui. Geste :
      centraliser, sans changer une seule version.
- [x] **Aucun champ `license` dans les 11 `Cargo.toml`.** Le dépôt est sous PolyForm Noncommercial 1.0.0 ; les
      manifestes ne le déclarent pas. Sans effet (les crates ne sont pas publiées), mais c'est une information
      manquante. Geste : ajouter `license-file` ou l'identifiant.
- [ ] **Preuve exigée, toujours en attente** : la demande de fusion fait tourner la CI sous Ubuntu **et** Windows, et le
      nouveau test de sortie redirigée (W-018, lot 12) y reçoit sa confirmation Windows. C'est la **seule** case du lot qui
      dépende d'autre chose que du dépôt : elle se cochera quand la CI de la demande de fusion sera verte des deux côtés.

### 16d — Chiffres faux et affirmations périmées dans la documentation

Chacun est un chiffre contredit par la mesure ou par un autre document :

- [x] **Taille du corpus** : `docs/architecture.md` annonce « environ 780 sessions (582 fichiers) et 638 captures ».
      Mesuré : **524** fichiers de logs et **595** captures. C'est le seul chiffre non daté du lot, donc le plus
      trompeur. `docs/validation.md` donne 529/712 (2026-11-04) puis 525/709 (2026-11-06) ; `docs/spec-ecrans.md` 638
      captures ; `docs/guide-utilisateur.md` « ~700 vols ».
- [x] **Fichiers de tests lisant le log d'essai** : `AGENTS.md` dit « 11 fichiers de tests », il y en a **14**.
- [x] **Exécutables de test** : `docs/architecture.md` dit « 74 », le compte actuel est d'environ **81**.
- [x] **Tests sous wine** : `CHANGELOG.md` dit « 85 sur 89 », la compétence `opentunebox-dev` « 85 sur 90 ».
- [x] **Durée de la CI Windows** : la compétence `opentunebox-dev` annonce « 10 à 25 min » ; les douze derniers
      passages donnent **6 à 7,5 min**.
- [x] **Objectif de concordance du décodeur** : la compétence `opentunebox-dev` fixe « ne pas faire baisser `ok`
      (95 %) » ; la mesure en vigueur est **88 %** (627 sessions sur 712) depuis la reclassification `gap`/`slow`. Un
      objectif inatteignable n'en est plus un.
- [x] **`validation.md`** s'annonce « Mesures du 2026-10-01 » alors qu'il contient des mesures jusqu'au 2026-11-06.
- [x] **`architecture.md`** situe les références de test dans `tests/data/*.json` : elles sont dans
      `rust/*/tests/data/` (le dossier racine ne contient qu'un log).
- [x] **`CONTRIBUTING.md`** : l'exemple d'étiquette d'essai `essai-v0.8.0` **échouerait** au contrôle de version
      (`Cargo.toml` est à 0.9.0) ; les exemples `0.2.0` renvoient à des étiquettes déjà posées. Et il affirme que
      l'audit tourne « à chaque envoi » : il n'écoute que `main`.
- [x] **Index des décisions** : `docs/decisions.md` liste D-001, D-002, D-009, D-030 et D-043 sans marquer qu'elles ne
      sont plus en vigueur (Python, PySide6, ruff, GPL). La marque existe dans le corps, pas dans l'index.
- [x] **Documents de plan périmés** : `docs/README.md` et `docs/plan-chantiers.md` annoncent encore la publication de
      la 0.9.0 comme restant à faire.
- [x] **Compétences avec des restes de l'ère Python** : `assistant-seuils` (`provisional=True`, scripts supprimés, et
      un **chemin en dur vers le poste de Cédric**), `calculs-corpus` (`NUMBA_BOUNDSCHECK`, `PYTHONHASHSEED`, durées
      « non mesurées en Rust »), `guide-integre` (`exact=True`).

### 16e — Trous de documentation

- [x] **Exemples non décrits** dans `docs/architecture.md` : `rust/otb-gui/examples/check_tools.rs` (il rejoue les sept
      outils contre les références figées — c'est un outil important), et les bancs `banc_4b0`, `banc_4c0`,
      `banc_minmax` d'`otb-dsp`.
- [x] **Modes d'`otb-parity` incomplets** : `architecture.md` en cite 4 sur 11 ; `AGENTS.md` un seul.
- [x] **Commandes absentes d'`AGENTS.md`** : `otb-dev bench-open`, `otb-dev release notes`, `otb-dev icons --svg`.
- [x] **Où sont les fiches drone** : l'emplacement de `drones.json` n'est écrit **nulle part pour un pilote**
      (`%APPDATA%\OpenTuneBox\OpenTuneBox\` sous Windows, `~/.local/share/OpenTuneBox/OpenTuneBox/` sous Linux). Un
      pilote qui veut sauvegarder ou déplacer ses fiches ne peut pas le savoir.
- [x] **Désinstallation Linux** : décrite seulement en commentaire de `packaging/linux/install_linux.sh`.
- [x] **Branche macOS résiduelle** : le code garde un chemin de données macOS alors que D-042 l'abandonne. À retirer
      ou à justifier en commentaire.
- [x] **Boutons non décrits** dans le guide : « Calculer », « Exporter image », « Tout voir », « Appliquer » du
      Simulateur (dont le fait que les spectres ne se recalculent **qu'au clic**, environ 1 s), et les boutons de
      fermeture des logs.
- [x] **README** : il ne renvoie ni à `docs/README.md` (l'index), ni aux trois registres.

### 16f — Redondances : un maître par sujet

Le projet a la règle « un maître par sujet » (D-018). Elle est enfreinte à trois endroits qui coûtent cher à maintenir :

- [x] **Seuils de l'assistant recopiés** dans `docs/guide-utilisateur.md` (−30 dB, −10 dB, pic < 7 dB, marge > 35°,
      1,05, 200 fenêtres, 1,03) alors que `docs/assistant.md` en est le maître. Si un seuil change, il faut le changer
      à deux endroits — et rien ne le vérifie. Geste : le guide dit ce que le pilote doit faire et **renvoie** à
      `assistant.md` pour la valeur, ou cite la valeur avec une mention explicite de sa source.
- [x] **Pièges dupliqués mot pour mot** entre `AGENTS.md` et `docs/pieges-par-domaine.md` (qui l'avoue lui-même).
- [x] **Mesures recopiées** : statuts de `corpus validate`, durées et pics mémoire présents à la fois dans
      `validation.md`, `architecture.md` et deux compétences (dont deux fois dans le même fichier).
- [x] **Contrôle à ajouter** : `otb-dev --test docs` sait vérifier qu'un numéro de registre existe ; il ne sait pas
      vérifier qu'un **seuil** cité hors de son maître correspond à la valeur du code. C'est ce qui a permis à ces
      doublons de vivre. Geste : un contrôle pour les seuils les plus recopiés, avec sa contre-preuve.

### 16g — Amendement de D-050 et mesure des données de test

- [x] **D-050 amendée** (choix de Cédric) : sa formulation « aucun nom de personne, nulle part » visait l'auteur de
      PIDtoolbox ; les **références bibliographiques** d'ouvrages publiés gardent leurs auteurs, comme l'exige D-005.
      Écrire l'amendement dans `docs/decisions.md`, et ajuster la phrase correspondante d'`AGENTS.md`.
- [x] **Compression des données de test : mesurée le 2026-11-08, décision à prendre.** Les 31 fichiers de cas pèsent
      **133 Mo**, soit la quasi-totalité du dépôt. Sur les dix plus gros :

      | | actuel | compressé | gain | décompression |
      |---|---|---|---|---|
      | `step.json` | 15,8 Mo | 7,2 Mo | ÷2,2 | 53 ms |
      | `step_tuning.json` | 10,5 Mo | 0,6 Mo | **÷17,9** | 7 ms |
      | `loop_spectra.json` | 12,5 Mo | 4,8 Mo | ÷2,6 | 35 ms |
      | **les dix** | **93 Mo** | **31 Mo** | **÷3,0** | **215 ms en tout** |

      **Ce que ça coûte** : 215 ms de décompression répartis sur une suite qui dure 126 s — invisible. `flate2` est déjà
      une dépendance de test et `log_check.json.gz` prouve que le procédé marche (26 → 5,2 Mo).
      **Ce que ça coûte vraiment** : un `.gz` est **opaque pour git**. Aujourd'hui, regénérer un fichier de cas ne
      stocke que l'écart ; compressé, chaque régénération stocke un fichier entier. Le dépôt gagnerait ~60 Mo au
      clone mais grossirait plus vite à chaque réenregistrement des références (W-028).
      **Tranché par Cédric le 2026-11-08 : `step_tuning.json` seul**, et rien d'autre (D-052). Fait : 10,5 Mo → 0,53 Mo
      (**÷19,4** par `gzip -9 -n`, un peu mieux que la mesure d'essai), le test passe en 0,80 s, `.gitattributes` déclare
      `*.gz binary` et la procédure de régénération (`docs/validation.md`) dit de recompresser. Les neuf autres restent en clair :
      ÷2,2 à ÷2,6 seulement, et un `.gz` est opaque pour git — le réenregistrement des références (W-028) les ferait grossir
      plus vite qu'ils n'allègent.
- [x] **Code mort retiré** : quatre fonctions publiques sans **aucun** appelant, nulle part (ni code, ni test, ni
      exemple) — `list_sessions` du décodeur, `set_right` et `set_smoothing` de la page Spectre, `set_profile` de la
      page Assistant. Vérifié une par une avant de les retirer ; l'espace de travail compile sans elles.
- [x] **Les 19 `#[allow(clippy::too_many_arguments)]` disent maintenant pourquoi.** Trois raisons distinctes, écrites
      d'après ce que fait le code : la boucle du décodeur passe ses champs à plat (une structure par trame dans le
      chemin le plus chaud), les analyses et l'assistant gardent la signature de l'étalon (`otb-parity` les rejoue par
      nom avec leurs arguments dans l'ordre), les fonctions de dessin prennent des grandeurs d'écran.

## Ce qui est volontairement écarté

Conformément à l'arbitrage de Cédric (« rien sans bénéfice démontrable ») :

- **Découper `rules.rs` (3 631 lignes), `log_check.rs` (1 356) et `guide_page.rs` (1 053)** : ces fichiers portent
  plusieurs sujets, mais ils sont testés et leurs calculs sont validés au bit près contre l'étalon. Les remanier est un
  risque sans effet visible. À rouvrir si un travail futur doit s'y plonger.
- **Factoriser la duplication des pages d'outils** (découpe de période ×6, `breaks` ×5, découpe bornée ×9, listes
  déroulantes ×5, `reset_view` ×5) : ce sont des répétitions d'une à quinze lignes, sans risque de divergence
  silencieuse. Noté, non fait. Le lot 15 de `plan-suite.md` reste le bon endroit si l'interface est reprise.
- **Les ~300 `pub` inutilement publiques** : exposition trop large, pas du code mort. Sans effet.
- **Les 35 crates présentes en deux versions** dans `Cargo.lock` : héritées d'eframe, déclarées « warn » dans
  `deny.toml`, rien à y faire de notre côté (W-042).
- **Un cache Cargo en CI** : gagnerait quelques minutes par passage, mais ajoute une mécanique à entretenir et un
  risque de cache périmé, pour un projet dont la CI dure 7 min. À rouvrir si elle s'allonge.

## Preuves exigées avant de livrer

1. `cargo run -p otb-dev -- verify --strict` avec le corpus, `$OTB_GOLDEN` et `OTB_BITEXACT=1` : **0 saut, tout passe**.
2. Chaque test ajouté **vu en échec** en sabotant ce qu'il protège, le sabotage consigné.
3. Chaque libellé d'interface corrigé **relu dans l'application lancée**, pas seulement dans le code.
4. Chaque chiffre corrigé **remesuré**, pas recopié d'un autre document.
5. La CI verte sous Ubuntu **et** Windows sur la demande de fusion.
6. `CHANGELOG.md` (*Non publié*) à jour, et les registres (`D-`, `L-`, `W-`) renseignés pour ce qui est découvert en
   route.

## Critères d'arrêt

- Un constat cache un **défaut de calcul** (pas seulement un écart de documentation) → s'arrêter, le consigner, revenir
  vers Cédric : il mérite son propre lot.
- Un test nouveau refuse d'échouer quand on sabote ce qu'il protège → il ne prouve rien, ne pas le garder.
- Une correction de libellé demanderait de changer le comportement de l'interface → la sortir de ce lot et la proposer
  à part.
