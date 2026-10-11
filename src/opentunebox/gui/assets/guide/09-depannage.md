# Dépannage

## Messages fréquents

| Message | Ce que ça veut dire | Que faire |
|---|---|---|
| Fréquence d'enregistrement trop basse | le bruit rapide se replie en fausses raies | onglet **Blackbox**, fréquence plus haute ([étape 2](otb:step/enregistrement)) |
| Gyro non filtré absent | impossible de mesurer ce que font les filtres | 4.4 : mode débug GYRO_SCALED ; 4.5 et plus : cocher « Gyro (Unfiltered) » dans les champs de débogage |
| Pas de télémétrie RPM | filtre RPM inactif, vitesse des moteurs inconnue | onglet **Moteurs** : DShot bi-directionnel (ESC compatibles) |
| Pas assez de mouvements | trop peu de coups de manche pour la réponse indicielle | 20 à 30 s de flips et rolls francs |
| Retard des filtres peu représentatif | vol stationnaire : les filtres dynamiques ne travaillent qu'à un régime | ajouter un vol avec des mouvements francs |
| Feedforward actif | il fausse la lecture d'une série PID | curseur Réponse des sticks à 0, ou vols en mode Angle |
| Curseur I hors de 0,2–0,3 | le terme I change la forme de la réponse | curseur Dérive - Oscillations entre 0,2 et 0,3 pendant les séries |
| Seuils provisoires pour cette taille | encore peu de drones de cette taille pour comparer | conseils indicatifs : confirmez au ressenti |

## Problèmes de fichiers

- **« aucune session exploitable »** : le fichier est vide (armé sans voler) ou trop abîmé. Refaites le
  vol ; vérifiez qu'il reste de la place dans la mémoire.
- **Trames corrompues** : la carte SD ou la flash n'a pas suivi par moments. Sans conséquence pour
  l'analyse ; si c'est fréquent, carte SD plus rapide ou fréquence d'enregistrement plus basse.
- **Plusieurs logs pour un seul fichier** : un log par armement. Fermez ceux qui ne servent pas (icône ✕ de
  la barre d'outils).
- **Crash détecté** : la période d'analyse s'arrête avant. Ajustez-la dans le visualiseur si besoin.
- **Log trop court** : visez 20 à 40 s de vol utile par vol de test.

## Un réglage a empiré les choses

Revenez à la valeur précédente ([Appliquer une modification](otb:step/appliquer)), ou rechargez le
`diff all` du départ. Ne changez qu'une chose à la fois : c'est la seule façon de savoir laquelle compte.

Retour au [début du guide](otb:step/bienvenue).
