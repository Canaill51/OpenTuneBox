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

### L-055 : la largeur d'une raie sur un spectre plat est une fonction discontinue du dernier bit : prouver le port sur le spectre exact de Python
- **Symptôme** — 2026-10-30 (4c-3) : sur 147 713 clés du corpus, un seul écart franc (`GF7037-3_carbon_nylon.bbl`, roll) : largeur de la 2ᵉ raie
  0 Hz en Python, 4,45 Hz en Rust, alors que tout le reste concorde à 1e-11.
- **Cause** — mesurée : le spectre masqué est plat à 1e-22 près autour de 421,5 Hz (6,9247e-08), `S[k]` dépasse la moyenne du bruit d'**1 ulp**,
  `level` se confond avec `S[k]` et les deux bords tombent sur `f[k]` (largeur 0) ; au dernier bit près la FFT de Rust donne un spectre un
  rien différent et la boucle `S[j+step] > level` s'arrête une case plus loin. Donné le spectre **exact de Python**, `half_power_width` de Rust rend 0 aussi :
  le port est correct, c'est l'entrée qui est dégénérée (raie de hauteur −1e-15 dB : elle n'existe pas).
- **Règle** — devant un écart franc sur une décision discrète, ne pas relâcher la tolérance : rejouer la **fonction seule sur l'entrée exacte de
  Python** (spectre sauvegardé) ; si elle concorde, l'écart vient du bruit de l'entrée. Dans la couche corpus, exempter **explicitement et
  compter** les largeurs de raies dont la hauteur de référence est < 1e-6 dB (ici 1 sur 413 sessions à raies). ⚠️ Conséquence pour l'assistant (phase
  5a) : `dyn_notch_q_need` ignore une largeur nulle (`w > 0`) mais accepterait 4,45 Hz et rendrait `inf` au lieu de `None` pour une raie inexistante :
  la parité des conseils (0 session différente sur 630) tranchera.
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
- **Preuve** — `rust/otb-spike-egui` (`rust/otb-spike-egui/tests/interaction.rs` échouait avant la correction, `rust/otb-spike-egui/tests/tabs.rs` produit les captures).

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
- **Preuve** — `tests/test_workflows.py:test_build_workflow_installs_what_its_tests_import` (échoue si on retire
  `scipy numba` de la ligne d'installation).

### L-036 : `PyReadonlyArray2::as_slice` accepte l'ordre Fortran et le lit comme du C : résultat faux, sans erreur
- **Symptôme** — relu avant la fusion : `_native.run_section(np.asfortranarray(x), …)` rend autre chose que
  la version rangée normalement, sans exception. Aujourd'hui `bf_filters._apply` protège l'appel, un appel direct
  (une transposée) ne l'est pas.
- **Cause** — `as_slice` demande un tableau contigu, pas contigu **ligne par ligne**.
- **Règle** — un tableau 2D passé au Rust comme `&[T]` à plat se contrôle par `is_c_contiguous()` ; un test
  avec `np.asfortranarray` accompagne chaque entrée 2D.
- **Preuve** — `rust/otb-py/src/lib.rs:run_section` ;
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
