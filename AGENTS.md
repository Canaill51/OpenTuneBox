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
  analysis/   calculs purs numpy/scipy : spectral, step (Wiener), delay, bf_filters, sliders, motors, stats, period
  advisor/    profils de taille (profiles.py) et règles (rules.py)
  gui/        app.py, main_window.py (visualiseur), state.py, worker.py, tools/ (une fenêtre par outil), widgets/
  cli.py      commandes `opentunebox info` et `opentunebox report`
tests/        pytest ; les tests marqués `corpus` utilisent de vrais logs (ignorés s'ils sont absents)
scripts/      validate_corpus.py (décodeur vs référence), calibrate_advisor.py (seuils de l'assistant),
              release.py (préparer une version, notes de la page Releases)
packaging/    opentunebox.spec (PyInstaller), launcher.py, icônes (make_icons.py), installateurs :
              windows/ (Inno Setup), macos/ (.dmg), linux/ (AppImage)
docs/         guide utilisateur, architecture, validation, assistant, spécifications
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
.venv/bin/python -m pyflakes src tests scripts       # lint (doit être vide)
.venv/bin/opentunebox-gui [LOG…]                     # interface
.venv/bin/opentunebox report LOG.bbl                 # diagnostic en ligne de commande
.venv/bin/python scripts/validate_corpus.py [DOSSIER] [--limit N]   # décodeur vs blackbox_decode
.venv/bin/pyinstaller packaging/opentunebox.spec --noconfirm        # exécutable autonome
.venv/bin/python scripts/release.py prepare X.Y.Z                 # préparer une version (CONTRIBUTING.md)
```

Variables d'environnement : `OTB_LOGS` (dossier de vrais logs, défaut `~/Documents/Drones/Logs`),
`OTB_REF_DECODER` (`blackbox_decode` de référence pour `validate_corpus.py`).

Tests d'interface et captures sans écran : `QT_QPA_PLATFORM=offscreen`, puis `widget.grab().save(...)` et
regarder l'image produite. **Terminer tout script Qt par `opentunebox.gui.app.hard_exit(code)`** :
PySide6 6.11 + pyqtgraph plantent (segfault) pendant le nettoyage automatique de fin de Python.

## Pièges connus

- `blackbox_decode` (référence de PIDtoolbox) compte mal `loopIteration` avec les en-têtes `P interval:N`
  (Betaflight récents) et jette les trames P qui suivent la 1re trame I d'un bloc : comparer en alignant sur
  `time`. Il accepte aussi des trames corrompues (tronquées avant un trou de carte SD, trames S parasites)
  et se bloque sur certains fichiers ; notre décodeur suit le firmware (itération = boucle PID).
- `FlightLog._drop_implausible` écarte les trames impossibles (valeurs ±2³¹) ; une session corrompue à
  plus de 50 % est ignorée.
- Numéros de `debug_mode` différents selon la version (FFT_FREQ = 17 en 4.x, 16 depuis 2025.12) → `log/debug_tables.py`.
- 4.4/4.5 : `rollPID` D = D max et `d_min` = D de base ; 2025.12+ : l'inverse avec `d_max` → normalisé dans `log/settings.py`.
- Retards de filtres : méthode de **phase** (10–100 Hz, pondérée par la cohérence), pas l'intercorrélation
  (dominée par le bruit haute fréquence).
- Réponse indicielle : fenêtres de 2 s, pas de 62,5 ms, consigne min 20 °/s, régularisation 1e-4 ; pic
  cherché dans max(150 ms, 3 × latence). Bande passante calculée à partir de 2 Hz, lissage `uniform_filter1d(mode="nearest")`.
- Simulateur de filtres : méthode **relative** `bf_filters.whatif_spectrum` (spectre loggé × |H_modifié|²/|H_actuel|²
  à la cadence de la boucle PID, fenêtre par fenêtre), exacte quand rien ne change. La simulation temporelle
  `simulate_gyro` est approximative au-dessus de ~0,4 × fréquence du log.
- Seuils de l'assistant = 90e percentile du corpus par taille de drone (`docs/assistant.md`) : les recalculer
  avec `scripts/calibrate_advisor.py`, ne pas les inventer.
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
3. `pytest` et `pyflakes` propres avant chaque commit ; mettre à jour `CHANGELOG.md` (section *Non publié*).
4. Commits en français, à l'impératif ou descriptifs, un sujet par commit. Auteur `Cedric <desmetgerry@gmail.com>`.
   Ne jamais pousser sur GitHub ni publier quoi que ce soit sans l'accord de Cédric.
5. Une branche par sujet, nommée `type/description-courte` (convention GitHub : `feat`, `fix`, `docs`,
   `refactor`, `test`, `chore` ; minuscules, tirets, anglais). La PR vers `main` a pour titre **exactement
   le nom de la branche** (détails dans `CONTRIBUTING.md`).
