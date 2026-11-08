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
- Simulateur de filtres : méthode **relative** `bf_filters.whatif_spectrum` (spectre loggé × |H_modifié|²/|H_actuel|²
  à la cadence de la boucle PID, fenêtre par fenêtre), exacte quand rien ne change. La simulation temporelle
  `simulate_gyro` est approximative au-dessus de ~0,4 × fréquence du log. D-008, L-011, L-024

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

- **Contour de la fenêtre** (barre de titre, bordures, icône) : il appartient au système, ni les tests ni les captures hors écran ne le voient — il se vérifie sur une vraie machine. egui l'habille au **premier** cadre (`sync_window_theme`), trop tôt pour Windows, qui ne repeint pas le contour d'une fenêtre qu'il vient de créer : lui retirer ce travail et annoncer le thème une fois la fenêtre posée. L-091
- Fonction paramétrée par un système (`os`, comme `guide::cards_path_in`) : appliquer les règles de ce système, jamais celles de l'hôte (`Path::is_absolute` refuse `/data` sous Windows) ; la CI Windows est le seul témoin. L-074
- Mesure de la carte graphique par `egui_kittest` : il prend par défaut le rendu logiciel (llvmpipe) et `render` relit l'image (≈ 10 ms fixes) ; choisir une vraie carte (`native_adapter_selector`), l'afficher, soustraire un écran sans courbe, décider sur le temps du processeur. L-084
- Test egui qui attend un calcul en arrière-plan : `step()` dans la boucle d'attente puis `run_steps(n)`, jamais `run()` (il panique si le fil redemande une repeinte, CI Windows). L-077
- Panneau egui redimensionnable : son contenu est toujours dans un `ScrollArea` qui remplit (`auto_shrink([false, false])`), même dans la branche « rien à montrer » ; sinon il garde la taille de son premier cadre. Un test part de l'état vide, et on regarde une capture. L-073
- Tout ce qui ouvre une fenêtre du système (boîte de fichiers…) passe par un champ remplaçable, inerte dans `App::new` ; scripts de mutations lancés sans écran ni bus de session, avec délai. L-075
- Identité d'un log : `LoadedLog::uid` (ou `LogInput::uid`), jamais l'adresse de son `Arc` (elle peut resservir à un autre log). L-072
- Calcul lancé dans un fil : chaque demande porte un numéro (`ToolPage`), un résultat qui n'a plus le dernier numéro se jette, et vider l'état compte comme une demande neuve ; le test change l'état **pendant** le calcul (barrière, une seule image), sinon il ne prouve rien. L-088

## Outillage, vérification et crochets

- Un crochet ou un script de vérification ne dépend que de ce qui est sûr d'être présent sur les deux postes (pas de `jq`) :
  ce qu'il n'a pas pu vérifier se dit sur la sortie d'erreur, ce qui l'empêche de travailler échoue **visiblement** (code 2),
  jamais `exit 0` sur un chemin d'erreur — sinon il rend « tout va bien » sans rien avoir lu. L-087
- **Jamais de capture de l'écran de Cédric** : `spectacle -a` photographie la fenêtre *active* (la sienne), sans erreur visible ;
  pour voir l'interface, les captures hors écran du dépôt (`--example capture`). Image prise par erreur : la supprimer, le dire,
  ne pas la décrire. L-090

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
