# Journal des versions

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; numérotation
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

### Ajouté
- Plan par lots de la suite (`docs/plan-suite.md`), établi le 2026-11-06 sur l'état réel des registres : les cinq chantiers de l'audit sont traités, il reste à publier la première version Rust (lots 6 et 7), à solder les travaux reportés devenus caducs sans le Python (lot 12), à nettoyer après la publication (lot 13), à ouvrir plusieurs logs en parallèle (lot 11), à finir la calibration des seuils de l'assistant (lot 14) et, sur déclencheur mesuré seulement, à reprendre la dette de l'interface (lot 15). Numérotation des lots continuée depuis `docs/plan-chantiers.md` ; cinq points sont à trancher par Cédric (plafond de mémoire du lot 11, AppImage, aide en ligne de commande, textes des licences, ordre après la 0.9.0).
- Contrôle Windows sur le poste Linux (compétence `opentunebox-dev`) : avec `windres`, l'éditeur de liens mingw et `wine64`, tout l'espace de travail se compile pour Windows (`cargo check --target x86_64-pc-windows-gnu`, quelques secondes) et les tests tournent sous `wine` (85 exécutables sur 89 ; les 4 autres échouent à cause de l'environnement, liste et causes dans la compétence). Plus besoin de retirer les dépendances de test d'`otb-dev` pour le vérifier.
- Banc de temps de cadre de l'interface (lot 8 du plan des chantiers, W-049) : `cargo run --release -p otb-gui --example banc_cadre -- LOG` mesure l'écran Guide, le viewer en vue entière (session la plus longue) et la tessellation ; `--gpu` ajoute le rendu sur une vraie carte graphique. Il remplace le banc perdu de l'audit de performance et retrouve ses chiffres (7,9 ms par cadre sur `FINAL.bbl`). Mesure neuve sur la plus longue session du corpus (620 079 échantillons) : 12,6 à 13,0 ms, sous le budget de 16,7 ms.
- Décodeur robuste face à un en-tête hostile (D-044) : plus de 512 champs ou un intervalle hors de 1 à 65 535 → fichier refusé avec un message en français (barre d'état de l'interface, `illisible` en ligne de commande) au lieu d'une boucle sans fin ou d'une allocation de dizaines de Go ; une mémoire insuffisante est une erreur, plus un abandon du programme. Réglages d'en-tête à zéro (« 1 000 000 Hz ») : constat « réglages de l'en-tête invraisemblables ». Aucun vrai log ne change de résultat.
- `otb-dev verify [--quick] [--strict]` : vérification complète en une commande (formatage, lint, tous les tests, `cargo deny`), code de sortie 0 seulement si tout passe ; les tests sautés faute de corpus (`OTB_GOLDEN`, `OTB_BITEXACT`) sont comptés et affichés, `--strict` en fait un échec. Le crochet de pré-commit n'utilise plus `jq` (absent du poste Windows : il laissait passer tous les commits) et reconnaît aussi la forme avec `-C dossier`. La CI et la documentation n'ont plus la liste des dix `-p`.
- Plan par lots des cinq chantiers après la migration Rust (`docs/plan-chantiers.md`), issu de l'audit du 2026-11-04 : robustesse du décodeur face à un en-tête hostile (boucle sans fin, allocation géante), résultat périmé affiché par le socle des outils, commande unique de vérification `otb-dev verify` et crochet de pré-commit sans `jq`, mémoire du décodeur, consignes et CI remises à jour. Une PR par lot, preuves et contre-preuves exigées, décisions de Cédric consignées (bornes du décodeur : 512 champs, intervalles 1 à 65 535).
- Garde-fous pour un agent qui travaille seul : `.claude/settings.json` met les commandes `git push`, `gh pr merge`, `git tag` et `gh release` en « demander » (l'agent ne les lance qu'avec la confirmation de Cédric à chaque fois) ; côté GitHub, la règle `protection-main` exige désormais une approbation par PR.

### Modifié
- Visualiseur plus fluide sur les longs vols (lot 10 du plan des chantiers, W-049, D-048) : chaque courbe affichée reçoit un index multi-résolution min/max (minimum et maximum par blocs de 8 échantillons, puis de 8 blocs…), et le tracé ne parcourt plus toute la série à chaque image. Sur la plus longue session du corpus (620 079 échantillons) le cadre passe de 10,5–12,1 ms à 4,6–4,8 ms, **y compris pendant un glissement** (10,3–10,8 → 4,8–4,9 ms) ; sur `FINAL.bbl`, de 6,9–8,7 à 3,1–3,2 ms, et de 6,2 à 0,95 ms en fenêtre étroite. Le tracé est inchangé : les captures du viewer sont identiques au bit près, aux deux thèmes. L'index pèse 6,4 à 7,8 % du pic de mémoire, qui ne bouge pas (il est atteint pendant le décodage). Un seul changement visible, sur les courbes à trous (rotation des moteurs en Hz) : un paquet qui contient un trou est désormais toujours tracé comme un trou, là où l'ancien calcul coupait la ligne ou non selon la position du trou. Bancs : `otb-gui --example banc_cadre LOG [--glissement] [--zoom 8]` et `otb-dsp --example banc_minmax`.
- Ouverture d'un log deux fois moins gourmande en mémoire et plus rapide (lot 5 du plan des chantiers, L-081) : le décodeur range les trames principales en `i32` (valeurs déjà ramenées aux 32 bits, seuls `loopIteration` et `time` restent en 64 bits) et la table est rendue dès que les colonnes utiles sont extraites. Log le plus gourmand du corpus : 481 → 274 Mo (÷ 1,75), 0,63 → 0,54 s ; les `FlightLog` des 528 fichiers du corpus sont identiques au bit près. Banc : `otb-dev bench-open LOG`.
- Police de l'interface : Ubuntu Light (police par défaut d'egui) est gardée, sous sa propre licence, comme Hack (D-046). La question ouverte au passage sous GPL 3 (W-046) disparaît avec la licence PolyForm Noncommercial, qui n'est pas un copyleft ; la première publication de la version Rust n'est plus bloquée.
- Consignes des agents remises à jour (lot 2 du plan des chantiers) : `AGENTS.md` ne dit plus « interface Qt », « pas de compilateur C » ni que la migration est en cours ; allégé de 12,6 à 8,4 Ko (organisation en une ligne par crate, détail dans `docs/architecture.md`) ; nouvelles sections « Interdits » (push, tag, fusion, publication seulement avec la confirmation de Cédric ; ne pas toucher au corpus ni affaiblir un test) et variables `OTB_DEV_ROOT`, `OTB_BITEXACT`, `OTB_FORMAT_CASES`. `tests.yml` et `audit.yml` n'écoutent plus `feat/rust-migration` ; `CONTRIBUTING.md` et `docs/migration-rust.md` marquent la migration comme terminée.
- Le dépôt est **public**, sous licence **PolyForm Noncommercial 1.0.0** (D-045, qui amende la GPL 3 de D-043 le jour même) : le code est ouvert, mais **tout usage commercial est interdit** — pas de revente, pas de service payant, pas d'usage professionnel, et aucune licence commerciale n'est proposée (pas de double licence, l'auteur compris). Les contributions sont publiées sous cette seule licence, sans CLA. `LICENSE` contient le texte officiel précédé de sa ligne `Required Notice:` ; README, CONTRIBUTING, AGENTS et `deny.toml` sont à jour. L'installateur Windows, l'AppImage et `install_linux.sh` livrent désormais `LICENSE` à côté de `THIRD-PARTY-LICENSES.txt` (section « Notices » de la licence) et l'installateur Windows l'affiche à l'installation. Aucune dépendance à copyleft n'est plus acceptable. Conséquence assumée : plus « open source » au sens OSI, donc hors des dépôts Debian, Fedora et Flathub. Les versions v0.2.0 à v0.8.0 (builds Python, PySide6/Qt, macOS) ont été retirées de la page Releases (les étiquettes git restent). Les pseudos de pilotes tiers sont remplacés par des noms neutres dans la documentation.
- Mentions de licences livrées (W-043) : un fichier `THIRD-PARTY-LICENSES.txt` (324 composants : bibliothèques Rust, polices Hack et Ubuntu Light, logos d'egui, icônes Lucide) est installé avec le programme par l'installateur Windows, l'AppImage et `install_linux.sh`. Les licences des polices (Ubuntu Font Licence, MIT, Bitstream Vera) et des bibliothèques (MIT, Apache 2.0, BSD…) demandent de conserver leur avis dans les copies livrées. Le fichier est produit par `otb-dev licenses` (lecture des fichiers de licence de chaque paquet, textes identiques regroupés ; 22 composants, surtout la famille egui, n'en ont pas dans leur paquet : ils sont listés avec leur licence et leurs auteurs déclarés). La licence OFL de la crate des polices ne concerne que NotoEmoji, non compilée chez nous : un test signale son apparition.
- Phase 7-5 de la migration Rust (D-035) : **plus aucun Python dans le dépôt**. Supprimés : `src/opentunebox/` (application, 58 modules), `tests/*.py`, `scripts/*.py` (50 scripts), `rust/otb-py` (pont PyO3), `pyproject.toml`, l'emballage PyInstaller (`packaging/opentunebox.spec`, `launcher.py`, `make_icons.py`) et les restes de macOS (`packaging/macos/`, `.icns`). `accueil.md` et `opentunebox.svg` vivent désormais dans `rust/otb-gui/assets/`. La CI (`tests.yml`, `audit.yml`, `dependabot.yml`, modèle de PR) ne parle plus que de Rust : formatage, lint, tests sur Ubuntu et Windows, `cargo-deny`. `otb-dev release` ne met plus à jour que `rust/Cargo.toml` ; le contrôle des chemins cités dans la documentation n'accepte plus de chemin Python que dans les registres d'histoire (`HISTORICAL_DOCS`), avec un test qui échoue si l'exemption s'étend aux autres documents. Documentation réécrite pour le Rust (architecture, AGENTS, compétences, CONTRIBUTING, README) ; procédure « Refaire une référence Python » dans `docs/validation.md`.
- Les actions GitHub des workflows sont épinglées par leur SHA de commit (`uses: X@SHA # v7.0.1`) au lieu d'une étiquette de version, qu'un dépôt tiers peut déplacer ; SHA relevés par `gh api`. Dependabot (`github-actions`) met à jour le SHA et le commentaire ensemble.
- macOS abandonné : l'application sera livrée pour Windows et Linux seulement (D-042).
- Lint par `ruff check` (règles de pyflakes, mêmes contrôles) à la place de `pyflakes`, en local et en CI (D-030).
- Les lissages, la fenêtre de Hann et l'intercorrélation ne passent plus par scipy mais par `analysis/_numeric.py` (numpy seul, écart à scipy ≈ 1e-14 relatif) ; les 4 noyaux numba des filtres Betaflight sont réécrits en Rust (`rust/otb-dsp`, identiques à numba au bit près, sauf la réponse en fréquence : 1e-15 relatif). scipy, numba et llvmlite (≈ 350 Mo installés) ne sont plus nécessaires à l'application (dossier de l'exécutable Linux : 206 Mo au lieu de 463 Mo) : ils passent dans l'extra `dev`, comme références des tests (D-032).

### Corrigé
- Test `golden.rs` (concordance avec les références Python figées) : il exigeait le bit près pour toutes les fonctions, alors que 8 d'entre elles (FFT, `sin` / `cos`, `exp`, SVD) ont une tolérance mesurée ; rouge par construction avec `OTB_GOLDEN` (W-048). Il applique désormais la tolérance de chaque fonction, comme `otb-parity check` (bit près par défaut), et liste tous les écarts (D-047, L-083).
- Test `install_linux` instable (W-047, cause enfin trouvée : L-082) : exécuter le programme fraîchement copié échouait par intermittence avec « Text file busy » (`ETXTBSY`), parce que les cas du fichier tournent en parallèle dans le même processus et qu'un `fork` pendant la copie empêche le noyau d'exécuter le fichier. Le lancement est réessayé sur ce code d'erreur ; aucune assertion n'a été touchée. Avant : 2 échecs sur 5 exécutions ; après : 10 au vert.
- Interface : une page d'outil n'affiche plus le résultat d'un calcul périmé. `ToolPage` numérote ses demandes et jette le résultat d'une demande qui n'est plus la dernière : après le vidage des logs, un calcul encore en cours faisait réapparaître son résultat sous « Sélectionnez au moins un log. » ; après un changement de sélection, le résultat de l'ancienne sélection pouvait s'afficher (lot 3 du plan des chantiers).
- Test de l'interface egui (`un_fichier_illisible_n_empeche_pas_les_suivants`) : échec sur la CI Windows (`exceeded max_steps`), le test avance désormais au pas pendant le calcul en arrière-plan (L-077).
- Interface Rust : les onglets de la barre du haut ont de nouveau leurs icônes colorées (comme dans la version Python) ; la fenêtre Linux porte l'identifiant `OpenTuneBox` (L-076).
- Simulateur de filtres (interface Rust) : pendant le calcul d'un nouveau log, les spectres de l'ancien ne sont plus affichés.
- Interface Rust sous Windows : plus de fenêtre de console à l'ouverture, et l'icône d'OpenTuneBox sur l'exécutable et sur la fenêtre.
- CI Ubuntu à court de disque avec les tests de l'interface egui (`No space left on device`) : les infos de débogage du profil `dev` Rust sont réduites aux tables de lignes (binaires de test de 490 à 186 Mo, 6,7 Go pour toute la séquence) ; L-069.
- Simulateur de filtres : les notchs dynamiques ajoutés au-delà de ceux du log ne creusent plus à des fréquences fixes sans raie (centres initiaux du
  firmware) ; ils sont placés sur les pics du gyro non filtré, sinon à leur centre initial. Mesuré sur 383 sessions de 64 drones : 45 % des positions
  réelles retrouvées contre 28 % (D-041).
  Le même modèle est porté dans le Rust (`rust/otb-analysis/src/bf_filters.rs`), identique à Python sur les cas de test (W-040).
- L'assistant ne plante plus sur une session de 0,1 à 0,2 s (reste d'enregistrement après un rebranchement de la batterie) : trop
  courte pour avoir une fréquence entre 80 Hz et la limite des filtres, elle faisait échouer toute l'analyse (`ValueError: a cannot be
  empty`, vu sur 2 logs du corpus). La prévision des filtres est alors absente, donc aucun des deux conseils qui en dépendent (LPF1 du gyro
  superflu, filtre D trop fort). Aucun conseil ne change sur les 631 sessions du corpus (`advisor_rates.py --compare`). Le même correctif est porté dans le Rust (`rules.rs`) ; `otb-parity check-cli` rejoue les 2 fichiers qui plantaient (1 500 textes, 0 écart), `check-advisor` 631 sessions sans écart (références de l'assistant et des journaux refaites, `Hover_Test5.bbl` comprise).

### Ajouté
- Phase 7-4, suite : une étiquette `essai-v0.8.0` fait essayer toute la chaîne de publication de `construire.yml` (notes, brouillon, construction, envoi, contrôle de la liste des fichiers) sans rien publier : la page reste un brouillon, jamais « dernière version » (`CONTRIBUTING.md`).
- Phase 7-4, suite : l'AppImage est construite avec un runtime figé. Sans `--runtime-file`, appimagetool 1.9.1 télécharge le runtime depuis la version mouvante `continuous` sans contrôle d'empreinte (lu dans son code source) ; `construire.yml` télécharge désormais `type2-runtime` `20251108`, vérifie son SHA-256 et le passe par `APPIMAGE_RUNTIME` (`build_appimage.sh`, facultatif en local). Le runtime est plus ancien que celui des constructions précédentes : effet non mesuré.
- Phase 7-4, suites de la relecture : le test du nom du fichier `.desktop` lit désormais la ligne qui écrit le fichier (un commentaire citant le bon nom ne le satisfait plus), et le branchement du lancement est testé (`App::from_launch` : sans argument on reste sur le Guide, avec un log on ouvre les courbes ; le nom du programme n'est pas pris pour un fichier).
- Phase 7-4 de la migration Rust (D-035), suite : icône de la fenêtre sous Linux. Vu par Cédric (KDE Plasma, Wayland) : la fenêtre n'avait pas l'icône d'OpenTuneBox. Essayé pour de vrai sur son bureau (installation dans `~/.local`, retirée ensuite, état d'avant retrouvé) : le fichier `.desktop` doit s'appeler **exactement comme l'`app_id`** de la fenêtre, `OpenTuneBox.desktop` (avec `opentunebox.desktop`, l'icône reste générique, même avec `StartupWMClass`). `install_linux.sh` et `build_appimage.sh` écrivent ce nom (le premier supprime l'ancien fichier en minuscules) ; test `contracts.rs::le_fichier_desktop_porte_le_nom_de_l_app_id` (vu en échec avant la correction). Limites : une AppImage lancée telle quelle n'est pas connue du bureau (icône générique) ; sous X11 la propriété d'icône reste vide, cause non trouvée (W-045, L-076 mise à jour).
- Phase 7-4, relecture indépendante (agent `relecteur`, PR #93) : `construire.yml` ne laisse plus le jeton d'écriture dans `.git/config` pendant la compilation (`persist-credentials: false` aux deux `checkout`, `GH_REPO` explicite pour les commandes `gh` : les build.rs et macros de centaines de crates tierces y tournent). `actionlint` propre ; le chemin d'une vraie étiquette `v*` reste à essayer avant la première version.
- Phase 7-4, relecture indépendante (PR #93) : `install_linux.sh` ne s'arrête plus quand OpenTuneBox est ouvert pendant une mise à jour. Mesuré : `cp` sur un programme en cours d'exécution échoue (« Fichier texte occupé », code 1), le script copie maintenant à côté puis fait un `mv`, qui remplace le fichier d'un coup (donner le programme déjà installé en argument marche aussi). Il supprime aussi les PNG de 16 à 128 px posés par l'ancienne version, qui masquaient l'icône actuelle. 2 tests (`rust/otb-dev/tests/install_linux.rs`, Unix seulement) : le premier lance un vrai exécutable pendant la mise à jour, les deux vus en échec en sabotant le script (retour au `cp` direct ; retrait des PNG supprimé).
- Phase 7-4 : la CI de la PR #93 a refusé ma correction du README (le programme compilé, cité en code avec son chemin dans `rust/` : un fichier de compilation, absent d'un dépôt propre alors qu'il existe sur le poste). Reformulé sans le préfixe du dépôt ; vérifié sur une copie propre des fichiers suivis par git (Rust : échoue avec l'ancienne phrase, passe avec la nouvelle). L-079.
- Phase 7-4, relecture indépendante (PR #93) : le README ne cite plus PyInstaller pour fabriquer l'exécutable Linux (cargo, puis `install_linux.sh`) ; `docs/pieges-par-domaine.md` (L-020, `binutils`) et `AGENTS.md` précisent que PyInstaller ne concerne plus que la version Python (supprimée en 7-5).
- Phase 7-4 de la migration Rust (D-035), étape 5 : `construire.yml` en Rust, sans macOS. Matrice Windows + Ubuntu 22.04 ; `cargo build --release -p otb-gui` (Windows : `crt-static`, puis `dumpbin /dependents` refuse toute DLL de Visual C++) ; essai de démarrage de 20 s avec un petit log en argument (Windows : exécutable seul, puis installateur installé / lancé / désinstallé ; Linux : AppImage sous écran virtuel `xvfb` et pilote Vulkan logiciel lavapipe) ; notes de version par `otb-dev release notes` ; la page Releases se publie avec **deux** fichiers (plus trois). README, guide utilisateur et CONTRIBUTING ne parlent plus de macOS ni de `release.py`. `actionlint` propre sur les quatre workflows (étapes Windows en `shell: pwsh` explicite). Le test Python `test_build_workflow_installs_what_its_tests_import` est supprimé : il surveillait l'installation de `scipy` / `numba` par les tests de l'ancien `construire.yml`, qui n'existent plus (la CI de la PR #93 l'a fait échouer, `StopIteration` ; L-037 mise à jour) ; le test de validité du YAML des workflows reste. **Mesuré par le premier passage à la main sur la branche (2026-11-03, run 41676827867, tout vert au premier essai)** : installateur Windows de **7,5 Mo** (88 Mo en Python 0.8.0) compilé par Inno Setup, installé, lancé 20 s avec un log, puis désinstallé sans reste (programme et association `.bbl`) ; `dumpbin /dependents` : seulement des DLL de Windows (`kernel32`, `user32`, `opengl32`, `dxgi`…), aucune de Visual C++ ; AppImage de **10 Mo** (167 Mo), démarrée 20 s sous `xvfb` et `lavapipe` (code 124). Le rendu `wgpu` démarre donc sans carte graphique sur les deux systèmes (si aucun adaptateur n'était trouvé, le programme s'arrêterait et l'essai échouerait). Durées : compilation 13 min 26 (Windows) et 8 min 44 (Linux), sans cache ; jobs de 15 min et 10 min. Reste non vérifié : l'installation sur un vrai Windows par-dessus la 0.8.0 (W-044) et le lancement de l'AppImage sur d'autres Linux que le poste de Cédric.
- Phase 7-4 de la migration Rust (D-035), étape 4 : installateur Windows en Rust. `packaging/windows/opentunebox.iss` emballe l'exécutable unique `opentunebox-gui.exe` (installé sous le nom `OpenTuneBox.exe`, `/DExePath=` pour un autre chemin) ; `AppId`, tâches, association `.bbl` / `.bfl` (`OpenTuneBox.exe "%1"`, ouverte par l'étape 1) et icône inchangés. Le nettoyage de `_internal` est **gardé** (écart voulu avec le plan global) : une mise à jour par-dessus la 0.8.0 y laisserait sinon le dossier PyInstaller. `construire_windows.ps1` compile avec cargo (bibliothèque C embarquée par `crt-static`, pour que le programme ne demande aucune DLL de Visual C++) puis fabrique l'installateur si Inno Setup est là. **Non vérifié en local** : aucun Windows ici, ni `iscc` ni PowerShell. Le fichier `.iss` a ensuite été compilé et essayé par la CI (voir l'étape 5) ; `construire_windows.ps1` n'a toujours pas été exécuté (W-044).
- Phase 7-4 de la migration Rust (D-035), étape 3 : fabrication Linux en Rust. `packaging/linux/build_appimage.sh` emballe le binaire `opentunebox-gui` seul (plus de dossier PyInstaller : le programme ne dépend dynamiquement que de la bibliothèque C, `ldd` ; le rendu et les bibliothèques d'affichage sont cherchés sur le système) et `packaging/linux/install_linux.sh` compile avec cargo puis installe sous `~/.local` (plus de `.venv` ni de PySide6 ; icône SVG et PNG 256 px). Mesuré : AppImage de **10,6 Mo** (167 Mo en Python 0.8.0), lancée avec un log en argument : le log s'ouvre sur l'écran Courbes (capture regardée) ; script d'installation essayé dans un faux dossier personnel (fichiers installés, `.desktop` correct, erreur claire si le programme manque), `shellcheck` propre sur les deux scripts. `construire.yml` appelle encore l'ancien script jusqu'à l'étape 5.
- Phase 7-4 de la migration Rust (D-035), étape 2 : audit des dépendances Rust par `cargo-deny` (`rust/deny.toml`, job `cargo-deny` de `audit.yml`, écosystème `cargo` de Dependabot). Mesuré sur le vrai `Cargo.lock` (Windows et Linux seulement, D-042) : failles, licences (13 licences permissives, aucun copyleft : `self_cell` et `r-efi` proposent GPL / LGPL en alternative « OR » à MIT / Apache-2.0), origine des crates (crates.io seul) : vert. Premier passage : une crate retirée du registre (`async-executor` 1.14.0, remplacée par la 1.14.1) et 4 avis « non maintenu » hérités d'eframe, ignorés avec leur raison (W-042) ; W-043 : obligations des licences des polices d'egui, non vérifiées. Contre-épreuves vues en échec : MIT ou Apache-2.0 retirées de la liste `allow`, un avis retiré de `ignore`.
- Phase 7-4 de la migration Rust (D-035), étape 1 : l'interface egui ouvre les logs passés en argument (`opentunebox-gui LOG…`, `app::files_from_args` et `App::open_at_launch`, `rust/otb-gui/src/`), directement sur l'écran Courbes. Sans cela, l'association `.bbl` / `.bfl` de l'installateur Windows (`OpenTuneBox.exe "%1"`) et l'AppImage (`%F`) auraient lancé l'application sans ouvrir le fichier. Les options (`-x`) sont ignorées, comme en Python. Un essai réel a montré qu'ouvrir le log ne suffisait pas : l'écran restait sur la page du Guide restaurée de la session précédente, d'où `open_at_launch`. 2 tests, 2 sabotages vus en échec (options non filtrées, écran inchangé) ; essai réel avec `tests/data/jeno5_5s.bbl` : courbes affichées (capture regardée).
- Phase 7-3 de la migration Rust (D-035), étape 5 : `otb-dev corpus validate [DOSSIER_OU_FICHIER …] [--decoder CHEMIN] [--limit N] [--start N] [-j FILS]` (`rust/otb-dev/src/corpus/validate.rs`) refait `scripts/validate_corpus.py` : lance `blackbox_decode` (délai de 180 s, puis arrêt) et compare ses CSV à notre décodeur, session par session, sur l'horodatage `time`. Mêmes statuts, mêmes messages et même code de sortie que Python. Preuve : passage complet du corpus (528 fichiers, 712 sessions : 627 `ok`, 67 `gap`, 8 `slow`, 5 `values`, 3 `refhang`, 2 `rows`), sortie identique à celle de Python ligne pour ligne (à la numérotation `[k/n]` près, voir ci-dessous) ; 13 tests sur cas fabriqués, 6 sabotages sur 7 détectés (le septième était équivalent). Différences assumées : Python compte à tort un *dossier* nommé `….bbl` comme un log (529 fichiers), Rust l'ignore ; 4 fichiers traités à la fois par défaut (≈ 1 Go de mémoire au pic, corpus entier en 6 min) ; une référence introuvable ou un CSV sans colonne `time` arrête l'outil avec un message clair. Les quatre outils du corpus sont portés.
- Phase 7-3 de la migration Rust (D-035), étape 4 : `otb-dev corpus labels [--limit N] [-j FILS] [--csv CHEMIN]` (`rust/otb-dev/src/corpus/labels.rs`) refait `scripts/corpus_labels.py` : le tableau `corpus.csv` (une ligne par drone, taille détectée, fréquence moteur, cellules, état et taille pré-remplis d'après les noms). Les colonnes de Cédric (`etat`, `taille_reelle`, `remarque`) ne sont jamais réécrites une fois la ligne créée. Le fichier est écrit comme Python l'écrit (BOM, `;`, fins de ligne `\r\n`, guillemets du module `csv`) : Excel et les deux outils le relisent. Preuve, sur une copie du vrai tableau (le vrai fichier est resté intact, même empreinte avant et après) : 101 drones dont 5 nouveaux, fichier identique à celui de Python octet pour octet, les 96 lignes de départ et leurs colonnes de Cédric retrouvées sans changement ; 12 tests, dont 4 sabotages détectés. Les regex à « lookbehind » de Python sont refaites à la main (la crate `regex` n'en a pas).
- Phase 7-3 de la migration Rust (D-035), étape 3 : `otb-dev corpus calibrate NB_FICHIERS SORTIE.json` (`rust/otb-dev/src/corpus/calibrate.rs`) refait `scripts/calibrate_advisor.py` : mesures brutes de chaque session pour calibrer les seuils de l'assistant, et `--resume SORTIE.json` pour le tableau p10 / p50 / p90 par taille de drone. Le JSON a les mêmes clés que celui de Python et se lit dans les deux sens (`NaN` et `Infinity` compris, `rust/otb-dev/src/corpus/json.rs`). Preuve : sur les 632 sessions du corpus, 96 clés, écart relatif maximal 1,5e-10 entre Python et Rust ; `--resume` rend le même texte, octet pour octet, que Python sur le même JSON. Différence assumée : avec moins de fichiers que le corpus, Python en tire au hasard (`random.seed(0)`), Rust prend les premiers de la liste.
- Phase 7-3 de la migration Rust (D-035), étapes 1 et 2 : `otb-dev corpus` (`rust/otb-dev/src/corpus/`), les outils du corpus de vrais logs en Rust. Le socle répartit les fichiers sur un pool rayon, avec reprise des fichiers en échec. `otb-dev corpus rates SORTIE.json [--limit N]` refait `scripts/advisor_rates.py` (aussi `--resume` et `--compare AVANT.json APRÈS.json`, code 1 si un conseil diffère) ; le JSON a les mêmes clés que celui de Python. Preuve : sur les 632 sessions du corpus, `--compare` entre le passage Python et le passage Rust ne trouve aucune session différente (texte complet des constats, chiffres compris). `calibrate`, `labels` et `validate` suivent.
- Phase 7-2 de la migration Rust (D-035) : binaire `otb-dev` (`rust/otb-dev`). `otb-dev release prepare X.Y.Z` et `notes vX.Y.Z` refont `scripts/release.py` (numéro dans `rust/Cargo.toml`, plus `pyproject.toml` et `__init__.py` tant qu'ils existent ; sortie identique à celle de Python sur le vrai CHANGELOG) ; `otb-dev icons` refait le PNG Linux et le `.ico` Windows depuis `opentunebox.svg` avec `resvg` et `ico` (Rust pur, sans `.icns` : macOS abandonné, D-042). Le rendu diffère de celui de Qt seulement par le lissage du contour des traits (écart moyen 1,5 sur 255, 1,4 % des pixels à plus de 32) ; le `.ico` contient désormais 7 tailles (16 à 256 px) et pèse 21 Ko au lieu de 270 Ko.
- Phase 7-1 de la migration Rust (D-035) : crate `rust/otb-dev` (outils de développement, jamais livrés) ; `cargo test -p otb-dev --test docs` refait en Rust les 13 vérifications de la documentation de `tests/test_docs.py` (liens, chemins et symboles, outils, guide, modules, index, fichiers publiés, commandes, langue, registres, seuils de l'assistant), et `rust/otb-dev/tests/checks.rs` prouve que chacune échoue sur un cas fabriqué. 14 sabotages de la vraie documentation font échouer la suite Rust et la suite Python de la même façon. Nouveautés : les chemins `rust/…` cités par la documentation sont vérifiés, et `otb_cli::COMMANDS` liste les commandes de la ligne de commande. Documentation corrigée au passage : modules d'`otb-parity` et `frame.rs` non cités, chemin de `log_check.json` dans `docs/travaux-reportes.md`.
- Interface Rust : boutons « Fermer » (log affiché) et « Tout fermer » dans la barre du haut, grisés sans log (l'interface ne pouvait fermer aucun log) ; le bouton « Ouvrir… » est plein, en couleur d'accent, comme dans la version Python (W-039, D-040).
- Phase 6-6a de la migration Rust (D-035) : finitions de l'interface egui. « Exporter image » dans chaque outil (capture la page) et icône dans la barre du haut (capture toute la fenêtre), nom proposé `OTB_FIGS_AAAA-MM-JJ-N.png`, dernier dossier retenu ; « Calculer » (relance à la demande, le calcul reste automatique) ; « Tout voir » sur les graphiques zoomables ; liste des jeux de données du Spectre sans ascenseur ; marge sous le bas des diagrammes de Bode ; groupes de la Configuration repliables d'un clic (`rust/otb-gui/src/export.rs`, `rust/otb-gui/src/tools/common.rs:ViewMemo`, W-034 à W-036, W-038, L-075).
- Phase 6-5 de la migration Rust (D-035, D-039) : la page « Guide » de l'interface egui (`rust/otb-gui/src/guide.rs` et `guide_page.rs`) : accueil, quatre phases à cocher avec leur état automatique (« Vérifié » / « À revoir »), fiches drone (création, modification, remplissage d'après le log ouvert, Recommencer, Supprimer), logs du drone vérifiés en arrière-plan et reclassables entre filtres et séries PID, copie des commandes CLI. Les fiches sont dans le même `drones.json` que la version Python, aux mêmes octets ; un fichier illisible n'est plus écrasé en silence (il est mis de côté sous `drones.json.illisible`, le Guide le dit). La phase, la fiche et le rôle des logs sont retrouvés au lancement suivant. `DroneCard::from_value` tolère les types faux (W-029 point 1). Police Hack en dernier recours du texte (« → ▲ ») et trois icônes Lucide (`check`, `info`, `triangle-alert`). « Supprimer » et « Recommencer » visent la fiche sur laquelle on a cliqué, même si un log d'un autre drone change la fiche choisie pendant la confirmation (défaut de la version Python, non reproduit). Le Guide a révélé un panneau qui gardait la taille de son premier cadre (L-073). Écarts connus : W-037.
- Phase 6-4 de la migration Rust (D-035, D-038) : les outils « Assistant », « Configuration » et « Simulateur de filtres » de l'interface egui (`rust/otb-gui/src/tools/advisor.rs`, `setup.rs`, `filter_sim.rs`, `filter_sim_calc.rs`). Assistant : choix de la taille du drone, constats groupés par catégorie, petites courbes de réponse indicielle par axe. Configuration : réglages des logs côte à côte, différences surlignées, recherche, paramètres bruts, bouton dans le visualiseur ; **les lignes sont alignées par nom de réglage** (la version Qt les alignait par position, ce qui décalait les valeurs quand les curseurs simplifiés diffèrent). Simulateur : éditeur à trois onglets (Gyro, D, Notch dyn / RPM), gain et phase en échelle logarithmique, spectres avant / après filtres, commandes CLI à copier, retards ; le calcul ne se relance qu'à « Appliquer ». Preuve : `check_tools` rejoue 7 outils sur 20 journaux (26 883 clés de Python, 0 écart), 742 commandes CLI identiques au caractère près (`scripts/filter_sim_cases.py`), 37 tests. Écarts connus : W-036. Pas encore dans l'application : l'interface Qt reste celle des exécutables.
- Phase 6-3 de la migration Rust (D-035, D-037) : les outils « Réponse indicielle » et « Curseurs PID » de l'interface egui (`rust/otb-gui/src/tools/step.rs`, `pid_slider.rs`, `step_common.rs`). Réponse indicielle : réponse moyenne par axe (brutes et bande ± écart-type en option), deux colonnes de métriques au choix avec barres d'erreur, point plein / creux / triangle selon que la boucle est mesurée, approchée ou un minorant, vols conseillés cerclés, « Diagnostic du réglage » et curseurs Betaflight à droite, lissage du gyro de 20 à 60 échantillons. Curseurs PID : graphique des 8 curseurs, simulation « et si… » (PID résultants et écarts, filtres, commandes CLI à copier). Les résultats sont gardés par log (`state::Cache`, clé = réglages + période) : changer un réglage d'affichage ne refait pas le calcul. Dans `otb-analysis`, le lissage du gyro (`box_smooth_same`) et `segment_metrics`. Preuve : `check_tools` rejoue les 5 outils sur 20 journaux (7 682 clés de Python, 0 écart), 32 tests, 48 mutations tuées sur 49 (la dernière est équivalente). Écarts connus : W-035.
- Phase 6-2 de la migration Rust : les trois outils spectraux de l'interface egui (`rust/otb-gui/src/tools/`) : Analyse spectrale (avec le rapport « Diagnostic du bruit »), Fréquence × gaz et Fréquence × temps, palettes `hot` / `inferno` / `viridis` / `turbo` / `jet` identiques à celles de l'interface Qt, barre de couleurs graduée, grille à repères ronds, noms accessibles des listes et champs. Calculs identiques à ceux de Python sur 20 logs du corpus (`cargo run --release -p otb-gui --example check_tools`, 2 445 clés, valeurs continues ≤ 3e-7 relatif, décisions discrètes identiques) ; L-067, L-068, W-034. Pas encore dans l'application : l'interface Qt reste celle des exécutables.
- Phase 6-1 de la migration Rust : page « Courbes » de l'interface egui (trois axes et moteurs, période et curseur déplaçables, lissage, focus d'un axe, ouverture par glisser-déposer, mesure entre deux points, lecteur avec manettes et valeurs au curseur). La Configuration est venue avec la phase 6-4.
- Phase 6-0 de la migration Rust (D-035, D-036) : nouvelle crate `rust/otb-gui`, socle de l'interface egui (thème clair / sombre, état des logs, calculs en arrière-plan avec file d'attente, trait `Tool`, contournements d'egui_plot, icônes Lucide, fenêtre principale à onglets). `Heatmap` d'egui_plot mesuré et écarté au profit de `PlotImage` (30 ms contre 0,05 ms par cadre à 10⁵ cases). Pas encore d'outil : l'interface Qt reste celle de l'application.
- Phase 5c de la migration Rust (D-035, D-036) : nouvelle crate `rust/otb-cli`, binaire `opentunebox info` / `report` en Rust, texte identique à celui de Python caractère par caractère sur les 500 logs distincts du corpus pour `info`, `report` et `report --profil whoop` (`scripts/golden.py record-cli` puis `otb-parity check-cli`, texte écrit avant un plantage compris), sur le texte complet d'un log de CI (`scripts/cli_cases.py`) et sur des cas fabriqués (`rust/otb-cli/tests/cli.rs`) ; analyse des arguments au plus près d'`argparse` (même code de sortie sur 792 combinaisons : `scripts/cli_args_check.py`). Le Python reste la commande en vigueur jusqu'à la phase 7. Deux logs du corpus font planter l'assistant, en Python comme en Rust (W-030) ; pas d'aide détaillée (W-031).
- Phase 5b de la migration Rust (D-035) : `advisor/drone_card.py` (fiche drone), `advisor/log_check.py` (vérification des logs, repères de la phase Filtres, groupes de logs) et `advisor/tuning_path.py` (parcours et états automatiques des cases) en Rust (`rust/otb-advisor/src/`), identiques à Python sur des cas écrits (`scripts/drone_card_cases.py`, `scripts/log_check_cases.py`, `scripts/tuning_path_cases.py`) et sur les 668 sessions du corpus (`golden.py record-guide`, `otb-parity check-guide` : 204 702 clés, 0 écart) ; relecture indépendante : ordre des cases cochées, `cli_commands` et rôle vide corrigés (L-063, W-029).
- Phase 5a-4 et 5a-5 de la migration Rust (D-035) : `advisor/noise.py` (diagnostic du bruit : blocs de filtres, verdicts, stratégie) et `advisor/step_tuning.py` (diagnostic du réglage par la réponse indicielle) en Rust (`rust/otb-advisor/src/noise.rs`, `step_tuning.rs`), identiques à Python caractère par caractère : 79 et 2 776 cas écrits (`scripts/noise_cases.py`, `scripts/step_tuning_cases.py`), 630 sessions du corpus (361 522 clés) et 80 dossiers de drone (13 721 clés, nouvelle référence `golden.py record-tuning`), 0 écart ; relecture indépendante (L-062, W-028).
- Phase 5a-0 et 5a-1 de la migration Rust (D-035) : nouvelle crate `rust/otb-advisor` (assistant) ; `advisor/profiles.py` (5 profils, taille d'après la fréquence moteur, les cellules et le nom du drone, expressions régulières réécrites à la main) en Rust, identique à Python sur 705 détections écrites et sur les 630 sessions du corpus ; filet de l'assistant : `golden.py record-advisor` (rapport et diagnostic du bruit complets, 97 Mo) et `otb-parity check-advisor` ; L-060.
- Phase 4c-B(d) et (e) de la migration Rust (D-035) : `step.py` (réponse indicielle, métriques, forme de la montée) et `step_series.py` (conditions, signature des filtres, `flight_step`, `find_series`) en Rust, identiques à Python : au bit près sur 170 cas écrits et sur les métriques des 668 sessions du corpus (215 998 valeurs comparées, `otb-parity check-analysis`) ; L-058.
- Phase 4c-B(c) de la migration Rust (D-035) : `loop_from_terms` et le jackknife en Rust : `analysis/loop.py` est entièrement porté, identique à Python sur 82 boucles fermées simulées (`scripts/loop_terms_cases.py`) et sur les 668 sessions du corpus (174 408 valeurs, `otb-parity check-analysis`).
- Phase 4c-B(b) de la migration Rust (D-035) : `loop_spectra` et `estimate` de `analysis/loop.py` en Rust, identiques à Python sur des cas écrits (`scripts/loop_spectra_cases.py`) et sur les boucles mesurées des 668 sessions refaites depuis le journal (écart maximal 1,9e-7 sur les fonctions de transfert, masque des points valides identique) ; L-057.
- Phase 4c-B(a) de la migration Rust (D-035) : `margins`, `loop_from_T` et `dterm_model` de `analysis/loop.py` en Rust (`rust/otb-analysis/src/open_loop.rs`), identiques à Python (à ≤ 2e-15 près, `L` et `dterm_model` au bit près) sur 718 cas écrits (`scripts/loop_cases.py`) et sur les boucles des 668 sessions du corpus (`otb-parity check-analysis`).
- Phase 4c-A de la migration Rust (D-035) : `datasets`, `period`, `stats`, `delay` et la partie Python de `motors` (`rust/otb-analysis/src/`, primitives `float32` de numpy dans `npstat.rs`) en Rust, prouvés équivalents à Python : identiques au bit près (statistiques, périodes, vitesses moteur, centres des notchs, pics) ou à ≤ 1,5e-11 (spectres en dB) et ≤ 2,6e-10 (retards) près, sur des cas limites écrits (`scripts/flight_stats_cases.py`, `delay_cases.py`, `motors_cases.py`) et sur les 668 sessions du corpus (147 712 valeurs, `golden.py record-analysis` puis `otb-parity check-analysis`) ; relecture indépendante (L-051 à L-055, W-023 à W-026).
- Phase 4b de la migration Rust (D-035) : `_numeric` (`rust/otb-dsp/src/numeric.rs`), `spectral`, `bf_filters` (chaînes, réponses, simulation, « et si ») et `sliders` (`rust/otb-analysis/src/`) en Rust, prouvés équivalents à Python : arithmétique pure identique au bit près, FFT / `log10` / `exp` à une tolérance mesurée (1e-14 à 1e-9 selon la fonction), 1 564 appels du corpus rejoués sans écart ; `np.linalg.lstsq` remplacé par une SVD de Jacobi (écart 1,6e-15 sur les 680 sessions). Nouvelles commandes de parité : `golden.py record-sliders` et `otb-parity check-sliders`. L'application utilise encore le code Python.
- `rust/otb-log/src/flightlog.rs` : `FlightLog` en Rust (construction `from_session` / `load`, `pid_sum`, `pid_error`, `d_term_prefilter`, `cells`, `armed`, `pid_terms`, `chirp_excitation`, `segments`, `breaks`, `recording_gaps`, `index_of`, `slice_time`), identique à Python au bit près sur les 668 sessions du corpus et 62 fichiers fabriqués ; `pairwise_sum` de numpy déplacé dans `otb-dsp` ; `golden.py record-flightlog`, `otb-parity check-flightlog` (D-035, phase 4a-5, L-047).
- `rust/otb-log/src/settings.rs` : réglages de l'en-tête (`Settings`, `parse_settings`) en Rust, identiques à Python sur 1 376 cas limites et les 680 sessions du corpus ; `golden.py record-settings`, `otb-parity check-settings` (D-035, phase 4a-4, L-046).
- `rust/otb-decoder/src/file.rs` : décodage d'un fichier Blackbox de bout en bout en Rust (sessions, contexte, trames lentes remplies, statistiques), identique à Python sur tout le corpus (680 sessions) et 34 fichiers fabriqués ou abîmés ; `otb-parity check-decoder`, `scripts/decoder_fixtures.py` (D-035, phase 4a-3).
- `rust/otb-decoder/src/header.rs` : lecture des en-têtes et découpage en sessions en Rust, identique à Python sur 477 cas limites et les 680 sessions du corpus ; `golden.py record-header`, `otb-parity check-header` (D-035, phase 4a-2, L-045).
- `rust/otb-log` : tables de modes de débogage et de boîtes (`debug_tables`, `modes`) portées en Rust, identiques à Python sur toutes les tables et une grille de cas ; `scripts/log_tables_cases.py` (D-035, phase 4a).
- `rust/otb-parity` : comparateur qui relit les références par module et rejoue en Rust les fonctions portées ; `pt1_gain` et `notch_q` portés dans `otb-dsp` (D-035).
- `golden.py record-modules` et `check-modules` : références par module (entrées et sorties de chaque fonction publique de `analysis/` et `log/`, fichiers npz) pour le comparateur Rust (D-035).
- `PidTerms` déplacé de `analysis/loop.py` vers `log/pid_terms.py` (réexporté par `loop.py`, comportement inchangé) : prépare le noyau Rust autonome (D-035).
- `rust/otb-format` : formats de nombres de Python reproduits en Rust (`{:.2f}`, `%g`, `repr`, `round`), identiques sur 12 M de cas ; générateur `scripts/format_cases.py` (D-035, L-044).
- Prototype d'interface egui (`rust/otb-spike-egui`, jetable, hors CI, supprimé ensuite) sur un vrai log : verdict de l'étape E1 de la migration, aucun point bloquant (D-035, L-043).
- Boucles par fenêtre de l'analyse du bruit moteur (hauteur des harmoniques, spectres sans les raies moteur, réponse du filtre RPM) en Rust (`rust/otb-analysis`) : l'assistant est 1,29 fois plus rapide à un fil (666 → 515 ms par session), avec exactement les mêmes constats sur les 630 sessions du corpus (D-034, L-041, L-042).
- Ouverture d'un log : le rejet des trames impossibles et les conversions en float32 sont faits en une seule passe en Rust (`rust/otb-log`) ; un fichier de 34 Mo s'ouvre 1,6 fois plus vite (838 → ≈ 530 ms) et garde 10 % de mémoire en moins, avec un `FlightLog` identique au bit près (D-034).
- Réponse indicielle (`step_response`) : fenêtrage, FFT, déconvolution et contrôle qualité en un noyau Rust parallèle (`rust/otb-dsp/src/step.rs`), résultat identique quel que soit le nombre de fils ; 1,7× plus rapide à un fil, 4,9× avec tous les cœurs ; l'assistant complet gagne 1,2× à 1,4× (D-033).
- Décodeur Blackbox en Rust (`rust/otb-decoder`, extension `opentunebox._native` par PyO3 et maturin), moteur par
  défaut : mêmes tableaux que numba au bit près sur tout le corpus, environ 6 fois plus rapide (22 Mo : 1,94 s →
  0,29 s) ; `OTB_BACKEND=python` garde numba pendant la migration (D-031, L-032).
- `scripts/golden.py` : références figées du décodeur, des outils et de l'assistant pour prouver qu'une réécriture
  en Rust ne change rien ; plan de migration `docs/migration-rust.md` (D-031).
- `tests/test_ci_audit.py` : le workflow d'audit garde sa forme (pip à jour, pip-audit dans un environnement à
  part, dirigé par `--path`) ; leçon L-031.
- La CI publie la couverture de tests (mesure seule, sans seuil) dans le résumé de l'exécution « Tests », décodeur mesuré en
  Python pur compris ; `pytest-cov` rejoint les outils de développement.
- Workflow `audit.yml` : contrôle des failles connues des dépendances d'exécution (`pip-audit`) à chaque envoi,
  chaque demande de fusion et chaque lundi.
- Dependabot : une demande de fusion hebdomadaire pour les actions GitHub (`.github/dependabot.yml`) ; les
  dépendances Python restent des planchers `>=`, que Dependabot ne relève pas.

## [0.8.0] — 2026-10-13

### Ajouté
- `scripts/advisor_rates.py --compare AVANT.json APRÈS.json` : sessions du corpus dont un constat de l'assistant
  a changé, texte et chiffres compris, et temps de calcul gagné ; chaque passage garde le texte complet des
  constats. Sert à prouver qu'une optimisation ou une refactorisation ne change aucun conseil.

### Modifié
- Assistant et Simulateur de filtres plus rapides : la réponse des filtres se calcule étage par étage dans un
  noyau numba (au lieu de polynômes reconstruits pour chaque fenêtre), les notchs RPM de tous les moteurs
  en une fois, et le simulateur partage les réponses de chaque fenêtre entre les axes, le gyro et le terme D.
  Sur tout le corpus, l'assistant passe de 3743 s à 2339 s de calcul cumulé (session la plus lente : 84 s →
  28 s ; `opentunebox report` sur le plus gros log : 28 s → 13,5 s), avec les mêmes constats sur les 705
  sessions.
- Moins de mémoire : les courbes d'un log sont gardées en simple précision (float32, D-029), le décodeur rend
  ses tableaux à la taille exacte et l'écart des trames impossibles ne recopie plus tout le log. Plus gros
  log du corpus (39 Mo) : 458 Mo une fois chargé au lieu de 708 Mo, pic du chargement 1347 Mo au lieu de
  2047 Mo. Mêmes constats de l'assistant sur tout le corpus (9 chiffres affichés décalés d'une unité).
- Simulateur de filtres et Réponse indicielle plus rapides : le Simulateur partage les réponses des filtres
  entre les trois axes et le terme D (plus gros log du corpus : 19,5 s → 2,5 s de calcul) ; la Réponse
  indicielle passe par des FFT réelles et réutilise ses spectres (2,7 s → 1,7 s, résultats identiques).
- Assistant : chaque constat porte une clé stable (`Finding.key`) ; les règles, la stratégie de filtrage, le
  diagnostic du bruit et les tests s'y réfèrent au lieu de relire les titres et les actions, et un constat
  qui conseille `rpm_filter_q` ou `dyn_notch_count` le déclare (`Finding.settings`). `analyze` et la
  vérification des logs du guide sont découpées en sections. Aucun changement des conseils : mêmes constats
  sur tout le corpus avant et après (D-028). `scripts/advisor_rates.py` regroupe les taux par clé.

## [0.7.0] — 2026-10-13

### Ajouté
- Un vrai extrait de log Blackbox (`tests/data/jeno5_5s.bbl`, 5 s d'un vol du JeNo 5 de Cédric, Betaflight
  2026.6.0-rc3) versionné et décodé par les tests sans corpus (`tests/test_real_log.py`), y compris en Python
  pur sur GitHub : en-tête, encodages et prédicteurs réels, unités physiques, commandes `info` et `report`
  (W-009 clos).
- Tests sans corpus des analyses de vol (`tests/test_analysis_flight.py` : fréquence moteur mesurée ou
  estimée par KV ou multiplicateur, médiane en stationnaire, bruit aux harmoniques, statistiques de vol,
  erreur PID robuste, détection de crash et période automatique), du diagnostic du bruit
  (`tests/test_noise_report.py` : rangement des constats, verdict de chaque bloc selon l'eRPM, le gyro
  non filtré, les filtres coupés et la cadence de la boucle), des cas limites du diagnostic du réglage
  (séries incomplètes, « déjà sans dépassement », cassure forte ou incertaine, axe non lu), des textes du
  parcours du guide qui doivent suivre les constantes du code, des scripts du corpus (passage parallèle avec
  reprise des fichiers en erreur, tableaux récapitulatifs) et de la recette PyInstaller (chemins cités,
  filtres Qt et pyqtgraph, lanceur). `tests/test_docs.py` vérifie que `docs/assistant.md` porte chaque
  seuil de `advisor/profiles.py` (bruit, latence, retards, tableau des seuils mécaniques).
- Tests des encodages groupés du décodeur par aller-retour avec des encodeurs de la fabrique
  (`TAG2_3SVARIABLE`, `TAG2_3S32`, `TAG8_4S16` v1 et v2 avec quartets décalés, `TAG8_8SVB`, `NEG_14BIT`),
  trame mêlant tous les encodages, groupe plus long que la définition, encodage inconnu ; lecture de
  l'en-tête (fin de ligne Windows, ligne sans deux-points, octets parasites, trames P héritant de I,
  replis numériques). Simulation des filtres : chaque passe-bas égal à `lfilter`, filtre au-delà de la
  cadence du log transparent, état initial stationnaire, notch à fréquence variable qui suit une raie
  mobile, centres `FFT_FREQ` selon la version, `simulate_gyro` sur le vol synthétique.
- Tests du décodeur sur des fichiers Blackbox fabriqués (`tests/bbl_factory.py`) : `P interval` (N et a/b),
  trames corrompues et resynchronisation, trame trop longue, saut d'itération refusé, `LOGGING_RESUME`,
  événements, trames S, sessions multiples, prédicteurs (dont la troncature vers zéro de `AVERAGE_2`).
- Tests des contrats de l'interface sans corpus : `analysis/`, `advisor/`, `log/`, `decoder/` et la CLI
  n'importent jamais Qt (contrôle du code et à l'exécution), icônes citées présentes avec la licence Lucide,
  deux thèmes sur la fenêtre principale et sur chaque outil, demande rejouée après un calcul, erreur de
  calcul affichée sans plantage.
- Tests de l'assistant sans corpus : chaque seuil de profil encadré (constat présent juste sous le seuil,
  absent juste au-dessus, seuil cité dans le texte), deux branches du « Terme D bruité », repères de retard,
  bornes de la taille par la fréquence moteur, instantané des profils et des constantes (toute modification
  volontaire passe par la compétence `assistant-seuils`), paliers de pic avec et sans feedforward, marges de
  stabilité et de phase, règles de passe-bas, poids RPM et garde de version 4.5, repères de fin des filtres,
  vérification des logs du guide (cadences, version, champs, rôles, usages, mode de vol, curseur I).
- Tests d'invariants des analyses sans corpus : DSP égale à `scipy.signal.welch` (fenêtre paire ou impaire),
  puissance d'un sinus et Parseval, échelle de PIDtoolbox appliquée une seule fois (contrôle statique des
  appelants), valeurs par défaut de la réponse indicielle figées, compte exact des fenêtres, horizon du pic,
  réponse identité et retard pur, bande passante dès 2 Hz, marges (gain × k, retard pur), retards par la
  phase et par intercorrélation, `compute_delays`.
- Tests de la couche log sans corpus : trames impossibles et session corrompue à plus de 50 %, unités de
  `from_session` (DShot / PWM, eRPM ou debug, debug `GYRO_SCALED` comme gyro brut), masques Angle / Horizon et
  instrument de la boucle ouverte, excitation chirp, conventions D / D max aux limites, cadences et
  `P interval`, familles de version, tables de debug et de modes, replis de l'en-tête.
- Tests sans corpus : logs synthétiques (`tests/synthetic.py`) et fixtures communes (`tests/conftest.py`,
  réglages de l'application isolés pendant les tests) ; chaque page d'outil et la fenêtre principale sont
  exercées en intégration continue ; les tests des fonctions pures de l'assistant ne dépendent plus des
  vrais logs ; passe du décodeur en Python pur (numba désactivé) dans le workflow de tests.
- Documentation de développement : journal des décisions (`docs/decisions.md`), leçons tirées des pièges
  (`docs/lecons.md`) et travaux reportés (`docs/travaux-reportes.md`), numérotés `D-`, `L-`, `W-` ; repris de
  `AGENTS.md` et `docs/architecture.md` avec contexte, options écartées et preuves. Numéros vérifiés à chaque
  `pytest`.

### Modifié
- « Raie fixe » : une raie proche de la fréquence des moteurs n'est plus écartée d'office. L'Assistant regarde,
  d'une tranche de gaz à l'autre, si elle suit la vitesse des moteurs ou reste à la même fréquence : une
  résonance du châssis proche de la fondamentale est maintenant signalée (59 vols du corpus au lieu de 45).
  Les harmoniques 2 et 3 gardent l'écart de ± 8 % (D-027).
- Retard des filtres : jugé dès que la consigne dépasse 30 °/s au moins 5 % du temps (au lieu de 50 °/s). Les
  wobbles du protocole en mode Angle limité à 30° (consigne vers 47 °/s) n'étaient pas jugés, comme un vol
  stationnaire. Même seuil pour la découpe « stationnaire / mouvements » du guide (D-026).
- Réponse indicielle : un vol en mode Angle est jugé comme un vol sans feedforward (le firmware l'y coupe),
  même si F > 0 dans les réglages ; il ne reçoit plus « Dépassement dû au feedforward ». Paliers du dépassement
  recalculés sur le corpus : sans feedforward 1,20 (conseil) / 1,25 (attention), avec feedforward 1,60 / 1,70
  (avant : 1,15 / 1,25 et 1,40 / 1,55, calculés en comptant les vols en Angle avec le feedforward). Dépassement
  signalé sur 11 % des vols du corpus au lieu de 13 % (D-025).
- Raies fixes et raies encore visibles après filtrage : l'Assistant ne propose plus de notch statique, sauf si le
  notch dynamique ne peut pas traiter la raie (boucle PID sous 2 kHz, 7 notchs en service, raie au-delà de
  980 Hz), et jamais sous 100 Hz. Il conseille la mécanique, puis la plage ou le nombre de notchs du notch
  dynamique (D-024).
- Notch dynamique : « Q peut monter » propose jusqu'à 500 (au lieu de 450), la valeur du configurateur avec le
  filtre RPM et la plus fréquente des presets officiels (W-003).
- Notch dynamique : le Q conseillé ne descend plus sous 250 (au lieu de 100), le plus bas Q utilisé avec le
  filtre RPM dans les presets officiels et dans le corpus. Un notch plus large ajouterait beaucoup de retard :
  quand une raie demanderait moins, l'Assistant renvoie à sa cause mécanique. Concerne 8 vols du corpus (W-003).
- Filtre RPM : les poids conseillés (« Filtre RPM plus fort que nécessaire ») visent la raie 3 dB sous le
  bruit voisin au lieu de 6 dB, d'après 1 186 harmoniques dont les pilotes du corpus ont déjà baissé le
  poids : elles restent invisibles après filtres jusqu'à 0 dB. Poids proposés plus bas, conseil plus
  fréquent (6 → 18 % des vols) ; chaque baisse de 0,5 retire environ 0,1 à 0,2 ms de retard (W-003).
- Notch dynamique : « Q trop haut pour les raies à suivre » ne s'affiche plus que si une raie à suivre se voit
  encore dans le gyro filtré ; avant, il se trompait une fois sur deux. Les restes d'harmoniques laissés par
  le filtre RPM entrent maintenant dans le Q calculé (106 vols du corpus de plus en ont un).
- Vol stationnaire : les raies hors moteurs proches des harmoniques 2 et 3 ne sont plus cachées (marge de 5 %
  de la fréquence moteur, au lieu de 10 et 15 %), mesurée sur 119 vols stationnaires (D-023, W-012).
- Notch dynamique (Assistant, étape « notch dynamique ») : les pics à suivre sont prévus d'après la réponse
  réelle du filtre RPM (Q, poids de chaque harmonique), au lieu de retirer les harmoniques ± 5 %. Comparé aux
  fréquences que Betaflight suit vraiment (debug FFT_FREQ, 598 vols), le modèle en retrouve 69 % au lieu de
  48 % : une harmonique à poids réduit laisse un reste que le notch dynamique doit suivre. « Moins de notchs
  suffiraient » devient plus rare (97 → 64 vols du corpus), « plus de notchs » un peu plus fréquent (45 → 56,
  puis 62 après les calibrations suivantes).
  L'Assistant ne propose plus de relever `dyn_notch_min_hz` : en vol, les notchs n'approchent presque jamais
  leur minimum, le retard gagné serait d'environ 0,002 ms (D-022, W-003).
- Assistant : les curseurs conseillés suivent la méthode des séries de vols. Filtre gyro par 0,2 ; Damping
  +0,2 sur un fort dépassement, 0,1 en réglage fin ; Master −0,1 à −0,2 ; le feedforward renvoie à la série
  du guide (0, 0,5, 1… en Acro) au lieu d'un pas de 0,1 ; le curseur Tracking n'est plus conseillé seul
  (D-021, W-003).
- « Trames corrompues » devient « Trous dans l'enregistrement », dans l'Assistant et dans la vérification
  des logs du guide : il compte les trous de la période analysée et le temps perdu, au lieu des tentatives
  de resynchronisation du décodeur, qui manquaient des vols vraiment troués (W-003).
- Diagnostic du réglage, série Master : les vagues ne sont plus lues sur un axe de moins de 200 fenêtres
  (« vagues non lues ») ; le bruit seul y donne déjà environ 2 fois le bruit attendu, et une vraie vague de
  10 % passe inaperçue. Le seuil de 2,5 est gardé, désormais mesuré sur le corpus : il ne signale que 0,6 % des
  vols ordinaires sans feedforward et détecte une vague d'environ 3 % sur un vol bien fourni (W-003).
- Notch statique conseillé (« Raie fixe », « Raie à X Hz encore visible ») : la coupure `gyro_notch1_cutoff`
  est calculée d'après la largeur et la hauteur de la raie, comme le Q du notch dynamique (0,62 à 0,89 × sa
  fréquence), au lieu de 0,7 × la fréquence dans tous les cas : notch plus étroit, donc moins de retard, pour
  une raie fine. 0,7 reste le repli quand la raie ne se mesure pas (W-003).
- Réponse indicielle : avec moins de 200 fenêtres (environ 20 s de wobbles), les constats de dépassement, de
  réponse molle et la ligne « ok » ajoutent « mesure moins sûre » : sur le corpus, le pic mesuré sur 20
  fenêtres s'écarte jusqu'à 0,20 de celui du vol entier, 0,07 sur 200. « Réponse molle » se déclenche sous
  un pic de 1,01 (10e percentile du corpus) au lieu de 0,98 (W-003).
- Une raie moteur n'est plus dite « visible » après filtrage qu'au-delà de 4 dB au-dessus du bruit voisin
  (3 dB avant) : mesuré sur le corpus là où il n'y a aucune raie, 3 dB donnait 1,4 à 2,8 % de fausses alertes,
  4 dB moins de 1 %. Moins de conseils sur le filtre RPM et les raies restantes (« Raie moteur encore visible
  après le filtre RPM » 9 → 6 % des sessions), Q du notch dynamique calculé pour laisser 4 dB (W-003).
- Le vol chirp de Betaflight n'est plus conseillé (guide utilisateur) : il faut le compiler à part dans le
  firmware, ses données changent encore d'une version à l'autre et aucune méthode publiée n'en tire un
  réglage. OpenTuneBox suit la méthode de Brian White : quand le pic de sensibilité tombe au bord des
  fréquences mesurables, le constat « Marge de stabilité faible » conseille des wobbles plus rapides et plus
  secs. Un log chirp reste lu et mesuré comme avant (D-019, W-004).
- Toutes les couleurs fixes de l'interface (courbes et repères des graphiques, badges de l'assistant, curseur et
  textes posés sur les spectrogrammes, surlignage de la page Configuration) sont regroupées dans `gui/theme.py` ;
  les écarts verts / rouges du tableau des curseurs PID et des retards du simulateur, le titre d'alerte du
  simulateur et le contour des points de série suivent désormais les couleurs du thème (lisibles en clair comme
  en sombre). Un test refuse toute couleur en dur hors de `theme.py` (W-008 clos).

### Corrigé
- Logs avec des trous d'enregistrement (pauses d'écriture de la carte SD ou de la mémoire flash, 7,5 % des
  sessions du corpus) : les analyses recollaient le vol de part et d'autre de chaque trou, ce qui ajoutait un
  faux bruit (jusqu'à +18 dB sur un vol de wobbles) et faussait spectres, réponse indicielle, retards des
  filtres et conseils de filtrage. Les fenêtres qui chevauchent un trou sont désormais écartées dans tous
  les outils ; Fréquence × temps laisse ces colonnes vides (en gris) et s'étend sur la vraie durée du vol :
  il finissait trop tôt, décalé des courbes des moteurs (D-020, W-010).
- Changer de thème (clair / sombre) repeint aussi les outils déjà ouverts : leur fond de graphique gardait
  les couleurs de l'ancien thème jusqu'à un nouveau calcul.
- Spectres : avec une fenêtre d'un nombre impair d'échantillons (fréquence d'enregistrement non entière),
  la dernière case n'était pas doublée comme chez scipy (3 dB sur une seule case au bord, sans effet visible).
- Simulateur de filtres : sur les fréquences exactement au creux d'un notch (RPM ou dynamique), le spectre
  simulé s'effondrait d'environ 90 dB, même sans aucun changement de réglage (plancher numérique appliqué
  au seul dénominateur du rapport |H_modifié|² / |H_actuel|²).
- Documentation de l'assistant : tableau « Origine des seuils » réaligné sur `profiles.py` (écart roll /
  pitch, déséquilibre, terme I, résonance, règle du nom) ; `hover_hz` sert à la fiche drone du guide.
- Assistant de réglage : la taille de drone choisie était lue dans le widget depuis le thread de calcul ;
  tous les outils passent maintenant leurs réglages par `snapshot_options` (sans repli lisant les
  widgets), ce qu'un test vérifie sur chaque page du registre.
- `opentunebox report` : un fichier illisible ou sans session est signalé en une ligne, comme avec `info`,
  au lieu d'interrompre la commande avec une trace d'erreur.
- Décodeur : un en-tête tronqué dont la trame GPS a moins de prédicteurs que de noms faisait planter la
  lecture (`IndexError`) au lieu d'ignorer la session.
- Assistant, taille d'après le nom du drone (sans télémétrie moteur ni tension) : « 10in » donnait 0 pouce
  (profil 2–3,5") et un nombre contenant 65 ou 75 (« 1750 ») passait pour un whoop.
- Logs en haute résolution (`blackbox_high_resolution = ON`) : le gaz était dix fois trop faible (le gaz du
  mixeur `setpoint[3]` n'est pas mis à l'échelle par le firmware, contrairement aux quatre voies
  `rcCommand`, qui le sont) ; vérifié dans `blackbox.c` 4.4 → 2026.12, spécification corrigée.
- Tests sur les vrais logs : logs supprimés du corpus remplacés par des logs aux mêmes propriétés ; un log
  absent fait ignorer le test en le nommant au lieu de le faire échouer.
- Scripts du corpus sous Linux : les processus parallèles partent d'un serveur de processus (`forkserver`)
  au lieu de bifurquer le processus courant, que Python 3.14 refuse dès qu'il a plusieurs fils (numba, Qt
  pendant les tests) ; mêmes résultats, même durée.
- Documentation relue avant la version : guide utilisateur (série Damping courte dès 6", pas 7"),
  `docs/assistant.md` (Q minimum du notch dynamique 250, pas de `rpm_filter_q` −100, marge des poids RPM
  3 dB, taille par le nom en service), `docs/spec-methodes.md` (bornes de fréquence moteur par taille
  remplacées par celles du code, protocole d'enregistrement du guide), `docs/validation.md`, compétences ;
  leçons L-022 à L-028 rangées par domaine, nouvelle section « Outillage ». Captures du README refaites
  (visualiseur, réponse indicielle, Assistant) : elles dataient d'avant le Guide et montraient d'anciens
  conseils de curseurs.

## [0.6.0] — 2026-10-12

### Ajouté
- **Guide** intégré (bouton de la barre d'outils, affiché au démarrage sans log) : parcours de réglage à
  cocher, par drone, en quatre phases (préparer le drone, filtres, PID par séries, finitions). **Fiche
  drone** (version de Betaflight, taille, boucle PID, ESC ; préremplie d'après le log ouvert ; boutons
  Nouveau drone, Modifier, Recommencer, Supprimer) : fréquences d'enregistrement, listes des séries et plage
  du terme I s'y adaptent, et chaque drone garde son avancement. Changements donnés en positions de curseurs
  du configurateur, le CLI en repli ; boutons qui ouvrent l'outil utile sur la bonne partie du vol.
- Guide : les logs du drone (reconnus par le nom dans Betaflight) **cochent ce qu'ils prouvent** ou signalent
  ce qui est à revoir, avec la raison. Logs des **filtres** et logs des **séries PID** demandés et jugés à
  part : bouton d'ouverture dans chaque phase, panneau des logs en deux parties, lien pour changer le rôle
  d'un log. Le vol de réglage des filtres (stationnaire puis wobbles) est découpé automatiquement ; protocole
  de Brian White vérifié (mode Angle, ou Acro avec le feedforward à 0 ; rates ACTUAL 200 / 200 / 0 ; limite
  d'angle 30° en mode Angle ; roll et pitch mêlés ; Dynamic Damping à 0 ; curseur I ; au moins 200
  fenêtres) ; chaque série d'un drone reconnue ; repères de fin des filtres mesurés ; copie des commandes
  CLI équivalentes.
- **Série Feedforward** (curseur « Réponse des sticks », pas de 0,5 : 0, 0,5, 1…, en Acro uniquement) :
  l'Assistant donne le **retard consigne → gyro** en roll et en pitch de chaque vol, et le guide le résume
  sous les logs des séries. La Réponse indicielle reconnaît la série sans la juger (le feedforward fausse la
  réponse).

### Modifié
- Fréquence d'enregistrement : minimum propre aux **PID** (1 kHz), distinct de celui des **filtres** (2 kHz
  ou 1,6 kHz, plus pour les petits drones). Un vol de série PID à 1 kHz n'est plus signalé « trop lent » par
  l'Assistant (simple conseil pour les filtres) ni bloqué par la vérification des logs du guide, qui indique
  les deux fréquences.
- Réponse indicielle, Diagnostic du réglage : un vol de série avec moins de 200 fenêtres en roll ou en pitch
  est signalé « mesure moins sûre » (consigne de Brian White) ; sous 20, il n'est toujours pas lu.
- Spectre, Fréq. × gaz, Fréq. × temps et Filtres : les niveaux en dB sont affichés sur l'échelle de
  PIDtoolbox (6,5 dB plus bas qu'avant), pour lire directement les repères habituels (gyro sous −30 dB,
  terme D sous −10 dB) et comparer aux captures de PIDtoolbox.
- Spectre : résolution par défaut 0,5 Hz (fenêtres de 2 s), comme PIDtoolbox.

## [0.5.0] — 2026-10-10

### Ajouté
- Réponse indicielle : **Diagnostic du réglage**, toujours affiché à côté des courbes, méthode de Brian White. Reconnaît
  les séries de vols chargées (un seul curseur changé, mêmes filtres) et conseille le Damping (équilibre P/D),
  le Master (cassure « ripple », vagues et plateau de la latence) et l'équilibre roll / pitch (écart de latence
  de 1 à 2 ms au plus) ; vérifie le feedforward (0 ou mode Angle), le curseur I (0,2–0,3) et le pas (0,2).
  Vols conseillés entourés dans les colonnes de mesures, cassure marquée sur les courbes, colonne
  « Cassure (ripple) ».
- Spectre : case **Diagnostic du bruit** (cadre « Diagnostic », en bas du panneau), rapport à côté des courbes sur le bruit du gyro et du terme D
  avant et après filtrage (niveaux par axe), un bloc par filtre (filtre RPM, notch dynamique, passe-bas
  du gyro et du D) avec ses réglages, ses mesures et un verdict même sans conseil (« rien à changer »
  ou « impossible à juger », avec la raison), et stratégie de filtrage conseillée, avec les constats et
  seuils de l'assistant.
- Visualiseur : bouton **Mesurer**, écart de temps (ms) et fréquence correspondante (Hz) entre deux points
  cliqués sur les graphiques roll, pitch ou yaw ; repères déplaçables à la souris.

### Modifié
- Interface : plus aucune nouvelle fenêtre. Les boutons de la barre d'outils servent d'**onglets** : un clic
  affiche l'outil dans la fenêtre principale, nouveau bouton **Visualiseur** pour revenir aux courbes du vol,
  bouton de l'affichage en cours mis en valeur. Icônes des outils en couleur (une couleur par outil, lisible dans les
  deux thèmes).
- Visualiseur : barre d'outils allégée. Fréq. × gaz et Fréq. × temps s'ouvrent depuis l'outil Spectre ;
  le bouton Configuration passe en bas du panneau de droite.
- Réponse indicielle : courbes plus larges (gains, pic, latence et cassure de chaque fichier écrits dans le
  graphique, plus de colonne à part), durée affichée au choix (250 ms par défaut, 500 ou 150 ms), yaw
  masqué au départ, curseurs PID Betaflight toujours affichés sous le diagnostic (plus de case « Vue curseurs
  Betaflight »).
- Curseurs PID Betaflight (Réponse indicielle et Curseurs PID) : noms du configurateur Betaflight (Damping /
  D Gains, Tracking / P & I Gains, Stick Response, Dynamic Damping, Drift - Wobble, Pitch Damping, Pitch
  Tracking, Master Multiplier) au lieu de traductions maison.
- Spectre et Réponse indicielle : le panneau de réglages garde sa largeur quand le rapport s'ouvre (il
  laissait un grand vide à droite).
- Assistant : `rpm_filter_q` conseillé par pas de 100 dans les deux sens (au lieu de +250 et −30 %) ;
  nouveau conseil « Filtre RPM : Q peut monter » quand les harmoniques traitées sont effacées et que le
  bruit est sous les repères (jusqu'à 1000) ; nouveau conseil « Notch dynamique : Q peut monter » (+50
  jusqu'à 450 avec filtre RPM quand aucune raie ne passe).
- Assistant : `dyn_notch_q` calculé d'après la largeur et la hauteur des raies à suivre (baissé jusqu'à
  100 si une raie l'exige, monté jusqu'à 450 sinon) ; `dyn_notch_count` augmenté (un notch par pic) quand
  une raie passe ou que le gyro est bruité.
- Assistant : nouveau conseil « Terme D propre : ses filtres peuvent être relevés » sur tout vol, quand le
  D est sous −20 dB PIDtoolbox : le simulateur choisit le plus grand pas du curseur « filtre D » (jusqu'à
  1,2) qui garde le D sous les repères.
- Visualiseur : la période d'analyse se règle sur les graphiques roll, pitch et yaw, plus seulement sur
  celui des moteurs ; les quatre zones restent synchronisées pendant le glissement.
- Visualiseur : info-bulle sur les bords de la période d'analyse (début ou fin, avec le temps).

### Corrigé
- Réponse indicielle : les noms des curseurs Betaflight (sur deux lignes) ne se chevauchent plus sous le
  diagnostic ; le graphique reçoit la hauteur nécessaire, le rapport défile.

### Supprimé
- Fenêtre Statistiques de vol (peu utile ; les mesures restent utilisées par l'assistant).

## [0.4.0] — 2026-10-06

### Ajouté
- Assistant : constat « Seuils provisoires pour cette taille » pour les whoops, les 2–3,5" et les 8"+
  (trop peu de drones dans le corpus de calibration).
- `scripts/calibrate_advisor.py` : déséquilibre, terme I, saturation, résonance, fréquence moteur et nom
  du drone mesurés ; `--resume` affiche les percentiles par taille, par session et avec une voix par drone.
- `scripts/corpus_labels.py` : tableau d'étiquettes du corpus (`corpus.csv` dans le dossier des logs, une
  ligne par drone : état, taille réelle, fréquence moteur, KV, cellules), pré-rempli puis complété à la
  main ; un nom porté par plusieurs drones (« DroneX ») est complété par le dossier ; les logs en double
  (même contenu) et les dossiers `__MACOSX` sont ignorés, aussi par `calibrate_advisor.py` ; `calibrate_advisor.py --resume` affiche à côté de chaque groupe son corpus de référence (drones
  réglés ou d'origine, hors demandes d'aide).

- Réponse indicielle : **boucle ouverte mesurée** à partir des termes P, I, D et F du log (D sur le gyro,
  feedforward sur la consigne, comme dans Betaflight) au lieu de l'approximation T/(1 − T), qui plaçait
  le croisement environ 2 fois trop bas et surestimait la marge de phase. Marge de gain ajoutée ; barres
  d'erreur (jackknife) sur les marges ; point creux quand la valeur n'est qu'approchée (termes absents),
  triangle quand le pic de sensibilité n'est qu'un minimum (au-delà des fréquences mesurables).

- Assistant : repères de Brian White (PIDtoolbox) sur l'échelle de ses spectres (nos DSP sont 6,5 dB
  plus hautes) : terme D signalé aussi quand le sommet de son spectre dépasse −10 dB ; niveau du gyro
  filtré affiché face au repère −30 dB ; objectifs de retard des filtres selon la taille (graphique de
  Brian White) rappelés dans les textes ; conseil « filtrage gyro trop prudent » complété par
  `rpm_filter_q` + 250, par pas jusqu'à 1000 (documentation de Betaflight 4.5).
- Assistant : poids du filtre RPM (`rpm_filter_weights`, 4.5 et plus) — poids juste suffisant calculé pour
  chaque harmonique à partir de sa hauteur au-dessus du bruit voisin et de la formule du firmware ;
  conseil quand la 2e ou la 3e harmonique est filtrée bien plus que nécessaire, ou quand une raie reste
  visible alors que son poids a été baissé.
- Assistant : notch dynamique (notes de réglage Betaflight 4.3 et code du firmware) — pics qu'il a à suivre
  après le filtre RPM ; « moins de notchs suffiraient » (info) ; plage élargie quand une raie fixe en sort
  (20 Hz de marge, jusqu'à la limite de 20 Hz du firmware) ; `dyn_notch_min_hz` relevé dans le conseil
  « filtrage gyro trop prudent ».
- Assistant : raies fixes (résonances) cherchées dès 20 Hz au lieu de 60 Hz.
- Assistant : **stratégie de filtrage** en tête du diagnostic, qui ordonne les réglages (enregistrement,
  bruit à corriger, mécanique, filtre RPM, notch dynamique, passe-bas du gyro et du D).
- Assistant : vol stationnaire exploité pour identifier le bruit (« Vol stationnaire : bruit identifié » :
  harmoniques avant / après filtres, autres raies, bruit restant) et conseiller un notch pour chaque raie
  encore visible ; harmonique moteur hors du filtre RPM (`rpm_filter_harmonics`) ou visible malgré un poids
  de 100 (`motor_poles`, `rpm_filter_q`).
- Assistant : fréquence d'enregistrement minimale selon le gyro (2 kHz à 8 kHz, 1,6 kHz à 3,2 kHz, plus
  pour voir la 3e harmonique des petits drones) au lieu de 900 Hz ; sous ce minimum, aucun allègement de
  filtre n'est proposé.
- Retards des filtres jugés seulement sur un vol avec des mouvements (consigne au-delà de 50 °/s au moins
  5 % du temps) : sur un vol stationnaire, l'assistant affiche « Retard des filtres peu représentatif »
  sans conseil, et l'outil Spectre marque le retard « peu fiable ».
- Assistant : passe-bas du gyro et du D (configurateur et notes de réglage de Betaflight) — LPF2 du gyro
  coupé avec une boucle PID plus lente (repliement) ; D filtré à l'ordre 1 seulement ; LPF1 du D statique ;
  biquad sur le gyro ; « Le LPF1 du gyro peut être coupé » quand le simulateur de filtres prévoit un bruit
  encore sous les repères ; prévision du simulateur dans le conseil sur le filtrage du D, qui n'est plus
  proposé si le D dépasserait les repères.
- `scripts/calibrate_advisor.py` : sommets des spectres gyro et D sur l'échelle de PIDtoolbox, retard gyro
  par intercorrélation, réglages du filtre RPM, hauteur des harmoniques avant et après filtres.
- Scripts du corpus en parallèle sur tous les cœurs (`-j N` pour choisir, `-j 1` = un fichier à la fois) :
  `calibrate_advisor.py` en 2 min 40 au lieu d'une dizaine de minutes, `corpus_labels.py` en 40 s ; un
  fichier en erreur dans un processus est refait seul à la fin. Nouveau `scripts/advisor_rates.py` : part
  des sessions du corpus signalées par chaque constat de l'assistant (vols avec mouvements, stationnaires).

### Modifié
- Assistant : sur un whoop, les constats de marge de stabilité expliquent la cause habituelle (boucle
  rapide, D dominant, filtres du D qui retardent au croisement). Diagnostic des whoops souvent signalés :
  la mesure est vérifiée (simulation, vols chirp de l'AIR65), la marge est réelle, seuils inchangés.
- Assistant : paliers du pic de réponse indicielle calibrés sur le corpus et séparés selon le feedforward
  (qui gonfle le pic) : sans feedforward, conseil au-delà de 1,15 et attention au-delà de 1,25 ; avec, 1,40
  et 1,55 (avant : 1,10 et 1,20 pour tous, 46 % des sessions signalées, maintenant 13 %). L'objectif
  « 1,02–1,08 », sans source, est remplacé par le repère de la série BlackSummer (environ 1,03 sans
  dépassement visible), dans les constats, l'outil « Indicielle » et le guide.
- `scripts/calibrate_advisor.py` : gains P, D et F de chaque axe et positions des curseurs enregistrés.
- Assistant : repères de retard des filtres recalculés sur les seuls vols avec mouvements, pour toutes les
  tailles (gyro / D : whoop 1,5 / 2,5 ms, 2–3,5" 1,8 / 2,6, 5" 1,9 / 2,7, 7" 2,6 / 2,8, 8"+ 3,5 / 3,1) :
  les vols stationnaires les avaient fait monter, et les conseils sur le retard ne touchaient plus que 2 à
  4 % des vols.
- Assistant : la stratégie de filtrage n'apparaît plus qu'à partir de deux étapes de filtrage à revoir
  (37 % des sessions du corpus au lieu de 79 %) ; une seule étape, le constat seul suffit.
- Assistant : un retard des filtres gyro au-dessus du repère n'est plus affiché « ok » quand le bruit
  empêche de filtrer moins (info) ; nouvelle ligne « ok » pour le retard des filtres du D.
- Assistant : taille du drone détectée par la fréquence des moteurs **et** le nombre de cellules
  (1S → whoop ; 2 à 4S entre 200 et 290 Hz → 2–3,5") ; limite 5"/7" abaissée de 175 à 160 Hz. Juste pour
  38 drones de taille connue sur 41, au lieu de 27 : les 3,5" et 2,5" ne sont plus pris pour des 5".
- Assistant : seuils recalibrés sur tout le corpus avec la nouvelle détection (2–3,5", 5", 7" ; whoop et
  8"+ inchangés) ; stabilité en conseil au-delà de 7 dB de pic de sensibilité ou sous 35° de marge de phase
  (au lieu de 6 dB et 40°). Plus de corpus de référence : `calibrate_advisor.py --resume` compte tout le
  corpus.
- Assistant : constats de stabilité sur la boucle mesurée, en conseil (et non plus en alerte) jusqu'à la
  recalibration du seuil ; quand le D domine déjà au croisement, il conseille Master −0,1 plutôt que plus
  de D.
- Documentation : chaque repère de réglage « communauté » a sa source (auteur, date, adresse, citation
  exacte) dans `docs/spec-methodes.md`, ou est reclassé en estimation. Les paliers chiffrés du pic de
  réponse (1,02–1,08, 1,10, 1,20) et le tableau des fréquences moteur par taille n'ont aucune source
  publique : ce sont des estimations. Repères recoupés avec les vidéos de Brian White (chaîne PIDtoolbox),
  citées avec le moment de la vidéo : terme D sous −20 dB (−10 au plus), retard gyro ≤ 1,5 ms pour un 5".
- Assistant : seuils mécaniques calibrés sur 90 drones (déséquilibre 11–14 % au lieu de 12 %, terme I
  29–33 au lieu de 40, saturation 1,5 % du temps au lieu de 5 %, écart roll / pitch 0,07–0,12 au lieu
  de 0,08, résonance 7,3–7,9 dB au lieu de 6 dB).
- Assistant : limite whoop abaissée de 420 à 345 Hz (Meteor 75 et P85 reconnus comme whoops) ; seuils
  whoop recalculés avec une voix par drone, seuils 2–3,5" recalculés sans eux.
- Documentation de l'assistant : origine de chaque seuil (corpus, communauté, théorie, firmware ou
  estimation), comparaison avec le corpus et champs de profil inutilisés (`docs/assistant.md`) ;
  sources vérifiées (Skogestad et Postlethwaite, Åström et Murray, firmware et configurateur Betaflight) ;
  limite connue du pic de sensibilité et de la marge de phase (L = T/(1 − T), D sur le gyro) ; l'onglet
  Autotune du configurateur examiné et écarté comme source.

### Corrigé
- Réponse indicielle : les vols faits avec le chirp de Betaflight (debug CHIRP) en mode angle sont
  maintenant mesurés (boucle ouverte, marges) ; l'excitation du chirp, absente de la commande radio, servait
  de signal de référence manquant (« cohérence insuffisante »).
- Marge de phase de la réponse indicielle : la phase est maintenant déroulée. Une phase sous −180° au
  croisement donnait une marge repliée vers +180° (ex. +299° au lieu de −109°), donc aucune alerte de
  l'assistant ; elle est désormais négative. Croisement interpolé entre deux points de mesure.

## [0.3.0] — 2026-10-05

### Ajouté
- Installateurs construits par GitHub (`construire.yml`) et joints à la page Releases :
  - Windows : `OpenTuneBox-Windows-Setup.exe` (Inno Setup), sans droits administrateur, menu Démarrer,
    raccourci du Bureau et ouverture des `.bbl` / `.bfl` en option, désinstallation depuis les
    Paramètres, mise à jour par-dessus l'ancienne version. Il remplace le `.zip` : une seule version
    Windows, pour ne pas faire hésiter l'utilisateur.
  - macOS : image disque `OpenTuneBox-macOS.dmg` (glisser dans Applications), icône de l'application,
    numéro de version et ouverture des `.bbl` / `.bfl` depuis le Finder.
  - Linux : `OpenTuneBox-Linux.AppImage`, un seul fichier à rendre exécutable (remplace le `.tar.gz`).
  - Essais automatiques de chaque fichier : démarrage sur les trois systèmes ; sous Windows,
    installation, lancement puis désinstallation complète.
- Icônes macOS (`.icns`) et Linux (`.png`) fabriquées depuis le SVG par `packaging/make_icons.py`.
- Documentation vérifiée automatiquement à chaque `pytest` et sur chaque demande de fusion
  (`tests/test_docs.py`) : liens et titres, chemins et fonctions cités, chaque outil dans le README et le
  guide, chaque module dans `AGENTS.md` / `docs/architecture.md`, fichiers publiés, commandes, langue.
  Compétence `docs-sync` (quels documents relire selon le changement) et crochet Claude Code qui bloque
  un commit si la documentation n'est plus cohérente.

### Modifié
- Assistant : taille du drone détectée par la fréquence des moteurs **et** le nombre de cellules
  (1S → whoop ; 2 à 4S entre 200 et 290 Hz → 2–3,5") ; limite 5"/7" abaissée de 175 à 160 Hz. Juste pour
  38 drones de taille connue sur 41, au lieu de 27 : les 3,5" et 2,5" ne sont plus pris pour des 5".
- Assistant : seuils recalibrés sur tout le corpus avec la nouvelle détection (2–3,5", 5", 7" ; whoop et
  8"+ inchangés) ; stabilité en conseil au-delà de 7 dB de pic de sensibilité ou sous 35° de marge de phase
  (au lieu de 6 dB et 40°). Plus de corpus de référence : `calibrate_advisor.py --resume` compte tout le
  corpus.
- Documentation entièrement relue et mise à jour : README (outils sous le nom de leur bouton, partie
  développeur séparée), guide (installation et ouverture des logs réorganisées), assistant (toutes les
  règles réelles, détection de la taille, profil 8"+ provisoire), méthodes d'analyse (retards, lissages,
  latence corrigés), architecture (fenêtres d'outils, distribution), validation ; spécifications
  traduites en français.
- Tests rapides (`-m "not corpus"`) de nouveau en ≈ 1 s : les tests d'interface sur vrais logs portent
  la marque `corpus`.

### Corrigé
- Linux : la fenêtre d'ouverture de fichiers était peu lisible (icônes claires presque invisibles avec le
  thème clair sur un bureau sombre, textes en anglais, colonnes tronquées). OpenTuneBox utilise désormais
  la fenêtre du bureau (KDE, GNOME, Cinnamon, XFCE…) par le portail XDG, quel que soit le bureau ; sans
  portail, la fenêtre de Qt reste lisible : icônes du bureau en variante claire ou sombre selon le thème
  choisi, et textes de Qt en français (aussi les boutons OK / Annuler sur tous les systèmes).
- Page Releases : la publication échouait parce que les archives `.zip` Windows et macOS étaient
  décompressées avant d'être jointes (`skip-decompress` de `download-artifact`).

## [0.2.0] — 2026-10-05

### Ajouté
- Structure GitHub : `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, modèles de tickets et de demande de fusion,
  tests automatiques à chaque envoi (workflow `tests.yml`), `.gitattributes`, `.editorconfig`.
- Construction Windows par double-clic (`packaging/windows/construire_windows.bat`) et workflow GitHub
  `construire.yml` (Windows, macOS, Linux).
- Icône d'application sous Linux (fenêtres, barre des tâches Wayland, menu) et sous Windows (`.ico`).
- Dossier de logs des tests réglable par la variable `OTB_LOGS`.
- Page de téléchargement **Releases** créée automatiquement pour chaque étiquette de version `v…`, avec
  les archives Windows, macOS et Linux et la section du CHANGELOG ; script `scripts/release.py` qui
  prépare une version (numéro dans `pyproject.toml` et `__init__.py`, section datée du CHANGELOG) et
  arrête la construction si l'étiquette ne correspond pas.
- Convention de nommage des branches (`type/description-courte`) et des demandes de fusion (titre = nom
  de la branche) dans `CONTRIBUTING.md`, `AGENTS.md` et le modèle de PR.

### Modifié
- Assistant : taille du drone détectée par la fréquence des moteurs **et** le nombre de cellules
  (1S → whoop ; 2 à 4S entre 200 et 290 Hz → 2–3,5") ; limite 5"/7" abaissée de 175 à 160 Hz. Juste pour
  38 drones de taille connue sur 41, au lieu de 27 : les 3,5" et 2,5" ne sont plus pris pour des 5".
- Assistant : seuils recalibrés sur tout le corpus avec la nouvelle détection (2–3,5", 5", 7" ; whoop et
  8"+ inchangés) ; stabilité en conseil au-delà de 7 dB de pic de sensibilité ou sous 35° de marge de phase
  (au lieu de 6 dB et 40°). Plus de corpus de référence : `calibrate_advisor.py --resume` compte tout le
  corpus.
- Scripts d'installation rangés par système : `packaging/linux/`, `packaging/windows/`.
- Interface plus lisible, identique sous Windows, macOS et Linux : style Qt « Fusion » avec palettes
  claire et sombre complètes (fini le gris beige et les boutons qui semblaient désactivés sous Windows),
  couleurs des boutons d'outils contrastées dans chaque thème, cases à cocher et boutons radio bien
  visibles, traces repérées par un trait de couleur, textes d'aide moins petits.
- Fenêtre principale modernisée : barre d'outils avec icônes (Lucide, licence ISC) à la place des boutons
  du panneau droit, texte 15 % plus grand, traces groupées (Gyro, PID, Moteurs) et traces absentes du log
  masquées, sections « Affichage » et « Lecteur » repliables, traces lissées d'épaisseur 2 par défaut,
  grille limitée aux graduations principales.
- Fenêtres plus grandes : au premier lancement, la fenêtre principale occupe 95 % de l'écran (90 % pour
  les outils), comme PIDtoolbox ; ensuite chaque fenêtre rouvre à la taille et à la position laissées.
- Captures du README (visualiseur, réponse indicielle, assistant) refaites avec la nouvelle interface.

### Corrigé
- Visualiseur sans log ouvert : les graphiques ne sont plus entièrement grisés.
- Le raccourci Linux ne se déclare plus capable d'ouvrir tous les fichiers binaires.
- Windows : erreur « 'NoneType' object has no attribute 'flush' » à la fermeture de l'exécutable
  (pas de console, donc pas de `sys.stdout`).
- Exécutables construits par GitHub (`construire.yml`) : une seule archive par système, qui garde les
  droits d'exécution et les liens symboliques (le programme Linux se lançait mal et l'application macOS
  pouvait être abîmée) ; macOS n'envoie plus l'application en double (481 Mo auparavant) ; version Linux
  fabriquée sur Ubuntu 22.04 pour fonctionner aussi sur les distributions plus anciennes, avec les
  bibliothèques `libxcb` embarquées et un essai de démarrage.
- Exécutable allégé de 488 à 460 Mo sous Linux : sans le clavier virtuel de Qt (et son moteur QML/Quick),
  le lecteur PDF, les traductions de Qt ni les exemples, la 3D et l'intégration Jupyter de pyqtgraph.
  La construction Linux ne plante plus en parcourant les exemples de pyqtgraph (ouverture d'une fenêtre
  sur un serveur sans écran).

### Problèmes connus
- Analyse spectrale : les courbes des notchs dynamiques chargent fortement le spectre complet.
- Fréquence × gaz : le terme D (surtout préfiltre) sature l'échelle par défaut ; « Échelle auto » compense.
- Assistant : le graphique yaw affiche un axe incohérent quand les données sont insuffisantes.

## [0.1.0] — 2026-10-01

Première version complète.

### Ajouté
- Décodeur Blackbox natif (numba) : tous les encodages, prédicteurs et trames I/P/S/G/H/E, plusieurs
  sessions par fichier, fichiers tronqués ; validé sur 568 fichiers (95 % des sessions identiques à
  `blackbox_decode`, le reste étant des trames corrompues que la référence accepte).
- Modèle `FlightLog` en unités physiques, réglages d'en-tête normalisés de Betaflight 4.4 à 2026.x,
  tables `debug_mode` par version, modes de vol.
- Analyses : spectres, fréquence × gaz, fréquence × temps, retards de filtres (méthode de phase),
  réponse indicielle par déconvolution de Wiener et métriques de boucle, simulation exacte des filtres
  Betaflight, curseurs simplifiés ⇄ PID, statistiques de vol, erreur PID, période automatique avec
  détection de crash.
- Interface PySide6 : visualiseur de log et 9 outils (analyse spectrale, fréquence × gaz, fréquence × temps,
  réponse indicielle, curseurs PID, simulateur de filtres, infos de configuration, statistiques, assistant).
- Assistant de réglage à règles, profils whoop / 2–3,5" / 5" / 7" / 8"+ détectés par la rotation moteur,
  seuils calibrés sur 708 sessions.
- Ligne de commande `opentunebox info` et `opentunebox report`.
- Lanceur Linux (menu des applications) et recette PyInstaller.
- Guide d'utilisation en français.

[Non publié]: ../../compare/v0.8.0...HEAD
[0.8.0]: ../../releases/tag/v0.8.0
[0.7.0]: ../../releases/tag/v0.7.0
[0.6.0]: ../../releases/tag/v0.6.0
[0.5.0]: ../../releases/tag/v0.5.0
[0.4.0]: ../../releases/tag/v0.4.0
[0.3.0]: ../../releases/tag/v0.3.0
[0.2.0]: ../../releases/tag/v0.2.0
[0.1.0]: ../../releases/tag/v0.1.0
