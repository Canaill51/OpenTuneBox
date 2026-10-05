# OpenTuneBox — guide d'utilisation

## 1. Installer

Télécharger le fichier de son système sur la page **Releases** du dépôt GitHub (colonne de droite de la
page d'accueil), section *Assets* de la dernière version. Un seul fichier par système ; Python et les
bibliothèques sont inclus.

**Windows 10 / 11** : `OpenTuneBox-Windows-Setup.exe`. Double-cliquer, puis suivre l'assistant ; en
option, raccourci sur le Bureau et ouverture des `.bbl` / `.bfl` par double-clic. Pas besoin des droits
administrateur (installation dans le dossier de l'utilisateur). Une nouvelle version s'installe
par-dessus l'ancienne ; désinstallation : *Paramètres → Applications → OpenTuneBox → Désinstaller*.
Le programme n'est pas signé : au premier lancement de l'installateur, SmartScreen peut afficher
« Windows a protégé votre ordinateur » → **Informations complémentaires → Exécuter quand même**.

**macOS** (Mac à puce Apple : M1 et suivants) : `OpenTuneBox-macOS.dmg`. L'ouvrir, puis glisser
**OpenTuneBox** sur **Applications**. L'application n'est pas signée par Apple : au premier lancement,
macOS refuse de l'ouvrir. Aller dans *Réglages Système → Confidentialité et sécurité*, en bas :
**Ouvrir quand même** (une seule fois). Un `.bbl` / `.bfl` s'ouvre ensuite dans OpenTuneBox par
double-clic (*Ouvrir avec* la première fois).

**Linux** (Ubuntu 22.04, Debian 12, Fedora 36… ou plus récent) : `OpenTuneBox-Linux.AppImage`, un seul
fichier. Le rendre exécutable (clic droit → *Propriétés* → *Autoriser l'exécution*, ou
`chmod +x OpenTuneBox-Linux.AppImage`), puis double-clic. S'il ne démarre pas (système sans FUSE) :
`./OpenTuneBox-Linux.AppImage --appimage-extract-and-run`.

Lancer OpenTuneBox depuis ses sources ou fabriquer l'exécutable soi-même : voir la partie
*Développement* du [README](../README.md#développement).

## 2. Ouvrir des logs

Bouton bleu **Ouvrir** de la barre d'outils (ou Ctrl+O), glisser-déposer dans la fenêtre, ou double-clic
sur un `.bbl` / `.bfl` (Windows et macOS, voir ci-dessus). Sous Linux, la fenêtre d'ouverture est celle
du bureau (KDE, GNOME…). Chaque session d'un fichier (un armement = une session) apparaît comme un log
séparé ; 10 logs au maximum, chacun avec sa couleur, que l'on retrouve dans tous les outils.

## 3. Bien enregistrer

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

## 4. Le visualiseur (fenêtre principale)

- **Barre d'outils** en haut : ouvrir et fermer les logs, puis un bouton par outil (Spectre, Fréq. × gaz,
  Indicielle…) ; l'icône tout à droite exporte la fenêtre en image. Survolez un bouton pour sa description.
- **Traces** à gauche, rangées par groupe (Gyro, Consigne et PID, Autres, Moteurs). Les traces que le log
  ne contient pas sont masquées.
- **Période d'analyse** : la zone claire sur le graphique des gaz. Tous les outils n'analysent que cette
  zone. Elle est choisie automatiquement (du décollage à l'atterrissage, en s'arrêtant avant un crash) ;
  déplacez ses bords à la souris pour l'ajuster, **Période auto** pour revenir au calcul automatique.
- **Lecteur** : curseur temporel, manches et valeurs à l'instant choisi ; ▶ pour rejouer le vol.
- **Affichage** (section repliable à droite) : lissage et épaisseur des traces, thème clair ou sombre.
  Les sections Affichage et Lecteur se replient d'un clic sur leur titre.
- Zoom : molette ; déplacement : clic-glisser ; **Vue entière** pour tout revoir.
- **Tous / Roll / Pitch / Yaw** : un seul axe sur toute la hauteur.

## 5. Les outils

Chaque outil s'ouvre par son bouton dans la barre d'outils (nom du bouton entre parenthèses).

### Analyse spectrale (Spectre)
Spectres (FFT) du gyro et des termes PID : on y voit où se trouve le bruit et ce que les filtres
enlèvent (préfiltre en pointillés, filtré en trait plein). Volet « < 100 Hz » pour les oscillations et
le propwash, avec les **retards des filtres** ; volet « bruit moteur » pour l'efficacité du filtre RPM
sur chaque harmonique. Les courbes en S montrent où se situent les harmoniques moteur pendant le vol.

### Fréquence × gaz (Fréq. × gaz)
Carte du bruit selon le gaz. Une **diagonale** qui monte avec le gaz = bruit moteur (normal, doit être
retiré par le filtre RPM). Une **ligne horizontale** = résonance de châssis ou pièce qui vibre. Une
tache basse fréquence à mi-gaz = oscillation de réglage ou propwash.

### Fréquence × temps (Fréq. × temps)
Le même bruit dans le temps : utile pour relier un bruit à un moment du vol (le curseur est lié au
lecteur ; un clic sur un spectrogramme le déplace).

### Réponse indicielle (Indicielle)
La réaction du drone à un ordre de manche « en marche d'escalier », moyennée sur tout le vol.
Objectifs : montée rapide, **pic entre 1,02 et 1,08**, pas de rebond. Colonnes de mesures au choix :
latence, temps de montée, bande passante, pic de sensibilité (< 7 dB), marge de phase (> 35°), marge de
gain, fréquence de croisement…
Les marges de stabilité sont **mesurées** à partir des termes P, I, D et F enregistrés dans le log (point
plein, barre d'erreur = incertitude de la mesure). Si un de ces termes manque, elles sont seulement
**approchées** (point creux) : à prendre avec prudence. Un triangle signifie « au moins cette valeur » :
le pic de sensibilité se trouve au-delà des fréquences que vos mouvements de manche permettent de mesurer.
Plus vous donnez de coups de manche francs et variés, plus la mesure va loin en fréquence.
« Vue curseurs Betaflight » : position des curseurs du réglage simplifié de chaque log.

### Curseurs PID
Positions des curseurs (lues dans le log, ou estimées si le réglage simplifié est désactivé) et
simulateur « et si » : bougez un curseur, voyez les PID et filtres obtenus, copiez les commandes CLI.

### Simulateur de filtres (Filtres)
Reconstitue exactement la chaîne de filtres de Betaflight à partir du log. Modifiez un filtre :
le programme montre la réponse (gain, phase), le retard ajouté et le bruit qui passerait, en rejouant
le vrai gyro du vol. Copiez ensuite les commandes CLI.

### Infos de configuration (Configuration)
Tous les réglages des logs côte à côte ; les différences sont surlignées (« Seulement les différences »).

### Statistiques de vol (Statistiques)
Utilisation des manches et des gaz, moyennes des termes PID, charge des moteurs, distribution de
l'erreur PID, batterie.

### Assistant de réglage (Assistant)
Diagnostic complet d'un log : qualité de l'enregistrement, réponse, bruit, filtres, moteurs,
mécanique. Chaque constat donne la mesure et le seuil (calibré sur ~700 vols réels, en comparant votre
drone à ceux de la même taille) et propose une action. Pour les tailles encore peu représentées (whoop,
2–3,5", 8" et plus), l'assistant prévient que ses seuils sont provisoires. Les conseils sont des points de départ : confirmez au ressenti et à la
température des moteurs.

## 6. Méthode de réglage conseillée

1. **Mécanique d'abord** : hélices neuves, vis serrées, moteurs sains. Un log de stationnaire + montée
   de gaz ; vérifiez Fréquence × gaz et l'assistant (résonances, déséquilibre).
2. **Filtres** : réglez le filtre RPM (motor_poles juste), puis les curseurs de filtres. Visez le moins
   de retard possible tant que le terme D reste peu bruité et les moteurs tièdes.
3. **PID** avec les curseurs, un à la fois, par pas de 0,1 : Damping (D) jusqu'à un pic ≈ 1,05,
   puis Tracking (P/I) et Master pour la vivacité, Stick response (FF) pour la réactivité au manche.
4. **Comparez** : ouvrez les logs successifs ensemble (réponse indicielle, infos de configuration).
5. Revenez en arrière dès qu'un changement dégrade le ressenti, le bruit ou la température.
