# Leçons

La mémoire partagée des pièges d'OpenTuneBox : chaque entrée est un problème réel, avec sa cause et la
règle qui l'évite. Le but est de **ne pas payer deux fois** la même erreur, ni soi-même ni un agent IA qui
arrive sans mémoire de ce qui s'est passé. Registre jumeau du [journal des décisions](decisions.md) (`D-xxx`)
et des [travaux reportés](travaux-reportes.md) (`W-xxx`).

## En début et en fin de session

1. **Avant d'écrire du code** : lire l'index rapide, les arrivées récentes et la section du domaine touché.
2. **Confronter à [AGENTS.md](../AGENTS.md)**, qui garde la règle courte de chaque leçon (« Pièges connus »,
   suivie de son numéro) ; ce fichier garde l'histoire complète.

   | Constat | Action |
   |---|---|
   | Une règle d'`AGENTS.md` n'a pas d'entrée ici | Rien à faire : toutes les règles ne viennent pas d'un piège. |
   | Un piège absent des deux | Ajouter une entrée ici (protocole ci-dessous). |
   | Un piège qui doit bloquer *toute* session | Proposer sa règle courte à Cédric pour `AGENTS.md`. |
   | Une règle contredite par le code réel | Le signaler explicitement, ne pas trancher en silence. |

3. **Avant de rendre la main** : du temps perdu sur quelque chose qu'une entrée aurait évité ? L'écrire.

## Protocole

**Quand écrire une entrée** — dès qu'un problème remplit l'un de ces critères :
- il a coûté plus d'un aller-retour de débogage ;
- il échoue **silencieusement** (pas d'erreur, résultat faux) ;
- sa cause est contre-intuitive ou propre à une bibliothèque, au firmware ou à l'outillage ;
- il a déjà été rencontré deux fois.

**Ne pas écrire** : une faute de frappe, un fait déjà décrit dans une spécification ou dans
[assistant.md](assistant.md) (mettre un lien), un choix (→ `D-xxx`). La leçon dit *ce qui casse*, la
décision dit *ce qu'on a choisi*.

**Numérotation** : une seule série `L-001`, `L-002`…, jamais réutilisée ; relire le dernier numéro de
l'index avant d'en prendre un. Une entrée neuve va en tête des **Arrivées récentes** et dans l'**Index
rapide** ; au tri (avant une version), elle descend dans sa section de domaine sans changer de numéro.

**Récidive** : le même piège ne prend pas de nouveau numéro ; ajouter sous l'entrée d'origine
`**⚠️ RÉCIDIVE le AAAA-MM-JJ** — ce qui s'est passé, règle précisée`. Deux récidives : proposer un contrôle
automatique (test) ou une règle plus visible.

**Règle d'or : ne consigner que ce qui est vérifié.** Une leçon fausse sera crue sur parole par les
sessions suivantes. Une cause non mesurée se dit « supposée ».

**Format**

```markdown
### L-0XX : <le piège en une phrase : ce qui casse, et le symptôme trompeur>
- **Symptôme** — date : ce qu'on observe (message exact ; « rien en console » se dit aussi).
- **Cause** — le mécanisme. **Mesurée** (chiffres, source lue) ou **supposée** : le dire.
- **Règle** — ce qu'il faut faire, applicable sans relire le reste.
- **Preuve** — commit, `fichier:fonction`, test qui échoue sans la correction, mesure de validation.md.
```

**Renvois** : dans le code, `# ⚠️ <ce qui casse si on l'ignore> (L-xxx)`, un problème par ligne.

Les entrées L-001 à L-021 ont été reprises le 2026-10-12 des « Pièges connus » d'`AGENTS.md` ; leur date
est celle du commit qui les a introduites.

## Arrivées récentes

### L-083 : un test qui saute en CI n'a pas suivi le contrat qu'il vérifie (tolérances ajoutées après lui)
- **Symptôme** — 2026-11-04 : `cargo test -p otb-parity --test golden` rouge avec `OTB_GOLDEN`, sur un seul fichier
  (`spectral.spectrum`, écart au 15ᵉ chiffre), alors que `otb-parity check` sur les mêmes références donne 0 échec.
- **Cause (mesurée le 2026-11-05)** — `golden.rs` exigeait le bit près pour toutes les fonctions depuis le 2026-10-30 ; le même jour,
  les tolérances mesurées (FFT, `sin` / `cos`, `exp`, SVD) sont entrées dans `registry::tolerance` et seul `check` les a appliquées. Le
  test ne tourne qu'avec les références, absentes de la CI : rouge par construction sans que personne le voie. Il s'arrêtait au
  premier écart et cachait les 242 autres (8 fonctions).
- **Règle** — un test et l'outil qui vérifient le même contrat lisent la **même** source de tolérance (`registry::tolerance`), jamais
  une valeur recopiée ; un test de concordance liste tous ses écarts, pas le premier. Avant de dire « test à ne pas affaiblir », compter
  ses écarts et les comparer au contrat documenté.
- **Preuve** — D-047 ; `rust/otb-parity/tests/golden.rs` ; contre-preuves : tolérance à zéro → 243 écarts, un bit de plus sur
  `uniform_filter1d` → 40.

### L-082 : exécuter un programme qu'on vient de copier échoue par intermittence (« Text file busy ») quand un test voisin lance un processus
- **Symptôme** — 2026-11-05 : `rust/otb-dev/tests/install_linux.rs` échoue 2 fois sur 5 exécutions isolées après l'ajout d'un 3ᵉ cas de test, sur `spawn()` ligne 49 : `Os { code: 26, kind: ExecutableFileBusy, message: "Text file busy" }`. Le cas qui échoue n'est pas le nouveau, et le même fichier passait auparavant (un seul échec constaté en six exécutions de `verify` : W-047, cause alors non trouvée).
- **Cause (reproduite)** — les cas d'un fichier de test tournent **en parallèle dans le même processus**. L'un copie `/bin/sh` puis l'exécute aussitôt ; les autres lancent le script d'installation. Quand un `fork` se produit pendant que la copie est encore ouverte en écriture, l'enfant hérite du descripteur le temps du `fork`/`exec`, et le noyau refuse d'exécuter un fichier ouvert en écriture (`ETXTBSY`). Plus il y a de cas qui lancent des processus, plus la fenêtre est touchée : d'où un piège qui dormait et devient fréquent.
- **Règle** — un test qui exécute un fichier **qu'il vient d'écrire** réessaie sur `ETXTBSY` (code 26) au lieu de `unwrap()` : voir `lance_malgre_etxtbsy` dans ce fichier. Réessayer la mise en place ne touche aucune assertion ; ne pas « régler » le problème en désactivant le parallélisme des tests ni en supprimant le cas gênant.
- **Preuve** — avant : 2 échecs sur 5 exécutions de `cargo test -p otb-dev --test install_linux` ; après le réessai : 10 exécutions de suite au vert (2026-11-05). Clôt W-047.

### L-081 : le pic de mémoire d'une ouverture se mesure par étape, et la « conversion session par session » ne sert qu'aux fichiers à plusieurs sessions
- **Symptôme** — 2026-11-04 (lot 5) : `LOG00008.BFL` (une session, 620 079 trames) demande 481 Mo. Le plan attendait un gain de la conversion session par session (5a) : elle a donné 513 Mo (+ 32 Mo, le fichier restant lu) ; aucun log du corpus de plus de 1 Mo n'a deux sessions.
- **Cause (mesurée par `VmHWM` à trois endroits de `from_session`)** — le pic n'était pas dans le décodeur (295 Mo) mais à la fin de `from_session` : la table d'entiers 64 bits (213 Mo) restait vivante pendant que les copies finales (`f.axes`, moteurs…) se construisaient (468 Mo).
- **Règle** — mesurer le pic par étape avant de choisir la piste ; rendre la grosse table dès que ses colonnes utiles sont extraites (`Fields::release_main`). Preuve d'identité : empreinte du `FlightLog` complet de chaque fichier du corpus, avant et après (528 fichiers identiques) ; `corpus validate` et `otb-parity` ne passent pas par `load` et ne la donnent pas. Mesure : 481 → 342 Mo, durée inchangée.
- **Suite (refonte)** — ranger les colonnes en `i32` dès le décodeur (`CompactTable`) : 274 Mo, 0,54 s. Piège rencontré : le décodeur note la position des trames lentes, GPS et événements avec le nombre de trames principales ; avec une table principale vide ce nombre vaut 0 et les drapeaux de mode de vol étaient faux (528 empreintes différentes, aucun test rouge). Toute valeur dérivée du nombre de trames doit passer par `main_rows`.

### L-080 : une allocation ratée n'est pas une panique : `catch_unwind` ne la rattrape pas, le processus est abandonné
- **Symptôme** — 2026-11-04 (audit du lot 4) : un en-tête qui déclare 20 000 champs fait demander 67 Go au décodeur ; Rust écrit « memory allocation of … bytes failed » et abandonne le programme entier, sans rien dire à l'interface (qui décode pourtant dans un fil).
- **Cause (mesurée)** — `Vec::extend_from_slice` et `push` appellent le gestionnaire d'erreur d'allocation, qui abandonne ; `std::panic::catch_unwind` ne voit que les paniques. Réserver d'avance « au mieux » (`try_reserve` dans `Table::new`) ne suffisait pas : la croissance ligne après ligne abandonnait quand même.
- **Règle** — tout tampon dont la taille vient d'un fichier se fait grandir avec `try_reserve` avant chaque ajout (erreur rendue, jamais d'abandon) ; ne jamais compter sur `catch_unwind` pour la mémoire. Les nombres annoncés par l'en-tête sont bornés avant de s'en servir (D-044).
- **Preuve** — `rust/otb-decoder/src/lib.rs` : `push_rend_une_erreur_quand_la_memoire_manque` (sans `try_reserve`, le test panique) ; `rust/otb-decoder/tests/robustness.rs`.

### L-079 : une doc qui cite un fichier produit par la compilation passe chez soi et échoue sur la CI
- **Symptôme** — 2026-11-03 (CI de la PR #93, jobs `tests` et `windows`) : `otb-dev/tests/docs.rs` panique, `chemins ou symboles introuvables : ["README.md : rust/target/release/opentunebox-gui"]`, alors que le même test passait en local juste avant le push.
- **Cause (mesurée)** — le README citait en code (`…`) la sortie de `cargo build`. Ce fichier existe sur le poste de développement (on y a compilé), pas dans un dépôt fraîchement cloné : le test de chemins exige que tout chemin cité qui commence par un dossier du dépôt (`rust/`, `packaging/`…) existe. Sur une copie propre, le même test échoue comme la CI (essayé).
- **Règle** — un fichier de compilation (`target/…`, `dist/…`) se cite sans le préfixe du dépôt (« le fichier `target/release/…` du dossier `rust` ») ou dans un bloc de code ; ne pas assouplir le test. **Avant de pousser un changement de documentation, lancer les deux contrôles sur une copie propre des fichiers suivis par git** : `git ls-files -z | xargs -0 -I{} cp --parents {} /tmp/copie/`, puis `cd /tmp/copie/rust && OTB_DOCS_ROOT=/tmp/copie cargo test -p otb-dev --test docs` et `cd /tmp/copie && python -m pytest tests/test_docs.py`.
- **Preuve** — `otb-dev/tests/docs.rs::code_paths_and_symbols_exist` : échoue sur la copie propre avec l'ancienne phrase du README, passe avec la nouvelle.

### L-078 : un test écrit avec `std::os::unix` ne compile pas sous Windows, et la CI Windows est le dernier endroit où on le voit
- **Symptôme** — 2026-11-03 (CI de la PR #91) : job `windows` en échec après 8 min, `error[E0433]: cannot find 'unix' in 'os'` et `error[E0599]: no associated function … 'from_mode'`, alors que tout passait sous Ubuntu. Un test échoue donc à la compilation, avant de tourner, et la même erreur coupe tous les tests du crate.
- **Cause** — mesurée : le test du délai de `corpus validate` fabriquait un faux `blackbox_decode` (script shell) et lui donnait les droits d'exécution par `std::os::unix::fs::PermissionsExt`, module qui n'existe pas sous Windows. Un second défaut du même commit n'est apparu qu'à la relecture : le rapport écrivait les chemins avec le séparateur du système, le test attendait des `/`. Même famille que L-074 (un test « portable » qui ne l'est pas), mais l'erreur est ici à la compilation.
- **Règle** — tout code ou test qui touche `std::os::unix`, un script shell, des droits de fichier ou un séparateur de chemin porte `#[cfg(unix)]` (le test) ou n'écrit que des `/` (la sortie). Avant de pousser : `cargo check --target x86_64-pc-windows-gnu` (voir la compétence `opentunebox-dev`, « Contrôler la compilation Windows »). Ce contrôle ne prouve pas que les tests réussissent : seule la CI Windows les exécute.
- **Preuve** — `rust/otb-dev/src/corpus/validate.rs::blocked_reference_is_killed_and_reported_as_refhang` (`#[cfg(unix)]`) ; sans l'attribut, `cargo check --lib --profile test --target x86_64-pc-windows-gnu` reproduit les deux erreurs de la CI ; avec, il passe, et la CI Windows de la PR #91 est verte.

### L-077 : `Harness::run()` d'egui_kittest panique si un fil de calcul redemande une repeinte pendant qu'il tourne
- **Symptôme** — 2026-11-03 (CI de la PR #90) : `un_fichier_illisible_n_empeche_pas_les_suivants` passait en local et sous Ubuntu, mais échouait sous Windows : `Harness::run exceeded max_steps (4). It would have settled after 6 steps`.
- **Cause** — le fil de calcul (`worker.rs`) appelle `ctx.request_repaint()` à la fin de chaque ouverture. Sur le poste lent de la CI, ces demandes arrivent pendant `run()`, qui exige un repos en 4 pas au plus.
- **Règle** — un test qui attend un calcul en arrière-plan avance au pas (`step()` dans la boucle d'attente, `run_steps(n)` ensuite), jamais par `run()`.
- **Preuve** — `rust/otb-gui/tests/app.rs::un_fichier_illisible_n_empeche_pas_les_suivants` (non reproduit en local ; la vérification sous Windows se fait par la CI).

### L-076 : l'icône donnée à la fenêtre ne s'affiche pas sous Linux Wayland : il faut un `.desktop` nommé comme l'`app_id`
- **Symptôme** — 2026-11-02 (6-6b), revu par Cédric le 2026-11-03 sur l'AppImage et sur l'exécutable de la CI : sous KDE Plasma (Wayland), la barre de titre montre un rond orange avec un « W » blanc (ressemble au logo de Wayland ; aucun fichier d'icône correspondant retrouvé dans le thème, donc **non confirmé**) au lieu de l'icône d'OpenTuneBox, alors que `ViewportBuilder::with_icon` était réglé.
- **Cause (vérifiée)** — (1) dans winit 0.30.13, `set_window_icon` est **vide** sous Wayland (`platform_impl/linux/wayland/window/mod.rs`) : l'icône du programme n'est jamais envoyée. Sous Wayland, le bureau cherche l'icône d'après l'`app_id` de la fenêtre dans un fichier `.desktop` installé. (2) **Le nom du fichier compte, majuscules comprises** : essayé le 2026-11-03 sur KDE Plasma Wayland, programme installé dans `~/.local/bin` avec l'icône dans `hicolor` : `opentunebox.desktop` (avec `StartupWMClass=OpenTuneBox`) **ne change rien** (icône générique) ; `OpenTuneBox.desktop` (= `app_id`) donne la bonne icône. Une règle écrite avant cet essai (« fournir l'icône par le fichier `.desktop` ») était donc insuffisante.
- **Règle** — le fichier `.desktop` s'appelle exactement comme l'`app_id` de la fenêtre (`OpenTuneBox.desktop`) : `packaging/linux/install_linux.sh` l'écrit sous ce nom (et supprime l'ancien `opentunebox.desktop`) et `build_appimage.sh` aussi ; garder `StartupWMClass=OpenTuneBox` pour X11. Une AppImage lancée telle quelle (non intégrée au bureau par AppImageLauncher ou équivalent) n'a pas de `.desktop` connu : elle garde l'icône générique sous Wayland, on n'y peut rien dans le programme. `with_icon` ne sert que sous X11 et Windows.
- **Non élucidé** — sous X11 forcé (`WINIT_UNIX_BACKEND=x11`, `WAYLAND_DISPLAY` retiré), la propriété `_NET_WM_ICON` de la fenêtre existe mais est **vide** (`xprop`), alors que le PNG se décode (256 × 256 RGBA) et que winit sait l'écrire : cause non trouvée (voir W-045).
- **Preuve** — source de winit citée ci-dessus ; captures de la barre de titre le 2026-11-03 (sans `.desktop` : icône générique ; `opentunebox.desktop` : générique ; `OpenTuneBox.desktop` : icône d'OpenTuneBox) ; `rust/otb-gui/tests/contracts.rs::le_fichier_desktop_porte_le_nom_de_l_app_id` (échouait avec le nom en minuscules).

### L-075 : un test qui peut ouvrir une vraie fenêtre du système l'ouvre, et elle ne se ferme plus
- **Symptôme** — 2026-11-02 (6-6a) : pendant une série de mutations, des dizaines de fenêtres « Exporter l'image » se sont ouvertes sur le bureau de Cédric, sans moyen de les fermer ; les tests restaient bloqués.
- **Cause** — la mutation « export de page inversé » faisait demander une capture à chaque image ; les bancs de test construits par `App::new(...)` qui n'avaient pas remplacé la boîte d'enregistrement ouvraient alors la **vraie** boîte du système (`rfd`, par le portail KDE), qui attend un humain pendant que le test attend la boîte. Un défaut du code, ou une mutation, suffit à transformer un test en ouverture de fenêtres.
- **Règle** — tout ce qui ouvre une fenêtre du système (boîte de fichiers, message…) passe par un champ remplaçable de l'application, et le constructeur des tests (`App::new`) le met d'office en « annulé » : seul le lancement réel (`App::with_guide`) garde la vraie boîte. Un script de mutations se lance **sans écran ni bus de session** (`DISPLAY`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS` retirés), avec un délai qui tue tout le groupe de processus ; on ne tue jamais un test par son motif (`pkill -f`, L-053) mais par son numéro de processus.
- **Complément** — le rendu par défaut d'`egui_kittest` est un vrai rendu wgpu : sans carte graphique (la CI), une capture demandée par l'application n'est jamais honorée. Les tests d'export fournissent leur propre moteur de rendu (`TestRenderer` qui rend un dégradé dont chaque pixel dit où il est : ce qui prouve aussi la position du recadrage). Le `Ui` racine d'egui laisse 8 points de marge de chaque côté : le rectangle d'un panneau n'est pas celui de la fenêtre.
- **Preuve** — `rust/otb-gui/tests/app.rs::app_new_n_ouvre_jamais_de_boite_d_enregistrement_du_systeme` et les tests d'export de ce fichier ; mutation « export de page inversé » : 27 tests échouent, aucune fenêtre ne s'ouvre.

### L-074 : un test qui simule le système A ne doit pas dépendre des règles de chemins du système qui l'exécute
- **Symptôme** — 2026-11-02 (CI de la PR #81) : `le_chemin_est_celui_de_qt_sous_linux_et_sur_ce_poste` passait sous Ubuntu et échouait sous Windows : `XDG_DATA_HOME=/data` était ignoré, le chemin retombait sur `~/.local/share`.
- **Cause** — `cards_path_in("linux", …)` jugeait le chemin « absolu » par `Path::is_absolute`, qui suit les règles **de la machine qui compile** : sous Windows, `/data` n'est pas absolu (il faut une lettre de lecteur). Le test simulait Linux, mais la fonction pensait en Windows.
- **Règle** — une fonction paramétrée par un système (`os`) applique les règles de **ce** système (ici : « commence par `/` »), jamais celles de l'hôte ; ses tests de chaque branche tournent sur les trois systèmes de la CI (Ubuntu et Windows existent, macOS par `construire.yml`). Lire la sortie de la CI Windows avant de croire un test « portable ».
- **Preuve** — `rust/otb-gui/tests/guide.rs::le_chemin_est_celui_de_qt_sous_linux_et_sur_ce_poste` (échouait sous Windows, vert après correction ; la vérification sous Windows se fait par la CI).

### L-073 : un panneau egui dont le contenu est petit au premier cadre garde cette petite taille
- **Symptôme** — 2026-11-02 (6-5) : sur la capture du Guide, le panneau des logs du drone ne montrait que 125 px au lieu de 250 : sa dernière ligne était coupée. Le même panneau, dans un banc de test qui commençait avec une fiche, faisait bien 250 px. Aucun des 30 tests ne le voyait.
- **Cause** — au premier cadre l'application n'avait aucune fiche : le panneau ne contenait qu'une phrase, donc `Panel::bottom(..).default_size(250.0)` s'est réduit à ce contenu (puis au minimum de 120 px). egui enregistre la taille réelle du panneau : quand la fiche est arrivée, le panneau est resté petit. Un `ScrollArea` à `auto_shrink([false, false])` le remplit, mais seulement dans la branche où il est dessiné.
- **Règle** — le contenu d'un panneau redimensionnable est **toujours** dans un `ScrollArea` qui remplit (`auto_shrink([false, false])`), y compris dans les branches « rien à montrer » ; un test commence l'écran **dans l'état vide** puis ajoute le contenu et mesure la position d'un titre. Regarder une capture, pas seulement les tests : seul le rendu a révélé le défaut.
- **Preuve** — `rust/otb-gui/tests/guide.rs::le_panneau_des_logs_garde_sa_hauteur_meme_s_il_commence_sans_fiche` (échouait : titre à 645 au lieu de 515).

### L-072 : l'adresse mémoire d'un log ne l'identifie pas : elle peut resservir à un autre log
- **Symptôme** — 2026-11-02 (relecture de la 6-4) : le Simulateur de filtres reconnaissait « son » log par `Arc::as_ptr` ; fermer le log A puis ouvrir B pouvait redonner la même adresse, donc ni l'éditeur ni les réglages appliqués n'étaient rechargés, et un « et si » était calculé avec les réglages de A contre la base de B, sans aucun message.
- **Cause** — une fois l'`Arc` libéré, l'allocateur peut redonner le même bloc à une allocation de même taille (vérifié par un petit programme : même pointeur) ; la probabilité dépend des allocations du décodage. Python protège par `(label, id(log))` avec le log gardé vivant.
- **Règle** — identifier un chargement par `LoadedLog::uid` (compteur de `AppState`, jamais réutilisé, copié dans `LogInput::uid`), jamais par une adresse. Même chose pour un résultat dessiné : garder l'`Arc` lui-même et comparer par `Arc::ptr_eq` (il reste vivant, son adresse ne peut pas resservir).
- **Preuve** — `rust/otb-gui/tests/filter_tools.rs::changer_de_log_recharge_l_editeur_et_oublie_les_reglages_appliques` (échoue avec un identifiant constant ; le test passe par « Courbes » pour que la page de l'outil ne se remette pas à zéro seule) et `un_identifiant_de_chargement_n_est_jamais_reutilise` (échoue avec un compteur figé).

### L-071 : un test de clé de cache peut passer avec la mauvaise clé quand une autre protection fait le travail
- **Symptôme** — 2026-11-02 (6-4a) : le test de l'Assistant disait « pas repris après une autre période » ; en retirant la période de la clé de cache (mutation), les 6 tests restaient verts.
- **Cause** — le test changeait la période par `AppState::set_period`, qui **vide déjà le cache** (D-037) : la clé n'était jamais consultée avec une période différente. Deux protections superposées, un seul témoin.
- **Règle** — pour prouver qu'une protection tient seule, neutraliser l'autre dans le test (ici : écrire `st.logs[0].period` à la main, sans passer par `set_period`) et **casser exprès la protection visée** pour voir le test échouer (L-034). Même réflexe pour toute règle d'affichage sans effet testable à l'écran : en tirer une fonction pure (`delay_trend`, `spectrum_top`, `y_bounds` du Simulateur) ; cinq mutations survivaient avant.
- **Preuve** — `rust/otb-gui/tests/filter_tools.rs::le_cache_sert_au_second_calcul_mais_ni_apres_un_autre_profil_ni_une_autre_periode` (échoue avec la clé sans la période) ; trois tests de règles d'affichage du Simulateur, cinq mutations tuées.

### L-070 : une liste déroulante d'egui est coupée à 200 px : les derniers choix sont hors de la zone cliquable
- **Symptôme** — 2026-11-02 (6-3) : le test qui parcourt les 11 métriques de la Réponse indicielle échouait à la neuvième (« Marge de phase ») : le clic sur le choix ne changeait rien, la colonne gardait l'ancienne valeur, sans aucune erreur.
- **Cause** — `egui::ComboBox` limite la hauteur de sa liste à 200 px par défaut et la rend défilante ; les choix du bas existent dans l'arbre d'accessibilité mais sont rognés par le cadre de défilement, le clic tombe à côté. Un utilisateur aurait dû faire défiler une liste de 11 choix que Qt affiche en entier.
- **Règle** — une liste de plus de 8 choix reçoit `.height(…)` assez grand pour tout montrer (`.height(320.0)` pour 11 choix) ; un test qui parcourt les choix d'une liste vérifie que la valeur affichée a changé après le clic, pas seulement que le choix existe.
- **Preuve** — `rust/otb-gui/tests/step_tools.rs::chaque_colonne_et_chaque_option_d_affichage_se_dessine` (échouait avant `.height(320.0)`).

### L-069 : chaque fichier de test de `otb-gui` lie egui et wgpu (≈ 490 Mo en debug) : la CI Ubuntu manque de disque
- **Symptôme** — 2026-11-01 (PR #78, 6-2) : le travail `tests` échoue après 7 min : `No space left on device (os error 28)` puis `ld terminated with signal 7 [Bus error]` (éditeur de liens tué) sur `otb-cli` et `otb-parity` ; Windows et l'audit passent. La PR #77 (11 binaires de test d'`otb-gui`) passait.
- **Cause** — chaque fichier de `rust/otb-gui/tests/` est un exécutable séparé qui lie egui, wgpu et leurs infos de débogage : 470 à 510 Mo chacun (mesuré). La 6-2 en a ajouté 6 et un exemple : 17 binaires ≈ 8 Go, plus les artefacts de clippy et des autres crates, au-delà des ≈ 14 Go libres d'un runner Ubuntu. Aucun test n'était en cause, rien à corriger dans le code.
- **Règle** — `[profile.dev] debug = "line-tables-only"` dans `rust/Cargo.toml` : 186 Mo par binaire au lieu de 490, 6,7 Go pour toute la séquence de la CI (clippy puis tests, dossier neuf), les messages de panique gardent leurs numéros de ligne. Avant d'ajouter des fichiers de test à `otb-gui`, regrouper plutôt que multiplier si l'espace redevient juste ; en cas de nouvel échec « No space left », mesurer `du -sh rust/target` d'une séquence neuve avant de toucher au code.
- **Preuve** — séquence `cargo clippy --all-targets -- -D warnings` puis `cargo test -p …` rejouée dans un dossier de compilation neuf : 6,7 Go, 0 échec ; la CI de la PR #78.

### L-068 : le contrôle d'accessibilité de kittest ne tourne qu'à la fin de `run()` ; et les polices d'egui n'ont pas « → »
- **Symptôme** — 2026-11-01 (6-2) : les tests de l'outil Fréquence × temps échouaient (« 7 widget(s) lack accessible name ») alors que ceux du Spectre, qui ont les mêmes listes déroulantes, passaient. Autre cas : le rapport du diagnostic du bruit faisait paniquer le banc (`No glyph for "→"`), alors que l'application aurait dessiné un carré.
- **Cause** — kittest vérifie que chaque champ a un nom accessible **seulement quand l'interface s'est stabilisée** (`run`, `run_ok`), pas dans `run_steps` ni `step` : les tests du Spectre n'appelaient jamais `run()` une fois la page affichée. Un `ComboBox` ou un `DragValue` sans libellé voisin n'a ni nom pour un lecteur d'écran, ni moyen d'être retrouvé par `get_by_label` (on les retrouve par `get_by_value`). Côté polices : « → ✔ ▲ ↑ ↓ ⚠ » sont absents des polices embarquées (mesure du 2026-11-01), tous les autres caractères des textes de l'assistant y sont ; le banc les refuse (`MissingGlyphPolicy::Panic`), l'application dessine un carré (`Tofu`).
- **Règle** — chaque liste déroulante et chaque champ reçoit `.accessible_name("…")` (ou un libellé lié par `labelled_by`) ; un test d'interface appelle `h.run()` au moins une fois page affichée. Un texte de l'assistant passe par `tools::common::safe_text` (« → » devient « -> ») avant d'être affiché ; les « ✔ » et « ▲ » du guide (`tuning_path.rs`) seront remplacés par des icônes en 6-5. **Complément 2026-11-02 (6-3)** : sondés un à un, « ● ○ ◇ ■ □ △ ▼ ◆ ◯ ✓ ↔ » sont absents aussi (présents : « ≥ ≤ ° · ± × « » – — … • ≈ ÷ Δ Ω µ ») ; une légende de point plein ou creux se dessine (`step_common::dot`), les autres signes passent par des mots. **Complément 2026-11-02 (6-5)** : la police Hack, livrée avec egui mais réservée au texte à chasse fixe, a « → ▲ ● ○ ↑ ↓ ← ◇ ■ » (`fc-query`) : `app::setup` la place en dernier recours du texte normal (D-039) ; « ✔ ✖ ℹ ⚠ » (polices d'emoji, non activées) deviennent des icônes Lucide.
- **Preuve** — `rust/otb-gui/tests/freq_time.rs` (un `run()` après affichage), `tests/report_ui.rs::la_fleche_est_remplacee_car_les_polices_d_egui_ne_l_ont_pas`.

### L-067 : egui_plot ne graduait pas les axes (grille en puissances de 10) ; un `Span` posé avant une image la recouvre
- **Symptôme** — 2026-11-01 (6-2) : les cartes de Fréquence × gaz n'avaient pour tout repère qu'un « 0 » dans le coin, et les spectres de l'Analyse spectrale aucune graduation sur l'axe des fréquences (0 à 1000 Hz). Le fond gris des zones sans données, posé par un `Span` avant l'image, la cachait entièrement.
- **Cause** — la grille par défaut d'egui_plot place des repères aux puissances de 10 et n'étiquette un repère que si deux repères du même rang sont écartés d'au moins 60 px (axe X) ou 20 px (axe Y) : sur 0 à 100 il ne reste que 0 et 100. Les bornes étaient pourtant bonnes (affichées par `plot_bounds`). Un `Span` ou une `Line` dessinés après (ou avant, selon l'ordre) l'image la recouvrent : l'ordre des éléments décide.
- **Règle** — tout tracé linéaire passe par `plots::nice_axes` (repères en 1, 2, 5 × 10^k : fin tous les 8 px, étiqueté tous les 64 px sur X et 24 px sur Y). Le gris des cases vides est mis **dans la texture** (`heatmap_pixels(…, empty)`), pas par un élément posé dessous. Regarder la capture avant de croire un tracé fini.
- **Preuve** — `tests/plots.rs::la_grille_lineaire_donne_des_repetes_ronds_et_etiquetables` ; captures des deux thèmes (`examples/capture.rs`).

### L-066 : un banc egui_kittest construit un premier cadre avant le code du test ; les icônes SVG y affichent « ⚠ » et la police panique
- **Symptôme** — 2026-11-01 (6-0) : `Harness::build_ui_state` suivi de `egui_extras::install_image_loaders(&h.ctx)` : panique `No glyph for "⚠️" (U+26A0) in Proportional` au premier cadre dessinant la barre d'outils.
- **Cause** — le banc dessine déjà un cadre en se construisant, avant toute ligne du test : les chargeurs d'images n'étaient pas encore installés, l'icône SVG échouait au chargement et egui la remplace par « ⚠️ », absent de ses polices embarquées. `main.rs` installait les chargeurs, pas le banc.
- **Règle** — tout réglage du contexte (chargeurs d'images, polices, thème) passe par `Harness::builder().with_context_setup(otb_gui::app::setup)` ; une seule fonction `app::setup` sert à `main` et aux tests. Un « ⚠ » dans un rendu d'egui = une image qui n'a pas pu se charger, pas un problème de police.
- **Précision** — la panique vient de la règle `MissingGlyphPolicy::Panic` de kittest ; dans l'application, la règle par défaut est `Tofu` (carré à la place de l'icône, sans plantage).
- **Preuve** — `rust/otb-gui/tests/contracts.rs::la_fenetre_se_dessine_dans_les_deux_themes_sans_log` (échoue sans `with_context_setup`).

### L-065 : une couche de contrôle du corpus qui écarte les sessions courtes ne voit pas un plantage sur une session courte
- **Symptôme** — 2026-11-01 : `opentunebox report` plantait (`ValueError: a cannot be empty`) sur 2 logs du dossier `cheussure`, et aucune comparaison de l'assistant sur le corpus ne l'avait vu : 631 sessions comparées, 0 écart, le Rust « identique » au Python qui plantait.
- **Cause** — le plantage venait des sessions de 0,1 et 0,2 s (restes d'enregistrement après un rebranchement de la batterie, aucune fréquence entre 80 Hz et la limite des filtres), et `advisor_rates.py` comme `golden_advisor.py` écartent les sessions de moins de 8 s. Seule la couche de la ligne de commande (`record-cli`, tous les fichiers, toutes les sessions) les rejouait. Le test synthétique écrit d'abord passait **avant** le correctif : la session fabriquée n'atteignait pas `predict_filter_change`.
- **Règle** — une couche qui filtre les sessions (durée, période, présence d'un signal) ne prouve rien sur celles qu'elle écarte : le dire dans sa documentation et garder une couche sans filtre (la ligne de commande) ; un test de non-régression d'un plantage doit être vu **échouer** sur le code d'avant, et viser la fonction fautive plutôt que passer par `analyze`.
- **Preuve** — `tests/test_advisor_synthetic.py::test_predict_filter_change_very_short_session` ; cas `bande vide` de `scripts/rules_delay_cases.py` ; `otb-parity check-cli` (2 fichiers qui donnaient `@@ERREUR`).

### L-064 : réduire un fichier de cas en changeant les données écrites sans changer celles que Python a utilisées donne un « identique » qui ne prouve rien
- **Symptôme** — 2026-10-31 (5b) : en mettant à zéro des colonnes du journal pour alléger `log_check.json`, la comparaison des réponses de Python « avant / après » rendait « tout identique », y compris en zérotant `gyro_unfilt`, dont dépendent pourtant les retards.
- **Cause** — la mise à zéro n'était appliquée qu'à la copie **écrite** dans le fichier ; Python calculait ses réponses sur le journal d'origine. Les deux fichiers contenaient donc les mêmes réponses, et Rust aurait été jugé sur des données qui ne correspondaient plus.
- **Règle** — transformer le journal **avant** de calculer la réponse de Python, puis comparer les réponses à celles de l'ancien fichier : identiques = colonne inutile, différentes = colonne lue. Mesuré ainsi : les gaz (`throttle`), `d_term` et `gyro_unfilt` sont lus ; termes P, I, F, commande radio, debug et moteurs ne le sont pas (pour ces cas). Puis relancer toutes les mutations : raccourcir des journaux a fait perdre une branche (`min` des fenêtres roll / pitch), rattrapée par un cas dirigé.
- **Preuve** — `scripts/log_check_cases.py` (`lite`, `UNREAD`).

### L-063 : une batterie de mutations qui remplace du texte exact se périme au premier `cargo fmt` et classe les mutations « ratées » comme survivantes
- **Symptôme** — 5b-2 (2026-10-31) : le script de mutations rendait 21 « survivantes » dont trois qui ne l'étaient pas : le motif cherché (`== LOG_RATE_DPS) && mx`) n'existait plus dans le fichier, que `cargo fmt` avait réécrit sur plusieurs lignes ; le script le notait « ne compile pas » au lieu de « motif absent ».
- **Cause** — le classement lisait `error:` dans la sortie de `cargo test`, qui s'écrit aussi après un test en échec (`error: test failed`) ; et un motif écrit avant le formatage ne correspond plus après.
- **Règle** — lancer `cargo fmt` avant d'écrire les motifs ; un motif absent est une erreur du script (arrêt), jamais une mutation comptée ; une mutation n'est « tuée » que si la sortie contient `FAILED` ; vérifier à la main (une commande) chaque survivante et chaque « ne compile pas » avant de conclure. Une comparaison `>=` contre `>` ne se tranche que par une valeur exactement sur le seuil : la fabriquer (fréquence `0,95 × base` pile, durée 15,0 s pile, N = 20 ou 200 fenêtres par une fréquence non entière, `x * 1,05 == y` cherché par `nextafter`).
- **Complément 2026-11-02 (6-5)** — le même piège touche les scripts qui **modifient un test** par `str.replace` : après un `cargo fmt`, le motif n'existe plus et le remplacement ne fait rien, sans erreur (une assertion ajoutée n'était pas là, la mutation correspondante « survivait » à tort). Toujours `assert ancien in texte` avant de remplacer, et relire le fichier (`grep`) après.
- **Preuve** — `scripts/log_check_cases.py` (section « valeurs exactement sur un seuil ») ; 66 mutations de `log_check.rs`, 65 tuées, 1 équivalente (`>` contre `>=` entre deux valeurs égales).

### L-062 : une couche de contrôle qui ne part que de ce que les références déclarent ne voit pas une référence manquante
- **Symptôme** — relecture de la 5a-5 (2026-10-31) : avec un seul `.npz` sur un `flightlog.json` filtré, ou avec `advisor_tuning/` vide, `otb-parity check-advisor` rendait 0 échec et le code de sortie 0.
- **Cause** — `tuning_layer.rs` calculait les vols des seuls fichiers que les références déclaraient (`tuning|files`), puis cherchait parmi eux les sessions « analysées mais absentes d'une référence » : l'ensemble cherché était vide par construction, et seul le dossier du premier niveau était testé.
- **Règle** — un contrôle de complétude part de la **liste attendue** (ici `flightlog.json`), jamais de ce que les fichiers à contrôler annoncent. Pour tout nouveau contrôle : le casser exprès (supprimer une référence, vider le dossier) et voir l'échec. Les commentaires « chaque référence est rejouée » se vérifient de la même façon.
- **Preuve** — `rust/otb-parity/src/tuning_layer.rs:check_tuning` (tous les fichiers de `flightlog.json`, tous leurs dossiers, échec si aucune référence) ; essai à la main : `Davy.npz` seul → « PiloteC/btfl_009.bbl : session analysée absente de la référence PiloteC.npz », dossier vide → « aucune référence ».

### L-061 : `np.convolve(x, np.ones(K) / K, "same")` : pour K = 15, somme croissante avec FMA ; pour K = 5, centre sans FMA et bords avec FMA ; seule la comparaison au bit près le montre
- **Symptôme** — 2026-10-31 (5a-3c, `frame_resonance`) : les trois autres façons de sommer le lissage par 15 valeurs (croissant sans FMA, décroissant avec ou sans FMA) diffèrent de numpy
  sur 1 772 à 2 222 valeurs sur 3 000 (dernier bit seulement).
- **Cause** — `np.convolve` passe par le produit scalaire de la bibliothèque BLAS de numpy ; sur ce poste la boucle de queue (moins de 16 valeurs) fusionne le produit et la somme
  (`acc = fma(x[j], 1/15, acc)`) en partant de la fenêtre la plus ancienne. Python 3.14 : `math.fma`. **Pour 5 valeurs** (relecture, 2026-10-31, mesuré sur 10 000 valeurs
  aléatoires puis sur 52 240 valeurs de longueurs 5 à 1 001) : le **centre** se somme dans l'ordre croissant **sans** FMA (`acc + x × 0,2`), les **deux premières et les deux dernières**
  valeurs (fenêtre tronquée à 3 ou 4 termes) **avec** FMA, toujours dans l'ordre croissant : 0 écart. « Croissant avec FMA » partout donne 4 181 écarts sur 10 000, « décroissant sans
  FMA » (le premier essai de Rust) 4 738. Pourquoi les bords seuls fusionnent n'est pas élucidé (non vérifié : chemin de calcul différent de la bibliothèque pour les fenêtres tronquées).
  Moins de 5 valeurs (numpy échange alors les tableaux) : aucune des quatre variantes ne rend les bits de numpy ; seule la tolérance est vérifiée.
- **Règle** — pour une convolution courte, mesurer les quatre variantes (ordre croissant ou décroissant, avec ou sans FMA) sur des signaux aléatoires avant de choisir, **séparément pour
  le centre et pour les bords** ; pour 15 valeurs seule « croissant + FMA » donne 0 écart sur 3 000 valeurs (`f64::mul_add` en Rust). Dépend du poste (BLAS) : à relire si la tolérance des
  mesures de raies (`FILTER_TOL`) est un jour trop juste ; la CI (Windows en particulier) peut différer d'un bit : le test compare à 1e-15 de la fenêtre et n'exige le bit près que
  si `OTB_BITEXACT=1` (L-059).
- **Preuve** — `rust/otb-advisor/src/rules.rs:smooth15` et `smooth5`, `rust/otb-advisor/tests/smooth.rs` (sorties de numpy dans `rust/otb-advisor/tests/data/smooth.json`, écrites par `scripts/smooth_cases.py` ;
  `OTB_BITEXACT=1 cargo test -p otb-advisor --test smooth` : 32 328 et 33 376 valeurs identiques au bit près) ; corpus : 630 sessions, écart maximal 2,3e-13 sur les mesures de filtres. (Avec un lissage « simple », l'écart serait de l'ordre de 1e-16 :
  invisible à la tolérance du corpus. L'ordre exact n'est donc pas prouvé par le corpus, seulement par la mesure ci-dessus.)
- **Complément (2026-10-31, PR de la 5a)** — deux autres formes du même piège, vues par la CI Windows : (1) un test d'`analyze` qui comparait au bit près les mesures de réponse indicielle (`steps`, 4 ulp d'écart) ; (2) un cas dont le **seuil était posé à un ulp** de la mesure (`down(gap)`) : l'écart venant d'une FFT, la décision bascule d'une plate-forme à l'autre. Règle : tout seuil de cas écrit qui touche une mesure issue d'une FFT se pose à ±1e-9 de la mesure, jamais à l'ulp ; `cargo test --no-fail-fast` dans le job Windows pour voir tous les échecs d'un coup.

### L-060 : `\w`, `\d` et `\s` de Python ne sont pas `is_alphanumeric`, `is_numeric` ni `is_whitespace` de Rust
- **Symptôme** — 2026-10-30 (5a-1, `profiles.py`) : le premier essai de `\w` par `c == '_' || c.is_alphanumeric()` diffère de Python pour **6 167 caractères** sur 1,1 M de points
  de code (premier : U+0345, un signe combinant) ; `\s` diffère par U+001C à U+001F (`str.isspace` les compte, pas `White_Space`).
- **Cause** — Rust prend la propriété Unicode `Alphabetic` (qui contient des signes combinants) et `Numeric` (qui contient `No` et `Nl`) ; Python prend les catégories `L*` et `Nd`
  (`isalnum`), et sa version d'Unicode (16.0 pour Python 3.14) n'est pas celle de Rust.
- **Règle** — une classe de caractères d'expression régulière se reproduit par **table générée depuis Python** (`scripts/profiles_cases.py TABLES.rs` → `src/pyclasses.rs`) et se
  vérifie sur tous les points de code ; jamais par une fonction de la bibliothèque standard « qui a l'air pareille ». `\d` sert aussi à `float()` : un chiffre Unicode se lit.
- **Preuve** — `rust/otb-advisor/tests/profiles.rs` (`classes_de_caracteres_de_python` : `\d`, `\s`, `\w` sur les 1 114 112 points ; `minuscules_comme_python`).

### L-059 : « identique au bit près » après une FFT n'est vrai que sur le poste qui a écrit la référence ; la CI Windows est la seule preuve
- **Symptôme** — 2026-10-30 (PR 4c-B) : la CI Windows échoue sur `step_response "a : termes PID, fenêtres de 2 s" : réponses : élément 0` : Rust −4,654068096933931e-4,
  Python −4,6540680969330825e-4 (écart relatif 1,8e-12) ; Linux : 0 écart.
- **Cause** — le noyau de déconvolution (`realfft`) est le même des deux côtés, donc identique **sur ce poste** ; les références ont été écrites sous Linux, et sous Windows la
  FFT (jeux d'instructions, `sin`/`cos` des facteurs) rend un dernier bit différent. J'avais écrit `assert_bits` sur les réponses, la moyenne, l'écart-type et les métriques
  en comptant sur L-049 (même noyau des deux côtés), sans me rappeler L-048 : une FFT n'est jamais identique d'une plateforme à l'autre.
- **Règle** — tout ce qui sort d'une FFT se compare à une tolérance dans les tests que la CI exécute (réponses, moyenne, écart-type : 1e-9 ; métriques qui en dérivent : 1e-6) ;
  « au bit près » reste réservé à l'arithmétique pure (somme, `gradient`, métriques de **réponses données**). `cargo test` s'arrête au premier binaire en échec : un échec
  Windows cache les suivants, donc relire toute la sortie de la CI après chaque correction.
- **Preuve** — `rust/otb-analysis/tests/step.rs` (`RESPONSE_RTOL`, `METRICS_RTOL`). La couche corpus (`otb-parity check-analysis`, Linux seulement) garde ses comparaisons au bit près : la CI n'a pas le corpus.

### L-058 : la disposition mémoire décide de l'ordre des sommes de numpy : masque sur le dernier axe et matrice d'une colonne
- **Symptôme** — 2026-10-30 (4c-B(d), `shape_metrics`) : `waves_ratio` diffère d'un dernier bit entre Rust et Python dès 39 réponses (9 blocs de jackknife), pas
  avant ; puis la moyenne d'une réponse d'**un seul point** (`resp_ms = 0`) diffère aussi.
- **Cause** — mesurée (`flags` du tableau) : `x[..., w]` avec un masque booléen rend un tableau rangé « Fortran » : `mean(axis=0)` et `sum(axis=0)` somment alors
  chaque colonne **par paires** (identique à la somme dans l'ordre jusqu'à 7 valeurs, différent à partir de 8). Une matrice `(k, 1)` est aussi « Fortran » :
  sa somme sur l'axe 0 est par paires. Une matrice C ordinaire à plus d'une colonne somme ligne après ligne (L-052).
- **Règle** — avant de reproduire une réduction sur l'axe 0, afficher `a.flags` ; un cas écrit doit dépasser 8 valeurs sommées (ici 8 blocs ou plus) et
  couvrir la matrice d'une colonne.
- **Preuve** — `rust/otb-analysis/src/step.rs` (`sum_rows`, colonnes de `shape_metrics`), cas `shape_metrics 39 réponses` et `réponse 0 ms` de `scripts/step_cases.py`.

### L-057 : des cas aléatoires qui n'atteignent jamais le régime « valide » laissent les seuils libres ; les mutations le montrent
- **Symptôme** — 2026-10-30 (4c-B(b), `estimate`) : les mutations du seuil `f ≤ 0,4 × fs`, de `isfinite(L)` et de la fin de `first_run` survivent, alors que les
  spectres tirés au hasard devaient les atteindre.
- **Cause** — mesurée : le masque `valid` des spectres aléatoires était presque toujours vide (0 à 1 point sur 40). La phase du spectre croisé était tirée **au
  hasard à chaque case** : le lissage sur 5 points ou plus les moyenne et la cohérence tombe à ~0,3 ; des puissances `xx`, `yy` tirées sur [0,5 ; 5] par case
  suffisent aussi à la faire passer sous 0,8. Sans point valide, aucun seuil de `valid` ne change le résultat.
- **Règle** — un cas fabriqué pour un seuil doit **atteindre le régime où ce seuil décide** : compter (dans le script de cas) les cas où le masque n'est pas
  vide, et exiger ce nombre dans le test ; phases lentes (le lissage les garde) et puissances voisines. Un zéro censé rendre `L` infini doit couvrir au moins la
  largeur du lissage, sinon les cases voisines le comblent.
- **Preuve** — `scripts/loop_spectra_cases.py` (`rnd`), `rust/otb-analysis/tests/open_loop_spectra.rs` (`with_valid >= 40`) ; les trois mutations sont maintenant tuées.

### L-056 : comparer en relatif une valeur qui est le résidu d'arrondi d'un zéro exact échoue sur une autre libm (CI Windows)
- **Symptôme** — 2026-10-30 (PR 4c-A) : la CI Windows échoue sur `rpm_attenuation "3h" k=1 nwin=3` : écart relatif case par case 0,93 ; le même test passe
  sous Linux (écart 2e-16).
- **Cause** — mesurée : la case est |H| = 9,9e-16 à 250 Hz pour un moteur à 250 Hz, poids 1, c'est-à-dire **au centre exact du notch**, où le vrai module
  vaut 0. Ce que Python rend est un résidu d'arrondi (cancellation) qui dépend de la libm (`sin`, `cos`, `hypot`) ; sous Windows il vaut autre chose : écart
  absolu ≈ 1e-15, relatif ≈ 1. J'avais durci la comparaison (« case par case en relatif », relecture 4c-A) sans voir que ses zéros exacts sont des résidus.
- **Règle** — le module `|H|` d'un notch se compare en **absolu** (rapporté au plus grand élément) ; un spectre (`P × |H|²`) se compare case par case avec un
  **plancher** (`|g − w| / (|w| + 1e-12 × max|w|)`) : sous le plancher la case est un résidu de zéro exact (≈ 1e-25 du maximum au centre d'un notch). Toute
  comparaison durcie doit passer par la CI Windows avant d'être crue (L-048).
- **Preuve** — `rust/otb-analysis/tests/common/mod.rs` (`CELL_FLOOR`, `assert_close_rel`), `rust/otb-parity/src/analysis_layer.rs` (`CELL_FLOOR`) ; spectre après
  filtre RPM : 3,7e-11 case par case avec plancher ; 3 sabotages (|H|², lobe, Q +1e-7) vus en échec.

### L-055 : la largeur d'une raie sur un spectre plat est une fonction discontinue du dernier bit : prouver le port sur le spectre exact de Python
- **Symptôme** — 2026-10-30 (4c-3) : sur 147 713 clés du corpus, un seul écart franc (`GF7037-3_carbon_nylon.bbl`, roll) : largeur de la 2ᵉ raie
  0 Hz en Python, 4,45 Hz en Rust, alors que tout le reste concorde à 1e-11.
- **Cause** — mesurée : le spectre masqué est plat à 1e-22 près autour de 421,5 Hz (6,9247e-08), `S[k]` dépasse la moyenne du bruit d'**1 ulp**,
  `level` se confond avec `S[k]` et les deux bords tombent sur `f[k]` (largeur 0) ; au dernier bit près la FFT de Rust donne un spectre un
  rien différent et la boucle `S[j+step] > level` s'arrête une case plus loin. Donné le spectre **exact de Python**, `half_power_width` de Rust rend 0 aussi :
  le port est correct, c'est l'entrée qui est dégénérée (raie de hauteur −1e-15 dB : elle n'existe pas).
- **Règle** — devant un écart franc sur une décision discrète, ne pas relâcher la tolérance : rejouer la **fonction seule sur l'entrée exacte de
  Python** (spectre sauvegardé) ; si elle concorde, l'écart vient du bruit de l'entrée. Dans la couche corpus, exempter **explicitement et
  compter** les largeurs de raies dont la hauteur de référence est < 1e-6 dB (ici 1 largeur sur 6 482, dans 667 sessions ; la hauteur suivante vaut 1,9e-4 dB, 190 fois le seuil). ⚠️ Conséquence pour l'assistant (phase
  5a) : `dyn_notch_q_need` ignore une largeur nulle (`w > 0`) mais accepterait 4,45 Hz et rendrait `inf` au lieu de `None` pour une raie inexistante :
  la parité des conseils (0 session différente sur 630) tranchera. Le nombre d'exemptions est borné (`MAX_EXEMPT` = 1) : au-delà, la vérification échoue. Le test unitaire `largeur_sur_spectre_plat_comme_python` (`motors.rs`, spectre sauvegardé dans `rust/otb-analysis/tests/data/flat_spectrum.json`) rend la preuve vérifiable dans le dépôt.
- **Preuve** — `rust/otb-parity/src/analysis_layer.rs` (`exempt`), spectre de la session rejoué en Rust : 0 Hz.

### L-054 : un retard par intercorrélation se compare en périodes d'échantillonnage, et sa tolérance se fixe aussi sur le corpus
- **Symptôme** — 2026-10-30 (4c-2) : sur les cas écrits, l'écart de `xcorr_delay_ms` montait à 100 % (« impulsions identiques », vrai retard nul :
  Python 2,2e-19 ms, Rust 4,3e-19 ms, du bruit d'arrondi) ; sur le corpus, un retard consigne → gyro du lacet de `HORUS/stock.bbl` dépassait la
  tolérance de 1e-10 posée d'après les seuls cas écrits (2,6e-10).
- **Cause** — mesurée : (1) l'écart *relatif* d'un nombre proche de zéro ne veut rien dire ; (2) l'interpolation parabolique divise par
  `a − 2b + d`, différence de trois valeurs voisines quand le pic est plat, qui amplifie l'arrondi de la FFT (≈ ×1e5 ici) ; les cas écrits sont
  bien conditionnés, le corpus non.
- **Règle** — comparer un retard à l'étalon en **périodes d'échantillonnage** (`1000 / fs` ms) avec le plancher d'une période, mesurer l'écart sur les
  cas écrits **et** sur le corpus avant de fixer la tolérance (ici 1e-8 : un saut d'indice du pic vaut ≥ 1 période, six ordres de grandeur de plus), et
  vérifier par sabotage que la couche corpus voit encore un changement d'échelle de 1e-7.
- **Preuve** — `rust/otb-analysis/tests/delay.rs` (`close_samples`), `rust/otb-parity/src/analysis_layer.rs` (`DELAY_TOL`), 5 sabotages vus en échec.

### L-052 : `a[:, masque].mean(axis=1)` additionne dans l'ordre, pas par paires : l'indexation avancée range le résultat « Fortran »
- **Symptôme** — 2026-10-30 (4c-1) : le déséquilibre des moteurs de `flight_stats` calculé en Rust diffère de 2e-4 relatif (0,08872 contre
  0,08874 %), bien au-delà d'un dernier bit : la différence `max − moyenne` de deux `float32` voisins amplifie un bit de chaque moyenne.
- **Cause** — mesurée : `m[:, flying]` (masque sur le 2e axe) a pour drapeaux `c_contiguous = False`, `f_contiguous = True`, pas
  `(4, 16)` ; `mean(axis=1)` y fait une somme **séquentielle** par ligne (identique au bit près à une boucle `s += x[j]`, différente
  de la somme par paires d'une ligne contiguë).
- **Règle** — avant de reproduire une réduction numpy, regarder la disposition en mémoire du tableau (`a.flags`) : un tableau né d'une
  indexation avancée sur un axe autre que le premier se somme dans l'ordre. Une ligne `m[k]` ou une copie contiguë se somme par paires.
- **Preuve** — `rust/otb-analysis/src/stats.rs` (moyennes des moteurs en vol), `tests/flight_stats.rs::flight_stats_identique`.

### L-053 : outil de mutation — restaurer avec une date de modification neuve, sinon cargo garde le binaire muté
- **Symptôme** — 2026-10-30 (4c-1) : `datasets_identiques` échouait sans raison après une batterie de mutations, puis repassait ; la référence
  « saine » d'un autre essai échouait aussi (journal `debug_1` : le jeu `debug` manquait).
- **Cause** — mesurée : `shutil.copy2` restaure le fichier **avec l'ancienne date** ; cargo juge le fichier inchangé et ne recompile pas : le
  binaire reste celui de la dernière mutation (`log.debug.k > 1`). Les résultats de la batterie restent valables (chaque mutation réécrit le
  fichier, donc recompile tout le crate avec les autres fichiers sains), seul l'état laissé après coup était périmé.
- **Règle** — l'outil de mutation restaure avec `copyfile` puis `os.utime` (date neuve), vérifie par `cmp`, et la référence est relancée
  **par son code de sortie** (jamais derrière `| tail`, L-022) avant et après la batterie. Ne jamais lancer `pkill -f MOTIF` dans une commande
  qui contient ce motif (elle se tue elle-même).
- **Preuve** — `mut.py` (dossier temporaire de la session), référence relancée après `touch` : code 0.

### L-051 : le produit complexe de numpy utilise le FMA du processeur ; en Rust, `mul_add` redonne ses bits, mais pas sur toute machine
- **Symptôme** — 2026-10-30 (4c-0) : `Σ conj(X)·Y` (spectres croisés de `loop_spectra`) recalculé en Rust avec `a*b + c*d` n'est identique à
  numpy que pour 1 176 valeurs sur 2 002 (écart relatif jusqu'à 5,8e-14 après les sommes).
- **Cause** — mesurée sur ce poste (banc `rust/otb-dsp/examples/banc_4c0.rs`) : avec `re = a.re.mul_add(b.re, a.im*b.im)` et
  `im = a.re.mul_add(b.im, -(a.im*b.re))`, les 2 002 valeurs sont identiques. NumPy multiplie donc les complexes avec des opérations
  fusionnées ici. Que ce soit le cas sur un processeur sans FMA n'est **pas vérifié** : traiter ces bits comme dépendants du poste (L-048).
- **Règle** — reproduire le produit complexe de numpy avec `mul_add` (fusion exacte, y compris sans instruction matérielle), mais ne jamais
  exiger plus qu'une tolérance sur ce qui en dépend ; exiger l'égalité exacte des décisions discrètes en aval (L-050).
  Autres primitives mesurées au même banc : division complexe (Smith) identique si le diviseur est réel (écart ≤ 1,7e-15 relatif sinon),
  `np.cumsum` complexe et `np.gradient(f, x)` (formule à pas variable) identiques au bit près, `|z|` (`hypot`), `|z|²` et `angle`
  (`atan2`) à ≤ 6,3e-16 relatif près (1 à 33 % de valeurs diffèrent du dernier bit).
- **Preuve** — `cargo run --release -p otb-dsp --example banc_4c0 -- DOSSIER` (entrées et attendus : script numpy du dossier temporaire).

### L-050 : une batterie de mutations n'est lisible qu'avec les vraies tolérances, des données qui atteignent chaque branche et un outil sûr
- **Symptôme** — 2026-10-30 (4b-4) : 14 mutations « survivaient » dont des erreurs franches (plafond du rapport, fenêtre minimale) ;
  plus tard, un chemin de sauvegarde mal recopié a **empilé** plusieurs mutations dans `bf_filters.rs`.
- **Cause** — mesurée : (1) la tolérance du test valait encore `1.0` (valeur provisoire pour afficher l'écart maximal) : rien ne
  pouvait échouer ; (2) des branches n'étaient atteintes par aucune donnée : la branche « signal avant filtres » ne change le
  résultat que si le spectre enregistré est plus faible que `préfiltre × |H_base|²`, les clés du cache (moteurs, centres) que si
  deux fenêtres de même gaz diffèrent par un seul des deux, `rpm_lpf_hz = 0` que si les harmoniques RPM sont actives ;
  (3) la fonction de mutation copiait la sauvegarde sans vérifier qu'elle existait.
- **Règle** — ne lancer la batterie qu'après avoir posé les tolérances finales (mesure d'abord, puis constantes, puis mutations).
  Pour chaque survivante, écrire la condition de données qui la rendrait visible, puis l'ajouter aux cas : sinon la raisonner
  et la noter équivalente. Utiliser un outil de mutation qui refuse de démarrer sans sauvegarde valide et qui restaure
  toujours le fichier (vérifier par `cmp`).
- **Preuve** — `scripts/bf_sim_cases.py` (journaux `flat_throttle`, `const_debug`, `const_motors`, `zero_debug_from`, réglages
  `extras`) ; `rust/otb-analysis/tests/bf_sim.rs`.

### L-049 : un test de parité qui compare Rust à Rust ne prouve que la colle
- **Symptôme** — 2026-10-30 (4b-3) : `coefficients` et `chain_response` ressortaient à 0 écart, exactement, dès le premier essai.
- **Cause** — mesurée : l'étalon Python appelle déjà le noyau Rust d'`otb-dsp` (`_native.biquad`, `_native.sections_response`). Le
  test compare donc ce noyau à lui-même ; il prouve seulement que la construction des étages (`_sections`, ordre, cadences,
  reshape) est la même, pas que le noyau est juste. Le noyau a été prouvé contre l'ancien code numba en 2a.
- **Règle** — avant de lire « 0 écart » comme une preuve, regarder si l'étalon passe par la fonction testée. Dans ce cas, noter
  ce que le test couvre vraiment (la colle) et d'où vient la preuve du noyau. Ne pas en tirer de tolérance « mesurée à 0 » :
  la tolérance se justifie par ce que peut changer une autre plate-forme (libm, BLAS), pas par ce nombre.
- **Aussi** — une frontière exacte (`|dd| = π` de `np.unwrap`) n'est jamais atteinte par de vraies données : une mutation de
  ce test survivait sur les 4 749 cas et le corpus. Elle est épinglée par un test unitaire dont les réponses viennent de numpy
  (`unwrap3_comme_numpy`), pas par un jeu de données.
- **Preuve** — `rust/otb-analysis/tests/bf_filters.rs`, `rust/otb-analysis/src/bf_filters.rs` (module `tests`).

### L-048 : l'étalon numpy dépend du processeur (`exp` AVX-512) ; un test « bit pour bit » sur `exp`, `cos`, `log10` est faux
- **Symptôme** — 2026-10-30 (4b-1) : `gaussian_filter` de Rust différait de Python de 2 ulp pour σ = 2, 5, 6,25 alors que les
  autres σ étaient identiques bit pour bit ; le noyau ne différait qu'à quelques indices symétriques (±1…).
- **Cause** — mesurée : sur ce poste (AVX-512), `np.exp` donne un autre dernier bit que `math.exp` pour 4,5 % des arguments
  (9 070 sur 200 001 entre −20 et 0). Avec `NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512_ICL"` (numpy 2.5 : niveaux `X86_V3` /
  `X86_V4`, pas `AVX512F`) `np.exp` redonne exactement `math.exp`, et le noyau de Rust. Le Rust appelle la bibliothèque
  mathématique du système (glibc, CRT de Windows, macOS) : elle peut elle aussi différer du dernier bit d'un système à l'autre.
  Idem pour `log10`, `atan2`, `hypot` (4b-0 : 1 à 7 % de dernier bit) et, par principe, `cos`.
- **Règle** — tout ce qui passe par `exp`, `log`/`log10`, `cos`/`sin`, `atan2`, `hypot` ou une FFT se compare à une tolérance
  (1e-14 du plus grand élément pour `cos` et `exp`, 1e-12 pour une FFT), jamais au bit près ; seule l'arithmétique pure
  (`+ − × ÷`, `sqrt`, sommes, arrondis) est exigée identique au bit près. Un test « bit pour bit » écrit avec les bits de ce
  poste échouerait sous Windows (la CI y exécute les tests Rust) ou sur une machine sans AVX-512. Sur le corpus enregistré, les
  sorties de `hann`, `uniform_filter1d` et `gaussian_filter` sont pourtant identiques au bit près (mesuré, 120 appels).
- **Preuve** — `rust/otb-dsp/tests/numeric.rs` (constantes `FN_RTOL`, `XCORR_RTOL`) ; `registry::tolerance` dans `rust/otb-parity`.
- **Spectres en dB (4b-2)** — l'écart de la FFT, relatif au plus grand élément, est de 4e-16 sur l'amplitude mais passe à 1,5e-10 (cas écrits) et 1e-10 (corpus, spectrogramme) une fois en dB : un petit élément, très en dessous du maximum, est relativement bien plus touché, et `10·log10` ne réduit rien. Tolérance fixée sur la mesure (1e-9), pas sur une estimation : celle de départ (1e-5) était 5 chiffres trop large.
- **Sans compter** — `np.fft` ≠ `realfft` à 9e-16 du plus grand élément (mesuré avec `rust/otb-dsp/examples/banc_4b0.rs`).

### L-047 : une mutation « survivante » peut être équivalente, ou ne rien avoir muté
- **Symptôme** — 2026-10-30 (4a-5) : changer le signe du gradient de `d_term_prefilter` ou le plancher de sa largeur de lissage
  laissait `check-flightlog` à 0 écart ; deux mutations de la batterie ne changeaient aucun fichier (motif absent après
  `cargo fmt`), donc comptées « vues » à tort si on lit seulement la sortie.
- **Cause** — mesurée : le gain `k` de `d_term_prefilter` est ajusté par moindres carrés, donc `raw·k` ne dépend pas du signe de
  `raw` ; la largeur vaut `max(3, int(fs/60))` ≈ 33, le plancher 3 n'intervient pas.
- **Règle** — après une mutation, `cmp` le fichier à sa sauvegarde (« PATRON ABSENT » sinon) ; une mutation qui survit se
  raisonne d'abord (équivalente ?) puis se couvre par un cas ou se note équivalente. Un seuil ou un plafond ne se prouve que
  par une valeur qui le franchit (gaz de repli : `fl_gaz_de_repli_hors_bornes`), l'égalité exacte aussi (`fl_moitie_exacte`).
- **Preuve** — `rust/otb-parity/tests/decode_fixtures.rs` ; `scripts/decoder_fixtures.py` (scénarios `fl_…`).
- **Suite** — la relecture indépendante (agent `relecteur`) a trouvé ce que ni les 668 sessions du corpus ni les 62 fixtures ne
  couvraient : `np.convolve` échange ses arguments quand le signal est plus court que le noyau (`n < w`, sessions de moins de
  133 trames à 8 kHz avec un terme D : gain `k` faux de 1,6e-3), `trim` de Rust au lieu de `strip` de Python dans
  `debug_mode_name`, un débordement de `+ 1` après un `as i64` saturé. Corrigés avec leurs tests (`fl_courte_avec_terme_d_*`,
  `settings_cases.py`, tests de `flightlog.rs`), chacun vu en échec sans le correctif. **Le corpus ne contient pas les cas
  courts** : toute branche numpy qui dépend de la longueur d'un tableau demande une fixture courte.

### L-046 : une couche de parité sur le corpus ne prouve pas les valeurs par défaut
- **Symptôme** — 2026-10-30 (4a-4) : changer le défaut de `looptime`, `motor_poles` ou `rpm_filter_q` dans `parse_settings` laissait
  `otb-parity check-settings` à 0 différence sur les 680 sessions ; seule la lecture d'une clé (`looptim`) échouait (69 sessions).
- **Cause** — mesurée : les logs du corpus contiennent ces clés, donc le défaut n'y est jamais utilisé.
- **Règle** — les défauts et les formes absurdes se prouvent par des cas écrits (`scripts/settings_cases.py`, aussi `{}`), le
  corpus prouve les valeurs réelles ; toujours les deux couches, et une mutation par couche.
- **Preuve** — `rust/otb-parity/tests/settings_cases.rs` (cas 0 : en-tête vide) contre `check-settings`.

### L-045 : lire un en-tête comme Python : espaces, hexadécimal, soulignés
- **Symptôme** — 2026-10-30 (4a-2) : le port Rust de `parse_header` acceptait `0x-1` (Python : valeur par défaut) ; `trim` de
  Rust gardait U+001C là où `str.strip()` de Python l'enlève ; un nombre `1__0` ou `0x1__f` n'est pas à traiter comme `10`.
- **Cause** — mesurée : `i64::from_str_radix` accepte un signe ; `str::trim` suit la propriété White_Space d'Unicode (sans U+001C
  à U+001F), `str.strip()` de Python inclut ces quatre séparateurs ; `float()` et `int(…, 0)` de Python n'acceptent un souligné
  qu'entre deux chiffres (ou juste après `0x`).
- **Règle** — passer par `header::py_strip`, `py_float` et `py_hex` ; ajouter un cas dans `scripts/header_cases.py` pour toute
  nouvelle forme de valeur, et laisser une mutation du port échouer avant de la croire couverte (le premier jeu de cas laissait
  survivre la mutation « soulignés doubles en hexa »).
- **Preuve** — `rust/otb-decoder/tests/header.rs` (477 cas), `otb-parity check-header` (680 sessions du corpus).

### L-044 : reproduire les textes de nombres de Python en Rust : `round`, `%g`, `repr`
- **Symptôme** — 2026-10-30 (E2) : sur 12 M de cas, `f64::round` et la sortie courte `{:e}` de Rust différaient de Python.
- **Cause** — mesurée : (1) `f64::round` arrondit les moitiés vers l'extérieur, `round` de Python vers le pair ; (2) Rust n'a pas
  de `%g` ; (3) sur une égalité exacte à 16 chiffres (ex. 696994262879799.25, valeurs ≥ 5·10¹⁴), Rust écrit …799.3, Python
  …799.2 (chiffre pair) ; (4) `round` de Python rend un entier sans limite, pas un `i64`.
- **Règle** — passer par `rust/otb-format` (`fixed`, `percent`, `general`, `repr`, `round_half_even`, `round_digits`), jamais
  `format!` ni `round` directement dans un texte comparé à l'étalon. `{:.N}` seul (décimales fixes) est identique à Python.
- **Preuve** — `rust/otb-format/tests/cas.rs` : `cas.tsv` (4 320 cas, CI) ; `OTB_FORMAT_CASES` pour 12 M de cas
  (`scripts/format_cases.py SORTIE.tsv 12000000`).

### L-043 : egui 0.37 : trois pièges vus pendant le prototype d'interface (E1)
- **Symptôme** — 2026-10-30 : (1) le thème « clair » s'affichait sombre dans la capture ; (2) la ligne d'un graphique ne se
  laissait pas saisir à la souris (le test de glisser échouait, coupure inchangée) ; (3) `TopBottomPanel` et
  `CentralPanel::show(ctx, …)` ne compilent plus.
- **Cause** — mesurée : (1) `set_visuals` ne vaut qu'au cadre suivant, et rien ne le redemandait ; (2) egui annonce `drag_started`
  après un déplacement : le curseur est déjà loin de la ligne quand on la cherche ; (3) egui 0.37 a fusionné les panneaux en
  `egui::Panel` et `show` prend un `Ui` (`show_inside` est déprécié).
- **Règle** — après `set_visuals`, appeler `request_repaint` ; chercher l'objet à glisser à `pointer.press_origin()`, pas à la
  position courante ; en test, ne pas se fier à « ne panique pas » : regarder la capture.
- **Preuve** — `rust/otb-spike-egui` (supprimé en 6-6b, retrouvable dans l'historique git ; `rust/otb-spike-egui/tests/interaction.rs` échouait avant la correction, `rust/otb-spike-egui/tests/tabs.rs` produit les captures).

### L-042 : reproduire numpy au bit près : `np.abs` complexe et `np.mean` ne font pas ce qu'on croit
- **Symptôme** — 2026-10-29 (phase 3b, D-034) : le port Rust de `rpm_attenuation` différait de 1 ulp de l'ancien code
  sur 4 811 valeurs de 12 040 (écart max 2,2e-16), puis celui de `harmonic_prominence` de 1 ulp sur 298 fenêtres de 471,
  alors que chaque étage de calcul était identique.
- **Cause** — mesurée : (1) `np.abs` d'un tableau complexe ≠ `np.hypot` ≠ `math.hypot` (libm) sur la même entrée ; numpy
  y emploie une version SIMD (cause **supposée** : `plus grand · √(1 + rapport²)` avec FMA), donc dépendante du
  processeur ; (2) `np.mean` d'un tableau 1D = `+0,0` plus la somme par paires de **tous** les éléments
  (blocs de 8 jusqu'à 128 valeurs, coupure en deux au-delà), et non « premier élément + somme par paires du reste ».
- **Règle** — ne pas refaire en Rust ce que numpy fait avec du SIMD : le noyau rend la réponse complexe et `np.abs`
  reste en Python (`motors.rpm_attenuation`). Pour une moyenne, reproduire la somme par paires de numpy
  (`otb-analysis:numpy_mean`) ; la vérifier sur toutes les longueurs, pas sur quelques-unes. Une somme sur l'axe 0
  d'une matrice, elle, est séquentielle ligne après ligne : plus simple à reproduire.
  Vérifié avec numpy 2.5.4 sur un processeur AVX-512 : une autre version ou un autre processeur peut changer ces bits
  (le test de `numpy_mean` échouerait alors bruyamment).
- **Preuve** — `tests/test_native_motors.py` (`numpy_mean` égal à `a.mean()` pour les longueurs 0 à 2000, trois échelles).

### L-041 : sur de vrais logs, `motor_hz` est en float32 : l'ancien code calculait les bandes en float32 (NumPy 2)
- **Symptôme** — 2026-10-29 (phase 3b) : en lisant l'ancien code avant de le porter, `h * lo_m[k]`, `marge * lo_m[k]`,
  `fs - b`, `1.3 * fc` et `b // fs` portent sur des `np.float32` ; un port tout float64 aurait échoué silencieusement.
- **Cause** — mesurée : `FlightLog.motor_hz` est en float32 (courbes en float32, D-029) ; avec NumPy 2 (NEP 50) un scalaire
  Python prend la précision du tableau, dans les opérations **et dans les comparaisons** (`np.float32(0,1) == 0,1` est vrai),
  alors que `float32 − float64` donne du float64. Effet réel faible mais non nul : sur 21 sessions du corpus, l'étalon
  rend un `_masked_spectrum` différent selon qu'on lui donne `motor_hz` en float32 ou converti en float64 (1 session sur 21).
- **Règle** — un noyau qui reprend une expression numpy suit la **précision des tableaux d'entrée**, pas celle de Python :
  `otb-analysis` est générique (`Real`, f32 et f64) et `motors._mhz_args` ne convertit jamais ces tableaux en float64.
  Avant de porter, afficher le `dtype` réel des entrées sur un vrai log, pas sur les logs synthétiques (ils n'ont pas
  d'eRPM : `motor_hz` y est estimé en float64).
- **Preuve** — `tests/test_native_motors.py` (cas float32 = principal, trois tests dont les bords tombent à quelques ulps
  d'une case, vérifiant que l'étalon float32 ≠ float64 puis que le noyau suit chacun).

### L-040 : des vues d'un même lot gardent en vie toutes ses lignes, même celles que plus rien n'utilise
- **Symptôme** — relu avant la fusion (phase 3a, D-034) : `from_session` lisait les 47 colonnes utiles en une passe et
  rendait des VUES du lot (gyro, PID, debug…) : les 14 lignes déjà recopiées ailleurs (consigne, moteurs, eRPM, tension,
  courant) restaient en mémoire tant que le log était ouvert, ≈ 34 Mo de trop pour 600 000 trames, à chaque log ouvert.
- **Cause** — une vue garde son tableau de base entier ; recopier les lignes inutiles n'en libère pas la mémoire.
- **Règle** — un lot lu en une passe ne contient que ce que l'objet garde en vue ; ce qui est recopié puis jeté va dans
  un second lot, libéré à la fin de la fonction (une passe de plus, ≈ 8–20 ms).
- **Preuve** — `tests/test_native_log.py:test_flightlog_arrays_do_not_keep_unused_rows_of_the_shared_batch_alive` (échoue
  avec un seul lot) ; mémoire conservée mesurée +154 Mo (ancien code) → +138 Mo.

### L-039 : mesurer un noyau Rust : `tracemalloc` ne voit pas ses tableaux, et un banc qui enchaîne les chargements ment
- **Symptôme** — 2026-10-29 (phase 3a) : `tracemalloc` annonçait un pic de 72 Mo contre 160 Mo avant (faux : les tableaux
  créés par Rust n'y figurent pas) ; un banc qui chargeait cinq fois le même log dans un processus annonçait +75 Mo de
  pic (faux : deux jeux de tableaux vivants en même temps), alors qu'un seul chargement par processus donnait +11 Mo.
  Premier critère de vitesse mal chiffré aussi : la durée de `from_session` avait été prise sous `cProfile`.
- **Cause** — l'allocateur de numpy et celui de Rust sont distincts ; `maxrss` d'un processus qui répète une opération
  mélange plusieurs vies de tableaux.
- **Règle** — mémoire d'un noyau Rust : `/proc/self/status` (`VmRSS` après libération, `VmHWM` après avoir écrit `5` dans
  `/proc/self/clear_refs`), **un chargement par processus**, plusieurs processus ; temps : `perf_counter` sans profileur,
  médiane de plusieurs essais, ancien et nouveau code dans le même banc.
- **Preuve** — mesures de D-034 ; banc dans le répertoire temporaire de la session.
- **⚠️ RÉCIDIVE le 2026-10-30** — pour décider de la 3c, le profil a été pris sous `cProfile` : `gyro_chain` y pesait
  0,51 s (« environ 1 s » avec ce qui l'entoure), contre 0,31 s mesurés avec de simples chronomètres sur les mêmes sessions.
  Une recommandation fondée sur ces chiffres était fausse. Règle précisée : aucune décision de vitesse sur un profil
  `cProfile` ; chronomètres autour des fonctions, plusieurs essais, et chaque affirmation sur la répartition du temps
  mesurée, pas déduite par soustraction.

### L-038 : rayon et les processus : un enfant créé par `fork` se bloque ; la variable de fils se pose dans chaque processus
- **Symptôme** — relu avant la fusion (2026-10-29, noyau de `step_response`, D-033) : un processus parent appelle
  `step_windows`, puis crée un `Pool` par `fork` ; dans les enfants, `step_windows` ne rend jamais la main.
  Autre piège vu en même temps : `RAYON_NUM_THREADS` posée dans le parent **après** le démarrage d'un forkserver
  n'atteint pas les processus de calcul, et `os.environ.setdefault` dans le parent ralentit tout ce qui suit.
- **Cause** — le pool de fils rayon (global, créé au premier appel) est copié par `fork` sans ses fils ; le
  forkserver reçoit l'environnement de son démarrage, pas celui d'après.
- **Règle** — passages parallèles en `forkserver` ou `spawn` seulement (jamais `fork` après un appel au noyau) ;
  `RAYON_NUM_THREADS=1` posée dans chaque processus par `Pool(initializer=…)` (`corpus_labels._one_thread`),
  jamais dans le parent. Python < 3.14 utilise `fork` par défaut sous Linux : le demander explicitement.
- **Preuve** — `tests/test_scripts.py:test_map_files_gives_each_process_one_rust_thread_and_leaves_the_parent_alone`
  (échoue sans l'initialiseur) ; `tests/test_native_step.py` (déterminisme à 1 et 8 fils) ; relecture indépendante.

### L-037 : déplacer une dépendance vers l'extra `dev` casse les workflows qui installent sans `dev`
- **Symptôme** — relu avant la fusion (2026-10-29) : `construire.yml` installe `. pyinstaller pytest`, lance des
  tests, puis PyInstaller. Après avoir sorti scipy et numba des dépendances, `tests/conftest.py` →
  `synthetic.py` → `scipy.signal.lfilter` aurait échoué à la collecte, sur les trois systèmes : aucun installateur.
- **Cause** — `test_workflows.py` ne vérifiait que la validité du YAML, pas que les tests lancés ont leurs imports.
- **Règle** — en changeant une dépendance de place, chercher tous les workflows (`grep -rn "pip install"
  .github`). Les références de test s'installent pour les tests, puis se **désinstallent avant PyInstaller** :
  l'exécutable ne peut pas les reprendre.
- **Preuve** — le test `test_build_workflow_installs_what_its_tests_import` de `tests/test_workflows.py` échouait si on retirait
  `scipy numba` de la ligne d'installation. **Supprimé en 7-4** (CI de la PR #93 : `StopIteration`) : `construire.yml` ne lance plus
  de tests Python ni d'installation de `scipy` / `numba` (il compile l'interface Rust), donc rien à surveiller ; le test de
  validité du YAML reste. La règle reste vraie pour `tests.yml` (il installe `.[dev]`).

### L-036 : `PyReadonlyArray2::as_slice` accepte l'ordre Fortran et le lit comme du C : résultat faux, sans erreur
- **Symptôme** — relu avant la fusion : `_native.run_section(np.asfortranarray(x), …)` rend autre chose que
  la version rangée normalement, sans exception. Aujourd'hui `bf_filters._apply` protège l'appel, un appel direct
  (une transposée) ne l'est pas.
- **Cause** — `as_slice` demande un tableau contigu, pas contigu **ligne par ligne**.
- **Règle** — un tableau 2D passé au Rust comme `&[T]` à plat se contrôle par `is_c_contiguous()` ; un test
  avec `np.asfortranarray` accompagne chaque entrée 2D.
- **Preuve** — `run_section` du pont PyO3 (supprimé en 7-5) ;
  `tests/test_native_dsp.py:test_run_section_rejects_fortran_order_instead_of_reading_it_wrong` (échoue sans le garde).

### L-035 : `ruff check --fix` supprime un import qui sert de réexport à un test
- **Symptôme** — 2026-10-29 : après avoir déplacé les constantes `CTX_*` de `decoder/core.py` vers `header.py`,
  `ruff check --fix` a retiré de `core.py` les imports qu'il n'utilisait pas lui-même : 10 tests de
  `test_decoder.py` et `test_bbl_decode.py` échouent avec `AttributeError: module '…core' has no attribute 'CTX_SIZE'`.
- **Cause** — un import non utilisé dans le module est vu comme mort ; les tests, eux, lisent `core.CTX_SIZE`.
- **Règle** — un nom réexporté volontairement porte `# noqa: F401` avec sa raison ; relancer la suite après tout
  `ruff check --fix`, et ne l'appliquer qu'après avoir lu le diff.
- **Preuve** — `src/opentunebox/decoder/core.py` (import de `header`) ; `pytest -m "not corpus"` : 585 passés.

### L-034 : un garde ou un test de parité qui ne sait pas échouer ne prouve rien (dépendance lourde, tolérance trop large)
- **Symptôme** — 2026-10-29 (phase 2a, D-032) : deux tests écrits « verts du premier coup » laissaient passer
  l'erreur qu'ils devaient attraper. (1) `test_no_heavy_deps.py` : avec `import scipy.ndimage` ajouté à `delay.py`,
  il passait. (2) `test_native_dsp.py` : plancher de gain du Rust changé de 1e-15 à 1e-14, il passait aussi.
- **Cause** — (1) le script du sous-processus fabriquait son vol avec `tests/synthetic.py`, qui importe scipy
  (aide de test), puis purgeait `sys.modules` : un import fait au chargement de l'application était effacé avant
  le contrôle. (2) tolérance absolue de 1e-12 : l'écart de 9e-15 du plancher y disparaît ; et le cas « centre
  d'un notch » choisi à la main (|g| ≈ 3e-15) n'atteignait même pas le plancher.
- **Règle** — avant de faire confiance à un garde ou à un test de parité, le **casser exprès** (importer scipy,
  changer une constante) et vérifier qu'il échoue. Garde d'import : bloqueur (`sys.meta_path`) posé **avant**
  d'importer l'application, levé seulement le temps de fabriquer les données de test, puis modules purgés et
  bloqueur remis. Valeur seuil (plancher, saturation) : comparaison **exacte**, sur un cas qui l'atteint
  vraiment (chercher un tel cas par balayage).
- **Preuve** — `tests/test_no_heavy_deps.py` (échoue avec « import interdit : scipy ») ;
  `tests/test_native_dsp.py:test_sections_response_floor_at_exact_notch_centre` (3 échecs avec 1e-14).

### L-033 : `Path.read_text()` sans encodage lit en cp1252 sous Windows : test vert sous Linux, `UnicodeDecodeError` en CI Windows
- **Symptôme** — 2026-10-29 : premier job Windows de la CI : `test_constants_match_python` échoue avec
  `UnicodeDecodeError: 'charmap' codec can't decode byte 0x8f` en lisant `frame.rs` (commentaires en français) ;
  les 126 autres tests passent, le même test passe sous Linux.
- **Cause** — mesurée (journal de la CI) : sans `encoding=`, Python prend l'encodage de la région (cp1252 sous
  Windows, UTF-8 sous Linux et macOS) ; un fichier UTF-8 accentué n'y est pas lisible.
- **Règle** — toujours `read_text(encoding="utf-8")` / `open(..., encoding="utf-8")` pour un fichier texte du
  dépôt, dans le code comme dans les tests.
- **Preuve** — `tests/test_native_decoder.py:test_constants_match_python` ; exécution Windows de la CI de
  `feat/rust-migration`.

### L-032 : numba lit hors tableau sans erreur ; le port Rust, lui, panique, et la panique n'est pas une `Exception`
- **Symptôme** — 2026-10-29 : le décodeur Rust, identique à numba sur tout le corpus et 6 000 flux aléatoires,
  plante sur un en-tête qui n'a qu'un seul champ I (`index out of bounds: the len is 1 but the index is 1`) ;
  numba, sur le même fichier, renvoie `[5 6]`. Dans l'interface, la tâche de calcul ne finit jamais.
- **Cause** — mesurée (relecture par agent, reproduite de bout en bout) : le décodeur lit toujours la case 1
  (temps) alors que les tableaux n'ont que `max(n_champs, 1)` cases. Numba ne contrôle pas les indices
  (comportement indéfini, il lit la mémoire voisine) ; Rust si. Une panique traverse PyO3 en
  `pyo3_runtime.PanicException`, qui hérite de `BaseException` : `except Exception` (`gui/worker.py`,
  `cli.py`) ne l'attrape pas.
- **Règle** — en portant du code numba : (1) tout indice que numba ne contrôlait pas reçoit un comportement
  défini (tampons assez grands, lecture tolérante `at()` qui rend 0) et documenté ; (2) le pont attrape les
  paniques (`catch_unwind`) et lève une exception ordinaire ; (3) une session sans champ de temps est
  ignorée par `reader.py`. Les tests de parité évitent les indices hors tableau de numba, dont la valeur est
  indéfinie : seuls ces cas peuvent légitimement différer.
- **Preuve** — `tests/test_native_decoder.py` (`test_single_field_header_is_ignored_by_both`,
  `test_random_streams_identical`, mutation d'une constante : 8 échecs) ; `rust/otb-decoder/src/frame.rs:at`.

### L-031 : `pip-audit` installé à côté du projet audite aussi pip et ses propres outils, et fait échouer la CI sur une faille étrangère au projet
- **Symptôme** — 2026-10-14 : premier essai de `audit.yml` dans un environnement vierge : `pip-audit` sort en
  erreur (code 1) sur `pip 25.1.1  PYSEC-2026-3721  26.2`, alors que `numpy`, `scipy`, `numba`, `PySide6`,
  `pyqtgraph` et `platformdirs` n'ont aucune faille connue.
- **Cause** — mesurée : sans argument, `pip-audit` contrôle **tout** l'environnement courant, `pip` compris ;
  le `pip` fourni avec Python peut être en retard (25.1.1 ici, corrigé en 26.2). Supposée, non mesurée : installé
  dans le même environnement que le projet, `pip-audit` y ajoute ses propres bibliothèques (`requests`,
  `urllib3`…), dont une faille ferait échouer le contrôle sans rapport avec ce qu'on livre.
- **Règle** — mettre `pip` à jour avant l'audit, et installer `pip-audit` (version épinglée) dans un
  environnement à part, dirigé sur celui du projet : `pip-audit --skip-editable --path <site-packages>`.
  Un échec de l'audit doit toujours dire « une dépendance livrée a une faille », jamais « un outil de la CI ».
- **Preuve** — `.github/workflows/audit.yml` ; `tests/test_ci_audit.py` (3 tests qui lisent le workflow : ils
  échouent sur la première version du commit `21708fe`, et sans la mise à jour de pip) ; essai local sans
  faille avec la forme corrigée (`No known vulnerabilities found`) ; CI verte de la demande de fusion #60.

### L-030 : exclure `PySide6.QtOpenGL` de la recette passe la construction, puis l'application plante au premier graphique
- **Symptôme** — 2026-10-13 : pour alléger l'exécutable, `PySide6.QtOpenGL`, `QtOpenGLWidgets`, `QtTest` et
  `QtNetwork` ajoutés aux `excludes` de `packaging/opentunebox.spec` ; PyInstaller réussit sans avertissement,
  `opentunebox report` marche, puis l'interface s'arrête : `ModuleNotFoundError: No module named 'PySide6.QtOpenGL'`.
- **Cause** — mesurée : `pyqtgraph/Qt/OpenGLHelpers.py` importe `QtOpenGL` et `QtOpenGLWidgets` sans `try`,
  et `PlotCurveItem` l'importe toujours, même avec `useOpenGL=False`. Seul `QtTest` est importé sous `try`.
  La bibliothèque C++ `libQt6OpenGL` sert en plus au pilote Wayland (EGL). Gain restant possible (`QtTest`,
  `QtNetwork`) : environ 4 Mo sur 463, pas le risque d'une construction non essayée sur Windows et macOS.
- **Règle** — ne jamais exclure un module de Qt de la recette sans l'exécutable d'essai de la compétence
  `opentunebox-dev` (rapport à cache numba vide, puis chaque outil ouvert hors écran) : la construction et
  la ligne de commande ne chargent pas pyqtgraph.
- **Preuve** — exécutable d'essai du 2026-10-13 (traceback ci-dessus) ; recette inchangée.

## 1. Décodeur et logs

### L-001 : `blackbox_decode` n'est pas une vérité : il compte mal les itérations et accepte des trames corrompues
- **Symptôme** — 2026-10-01 : comparé ligne à ligne, notre décodeur semblait décalé et perdre des trames.
- **Cause** — mesurée : avec les en-têtes `P interval:N` (Betaflight récents), la référence compte mal
  `loopIteration` et jette les trames P qui suivent la 1re trame I d'un bloc ; elle accepte des trames
  tronquées avant un trou de carte SD et des trames S parasites, et se bloque sur certains fichiers.
- **Règle** — comparer en alignant sur `time`, jamais sur `loopIteration` ; notre décodeur suit le firmware
  (itération = boucle PID).
- **Preuve** — [validation.md](validation.md#décodeur) : 568 fichiers, 95 % des sessions identiques, la
  référence se bloque sur 3 fichiers ; `scripts/validate_corpus.py`.

### L-002 : des trames impossibles passent le décodage et faussent toutes les analyses
- **Symptôme** — 2026-10-01 : pics de gyro ou de terme PID absurdes dans des logs par ailleurs lisibles.
- **Cause** — mesurée : trames corrompues valides en apparence (gyro ou consigne > 4000 °/s, terme PID
  > ±65535).
- **Règle** — `log/flightlog.py:_drop_implausible` les écarte ; une session corrompue à plus de 50 % est ignorée.
- **Preuve** — commit `d1e5520` ; [validation.md](validation.md#décodeur).

### L-003 : le même numéro de `debug_mode` ne désigne pas le même debug selon la version
- **Symptôme** — 2026-10-01, non consigné à l'époque ; si on l'ignore : un debug lu comme un autre, sans erreur.
- **Cause** — mesurée dans les sources du firmware : FFT_FREQ = 17 en 4.x, 16 depuis 2025.12.
- **Règle** — toujours passer par `log/debug_tables.py`, jamais un numéro en dur.
- **Preuve** — `log/debug_tables.py` ; [spec-betaflight.md](spec-betaflight.md).

### L-004 : D et D min sont inversés entre Betaflight 4.4/4.5 et 2025.12
- **Symptôme** — 2026-10-01, non consigné à l'époque ; si on l'ignore : D de base et D max échangés selon la
  version du log.
- **Cause** — mesurée dans les en-têtes : 4.4/4.5, `rollPID` D = D max et `d_min` = D de base ;
  2025.12+, l'inverse avec `d_max`.
- **Règle** — lire les PID normalisés par `log/settings.py`, jamais l'en-tête brut.
- **Preuve** — `log/settings.py` ; curseurs simplifiés recalculés juste pour 740 sessions sur 741
  ([validation.md](validation.md#autres-contrôles)).

### L-005 : le feedforward n'est nul qu'en mode Angle, pas en Horizon
- **Symptôme** — 2026-10-10 : un masque Angle + Horizon aurait jugé des vols Horizon comme sans feedforward.
- **Cause** — mesurée dans le firmware : le feedforward est forcé à 0 en mode Angle seulement.
- **Règle** — `flightlog.ANGLE_MODE_MASK` pour le feedforward ; `ANGLE_HORIZON_MASK` reste l'instrument de
  la boucle ouverte. Décision D-014.
- **Preuve** — commit `3e75e1d`.

**⚠️ RÉCIDIVE le 2026-10-13** — les paliers du pic de réponse (2026-10-05) et la règle de dépassement lisaient le
F de l'en-tête : 483 axes de vols en Angle (50 drones) comptés « avec feedforward », jugés avec des paliers
trop indulgents et parfois « Dépassement dû au feedforward ». Règle précisée : tout code qui juge la réponse
selon le feedforward lit `FlightConditions.ff_gain(axe)` (0 en Angle), jamais `settings.pids[axe].F`. Contrôle
automatique : `tests/test_advisor_synthetic.py:test_step_peak_angle_mode_uses_thresholds_without_feedforward`
(D-025).

### L-027 : `corrupt_frames` compte des tentatives de resynchronisation, pas du temps perdu
- **Symptôme** — 2026-10-12 : le constat « Trames corrompues » (> 20) manquait des vols qui perdent 3,7 % de
  leur durée (9 tentatives) et signalait un vol qui en perd 1,2 % (4 842 tentatives).
- **Cause** — lue dans `decoder/core.py` : le compteur augmente à chaque reprise de lecture un octet plus loin ;
  une zone abîmée en ajoute beaucoup, un trou net (pause d'écriture) parfois aucun. Les analyses, elles,
  recollent le vol de part et d'autre du trou : faux bruit jusqu'à +18 dB (W-010).
- **Règle** — juger un log abîmé par ses trous de temps (`FlightLog.recording_gaps`), jamais par
  `corrupt_frames` ; mesurer l'effet d'un défaut d'enregistrement en recalculant sans les fenêtres touchées.
  Toute nouvelle analyse à fenêtres prend `breaks` (`FlightLog.breaks`, D-020).
- **Preuve** — branche feat/calibrate-corrupt-frames ([assistant.md](assistant.md#trous-dans-lenregistrement-2026-10-12)).

## 2. Analyses

### L-006 : l'intercorrélation donne des retards de filtres dominés par le bruit haute fréquence
- **Symptôme** — 2026-10-01 : retards instables d'un log à l'autre ; en vol stationnaire, erreur d'au
  moins 0,5 ms une fois sur deux.
- **Cause** — mesurée : l'intercorrélation pèse surtout les hautes fréquences, où le bruit domine.
- **Règle** — méthode de phase entre 10 et 100 Hz, pondérée par la cohérence (D-007). Les objectifs de
  retard D de PIDtoolbox ne s'appliquent pas tels quels : il affiche un retard D 0,3 à 0,9 ms trop court
  (le gyro concorde à ±0,2 ms).
- **Preuve** — [validation.md](validation.md#recoupement-avec-pidtoolbox-pro-v094-mêmes-logs) (retards de
  Fred28 et Kris) ; commit `14010b4`.

### L-007 : en vol stationnaire, le retard des filtres mesuré n'est pas celui du vol réel
- **Symptôme** — 2026-10-05 : conseils de filtres fondés sur un retard trop optimiste.
- **Cause** — supposée le 2026-10-05 : les filtres dynamiques ne travaillent qu'à un régime ; les vols
  stationnaires du corpus restent sous 25 °/s de consigne. Mesurée le 2026-10-13 : l'écart moyen est faible
  (+0,015 ms entre vol stationnaire et vol avec mouvements du même drone, 18 drones), mais la dispersion d'un
  vol peu animé est 2 à 4 fois plus grande (tranches de 8 s : 0,09–0,13 ms sous 50 °/s, 0,03 ms au-delà de
  150 °/s).
- **Règle** — juger le retard seulement avec des mouvements (`delay.MOVING_SETPOINT_DPS`, consigne > 30 °/s
  au moins 5 % du temps). Décisions D-010, D-026. ⚠️ Un seuil plus haut (50 °/s) prend les wobbles du
  protocole en mode Angle limité à 30° (consigne plafonnée vers 47 °/s) pour des vols stationnaires.
- **Preuve** — commit `14010b4` ; [validation.md](validation.md) (« Retard des filtres et mouvements »,
  2026-10-13).

### L-008 : tout ce qui dépasse la moitié de la fréquence du log se replie dans le spectre
- **Symptôme** — raies de bruit à de fausses fréquences sur des logs lents ; à l'inverse, un vol de série
  PID à 1 kHz était signalé « trop lent » à tort (2026-10-12).
- **Cause** — mesurée : le blackbox sous-échantillonne sans filtre anti-repliement.
- **Règle** — deux minimums distincts : filtres (`rules.log_rate_needed`) et séries PID
  (`rules.pid_log_rate_needed`). Décision D-017.
- **Preuve** — commit `4787496`.

### L-009 : convertir deux fois à l'échelle de PIDtoolbox décale les spectres de 6,5 dB sans erreur
- **Symptôme** — 2026-10-05 : niveaux 6,5 dB trop bas par rapport aux repères, sans aucune erreur.
- **Cause** — mesurée : `analysis/` calcule la densité physique (= `scipy.signal.welch`), 6,5 dB au-dessus
  de PIDtoolbox ; l'interface et l'assistant convertissent chacun de leur côté.
- **Règle** — interface : `spectral.to_ptb_db` dans `compute` des outils ; assistant :
  `rules.band_peak_ptb_db` ; jamais de conversion dans `analysis/` ni deux fois. Décision D-016.
- **Preuve** — `spectral.PTB_PSD_OFFSET_DB` ; commits `7642145`, `2e4a602`.

### L-010 : S(u, gyro)/S(u, u) et L = T/(1 − T) donnent une boucle ouverte fausse sur Betaflight
- **Symptôme** — 2026-10-05 : marges de phase surestimées jusqu'à 31°, croisement à 7–10 Hz au lieu de 11–32 Hz.
- **Cause** — mesurée : la boucle est fermée (estimateur direct biaisé) ; Betaflight applique D au gyro et
  le feedforward à la consigne, ce que T/(1 − T) ignore.
- **Règle** — L = G·(Ce + Cd) mesurée avec un estimateur instrumental (D-012) ; T/(1 − T) seulement en
  repli « approché ». La bande mesurable s'arrête vers 20–40 Hz : pic de sensibilité souvent minorant (W-004).
- **Preuve** — [validation.md](validation.md#autres-contrôles) (simulation `tests/bf_loop_sim.py`, série
  BlackSummer) ; commit `7536105`.

### L-011 : la simulation temporelle des filtres est fausse au-dessus de ~0,4 × la fréquence du log
- **Symptôme** — 2026-10-01 : gyro simulé qui s'écarte du spectre attendu en haute fréquence.
- **Cause** — supposée : filtres appliqués au signal déjà sous-échantillonné, repliement compris.
- **Règle** — juger un changement de filtres avec `bf_filters.whatif_spectrum` (méthode relative, D-008) ;
  `simulate_gyro` n'est qu'un aperçu.
- **Preuve** — sans modification, le spectre « et si » égale le spectre loggé à 0,009 dB près
  ([validation.md](validation.md#autres-contrôles)) ; commit `adc63c9`.

### L-012 : l'incertitude jackknife d'un vol sous-estime la variation entre deux vols identiques
- **Symptôme** — 2026-10-10 : deux vols au même réglage donnent des cassures différentes au-delà de
  l'incertitude affichée.
- **Cause** — mesurée : la cassure varie d'environ 0,01 entre deux vols identiques ; le jackknife ne couvre
  que le bruit à l'intérieur d'un vol.
- **Règle** — ne pas départager deux réglages dont la cassure diffère de moins de ~0,01
  (`advisor/step_tuning.py`).
- **Preuve** — commit `3e75e1d` (10 séries Master du corpus).

### L-013 : quelques drones très loggés imposent leurs seuils à toute une taille
- **Symptôme** — 2026-10-05 : seuils whoop tirés vers les valeurs de quelques drones.
- **Cause** — mesurée : un drone avec beaucoup de sessions pèse autant que des dizaines d'autres.
- **Règle** — une voix par drone quand quelques drones dominent ; moins de ~20 drones → seuil commun ou
  profil provisoire (D-011, compétence `assistant-seuils`).
- **Preuve** — commit `95e41d6` (limite whoop 420 → 345 Hz, seuils recalculés).

### L-024 : un plancher numérique sur le seul dénominateur d'un rapport effondre le spectre simulé au creux des notchs
- **Symptôme** — 2026-10-12 : test synthétique d'identité de `whatif_spectrum` (mêmes réglages des deux
  côtés) : spectre « modifié » 90 dB sous le spectre enregistré sur les cases tombant exactement sur un
  centre de notch (raie à 200 Hz, pas de fréquence 3,33 Hz). Sur les vrais logs, l'identité tenait à
  0,009 dB (L-011) : les centres des notchs y tombent rarement pile sur une case.
- **Cause** — mesurée : au creux d'un notch, |H_actuel|² et |H_modifié|² valent tous deux ~1e-30 ;
  `r_mod / max(r_cur, 1e-12)` donnait ~1e-18 au lieu de ~1. Le plancher, posé contre une division par zéro,
  ne s'appliquait qu'à un côté du rapport.
- **Règle** — un rapport de deux grandeurs qui peuvent être minuscules ensemble se calcule sans plancher
  (`np.where(den > 0, num / den, 1.0)`) ; vérifier toute méthode « relative » par un test d'identité
  synthétique qui place une raie exactement sur une case et sur un centre de notch.
- **Preuve** — `tests/test_bf_filters.py:test_whatif_spectrum_identity_when_nothing_changes` (rouge avant,
  vert après) et `test_whatif_spectrum_follows_filter_change` (écart analytique à 0,1 dB).

**⚠️ RÉCIDIVE le 2026-10-13** — en accélérant `chain_response` (produit des étages au lieu des polynômes
convolués), le centre d'un notch tombé pile sur une case (harmonique à 450 Hz, `1. PTB/Sam/HoverTest24-48`)
valait exactement 0 dans les deux chaînes, au lieu de ~1e-33 d'arrondi : la garde `r_cur > 0` remettait le
rapport à 1, et le bruit prévu sans LPF1 changeait de 0,01 dB (2 sessions sur 705, chiffre affiché décalé
de 0,1). Règle précisée : chaque étage garde un gain plancher commun (`bf_filters._MIN_STAGE_GAIN`, 1e-15),
qui s'annule dans le rapport ; preuve `tests/test_bf_filters.py:test_exact_notch_centre_keeps_the_ratio_of_the_other_stages`.

### L-026 : injecter une oscillation en filtrant le gyro ne l'ajoute presque pas à la réponse indicielle
- **Symptôme** — 2026-10-12 : pour mesurer la sensibilité des vagues, un mode peu amorti à 45 ou 60 Hz
  ajouté au gyro réel (gyro + c × (mode(gyro) − gyro)) ne change presque rien, même à c = 0,2 ; parfois le
  rapport baisse.
- **Cause** — mesurée : le gyro contient très peu de mouvement lié à la consigne à ces fréquences ; la
  déconvolution ne voit que la part liée à la consigne, et l'oscillation ajoutée n'y pèse qu'environ 1 %.
- **Règle** — injecter la vague par la consigne : gyro + A × (mode peu amorti − mode amorti)(consigne), qui
  ajoute à la réponse une vague d'amplitude connue, avec le vrai bruit du vol. Choisir des vols au rapport
  ordinaire (≈ 1,2), pas ceux qui déclenchent déjà. Un rapport au bruit grandit avec le nombre de fenêtres :
  une ondulation de 0,35 % y dépasse 3 sur 1 000 fenêtres.
- **Preuve** — branche feat/calibrate-step-waves (`docs/assistant.md`, « Vagues »).

### L-028 : le filtre RPM n'efface pas toute l'harmonique ; la retirer du spectre cache ce que le notch dynamique suit
- **Symptôme** — 2026-10-13 : les pics prévus pour le notch dynamique ne retrouvaient que 48 % des fréquences
  que le firmware suit vraiment (debug FFT_FREQ, 598 sessions) ; moins qu'en ne retirant rien (50 %).
- **Cause** — mesurée : 417 sessions sur 598 ont une harmonique à poids réduit (« 100, 50, 80 »…), le filtre
  RPM en laisse passer (1 − poids), et ses flancs restent même à poids 100 ; le firmware suit ce reste.
  Retirer les bandes h × [moteur le plus lent ; le plus rapide] ± 5 % le cachait.
- **Règle** — pour prévoir ce qui passe après un filtre du firmware, appliquer sa réponse (|H|², avec poids
  et Q), jamais un masque ; vérifier contre le debug FFT_FREQ quand le log l'a (`bf_filters.fft_freq_centres`).
- **Preuve** — D-022 ; `tests/test_analysis.py:test_dyn_notch_targets_follow_rpm_weights` ;
  [assistant.md](assistant.md#notch-dynamique-2026-10-05).

## 3. Interface Qt

### L-014 : le style natif Windows 11 ignore la palette et grise les textes
- **Symptôme** — 2026-10-01 : sous Windows, boutons et listes grisés, à peine lisibles ; rien sous Linux.
- **Cause** — mesurée : le style « Windows 11 » et la palette standard ignorent une partie des couleurs.
- **Règle** — style Fusion imposé partout (`gui/theme.py`, D-009) ; vérifier sous Windows avant une version.
- **Preuve** — commit `861660d`.

### L-015 : sous Fusion (Qt 6.11), cases à cocher invisibles et listes déroulantes blanc sur blanc
- **Symptôme** — 2026-10-01 : contour des cases et boutons radio presque invisible ; en thème clair,
  élément survolé d'une liste déroulante illisible.
- **Cause** — mesurée : Fusion dessine ces contours presque transparents ; le délégué propre aux
  `QComboBox` ignore la feuille de style.
- **Règle** — cases et boutons radio redessinés par la feuille de style globale ;
  `theme._ComboDelegateFix` remplace le délégué par `QStyledItemDelegate`.
- **Preuve** — commits `861660d`, `d000968`.

### L-016 : une couleur ou une taille en dur casse l'un des deux thèmes
- **Symptôme** — 2026-10-01 : textes illisibles dans un seul des thèmes, tailles minuscules sur certains écrans.
- **Cause** — mesurée : `#777` ou `gray` contraste dans un thème et pas dans l'autre ; une taille en pixels
  ne suit pas l'échelle du système.
- **Règle** — `theme.accent_style`, `theme.muted_style` et `theme.current().ui` ; vérifier les deux thèmes.
  Toute couleur fixe (courbe, repère, badge, texte sur image) vit dans `gui/theme.py` (`PLOT_COLORS`,
  `SEVERITY_COLORS`, `ON_ACCENT`…), jamais dans un outil.
- **Preuve** — commit `861660d` ; depuis le 2026-10-12 (W-008),
  `tests/test_gui_contracts.py:test_every_color_lives_in_theme` échoue sur tout code `#rrggbb`, nom CSS ou
  couleur numérique hors de `theme.py` (28 occurrences avant la correction).

### L-017 : sous Linux, la fenêtre de fichiers dépend du portail et du thème d'icônes du bureau
- **Symptôme** — 2026-10-05 : sans portail, icônes invisibles en thème clair sur un bureau sombre ; avec le
  portail sous KDE, la fenêtre porte le logo de KDE.
- **Cause** — mesurée : la fenêtre propre à Qt prend les icônes du thème du bureau ; le portail ne permet
  pas de changer le logo.
- **Règle** — portail XDG (D-013) ; sans portail, `theme._match_icon_theme` choisit la variante claire ou
  sombre. Le logo KDE est normal. Textes de Qt en français : `qtbase_fr.qm`, seule traduction gardée par la
  recette PyInstaller.
- **Preuve** — commit `8dfb661`.

### L-018 : une feuille de style Qt ne charge une image que depuis un fichier
- **Symptôme** — 2026-10-01, non consigné à l'époque ; si on l'ignore : icône recolorée absente, sans erreur.
- **Cause** — supposée : `url(...)` d'une feuille de style n'accepte pas de données en mémoire.
- **Règle** — écrire l'icône recolorée dans le dossier temporaire (`theme._colored_svg`).
- **Preuve** — commit `43e8dd9`.

### L-019 : PySide6 6.11 + pyqtgraph plantent à la fin de Python
- **Symptôme** — 2026-10-01 : erreur de segmentation à la sortie d'un script Qt ou de l'application,
  après que tout a fonctionné.
- **Cause** — mesurée : la destruction automatique des objets graphiques libère un objet Python pendant
  que Qt détruit la scène.
- **Règle** — terminer tout script Qt par `opentunebox.gui.app.hard_exit(code)`.
- **Preuve** — `gui/app.py:hard_exit` ; commit `8a5f48b`.

## 4. Construction et distribution

### L-020 : PyInstaller échoue sous Linux sans `objdump`
- **Symptôme** — 2026-10-01, non consigné à l'époque : la fabrication de l'exécutable s'arrête.
- **Cause** — mesurée : PyInstaller utilise `objdump` (paquet `binutils`) sous Linux.
- **Règle** — installer `binutils` (commande `sudo` lancée par Cédric) ; Windows et macOS : construire sur
  le système cible.
- **Preuve** — compétence `opentunebox-dev` (Fabriquer les exécutables).

### L-021 : dans l'exécutable Windows sans console, `sys.stdout` et `sys.stderr` valent `None`
- **Symptôme** — 2026-10-01, non consigné à l'époque ; si on l'ignore : erreur sur une écriture dans
  `sys.stdout` qui marche en développement.
- **Cause** — mesurée : `console=False` ne crée pas de flux de sortie.
- **Règle** — ne jamais utiliser `sys.stdout` / `sys.stderr` sans vérifier qu'ils existent.
- **Preuve** — `gui/app.py:hard_exit` ; commit `6c63e6e`.

## 5. Outillage

### L-022 : un commit est passé avec la documentation en échec : crochet contourné et code de sortie masqué
- **Symptôme** — 2026-10-12, branche docs/project-registers : commit enregistré alors que
  `tests/test_docs.py` affichait « 1 failed » ; aucun blocage.
- **Cause** — mesurée : le crochet `.claude/hooks/check-docs.sh` ne réagissait qu'au texte `git commit`, et
  la commande s'écrivait `git -c user.name=… commit` ; la vérification lancée à la main finissait par
  `| tail -1`, dont le code de sortie (0) remplace celui de `pytest`.
- **Règle** — le crochet reconnaît aussi `git -c … commit` ; ne jamais enchaîner `&&` après un `pytest` passé
  dans un tube (`| tail`, `| grep`) : lancer `pytest` seul, ou vérifier « failed » dans sa sortie.
- **Preuve** — avant correction, crochet à 0 sur `git -c user.name=X commit` avec un numéro inexistant
  dans `docs/lecons.md` ; après, code 2.

### L-023 : un log supprimé du corpus fait ignorer des tests sans bruit, ou les fait échouer
- **Symptôme** — 2026-10-12 : après la suppression de logs par Cédric, 6 tests en `FileNotFoundError` et
  6 autres simplement ignorés (`s` dans la sortie de `pytest`), dont le Spectre, le simulateur de filtres et
  le retard consigne → gyro : une perte de couverture invisible sans `pytest -rs`.
- **Cause** — mesurée : certains tests ne vérifiaient que l'existence du dossier du corpus, d'autres
  ignoraient le test dès qu'un fichier manquait (dossier `1. PTB/Allykill` supprimé).
- **Règle** — ouvrir un log du corpus par `tests/corpus_paths.py:corpus_log` (test ignoré en nommant le log) ;
  après un changement du corpus, lancer `pytest -rs` et remplacer chaque log nommé par un log aux mêmes
  propriétés, cherché par un passage sur tout le corpus (compétence `calculs-corpus`).
- **Preuve** — avant : 233 réussis, 6 en échec, 6 ignorés ; après : 246 réussis, aucun ignoré.

### L-025 : la couverture de tests ignore le code compilé par numba, et la CI ne voit pas le corpus
- **Symptôme** — 2026-10-12 : `pytest --cov` donne 14 % pour `decoder/core.py` avec ou sans corpus, alors
  que le décodeur tourne sur tout le corpus ; et 35 % au total sans corpus contre 86 % avec.
- **Cause** — mesurée : les fonctions `@njit(cache=True)` s'exécutent en code machine, invisible pour
  `coverage` ; et tous les tests marqués `corpus` sont ignorés sur GitHub (pas de vrais logs), y compris,
  avant cette date, des tests de fonctions pures placés dans un fichier marqué `corpus` en bloc.
- **Règle** — mesurer le décodeur avec `NUMBA_DISABLE_JIT=1` (le workflow de tests le fait sur
  `test_decoder.py`, `test_flightlog.py` et `test_real_log.py`) ; ne jamais marquer `corpus` un fichier entier qui contient des
  tests purs ; tout outil ou analyse doit avoir un test sur les logs synthétiques (`tests/synthetic.py`),
  le corpus ne sert qu'à vérifier des valeurs réelles. Un vrai extrait versionné (`tests/data/jeno5_5s.bbl`,
  `tests/test_real_log.py`) fait passer l'en-tête et les encodages réels en CI, numba désactivé compris.
- **Preuve** — mesure du 2026-10-12 (`pytest-cov`, depuis ajouté à `pyproject.toml` : la CI publie la couverture combinée, sans seuil, W-017) ; `tests/test_gui_tools.py`
  exerce les 8 pages d'outils sans corpus.

### L-029 : un script Qt lancé hors de pytest écrit dans les vrais réglages de l'utilisateur
- **Symptôme** — 2026-10-13 : après un script de captures hors écran (`MainWindow` + `load_paths` + `close()`),
  `~/.config/OpenTuneBox/OpenTuneBox.conf` porte la date du script ; `_change_theme("clair")` y aurait aussi
  remplacé le thème choisi par Cédric. Rien en console.
- **Cause** — lue dans `gui/main_window.py` : `load_paths` enregistre `last_dir`, `_change_theme` enregistre
  `theme`, `closeEvent` enregistre la géométrie de la fenêtre (`save_geometry`) ; `QSettings` écrit dans le
  dossier de l'utilisateur. Les tests sont protégés par `conftest.py` (`QStandardPaths.setTestModeEnabled`),
  pas un script lancé à la main. Géométrie écrasée par le script de ce jour : supposé (valeur d'avant inconnue).
- **Règle** — tout script Qt hors de pytest commence par `QStandardPaths.setTestModeEnabled(True)` avant
  `QApplication` (compétence `opentunebox-dev`, « Captures hors écran ») ; vérifier la date du fichier de
  réglages après un script douteux.
- **Preuve** — `tests/conftest.py:pytest_configure` ; dates de `OpenTuneBox.conf` (réel 18:06, dossier de test
  `~/.qttest/config/` 18:07 après la correction).

## Index rapide

| ID | Piège (et la règle, en une phrase) | Domaine |
|---|---|---|
| L-081 | Pic de mémoire de l'ouverture : mesurer par étape ; rendre la table `i64` dès que les colonnes utiles sont extraites, puis ranger en `i32` dès le décodeur (481 → 274 Mo) ; positions S/G/E via `main_rows` | Décodeur et logs |
| L-082 | Exécuter un fichier qu'on vient de copier échoue par intermittence (« Text file busy », `ETXTBSY`) quand un test voisin lance un processus dans le même binaire de test : réessayer sur le code 26, sans toucher aux assertions | Tests |
| L-083 | Test et outil qui vérifient le même contrat : même source de tolérance (`registry::tolerance`), jamais une copie ; lister tous les écarts (`golden.rs` rouge par construction, caché par la CI) | Tests |
| L-080 | Allocation ratée = abandon du programme, `catch_unwind` ne la voit pas : `try_reserve` avant chaque ajout, et borner les nombres lus dans l'en-tête (D-044) | Décodeur et logs |
| L-079 | Doc qui cite un fichier de compilation (`rust/target/…`) : le test de chemins passe chez soi, échoue sur la CI (dépôt propre) ; citer sans le préfixe du dépôt, et tester sur une copie propre des fichiers suivis (`git ls-files`) avant de pousser | Outillage |
| L-078 | Test écrit avec `std::os::unix` (script shell, droits) : ne compile pas sous Windows, vu seulement sur la CI ; `#[cfg(unix)]`, chemins en `/`, `cargo check --target x86_64-pc-windows-gnu` avant de pousser | Tests |
| L-077 | Banc kittest : `run()` panique (`exceeded max_steps`) si un fil de calcul redemande une repeinte pendant qu'il tourne, vu sur la CI Windows ; attendre un calcul avec `step()` / `run_steps(n)` | Tests |
| L-076 | Icône de fenêtre egui sans effet sous Linux Wayland : winit ignore `with_icon` ; le bureau cherche l'icône dans un `.desktop` dont le NOM est l'`app_id`, majuscules comprises (`OpenTuneBox.desktop`, pas `opentunebox.desktop`) ; AppImage non intégrée : icône générique | Interface |
| L-075 | Test qui peut ouvrir une vraie boîte de dialogue du système : elle bloque le test et ne se ferme plus ; boîte remplaçable, inerte dans `App::new`, mutations sans écran ni bus de session ; rendu de test sans carte graphique | Tests |
| L-074 | Fonction paramétrée par un système (`os`) : appliquer les règles de ce système, pas celles de l'hôte (`Path::is_absolute` sous Windows refuse `/data`) ; test vu échouer seulement sur la CI Windows | Tests |
| L-073 | Panneau egui redimensionnable : sans contenu au premier cadre il se réduit à ce contenu et garde cette taille ; `ScrollArea` qui remplit dans toutes les branches, test qui part de l'état vide | Interface |
| L-072 | Reconnaître un log à l'adresse de son `Arc` : l'adresse d'un log fermé peut resservir à un autre ; identifiant croissant `LoadedLog::uid` | Interface |
| L-071 | Test « la clé de cache contient la période » qui passe même avec la mauvaise clé : une autre protection (`set_period` vide le cache) fait le travail ; casser la clé exprès | Tests |
| L-070 | `ComboBox` d'egui : la liste est coupée à 200 px, les derniers choix d'une liste de 11 sortent de la zone cliquable (test : clic sans effet) | Interface |
| L-069 | CI Ubuntu à court de disque : chaque fichier de test d'`otb-gui` lie egui et wgpu (≈ 490 Mo) ; `debug = "line-tables-only"` dans le profil `dev` | Outillage |
| L-068 | Banc kittest : le contrôle d'accessibilité ne tourne qu'à la fin de `run()` (listes et champs sans nom) ; « → ✔ ▲ ↑ ↓ ⚠ » absents des polices d'egui | Tests |
| L-067 | egui_plot : axes sans graduation avec la grille par défaut (puissances de 10) ; l'ordre des éléments décide de ce qui recouvre l'image | Interface |
| L-066 | Banc kittest : un cadre est dessiné avant le code du test ; réglages du contexte par `with_context_setup`, sinon « ⚠ » et panique de police sur les icônes SVG | Tests |
| L-065 | Couche corpus qui écarte les sessions courtes (< 8 s) : aveugle à leurs plantages ; garder la couche CLI sans filtre, et voir le test échouer avant le correctif | Tests |
| L-064 | Alléger un fichier de cas : transformer le journal AVANT de calculer la réponse de Python, comparer aux anciennes réponses ; relancer les mutations (raccourcir perd des branches) | Outillage |
| L-063 | Mutations par remplacement de texte : `cargo fmt` d'abord, motif absent = erreur du script, « tuée » seulement sur `FAILED` ; un seuil `>=` / `>` demande une valeur exactement dessus | Outillage |
| L-062 | Contrôle de complétude de la couche corpus : partir de la liste attendue (`flightlog.json`), pas des fichiers que les références déclarent ; le casser exprès (référence supprimée) | Outillage |
| L-061 | `np.convolve(x, ones(K)/K, "same")` : K = 15 somme croissante avec FMA ; K = 5 centre sans FMA, 2 premières et 2 dernières valeurs avec FMA (mesuré, 4 variantes essayées) ; l'ordre « évident » diffère au dernier bit | Outillage |
| L-060 | `\w`, `\d`, `\s` de Python ≠ `is_alphanumeric`, `is_numeric`, `is_whitespace` de Rust : table générée depuis Python, vérifiée sur tous les points de code | Outillage |
| L-059 | « Identique au bit près » après une FFT : vrai sur le poste des références seulement ; tolérance dans les tests de la CI, relire toute la sortie Windows (cargo s'arrête au 1er échec) | Outillage |
| L-058 | Ordre des sommes de numpy : `x[..., masque]` et matrice d'une colonne sont « Fortran », somme par paires sur l'axe 0 (≥ 8 valeurs) | Analyses |
| L-057 | Cas aléatoires qui n'atteignent jamais le régime valide : seuils libres ; phases lentes, puissances voisines, compter les cas non vides | Outillage |
| L-056 | Un résidu d'arrondi de zéro exact (centre d'un notch) ne se compare pas en relatif : absolu pour |H|, plancher pour les spectres ; une comparaison durcie passe par la CI Windows | Outillage |
| L-055 | Largeur de raie sur spectre plat = fonction discontinue du dernier bit : rejouer la fonction seule sur l'entrée exacte de Python, exempter et compter | Analyses |
| L-054 | Retard d'intercorrélation : écart en périodes d'échantillonnage ; tolérance mesurée sur les cas écrits ET sur le corpus (pic plat = division mal conditionnée) | Analyses |
| L-053 | Outil de mutation : restaurer avec une date neuve (`utime`), sinon cargo garde le binaire muté ; référence relancée par code de sortie | Outillage |
| L-052 | `a[:, masque].mean(axis=1)` est rangé « Fortran » : somme dans l'ordre, pas par paires (`a.flags` avant de reproduire une réduction) | Analyses |
| L-051 | Produit complexe de numpy = FMA du processeur : `mul_add` en Rust, tolérance sur l'aval, décisions discrètes exactes | Outillage |
| L-050 | Batterie de mutations : tolérances finales d'abord, données qui atteignent chaque branche, outil de mutation qui refuse sans sauvegarde | Outillage |
| L-049 | Parité Rust contre un étalon qui appelle déjà le noyau Rust : 0 écart ne prouve que la colle ; frontières exactes épinglées par un test unitaire | Outillage |
| L-048 | Étalon numpy dépendant du processeur (`exp` AVX-512 ≠ `math.exp` pour 4,5 %) : `exp`, `cos`, `log10`, FFT à tolérance, jamais bit pour bit | Outillage |
| L-047 | Mutation survivante : équivalente (gain ajusté, plancher inactif) ou motif absent ; seuils prouvés par une valeur qui les franchit | Décodeur et logs |
| L-046 | Parité sur le corpus : ne prouve pas les défauts, il faut des cas écrits en plus | Décodeur et logs |
| L-045 | En-tête Blackbox lu comme Python : `strip` (U+001C…), `0x` sans signe, soulignés entre chiffres seulement (`header.rs`) | Décodeur et logs |
| L-044 | Textes de nombres Python en Rust : `round` au pair, `%g` et `repr` à refaire (`otb-format`) | Analyses |
| L-043 | egui 0.37 : thème appliqué au cadre suivant, glisser cherché au point pressé, panneaux fusionnés en `Panel` | Interface |
| L-042 | Reproduire numpy au bit près : `np.abs` complexe est SIMD (module pris en Python) ; `np.mean` = +0,0 + somme par paires de tous les éléments | Analyses |
| L-041 | `motor_hz` est en float32 sur de vrais logs : les noyaux suivent la précision des tableaux d'entrée (NEP 50), jamais float64 d'office | Analyses |
| L-040 | Vues d'un lot lu en une passe : elles gardent toutes ses lignes vivantes ; lot 1 = ce que l'objet garde, lot 2 = temporaire | Décodeur et logs |
| L-039 | Mesure d'un noyau Rust : `tracemalloc` ne voit pas ses tableaux ; un chargement par processus, `VmRSS`/`VmHWM`, temps sans profileur | Outillage |
| L-038 | Rayon : un enfant créé par `fork` se bloque ; `RAYON_NUM_THREADS=1` par `Pool(initializer=…)`, pas dans le parent | Outillage |
| L-037 | Dépendance déplacée vers `dev` : relire tous les workflows ; références de test installées puis désinstallées avant PyInstaller | Construction |
| L-036 | `as_slice` accepte l'ordre Fortran et le lit comme du C : `is_c_contiguous()` + test `asfortranarray` | Décodeur et logs |
| L-035 | `ruff check --fix` supprime un import qui sert de réexport à un test : `# noqa: F401`, relancer la suite | Outillage |
| L-034 | Garde ou test de parité jamais vu en échec : le casser exprès ; bloqueur d'import posé avant l'application ; valeur seuil comparée exactement | Outillage |
| L-033 | `read_text()` sans `encoding` lit en cp1252 sous Windows : toujours `encoding="utf-8"` | Outillage |
| L-032 | Numba lit hors tableau sans erreur, Rust panique (`BaseException`). Comportement défini, `catch_unwind`, session sans temps ignorée | Décodeur et logs |
| L-031 | `pip-audit` audite tout son environnement (pip compris) : pip à jour, outil dans un environnement à part, `--path` | Outillage |
| L-030 | Module Qt exclu de la recette : construction réussie, plantage au premier graphique. Exécutable d'essai | Construction |
| L-029 | Script Qt hors pytest : réglages réels écrasés. `QStandardPaths.setTestModeEnabled(True)` d'abord | Outillage |
| L-028 | Filtre RPM ≠ masque : appliquer sa réponse (poids, Q) ; vérifier sur le debug FFT_FREQ | Analyses |
| L-027 | `corrupt_frames` ≠ temps perdu. Trous de temps (`recording_gaps`) | Décodeur et logs |
| L-026 | Oscillation injectée par le gyro : invisible dans la réponse. Injecter par la consigne | Analyses |
| L-025 | Couverture aveugle : numba (`NUMBA_DISABLE_JIT=1`) et tests `corpus` ignorés en CI. Logs synthétiques | Outillage |
| L-024 | Plancher sur un seul côté d'un rapport : spectre simulé effondré au creux des notchs. Diviser sans plancher, test d'identité | Analyses |
| L-023 | Log du corpus supprimé : tests en échec ou ignorés sans bruit. `corpus_log`, `pytest -rs` | Outillage |
| L-022 | Commit passé malgré un test en échec. Crochet élargi, pas de `&&` après `pytest \| tail` | Outillage |
| L-021 | Exécutable Windows : `sys.stdout` vaut `None`. Vérifier avant d'écrire | Construction |
| L-020 | PyInstaller Linux sans `objdump`. Installer `binutils` | Construction |
| L-019 | Segfault à la fin de Python. Terminer par `hard_exit` | Interface |
| L-018 | Image de feuille de style seulement depuis un fichier. `theme._colored_svg` | Interface |
| L-017 | Fenêtre de fichiers Linux. Portail XDG, logo KDE normal | Interface |
| L-016 | Couleur ou taille en dur. Couleurs du thème, vérifier les deux thèmes | Interface |
| L-015 | Fusion : cases invisibles, listes blanc sur blanc. Feuille globale, `_ComboDelegateFix` | Interface |
| L-014 | Windows 11 grise les textes. Fusion partout | Interface |
| L-013 | Quelques drones dominent une taille. Une voix par drone | Analyses |
| L-012 | Jackknife d'un vol trop optimiste. Écart de cassure < 0,01 = égalité | Analyses |
| L-011 | `simulate_gyro` faux en haut du spectre. Juger avec `whatif_spectrum` | Analyses |
| L-010 | Boucle ouverte biaisée. Estimateur instrumental, T/(1 − T) en repli | Analyses |
| L-009 | Double conversion d'échelle PIDtoolbox. Une seule, hors de `analysis/` | Analyses |
| L-008 | Repliement au-dessus de fs/2. Deux fréquences minimales | Analyses |
| L-007 | Retard en stationnaire non représentatif. Vol avec mouvements | Analyses |
| L-006 | Intercorrélation dominée par le bruit. Méthode de phase | Analyses |
| L-005 | Feedforward nul en Angle seulement. `ANGLE_MODE_MASK`, `FlightConditions.ff_gain` (récidive 2026-10-13) | Décodeur et logs |
| L-004 | D / D min inversés selon la version. `log/settings.py` | Décodeur et logs |
| L-003 | Numéros de debug selon la version. `log/debug_tables.py` | Décodeur et logs |
| L-002 | Trames impossibles. `_drop_implausible` | Décodeur et logs |
| L-001 | `blackbox_decode` compte mal. Aligner sur `time` | Décodeur et logs |
