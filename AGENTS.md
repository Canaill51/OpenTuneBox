# AGENTS.md — consignes pour les agents IA

OpenTuneBox analyse les logs Blackbox Betaflight (`.bbl`/`.bfl`) pour régler les drones : décodeur natif,
analyses FFT / Wiener / filtres Betaflight, interface Qt, assistant à règles. Objectif : couvrir tous les
outils de PIDtoolbox Pro V0.94, en multiplateforme et hors ligne.

## Le propriétaire

Cédric pilote et règle des drones FPV ; il débute en programmation. **Lui répondre en français, expliquer
chaque étape simplement**, et justifier les choix techniques par leur effet concret. Il lance lui-même
les commandes qui demandent `sudo` : préparer la commande ou un script, ne pas la contourner.

## Décisions fixées (ne pas remettre en cause sans lui demander)

- Multiplateforme : Python + **PySide6** + **pyqtgraph** ; décodeur Python accéléré par **numba** (aucun binaire externe).
- Assistant **100 % hors ligne**, à règles : aucune API, aucune donnée envoyée.
- Projet **privé, tous droits réservés** (pas de licence libre).
- **Aucune copie de code PIDtoolbox** (binaire chiffré) ni d'autres projets : tout est réimplémenté à partir
  des formules du firmware Betaflight et de méthodes publiées (voir `docs/spec-*.md`).
  Seule exception, validée par Cédric : les icônes [Lucide](https://lucide.dev) (licence ISC) dans
  `gui/assets/icons/`, à garder avec leur fichier de licence.
- Interface, messages et documentation en français ; code et identifiants en anglais ; commentaires en français.

## Organisation

```
src/opentunebox/
  decoder/    décodage binaire BBL (numba, cache=True) → DecodedSession (valeurs brutes)
  log/        FlightLog (unités physiques), settings.py (en-tête normalisé par version), debug_tables.py, modes.py
  analysis/   calculs purs numpy/scipy : spectral, step (Wiener), step_series (séries de vols), loop (marges de
              stabilité), delay, bf_filters, sliders, motors, stats, period, datasets (jeux de données analysables)
  advisor/    profils de taille (profiles.py), règles (rules.py), diagnostic du bruit de Spectre (noise.py),
              diagnostic du réglage de la Réponse indicielle (step_tuning.py)
  gui/        app.py, main_window.py (visualiseur), state.py, worker.py, theme.py, icons.py, geometry.py,
              tools/ (une fenêtre par outil), widgets/, assets/
  cli.py      commandes `opentunebox info` et `opentunebox report`
tests/        pytest ; les tests marqués `corpus` utilisent de vrais logs (ignorés s'ils sont absents) ;
              test_docs.py vérifie la documentation
scripts/      validate_corpus.py (décodeur vs référence), calibrate_advisor.py (seuils de l'assistant),
              advisor_rates.py (taux de déclenchement des constats), corpus_labels.py (tableau d'étiquettes
              du corpus, passages parallèles map_files), release.py (préparer une version, notes de la page Releases)
packaging/    opentunebox.spec (PyInstaller), launcher.py, icônes (make_icons.py), installateurs :
              windows/ (Inno Setup), macos/ (.dmg), linux/ (AppImage)
docs/         guide utilisateur, architecture, validation, assistant, spécifications (index : docs/README.md)
```

Règles :
- `analysis/` et `advisor/` n'importent **jamais** `gui/` : tout calcul doit rester utilisable en ligne de commande et testable.
- `gui/` affiche seulement ce que `analysis/` calcule. Une fenêtre d'outil hérite de `gui/tools/base.py:ToolWindow`
  (`build_controls`, `snapshot_options`, `compute` en arrière-plan, `render`) et s'enregistre dans `gui/tools/__init__.py:REGISTRY`.
- Les widgets Qt ne se lisent pas dans `compute` (autre thread) : copier leurs valeurs dans `snapshot_options`.

## Commandes

```sh
python3 -m venv .venv && .venv/bin/pip install -e '.[dev]'
.venv/bin/pytest -q                                  # tous les tests (≈ 30 s avec le corpus, 1 s sans)
.venv/bin/pytest -q -m "not corpus"                  # tests rapides, sans vrais logs
.venv/bin/python -m pyflakes src tests scripts packaging/make_icons.py   # lint (doit être vide)
.venv/bin/pytest -q tests/test_docs.py               # cohérence de la documentation (< 1 s)
.venv/bin/opentunebox-gui [LOG…]                     # interface
.venv/bin/opentunebox report LOG.bbl                 # diagnostic en ligne de commande
.venv/bin/python scripts/validate_corpus.py [DOSSIER] [--limit N]   # décodeur vs blackbox_decode
.venv/bin/python scripts/corpus_labels.py                        # tableau d'étiquettes $OTB_LOGS/corpus.csv
.venv/bin/python scripts/advisor_rates.py SORTIE.json            # taux de chaque constat sur le corpus (≈ 2 min)
.venv/bin/pyinstaller packaging/opentunebox.spec --noconfirm        # exécutable autonome
.venv/bin/python scripts/release.py prepare X.Y.Z                 # préparer une version (CONTRIBUTING.md)
```

Variables d'environnement : `OTB_LOGS` (dossier de vrais logs, défaut `~/Documents/Drones/Logs` ; contient
aussi `corpus.csv`, les étiquettes des drones remplies par Cédric, à ne jamais écraser),
`OTB_REF_DECODER` (`blackbox_decode` de référence pour `validate_corpus.py`).

Tests d'interface et captures sans écran : `QT_QPA_PLATFORM=offscreen`, puis `widget.grab().save(...)` et
regarder l'image produite. **Terminer tout script Qt par `opentunebox.gui.app.hard_exit(code)`** :
PySide6 6.11 + pyqtgraph plantent (segfault) pendant le nettoyage automatique de fin de Python.

## Pièges connus

- `blackbox_decode` (référence de PIDtoolbox) compte mal `loopIteration` avec les en-têtes `P interval:N`
  (Betaflight récents) et jette les trames P qui suivent la 1re trame I d'un bloc : comparer en alignant sur
  `time`. Il accepte aussi des trames corrompues (tronquées avant un trou de carte SD, trames S parasites)
  et se bloque sur certains fichiers ; notre décodeur suit le firmware (itération = boucle PID).
- `log/flightlog.py:_drop_implausible` écarte les trames impossibles (gyro ou consigne > 4000 °/s, terme PID > ±65535) ;
  une session corrompue à plus de 50 % est ignorée.
- Numéros de `debug_mode` différents selon la version (FFT_FREQ = 17 en 4.x, 16 depuis 2025.12) → `log/debug_tables.py`.
- 4.4/4.5 : `rollPID` D = D max et `d_min` = D de base ; 2025.12+ : l'inverse avec `d_max` → normalisé dans `log/settings.py`.
- Retards de filtres : méthode de **phase** (10–100 Hz, pondérée par la cohérence), pas l'intercorrélation
  (dominée par le bruit haute fréquence). PIDtoolbox affiche un retard D 0,3 à 0,9 ms trop court (le gyro
  concorde) : ses objectifs de retard D ne s'appliquent pas tels quels.
- Retard des filtres : représentatif seulement avec des mouvements (`delay.MOVING_SETPOINT_DPS`, consigne
  > 50 °/s au moins 5 % du temps) ; en vol stationnaire, les filtres dynamiques ne travaillent qu'à un régime.
- Fréquence du log : au moins 2 kHz pour un gyro à 8 kHz, 1,6 kHz à 3,2 kHz, plus pour les petits drones
  (`rules.log_rate_needed`) ; le blackbox sous-échantillonne sans filtre, tout ce qui dépasse fs/2 se replie.
  Vol stationnaire = identifier le bruit ; vol avec mouvements = juger le retard.
- Feedforward : forcé à 0 par le firmware en mode **Angle** seulement (pas en Horizon) → masque
  `flightlog.ANGLE_MODE_MASK`, distinct de `ANGLE_HORIZON_MASK` (instrument de la boucle ouverte).
- Réponse indicielle par séries (`advisor/step_tuning.py`) : seuils tirés de 10 séries Master du corpus,
  choix des pilotes relus dans les vols suivants ; la cassure varie d'environ 0,01 entre deux vols
  identiques (incertitude jackknife plus faible : elle ne couvre que le bruit d'un vol).
- Repères en dB de PIDtoolbox (gyro −30, D −10) : nos DSP sont 6,5 dB plus hautes → `spectral.PTB_PSD_OFFSET_DB`,
  `rules.band_peak_ptb_db` (sommet du spectre lissé, sur l'échelle de PTB).
- Réponse indicielle : fenêtres de 2 s, pas de 62,5 ms, consigne min 20 °/s, régularisation 1e-4 ; pic
  cherché dans max(150 ms, 3 × latence). Bande passante calculée à partir de 2 Hz, lissage `uniform_filter1d(mode="nearest")`.
- Simulateur de filtres : méthode **relative** `bf_filters.whatif_spectrum` (spectre loggé × |H_modifié|²/|H_actuel|²
  à la cadence de la boucle PID, fenêtre par fenêtre), exacte quand rien ne change. La simulation temporelle
  `simulate_gyro` est approximative au-dessus de ~0,4 × fréquence du log.
- Seuils de l'assistant = 90e percentile du corpus par taille de drone (`docs/assistant.md`) : les recalculer
  avec `scripts/calibrate_advisor.py` (`--resume` pour les percentiles ; scripts du corpus en parallèle sur
  tous les cœurs, compétence `calculs-corpus`), ne pas les inventer. Compter **une voix
  par drone** quand quelques drones dominent une taille (whoop, seuils mécaniques) ; moins de ~20 drones →
  seuil commun ou profil provisoire. Chaque seuil a une origine notée (corpus, théorie, firmware…).
- Boucle ouverte (`analysis/loop.py`) : L = G·(Ce + Cd) **mesurée** à partir des termes P, I, D, F loggés ;
  G par estimateur instrumental (consigne, ou `rcCommand` en mode angle/horizon, plus l'excitation du debug
  CHIRP de Betaflight, ajoutée à la consigne avant le mode angle), jamais S(u, gyro)/S(u, u)
  (biaisé en boucle fermée). L = T/(1 − T) n'est qu'un repli « approché » (faux pour Betaflight : D sur le
  gyro, FF sur la consigne). La bande mesurable s'arrête vers 20–40 Hz (énergie du manche) : pic de
  sensibilité souvent minorant. L'onglet Autotune du configurateur n'est pas une source.
- Interface : style Qt **Fusion** imposé partout (`gui/theme.py`). Le style natif Windows 11 ignore la palette
  et grise les textes. Sous Fusion (Qt 6.11), les cases et boutons radio sont presque invisibles : ils sont
  redessinés par la feuille de style globale. Le délégué natif des `QComboBox` ignore cette feuille
  (texte blanc sur blanc) : `theme._ComboDelegateFix` le remplace par `QStyledItemDelegate`.
- Couleurs : jamais de couleur de texte en dur (`#777`, `gray`…) ni de taille en pixels ; utiliser
  `theme.accent_style`, `theme.muted_style` et les couleurs de `theme.current().ui`. Vérifier les deux thèmes.
- Linux : fenêtres d'ouverture / d'enregistrement par le portail XDG (`QT_QPA_PLATFORMTHEME=xdgdesktopportal`
  dans `gui/app.py`), indépendant du bureau (choix validé par Cédric). Sous KDE, la fenêtre du portail porte
  le logo de KDE et non celui d'OpenTuneBox : normal, le portail ne permet pas de le changer. Sans portail,
  Qt reprend sa fenêtre à lui, dont les icônes viennent du thème du bureau → `theme._match_icon_theme`
  choisit la variante claire / sombre. Textes de Qt en français : `qtbase_fr.qm`, seule traduction gardée
  par la recette PyInstaller.
- Une feuille de style Qt ne charge une image que depuis un fichier : icône recolorée écrite dans le dossier
  temporaire (`theme._colored_svg`).
- Pas de compilateur C sur le poste de développement : pas de dépendance à compiler.
- Linux : PyInstaller a besoin de `binutils` (`objdump`). Windows/macOS : construire sur le système cible.
- Exécutable Windows sans console (`console=False`) : `sys.stdout` et `sys.stderr` valent `None` ;
  ne jamais les utiliser sans vérifier (voir `gui/app.py:hard_exit`).

## Méthode de travail

1. Lire la doc liée à la tâche (`docs/spec-betaflight.md` pour toute formule firmware, `docs/spec-methodes.md`
   pour les analyses, `docs/architecture.md`).
2. Valider sur **de vrais logs** en plus des tests synthétiques ; recouper avec les captures PIDtoolbox
   (`PTB_FIGS_*` du corpus) quand elles existent (résultats de référence dans `docs/validation.md`).
3. **Documentation à jour à chaque modification**, dans le même commit que le code : suivre la compétence
   `.claude/skills/docs-sync` (quels documents relire selon le changement) ; `tests/test_docs.py` vérifie
   automatiquement liens, chemins, outils, modules, fichiers publiés et langue.
   `pytest` et `pyflakes` propres avant chaque commit ; mettre à jour `CHANGELOG.md` (section *Non publié*).
4. Commits en français, à l'impératif ou descriptifs, un sujet par commit. Auteur `Cedric <desmetgerry@gmail.com>`.
   Ne jamais pousser sur GitHub ni publier quoi que ce soit sans l'accord de Cédric.
5. Une branche par sujet, nommée `type/description-courte` (convention GitHub : `feat`, `fix`, `docs`,
   `refactor`, `test`, `chore` ; minuscules, tirets, anglais). La PR vers `main` a pour titre **exactement
   le nom de la branche** (détails dans `CONTRIBUTING.md`).
