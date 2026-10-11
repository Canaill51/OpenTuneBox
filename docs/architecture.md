# Architecture

## Origine et objectif

PIDtoolbox Pro V0.94 est un logiciel MATLAB fermé et payant, disponible sous Linux seulement via le
MATLAB Runtime. OpenTuneBox couvre les mêmes outils avec un code entièrement réécrit, multiplateforme et
hors ligne, pour régler des drones de toutes tailles (whoops 65–75 mm jusqu'aux 8"+).

Le corpus de mise au point (`OTB_LOGS`, par défaut `~/Documents/Drones/Logs`) contient environ 780 sessions
(582 fichiers, Betaflight 4.4.2 → 2026.6.2, DShot bidirectionnel, majoritairement 2 kHz, debug `FFT_FREQ`)
et 638 captures PIDtoolbox (`PTB_FIGS_*`) servant de référence visuelle.

## Décisions

| Sujet | Choix | Raison |
|---|---|---|
| Interface | PySide6 (Qt) + pyqtgraph | Un seul code pour Windows/macOS/Linux ; courbes fluides sur 100 000+ points |
| Décodeur | Python accéléré par numba | `blackbox_decode` n'existe pas sur toutes les plateformes ; il ne sert qu'à valider |
| Assistant | Règles intégrées, hors ligne | Aucune donnée envoyée, résultat explicable mesure par mesure |
| Code PIDtoolbox | Aucune copie | Réimplémentation depuis le firmware Betaflight et les méthodes publiées |
| Langue | Français (interface, docs, commentaires) ; anglais (code) | Utilisateur francophone |
| Licence | Privé, tous droits réservés | Choix du propriétaire |

## Flux de données

```
fichier .bbl/.bfl
  └─ decoder/header.py      en-têtes H, définitions de trames, débuts de sessions
  └─ decoder/core.py        décodage numba des trames I/P/S/G/H/E (encodages + prédicteurs)
  └─ decoder/reader.py      DecodedSession (entiers bruts, trames S propagées)
       └─ log/flightlog.py  FlightLog : temps, gyro, consigne, P/I/D/F, moteurs, gaz, debug, tension…
                            en unités physiques ; trames impossibles écartées ; settings.py normalise l'en-tête
            └─ analysis/*   calculs purs (aucun Qt)
                 ├─ advisor/rules.py      constats + actions, seuils par profil (advisor/profiles.py)
                 ├─ advisor/noise.py      diagnostic du bruit de la fenêtre Spectre (constats de rules.py rangés
                 │                        par partie : conditions, gyro, terme D, stratégie ; niveaux avant/après)
                 ├─ advisor/step_tuning.py  diagnostic du réglage de la Réponse indicielle (séries de vols :
                 │                        équilibre P/D, Master, roll / pitch ; analysis/step_series.py)
                 ├─ advisor/log_check.py  vérification rapide des logs ouverts (à quoi sert chaque log, quoi
                 │                        corriger : configurateur d'abord, CLI en repli) pour le guide intégré
                 ├─ cli.py                opentunebox info / report
                 └─ gui/tools/*           une page par outil (calcul en arrière-plan, affichage pyqtgraph)
```

## Modules d'analyse

| Module | Rôle |
|---|---|
| `spectral.py` | PSD de Welch, fréquence × gaz (fenêtres 300 ms, 100 tranches), fréquence × temps |
| `step.py` | Réponse indicielle par déconvolution de Wiener ; métriques (pic, latence, montée, cassure, vagues), bande passante, et boucle ouverte par `loop.py` |
| `step_series.py` | Séries de vols à un seul curseur changé (Damping, Master, Pitch Tracking, Pitch Damping) et conditions du protocole (feedforward, mode Angle, curseur I) |
| `loop.py` | Boucle ouverte L = G·(Ce + Cd) mesurée à partir des termes P, I, D, F (estimateur instrumental, jackknife), repli approché T/(1 − T) ; marges de phase et de gain, croisement, pic de sensibilité |
| `delay.py` | Retards des filtres (phase pondérée par la cohérence) et consigne → gyro |
| `bf_filters.py` | Filtres Betaflight exacts (PT1/2/3, biquad, SVF, RPM, notch dynamique, LPF dynamiques), chaînes gyro et D, spectre « et si », passe-bas relevé ou coupé |
| `sliders.py` | Curseurs simplifiés ⇄ PID et filtres, commandes CLI |
| `motors.py` | Fréquences moteur depuis l'eRPM, harmoniques, centres des notchs dynamiques, bruit moteur, hauteur des harmoniques et poids RPM juste suffisant, pics à suivre par le notch dynamique, raies hors moteurs (vol stationnaire) |
| `stats.py`, `period.py` | Statistiques de vol, erreur PID robuste, période automatique, détection de crash |
| `datasets.py` | Jeux de données analysables (gyro, gyro préfiltre, terme D…, comme les listes de PIDtoolbox) |

## Interface

- `gui/state.py:AppState` : logs ouverts (10 maximum, une couleur chacun), log courant, période d'analyse ;
  ses signaux relancent les outils ouverts.
- `gui/worker.py:run_async` : calcul dans un `QThreadPool`, résultat renvoyé au thread de l'interface.
- `gui/tools/base.py:ToolWindow` : sélection des fichiers, axes R/P/Y, boutons communs, file de recalcul.
- Outils (`gui/tools/`, une page chacun dans la fenêtre principale, enregistrés dans `gui/tools/__init__.py:REGISTRY`) :

  | Bouton | Module | Fenêtre |
  |---|---|---|
  | Spectre | `spectral_window.py` | analyse spectrale (+ `_spectral_common.py`, partagé avec les deux suivantes) |
  | Fréq. × gaz (dans Spectre) | `freq_throttle.py` | fréquence × gaz, jusqu'à 4 colonnes |
  | Fréq. × temps (dans Spectre) | `freq_time.py` | spectrogrammes, curseur lié au lecteur |
  | Indicielle | `step_window.py` | réponse indicielle (+ `_step_common.py`, partagé avec les curseurs PID) |
  | Curseurs PID | `pid_slider.py` | curseurs du réglage simplifié et simulation |
  | Filtres | `filter_sim.py` | simulateur de filtres |
  | Configuration (panneau de droite) | `setup_info.py` | réglages des logs côte à côte |
  | Assistant | `advisor_panel.py` | rapport de l'assistant de réglage |
- `gui/app.py` : démarrage. `hard_exit` sort sans le nettoyage automatique de PySide6 (contournement d'un
  plantage) ; `FileOpenFilter` charge les logs ouverts depuis le Finder de macOS (`QFileOpenEvent`) ;
  sous Linux, fenêtres d'ouverture du bureau par le portail XDG.
- `gui/main_window.py` : fenêtre principale. Barre d'outils (`TOOLS` : clé, libellé, icône, couleur, info-bulle)
  dont les boutons servent d'onglets : `QStackedWidget` (`pages`), une page pour le visualiseur et une par outil
  ouvert (`open_tool`, `show_viewer`), bouton coché = page affichée (`OTHER_TOOLS` : bouton d'origine de
  Fréq. × gaz, Fréq. × temps, Configuration) ; un outil caché ne recalcule qu'à son affichage
  (`ToolWindow._stale`). Visualiseur : traces
  groupées (`TRACE_GROUPS`, traces absentes du log masquées), sections repliables Affichage et Lecteur.
- `gui/theme.py` : thèmes clair et sombre. Style Qt **Fusion** sur tous les systèmes, palette complète,
  feuille de style globale (boutons, champs, listes, cases), couleurs d'accent par thème (`accent`, `accent_style` ; aussi celles des icônes d'outils),
  textes d'aide (`muted_style`), police de l'application × 1,15. Textes de Qt en français
  (`install_qt_translations`) ; thème d'icônes du bureau accordé au thème clair / sombre (`_match_icon_theme`).
- `gui/icons.py` : icônes Lucide (`gui/assets/icons/`, licence ISC) colorées selon le thème.
- `gui/geometry.py` : taille de la fenêtre principale (95 % de l'écran au premier lancement), puis taille et
  position mémorisées (`geometry/main` dans les réglages).
- `gui/widgets/` : `section.py` (section repliable mémorisée), `axis.py` (axes à grille principale seule),
  `sticks.py` (manches du lecteur).

## Distribution

- Exécutable autonome : PyInstaller (`packaging/opentunebox.spec`, point d'entrée `packaging/launcher.py`),
  allégé des parties de Qt et de pyqtgraph inutilisées.
- Installateurs, un par système, fabriqués et essayés par `.github/workflows/construire.yml`, envoyés dans un
  brouillon de la page Releases à chaque étiquette `v…`, publiée quand les trois y sont (sans *Artifacts* :
  quota de 500 Mo d'un dépôt privé ; `scripts/release.py`, procédure dans `CONTRIBUTING.md`) :
  Windows `packaging/windows/opentunebox.iss` (Inno Setup), macOS `packaging/macos/build_dmg.sh` (.dmg),
  Linux `packaging/linux/build_appimage.sh` (AppImage, appimagetool 1.9.1, construite sur Ubuntu 22.04).
- Icônes : `packaging/make_icons.py` fabrique `.icns` (macOS) et `.png` (Linux) depuis le SVG ; `.ico` pour Windows.
- Depuis les sources : `packaging/linux/install_linux.sh` (raccourci du menu Linux) ;
  `packaging/windows/construire_windows.bat` fabrique l'exécutable sur un PC Windows.

## Hors périmètre (pour l'instant)

Logs KISS (`.TXT`) et INAV.
