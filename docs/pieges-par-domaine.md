# Pièges par domaine

Règles courtes des pièges **propres à un domaine** ; histoire, cause et preuve de chacune dans [lecons.md](lecons.md) sous
le numéro indiqué. Les pièges qui valent pour presque **toute** tâche sont dans `AGENTS.md` (« Pièges connus ») et ne sont
pas répétés ici : une règle écrite à deux endroits finit par ne plus dire la même chose des deux côtés.

## Décodeur et journaux de vol

- `blackbox_decode` (référence de PIDtoolbox) compte mal `loopIteration` avec les en-têtes `P interval:N`
  (Betaflight récents) et jette les trames P qui suivent la 1re trame I d'un bloc : comparer en alignant sur
  `time`. Il accepte aussi des trames corrompues (tronquées avant un trou de carte SD, trames S parasites)
  et se bloque sur certains fichiers ; notre décodeur suit le firmware (itération = boucle PID). L-001
- `rust/otb-log/src/lib.rs:drop_implausible` écarte les trames impossibles (gyro ou consigne > 4000 °/s, terme PID > ±65535) ;
  une session corrompue à plus de 50 % est ignorée. L-002
- `stats["corrupt_frames"]` compte des tentatives de resynchronisation, pas du temps perdu : juger un log
  abîmé par ses trous (`FlightLog.recording_gaps`). Toute analyse à fenêtres prend `breaks=log.breaks(sel)` :
  sans lui, chaque trou ajoute un faux bruit (jusqu'à +18 dB). D-020, L-027
- L'en-tête n'est jamais cru sur parole : 512 champs au plus, intervalles de 1 à 65 535, sinon `DecodeError` (message en français) ; les tampons grandissent par `try_reserve`, car une allocation ratée abandonne le programme et `catch_unwind` ne la rattrape pas. D-044, L-080
- Toute modification du décodeur ou de la conversion (`otb-log`) passe l'essai de mutations prolongé (`OTB_MUTATIONS`, en `--release` ; commandes dans `docs/validation.md`) : la CI n'en fait que 30 cas. Un cas rouge se refait d'après son numéro. D-054
- Numéros de `debug_mode` différents selon la version (FFT_FREQ = 17 en 4.x, 16 depuis 2025.12) → `rust/otb-log/src/debug_tables.rs`. L-003
- 4.4/4.5 : `rollPID` D = D max et `d_min` = D de base ; 2025.12+ : l'inverse avec `d_max` → normalisé dans `rust/otb-log/src/settings.rs`. L-004

## Analyses (retards, fréquence du log, feedforward, échelles, boucle ouverte)

- Retards de filtres : méthode de **phase** (10–100 Hz, pondérée par la cohérence), pas l'intercorrélation
  (dominée par le bruit haute fréquence). PIDtoolbox affiche un retard D 0,3 à 0,9 ms trop court (le gyro
  concorde) : ses objectifs de retard D ne s'appliquent pas tels quels. D-007, L-006
- Retard des filtres : jugé seulement avec des mouvements (`delay.MOVING_SETPOINT_DPS`, consigne
  > 30 °/s au moins 5 % du temps) ; en vol stationnaire, les filtres dynamiques ne travaillent qu'à un régime.
  Pas 50 °/s : les wobbles en mode Angle limité à 30° plafonnent vers 47 °/s. D-010, D-026, L-007
- Fréquence du log : deux minimums à ne pas mélanger. **Filtres** : au moins 2 kHz pour un gyro à 8 kHz,
  1,6 kHz à 3,2 kHz, plus pour les petits drones (`rules.log_rate_needed`) ; le blackbox sous-échantillonne
  sans filtre, tout ce qui dépasse fs/2 se replie. **Séries PID** : 1 kHz (`rules.pid_log_rate_needed`). Le
  guide range les logs par rôle (`log_check.FILTERS` / `PID`) : chaque case ne lit que ceux de sa phase.
  Vol stationnaire = identifier le bruit ; vol avec mouvements = juger le retard. D-017, L-008
- Feedforward : forcé à 0 par le firmware en mode **Angle** seulement (pas en Horizon) → masque
  `flightlog.ANGLE_MODE_MASK`, distinct de `ANGLE_HORIZON_MASK` (instrument de la boucle ouverte). Juger la
  réponse selon `FlightConditions.ff_gain(axe)` (0 en Angle), jamais le F de l'en-tête. D-014, D-025, L-005
- Échelle des DSP : `otb-analysis` calcule la densité physique (= `scipy.signal.welch`), 6,5 dB au-dessus de PIDtoolbox
  (`spectral.PTB_PSD_OFFSET_DB`) ; l'interface affiche l'échelle de PTB (`spectral.to_ptb_db`, dans `compute` des
  outils), l'assistant convertit lui-même (`rules.band_peak_ptb_db`) : ne jamais appliquer la conversion deux fois. D-016, L-009
- Réponse indicielle : fenêtres de 2 s, pas de 62,5 ms, consigne min 20 °/s, régularisation 1e-4 ; pic
  cherché dans max(150 ms, 3 × latence). Bande passante calculée à partir de 2 Hz, lissage `uniform_filter1d(mode="nearest")`.
- Boucle ouverte (`rust/otb-analysis/src/open_loop.rs`) : L = G·(Ce + Cd) **mesurée** à partir des termes P, I, D, F loggés ;
  G par estimateur instrumental (consigne, ou `rcCommand` en mode angle/horizon, plus l'excitation du debug
  CHIRP de Betaflight, ajoutée à la consigne avant le mode angle), jamais S(u, gyro)/S(u, u)
  (biaisé en boucle fermée). L = T/(1 − T) n'est qu'un repli « approché » (faux pour Betaflight : D sur le
  gyro, FF sur la consigne). La bande mesurable s'arrête vers 20–40 Hz (énergie du manche) : pic de
  sensibilité souvent minorant. L'onglet Autotune du configurateur n'est pas une source. D-012, L-010
- Concordance avec Python (`otb-parity`) : tout test qui rejoue les références prend la tolérance de
  `rust/otb-parity/src/registry.rs:tolerance` (bit près par défaut), jamais une valeur recopiée, et liste tous ses écarts. D-047, L-083

## Filtres Betaflight et notch dynamique

- Notch dynamique : prévoir ses pics avec la **réponse** des notchs RPM du firmware (poids, Q), jamais en
  retirant les bandes des harmoniques : une harmonique à poids réduit laisse un reste qu'il suit. Le debug
  FFT_FREQ (`bf_filters.fft_freq_centres`) donne ce qu'il a vraiment suivi : y vérifier tout modèle. D-022, L-028
- Simulateur de filtres (page Filtres) : chaîne du firmware **rejouée** sur le gyro non filtré (`filter_replay::before_after`),
  spectre loggé × rejeu modifié / rejeu actuel, fenêtre par fenêtre : exact quand rien ne change. Sur un log enregistré une
  boucle PID sur 2 ou moins, le gyro non filtré est replié et le rejeu « actuel » s'écarte du log aux basses fréquences (W-067).
  L'assistant aussi (D-059) ; le rapport des réponses |H|² (`bf_filters.whatif_spectrum`) ne sert plus que sans gyro non
  filtré et pour les références Python (`FilterModel::Ratio`). D-058, D-059, D-008, L-024
- `gyroUnfilt` contre un signal filtré : retirer d'abord ⌊`pid_process_denom`/2⌋ / fréquence gyro (125 µs à 8 kHz,
  denom 2), décalage d'horloge du firmware et non retard de filtre. L-100
- Modèle contre firmware : filtres calculés pour la cadence **nominale** de l'en-tête (`filter_stages::model_rate_hz`), pas
  `FlightLog::fs` ; un contrôleur surchargé tourne moins vite qu'annoncé et coupe plus bas en hertz réels. L-101
- Notch dynamique rejoué : calculer la cadence en `f32` **comme la version du firmware** (`dyn_notch::DynNotchTracker::new`) ;
  quand Nyquist / `dyn_notch_max_hz` est entier, la 2026.6 moyenne 3 boucles au lieu de 4, et la cadence exacte fausse le suivi. L-102
- Gain mesuré par S(x, y) / S(x, x) sur une entrée **arrondie** (canaux debug au °/s) : retirer pas²/12 de S(x, x), sinon le
  firmware paraît filtrer plus. Une « variable instrumentale » faite de l'entrée décalée d'un échantillon ne le corrige pas
  (même bruit, phase tournée) ; preuve directe : l'étage horloge, de gain 1. L-103
- Chaque filtre se rejoue à la cadence où le firmware l'applique : **gyro** pour le LPF2 (depuis la 4.2), **PID** pour les autres,
  même quand le log saute des boucles ; à la cadence du log, il filtre trop (`bf_filters::gyro_stage_upsampled`). Échantillons
  manquants reconstruits par sinus cardinal, jamais linéairement (passe-bas caché). L-104
- Rapport « modifié / actuel » de deux spectres : sur des signaux **exacts** ; arrondis comme le log, chacun porte son bruit
  d'arrondi et la moyenne du rapport monte (filtrer plus « ajoutait » du bruit). L'arrondi ne sert qu'à la courbe comparée au
  log. L-105
- Effet d'un changement de modèle : comparer les spectres exacts, pas la courbe « actuel » arrondie ; sur un D enregistré sous
  l'unité, l'arrondi fait d'un écart de 0,001 dB un écart de 0,38 dB. L-108

## Assistant de réglage

- Réponse indicielle par séries (`rust/otb-advisor/src/step_tuning.rs`) : seuils tirés de 10 séries Master du corpus,
  choix des pilotes relus dans les vols suivants ; la cassure varie d'environ 0,01 entre deux vols
  identiques (incertitude jackknife plus faible : elle ne couvre que le bruit d'un vol). L-012
- Seuils de l'assistant = 90e percentile du corpus par taille de drone (`docs/assistant.md`) : les recalculer
  avec `otb-dev corpus calibrate` (`--resume` pour les percentiles ; outils du corpus en parallèle sur
  tous les cœurs, compétence `calculs-corpus`), ne pas les inventer. Compter **une voix
  par drone** quand quelques drones dominent une taille (whoop, seuils mécaniques) ; moins de ~20 drones →
  seuil commun ou profil provisoire. Chaque seuil a une origine notée (corpus, théorie, firmware…). D-011, L-013
- Les couches du corpus (`otb-dev corpus rates`, couches de `otb-parity`) écartent les sessions de moins de 8 s : elles ne voient pas un plantage sur une session courte ; seule la couche de la ligne de commande les rejoue. Voir le test échouer avant tout correctif. L-065

## Interface Qt (ancienne version Python, supprimée en 7-5)

Pièges de l'ancienne interface Qt, gardés pour mémoire dans `docs/lecons.md` (style Fusion : L-014, L-015 ; portail XDG et traductions de Qt :
L-017 ; icônes recolorées : L-018 ; plantage à la fin de Python : L-019, L-021). Aucun ne concerne l'interface egui.

## Interface egui et Rust

- Échéance (délai d'abandon) et résultat asynchrone : **relever d'abord** ce qui est déjà arrivé, n'examiner l'échéance
  qu'ensuite — sinon elle jette un travail terminé (l'export d'image échouait sur « capture pas arrivée » alors qu'elle
  l'était). Et un test ne prouve pas une règle de temps en **dormant** : rendre la condition immédiate (délai à zéro) et
  poser le résultat par une prise de test (`App::shot_sender`). L-095
- Résultat rangé dans le cache des outils (`state.rs:Cache`, jamais évincé) : regarder **ce que son type contient** et libérer
  ce qu'aucun lecteur n'affiche — le rapport de l'assistant transportait les fenêtres individuelles de la réponse indicielle,
  69 % de son poids (jusqu'à 72 Mo) pour rien. Une mémoire se **mesure** (`--example banc_cache`, allocateur compteur et
  témoin), elle ne se suppose pas. L-094
- **Contour de la fenêtre** (barre de titre, bordures, icône) : il appartient au système, ni les tests ni les captures hors écran ne le voient — il se vérifie sur une vraie machine. egui l'habille au **premier** cadre (`sync_window_theme`), trop tôt pour Windows, qui ne repeint pas le contour d'une fenêtre qu'il vient de créer : lui retirer ce travail et annoncer le thème une fois la fenêtre posée. L-091
- Fonction paramétrée par un système (`os`, comme `guide::cards_path_in`) : appliquer les règles de ce système, jamais celles de l'hôte (`Path::is_absolute` refuse `/data` sous Windows) ; la CI Windows est le seul témoin. L-074
- Mesure de la carte graphique par `egui_kittest` : il prend par défaut le rendu logiciel (llvmpipe) et `render` relit l'image (≈ 10 ms fixes) ; choisir une vraie carte (`native_adapter_selector`), l'afficher, soustraire un écran sans courbe, décider sur le temps du processeur. L-084
- Test egui qui attend un calcul en arrière-plan : `step()` dans la boucle d'attente puis `run_steps(n)`, jamais `run()` (il panique si le fil redemande une repeinte, CI Windows). L-077
- Panneau egui redimensionnable : son contenu est toujours dans un `ScrollArea` qui remplit (`auto_shrink([false, false])`), même dans la branche « rien à montrer » ; sinon il garde la taille de son premier cadre. Un test part de l'état vide, et on regarde une capture. L-073
- Clic de test sur un widget **sous le bas d'un `ScrollArea`** : `Node::click` est un clic de pointeur, il ne touche rien hors de la zone visible et l'échec vient au widget suivant (« No nodes found ») ; tout clic de test passe par `support::Cliquer` (`h.cliquer_label`…), qui fait défiler d'abord, et `rust/otb-gui/tests/contracts.rs` refuse le clic nu. L-114
- Clic par **libellé** dans un test ou l'exemple `capture` : si un autre widget prend ce libellé (un nom de log devenu une pastille de la barre), le clic tombe ailleurs sans erreur ; chercher les clics par ce libellé, viser par rôle, regarder les captures. L-112
- Panneau egui de **largeur fixe** : borner chaque élément à sa largeur (colonnes `min_col_width` = `max_col_width`, texte abrégé par `truncate_at_width`), sinon le panneau s'élargit et rogne le début des lignes, sans erreur ; seule la capture le montre. L-111
- Tout ce qui ouvre une fenêtre du système (boîte de fichiers…) passe par un champ remplaçable, inerte dans `App::new` ; scripts de mutations lancés sans écran ni bus de session, avec délai. L-075
- Identité d'un log : `LoadedLog::uid` (ou `LogInput::uid`), jamais l'adresse de son `Arc` (elle peut resservir à un autre log). L-072
- Calcul lancé dans un fil : chaque demande porte un numéro (`ToolPage`), un résultat qui n'a plus le dernier numéro se jette, et vider l'état compte comme une demande neuve ; le test change l'état **pendant** le calcul (barrière, une seule image), sinon il ne prouve rien. L-088

## Outillage, vérification et crochets

- Une substitution `perl`/`sed` dont le délimiteur apparaît dans le texte (fermetures Rust `|x|`, `#` et `|` du
  Markdown, `/` des chemins) écrit au mauvais endroit sans erreur (un `\|` échappé devient une alternative) : outil
  d'édition pour le multi-ligne, sinon `git diff --stat` et le début du fichier relus aussitôt. L-113
- Sur `tests/data/jeno5_5s.bbl` (coupé en plein vol), les limites automatiques couvrent tout le log : un test qui oppose une période à « limites auto » ou à « vol entier » part de bornes qui diffèrent des deux, et ne compte qu'après sa contre-preuve. L-110
- Un test qui attend une absence (« rien rangé », « rien affiché ») vérifie d'abord que le chemin heureux a eu lieu : un calcul qui échoue pour une autre raison (session 0, le décodeur compte à partir de 1) le fait passer sans rien prouver. L-115
- Un JSON qui doit rendre ses nombres au bit près se lit avec l'option `float_roundtrip` de `serde_json` : sans elle, un nombre sur quelques milliers revient avec un bit d'écart. L-116
- CodeQL classe tout nom en « uid » comme identifiant de compte et rougit dès qu'il est affiché ou écrit, empreinte comprise :
  l'identifiant de la carte s'appelle `chip_id` (`ChipId`) ; ne jamais recopier l'identifiant complet d'une carte dans le dépôt. L-109
- CodeQL prend aussi tout nom en « salt » pour un sel cryptographique (alerte critique sur un simple identifiant de liste
  egui) : l'identifiant d'un widget s'appelle `widget` ou `id`, `rust/otb-gui/tests/contracts.rs` refuse `salt`. L-117
- Un crochet ou un script de vérification ne dépend que de ce qui est sûr d'être présent sur les deux postes (pas de `jq`) :
  ce qu'il n'a pas pu vérifier se dit sur la sortie d'erreur, ce qui l'empêche de travailler échoue **visiblement** (code 2),
  jamais `exit 0` sur un chemin d'erreur — sinon il rend « tout va bien » sans rien avoir lu. L-087
- **Jamais de capture de l'écran de Cédric** : `spectacle -a` photographie la fenêtre *active* (la sienne), sans erreur visible ;
  pour voir l'interface, les captures hors écran du dépôt (`--example capture`). Image prise par erreur : la supprimer, le dire,
  ne pas la décrire. L-090
- **Un seul plan**, `docs/plan.md` : il dit ce qu'il reste à faire et garde une ligne par lot fait. Il **cite** les `W-xxx`
  sans recopier leur état (leur maître est `docs/travaux-reportes.md`, D-018), et un lot terminé ne laisse pas son plan
  derrière lui. Aucun test ne protège un plan : une copie périmée s'y installe sans rien faire rougir. L-092
- **Dossier temporaire d'un test : le nommer d'après le test**, pas seulement d'après le `pid`. Les tests d'un même fichier
  tournent en parallèle dans un seul processus, et deux tests qui partagent un dossier s'effacent leurs fichiers — l'échec se
  montre ailleurs, et seulement quand le fichier est lancé en entier. Modèle :
  `rust/otb-parity/tests/comparateurs.rs:dossier`. L-093
- **Étiquette attendue par un réglage (`dependabot.yml`) ou un workflow** : la créer sur le dépôt et la vérifier sur une vraie
  PR ; absente, elle n'est jamais posée et rien ne le dit. L-096
- **Notes générées par GitHub** : `release.yml` est lu **au commit de l'étiquette** ; une version étiquetée avant ce fichier
  sort sans rubriques, et `configuration_file_path` n'y change rien. L-097
- **Contrôle qui lit l'événement d'une PR** (`etiquette-pr.yml`) : ses passages se font dans l'ordre (`concurrency` par PR, sans
  annulation), sinon deux passages simultanés laissent l'ancien résultat gagner. L-098
- **PR de Dependabot qui monte une crate livrée** : elle arrive rouge, car `THIRD-PARTY-LICENSES.txt` cite les versions ; sur sa
  branche, `cargo run -p otb-dev -- licenses`, `verify`, commit du seul fichier (procédure dans `CONTRIBUTING.md`). L-099

## Construction des exécutables

- Fenêtre de l'application : dire comment elle s'ouvre (`with_maximized(true)` et une taille de repli `with_inner_size`), sinon
  winit l'ouvre en 800 × 600 quel que soit l'écran ; la géométrie enregistrée par eframe l'emporte ensuite. Taille, position et
  icône ne se voient ni dans un test ni dans une capture : les vérifier sur un vrai **premier** lancement. L-089
- AppImage : l'`AppRun` écrit le raccourci du menu et l'icône dans `~/.local/share` au premier lancement (sinon icône générique
  sous Wayland) ; une seule fois, jamais par-dessus un fichier existant, et un échec n'empêche pas le démarrage. W-045, L-076
- Test qui exécute un programme **qu'il vient de copier ou d'écrire** (scripts d'installation, AppImage) : réessayer sur « Text file busy »
  (`ETXTBSY`, code 26), car un test voisin du même binaire qui lance un processus fait échouer l'exécution par intermittence (L-082).
- Ancienne version Python (PyInstaller, supprimée en 7-5) : `binutils` requis sous Linux (L-020) ; exécutable Windows sans console : `sys.stdout`
  vaut `None` (L-021). Windows : construire sur le système cible.
