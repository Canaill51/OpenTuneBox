# OpenTuneBox — guide d'utilisation

## 1. Lancer le programme

- Menu des applications : **OpenTuneBox** (après `sh packaging/linux/install_linux.sh`).
- Ou dans un terminal : `~/Documents/Github/OpenTuneBox/.venv/bin/opentunebox-gui [logs…]`
- Diagnostic rapide sans interface : `opentunebox report LOG.bbl` ; résumé : `opentunebox info LOG.bbl`.
- Version autonome (sans Python) : dossier `dist/OpenTuneBox/` ; à partager **en entier**.

### Windows

L'exécutable Windows se fabrique sur un PC Windows (PyInstaller ne sait pas le faire depuis Linux) :
copier le dossier du projet sur le PC (sans `.venv`, `build`, `dist`), puis double-cliquer
`packaging\windows\construire_windows.bat`. Le script installe Python 3.13 si besoin (winget), les dépendances,
puis fabrique `dist\OpenTuneBox\OpenTuneBox.exe` et l'archive `dist\OpenTuneBox-windows.zip`.
Au premier lancement, Windows SmartScreen peut afficher « Windows a protégé votre ordinateur » (programme
non signé) : **Informations complémentaires → Exécuter quand même**.

Ouvrez un ou plusieurs fichiers `.bbl` / `.bfl` (bouton **Ouvrir…** ou glisser-déposer). Chaque session
d'un fichier (un armement = une session) apparaît comme un log séparé ; 10 logs au maximum, chacun
avec sa couleur, que l'on retrouve dans tous les outils.

## 2. Bien enregistrer

Dans le CLI Betaflight :

```
set blackbox_sample_rate = 1/2      # ≈ 2 kHz avec une boucle PID à 4 kHz (1/4 si la boucle est à 8 kHz)
set debug_mode = FFT_FREQ           # centres des notchs dynamiques + gyro avant notch
save
```

- **2 kHz** suffisent pour les PID et le bruit des drones de 3" et plus. Pour un whoop (moteurs à
  500–1400 Hz) ou pour étudier les filtres, enregistrez un vol court à 4 kHz.
- En **4.4**, préférez `debug_mode = GYRO_SCALED` (seul moyen d'avoir le gyro non filtré).
- **DShot bidirectionnel** activé : sans lui, ni filtre RPM ni fréquences moteur mesurées.

### Vols de test (20 à 40 s chacun)

| But | Comment voler |
|---|---|
| Bruit, filtres, résonances | Stationnaire puis montées lentes de gaz de 0 à 100 %, sans manœuvre brusque |
| Réponse PID (réponse indicielle) | Mouvements francs et rapides sur chaque axe, flips et rolls, retours au neutre nets ; inclure le yaw |
| Propwash | Descentes dans sa propre turbulence (split-S, gaz coupés puis remis) |
| Comparaison de réglages | Le même vol, la même batterie (charge similaire), un seul réglage changé entre deux logs |

## 3. Le visualiseur (fenêtre principale)

- **Traces** à gauche : gyro, gyro préfiltre, termes P/I/D/F, consigne, somme et erreur PID, debug ;
  en bas : moteurs (%), gaz (%), rotation des moteurs (Hz).
- **Période d'analyse** : la zone claire sur le graphique des gaz. Tous les outils n'analysent que cette
  zone. Elle est choisie automatiquement (du décollage à l'atterrissage, en s'arrêtant avant un crash) ;
  déplacez ses bords à la souris pour l'ajuster, **Période auto** pour revenir au calcul automatique.
- **Lecteur** : curseur temporel, manches et valeurs à l'instant choisi ; ▶ pour rejouer le vol.
- Zoom : molette ; déplacement : clic-glisser ; **Vue entière** pour tout revoir.
- **Tous / Roll / Pitch / Yaw** : un seul axe sur toute la hauteur.

## 4. Les outils

### Analyse spectrale
Spectres (FFT) du gyro et des termes PID : on y voit où se trouve le bruit et ce que les filtres
enlèvent (préfiltre en pointillés, filtré en trait plein). Volet « < 100 Hz » pour les oscillations et
le propwash, avec les **retards des filtres** ; volet « bruit moteur » pour l'efficacité du filtre RPM
sur chaque harmonique. Les courbes en S montrent où se situent les harmoniques moteur pendant le vol.

### Fréquence × gaz
Carte du bruit selon le gaz. Une **diagonale** qui monte avec le gaz = bruit moteur (normal, doit être
retiré par le filtre RPM). Une **ligne horizontale** = résonance de châssis ou pièce qui vibre. Une
tache basse fréquence à mi-gaz = oscillation de réglage ou propwash.

### Fréquence × temps
Le même bruit dans le temps : utile pour relier un bruit à un moment du vol (le curseur est lié au
lecteur).

### Réponse indicielle
La réaction du drone à un ordre de manche « en marche d'escalier », moyennée sur tout le vol.
Objectifs : montée rapide, **pic entre 1,02 et 1,08**, pas de rebond. Colonnes de mesures au choix :
latence, temps de montée, bande passante, pic de sensibilité (< 6 dB), marge de phase (> 45°)…
« Vue curseurs Betaflight » : position des curseurs du réglage simplifié de chaque log.

### Curseurs PID
Positions des curseurs (lues dans le log, ou estimées si le réglage simplifié est désactivé) et
simulateur « et si » : bougez un curseur, voyez les PID et filtres obtenus, copiez les commandes CLI.

### Simulateur de filtres
Reconstitue exactement la chaîne de filtres de Betaflight à partir du log. Modifiez un filtre :
le programme montre la réponse (gain, phase), le retard ajouté et le bruit qui passerait, en rejouant
le vrai gyro du vol. Copiez ensuite les commandes CLI.

### Infos de configuration
Tous les réglages des logs côte à côte ; les différences sont surlignées (« Seulement les différences »).

### Statistiques de vol
Utilisation des manches et des gaz, moyennes des termes PID, charge des moteurs, distribution de
l'erreur PID, batterie.

### Assistant de réglage
Diagnostic complet d'un log : qualité de l'enregistrement, réponse, bruit, filtres, moteurs,
mécanique. Chaque constat donne la mesure et le seuil (calibré sur ~700 vols réels de la même taille)
et propose une action. Les conseils sont des points de départ : confirmez au ressenti et à la
température des moteurs.

## 5. Méthode de réglage conseillée

1. **Mécanique d'abord** : hélices neuves, vis serrées, moteurs sains. Un log de stationnaire + montée
   de gaz ; vérifiez Fréquence × gaz et l'assistant (résonances, déséquilibre).
2. **Filtres** : réglez le filtre RPM (motor_poles juste), puis les curseurs de filtres. Visez le moins
   de retard possible tant que le terme D reste peu bruité et les moteurs tièdes.
3. **PID** avec les curseurs, un à la fois, par pas de 0,1 : Damping (D) jusqu'à un pic ≈ 1,05,
   puis Tracking (P/I) et Master pour la vivacité, Stick response (FF) pour la réactivité au manche.
4. **Comparez** : ouvrez les logs successifs ensemble (réponse indicielle, infos de configuration).
5. Revenez en arrière dès qu'un changement dégrade le ressenti, le bruit ou la température.
