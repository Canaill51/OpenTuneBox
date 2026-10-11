# 7. Régler les PID par séries

La méthode de Brian White (auteur de PIDtoolbox) : on fait **une série de vols** où **un seul curseur
change, par pas de 0,2**, et on lit dans l'outil Réponse indicielle quelle valeur est la bonne. OpenTuneBox
reconnaît la série tout seul et dit quelle valeur garder.

Filtres réglés d'abord ([étape 6](otb:step/filtres)) : les filtres ne doivent plus changer pendant une série.

## Préparer les séries

Onglet **Réglages de PID** du configurateur (Mode Expert activé) :

- **Réponse des sticks (Stick Response)** à **0** : le feedforward gonfle la réponse et cache ce que font P
  et D. Ou bien volez les séries en **mode Angle**, où Betaflight coupe le feedforward (pas en Horizon).
- **Dérive - Oscillations (Drift - Wobble)** entre **0,2 et 0,3** : le terme I change la forme de la
  réponse. Il sera remonté à la fin.
- **Multiplicateur Maître (Master)** à **1,0** (0,8 pour un drone très puissant).

## Comment voler chaque vol de la série

20 à 30 s de **flips, rolls et coups de manche francs** sur le roll et le pitch, retours au neutre nets.
Le même vol à chaque fois, même batterie, même charge. Au moindre bruit d'oscillation, atterrissez et
revenez à la valeur précédente. Touchez les moteurs après chaque vol.

Nommez chaque fichier d'après la valeur (`damping-0.8.bbl`…).

## Les séries, dans l'ordre

1. **Atténuation (Damping)**, l'équilibre P / D : par exemple 0,6 – 0,8 – 1,0 – 1,2 – 1,4. On garde la
   première valeur **sans dépassement visible** sur le roll, ou celle au-delà de laquelle le dépassement ne
   baisse plus.
2. **Atténuation du Pitch (Pitch Damping)**, seulement si le diagnostic le demande (le pitch, plus lourd,
   dépasse encore) : même lecture, sur le pitch.
3. **Multiplicateur Maître (Master)**, le niveau des gains : 1,0 – 1,2 – 1,4 – … On monte tant que la
   réponse devient plus rapide **sans cassure** (la montée s'arrête un instant avant la consigne, puis
   reprend lentement) ni **vagues** sur le palier. Au premier vol « trop haut », on garde la valeur d'avant.
4. **Suivi du Pitch (Pitch Tracking)** : il aligne la vitesse de réponse du pitch sur celle du roll. Écart
   visé sous 1 ms, acceptable jusqu'à 2 ms.

## Lire une série

1. Ouvrez **tous les logs de la série** ensemble.
2. [Vérifier mes logs](otb:step/verifier) confirme que chaque vol convient et que la série ne change qu'un
   curseur.
3. Ouvrez [Réponse indicielle](otb:tool/step) : le **Diagnostic du réglage**, à côté des courbes, dit quelle
   valeur garder ; dans les colonnes de mesures, un cercle entoure le vol conseillé.
4. [Appliquez](otb:step/appliquer) cette valeur, puis passez à la série suivante.

Étape suivante : [finitions](otb:step/finitions).
