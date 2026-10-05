# Journal des versions

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; numérotation
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

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

### Corrigé
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

[Non publié]: ../../compare/v0.2.0...HEAD
[0.2.0]: ../../releases/tag/v0.2.0
[0.1.0]: ../../releases/tag/v0.1.0
