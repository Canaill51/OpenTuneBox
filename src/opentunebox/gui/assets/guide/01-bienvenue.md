# Bienvenue dans OpenTuneBox

OpenTuneBox lit les enregistrements de vol (« logs Blackbox », fichiers `.bbl` ou `.bfl`) de votre drone
Betaflight et vous aide à le régler : moins de bruit et de moteurs chauds, des filtres juste assez forts, des
PID qui suivent vos manches sans rebond. Tout se fait sur votre ordinateur, sans connexion à Internet.

## Comment ça marche

Le réglage suit toujours la même boucle :

1. **Enregistrer** un vol court (20 à 40 s), dans des conditions précises ;
2. **Ouvrir** le log dans OpenTuneBox et lire ce que disent les outils ;
3. **Changer une seule chose** dans le configurateur Betaflight ;
4. **Revoler** de la même façon, et comparer les deux logs.

## Les trois phases

| Phase | Vols à prévoir | Ce qu'on règle |
|---|---|---|
| Mécanique et filtres | 2 à 6 vols | hélices, vibrations, filtre RPM, notch dynamique, passe-bas |
| PID par séries | 3 à 5 vols par série, 2 à 4 séries | Atténuation (Damping), Multiplicateur Maître (Master), équilibre roll / pitch |
| Finitions | 1 à 3 vols | terme I, feedforward, ressenti |

Comptez une à trois sessions de vol, avec des batteries chargées pareil.

## Lire ce guide

Suivez les étapes dans l'ordre (liste à gauche, ou bouton **Suivant**). Les liens ouvrent l'étape ou l'outil
cité, par exemple l'outil [Spectre](otb:tool/spectral). Le bouton **Guide** de la barre d'outils ramène ici à
tout moment.

Déjà des logs ? Ouvrez-les (bouton **Ouvrir**, ou glissez les fichiers dans la fenêtre), puis allez à
l'étape [Vérifier mes logs](otb:step/verifier) : elle dit à quoi chacun peut servir et quoi corriger.

Commencez par les [prérequis](otb:step/prerequis).
