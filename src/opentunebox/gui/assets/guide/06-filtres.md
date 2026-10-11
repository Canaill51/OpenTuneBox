# 6. Régler les filtres

Les filtres retirent le bruit du gyro (vibrations, moteurs) avant qu'il n'atteigne les PID. Trop peu de
filtrage : moteurs chauds, terme D bruité. Trop : du retard, un drone mou qui rebondit dans le propwash.
Le but est **le moins de retard possible tant que le bruit reste bas**.

## Les deux vols de test

| Vol | Comment voler | À quoi il sert |
|---|---|---|
| **Stationnaire** | 20 à 40 s de vol stationnaire, puis montées lentes des gaz de 0 à 100 %, sans manœuvre brusque | repérer le bruit : raies des moteurs, résonances du châssis |
| **Avec mouvements** | 20 à 40 s de mouvements francs : flips, rolls, changements de direction, coups de manche sur chaque axe | mesurer le retard des filtres (les filtres dynamiques ne travaillent qu'à un régime en stationnaire) |

## Lire les logs

1. [Vérifier mes logs](otb:step/verifier) : les deux logs doivent être utilisables pour le bruit, et le
   second pour le retard.
2. **Mécanique d'abord.** Ouvrez [Fréquence × gaz](otb:tool/freq_throttle) sur le vol stationnaire. Une
   **diagonale** qui monte avec les gaz = bruit des moteurs (normal, le filtre RPM le retire). Une **ligne
   horizontale** = résonance du châssis ou pièce qui vibre : à corriger mécaniquement (vis, hélice,
   moteur, antenne) avant de toucher aux filtres.
3. Ouvrez [Spectre](otb:tool/spectral) et cochez **Diagnostic du bruit** : un rapport donne, filtre par
   filtre, ce qui passe encore et ce qu'il faut changer, puis la **stratégie de filtrage** dans l'ordre.
   Repères (échelle de PIDtoolbox) : gyro filtré sous −30 dB, terme D sous −10 dB.
4. L'[Assistant](otb:tool/advisor) donne le même diagnostic, avec le reste du vol (moteurs, mécanique).

## L'ordre des réglages

Un réglage à la fois, dans cet ordre :

1. **Filtre RPM** : DShot bi-directionnel et pôles moteur justes ; harmoniques, Q et poids selon le
   diagnostic (onglet Réglages de PID, « Filtre Gyro RPM »).
2. **Notch dynamique** : nombre de notchs, Q, fréquences min et max (« Filtre coupe bande dynamique »).
   Avec un filtre RPM qui fonctionne, souvent 1 ou 2 notchs suffisent.
3. **Passe-bas du gyro** : curseur « Multiplicateur du Filtre Gyro ». Vers la droite : moins de filtrage,
   moins de retard.
4. **Passe-bas du terme D** : curseur « Multiplicateur de filtre D Term », même sens.

Avant de voler, l'outil [Filtres](otb:tool/filter_sim) montre ce que donnerait le changement : il rejoue
le vrai gyro du vol à travers les filtres modifiés (bruit qui passerait, retard ajouté).

## Après chaque changement

[Appliquez-le](otb:step/appliquer), refaites **le même vol**, ouvrez l'ancien et le nouveau log ensemble
et comparez dans le Spectre. Touchez les moteurs après le vol : tièdes, c'est bon ; chauds, revenez en
arrière.

On s'arrête quand le diagnostic ne demande plus rien (« rien à changer ») ou quand chaque pas de plus fait
remonter le bruit du terme D.

Étape suivante : [régler les PID par séries](otb:step/pid).
