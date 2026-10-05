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
                 ├─ cli.py                opentunebox info / report
                 └─ gui/tools/*           une fenêtre par outil (calcul en arrière-plan, affichage pyqtgraph)
```

## Modules d'analyse

| Module | Rôle |
|---|---|
| `spectral.py` | PSD de Welch, fréquence × gaz (fenêtres 300 ms, 100 tranches), fréquence × temps |
| `step.py` | Réponse indicielle par déconvolution de Wiener ; métriques (pic, latence, montée) et de boucle (bande passante, sensibilité, marge de phase, croisement) |
| `delay.py` | Retards des filtres (phase pondérée par la cohérence) et consigne → gyro |
| `bf_filters.py` | Filtres Betaflight exacts (PT1/2/3, biquad, SVF, RPM, notch dynamique, LPF dynamiques), chaînes gyro et D, spectre « et si » |
| `sliders.py` | Curseurs simplifiés ⇄ PID et filtres, commandes CLI |
| `motors.py` | Fréquences moteur depuis l'eRPM, harmoniques, centres des notchs dynamiques, bruit moteur |
| `stats.py`, `period.py` | Statistiques de vol, erreur PID robuste, période automatique, détection de crash |

## Interface

- `gui/state.py:AppState` : logs ouverts (10 maximum, une couleur chacun), log courant, période d'analyse ;
  ses signaux relancent les outils ouverts.
- `gui/worker.py:run_async` : calcul dans un `QThreadPool`, résultat renvoyé au thread de l'interface.
- `gui/tools/base.py:ToolWindow` : sélection des fichiers, axes R/P/Y, boutons communs, file de recalcul.
- `gui/app.py:hard_exit` : sortie sans le nettoyage automatique de PySide6 (contournement d'un plantage).
- `gui/main_window.py` : visualiseur. Barre d'outils (`TOOLS` : clé, libellé, icône, info-bulle), traces
  groupées (`TRACE_GROUPS`, traces absentes du log masquées), sections repliables Affichage et Lecteur.
- `gui/theme.py` : thèmes clair et sombre. Style Qt **Fusion** sur tous les systèmes, palette complète,
  feuille de style globale (boutons, champs, listes, cases), couleurs d'accent par thème (`accent_style`),
  textes d'aide (`muted_style`), police de l'application × 1,15.
- `gui/icons.py` : icônes Lucide (`gui/assets/icons/`, licence ISC) colorées selon le thème.
- `gui/geometry.py` : taille des fenêtres (95 % de l'écran pour la principale, 90 % pour les outils au
  premier lancement), puis taille et position mémorisées (`geometry/<fenêtre>` dans les réglages).
- `gui/widgets/` : `section.py` (section repliable mémorisée), `axis.py` (axes à grille principale seule),
  `sticks.py` (manches du lecteur).

## Distribution

- Linux : `packaging/linux/install_linux.sh` (raccourci du menu + icônes) ; PyInstaller pour un exécutable autonome.
- Windows : `packaging/windows/construire_windows.bat` sur un PC Windows.
- Installateurs (construits par `construire.yml`) : `packaging/windows/opentunebox.iss` (Inno Setup),
  `packaging/macos/build_dmg.sh` (.dmg), `packaging/linux/build_appimage.sh` (AppImage, appimagetool 1.9.1).
  Sous macOS, un log ouvert depuis le Finder arrive en `QFileOpenEvent` (`gui/app.py:FileOpenFilter`).
- Tous systèmes : workflow `.github/workflows/construire.yml`.

## Hors périmètre (pour l'instant)

Logs KISS (`.TXT`) et INAV.
