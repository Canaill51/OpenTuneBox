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

### L-025 : la couverture de tests ignore le code compilé par numba, et la CI ne voit pas le corpus
- **Symptôme** — 2026-10-12 : `pytest --cov` donne 14 % pour `decoder/core.py` avec ou sans corpus, alors
  que le décodeur tourne sur tout le corpus ; et 35 % au total sans corpus contre 86 % avec.
- **Cause** — mesurée : les fonctions `@njit(cache=True)` s'exécutent en code machine, invisible pour
  `coverage` ; et tous les tests marqués `corpus` sont ignorés sur GitHub (pas de vrais logs), y compris,
  avant cette date, des tests de fonctions pures placés dans un fichier marqué `corpus` en bloc.
- **Règle** — mesurer le décodeur avec `NUMBA_DISABLE_JIT=1` (le workflow de tests le fait sur
  `test_decoder.py` et `test_flightlog.py`) ; ne jamais marquer `corpus` un fichier entier qui contient des
  tests purs ; tout outil ou analyse doit avoir un test sur les logs synthétiques (`tests/synthetic.py`),
  le corpus ne sert qu'à vérifier des valeurs réelles. Un vrai extrait versionné (`tests/data/jeno5_5s.bbl`,
  `tests/test_real_log.py`) fait passer l'en-tête et les encodages réels en CI, numba désactivé compris.
- **Preuve** — mesure du 2026-10-12 (`pytest-cov`, installé dans `.venv` seulement) ; `tests/test_gui_tools.py`
  exerce les 8 pages d'outils sans corpus.

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
- **Cause** — mesurée : les filtres dynamiques ne travaillent qu'à un régime ; les vols stationnaires du
  corpus restent sous 25 °/s de consigne.
- **Règle** — juger le retard seulement avec des mouvements (`delay.MOVING_SETPOINT_DPS`, consigne > 50 °/s
  au moins 5 % du temps). Décision D-010.
- **Preuve** — commit `14010b4`.

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
- **Preuve** — commit `861660d`.

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

## Index rapide

| ID | Piège (et la règle, en une phrase) | Domaine |
|---|---|---|
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
| L-005 | Feedforward nul en Angle seulement. `ANGLE_MODE_MASK` | Décodeur et logs |
| L-004 | D / D min inversés selon la version. `log/settings.py` | Décodeur et logs |
| L-003 | Numéros de debug selon la version. `log/debug_tables.py` | Décodeur et logs |
| L-002 | Trames impossibles. `_drop_implausible` | Décodeur et logs |
| L-001 | `blackbox_decode` compte mal. Aligner sur `time` | Décodeur et logs |
