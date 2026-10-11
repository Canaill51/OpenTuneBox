# 8. Finitions

Les séries ont fixé l'équilibre P / D et le niveau des gains. Reste à remettre ce qui avait été mis de
côté, à votre goût.

1. **Dérive - Oscillations (terme I)** : remontez le curseur (souvent vers 0,8 à 1,0). Trop bas, le drone
   dérive dans le vent et en virage ; trop haut, il oscille lentement après un mouvement.
2. **Réponse des sticks (feedforward)** : remontez-le progressivement. Il rend le drone plus vif aux
   manches ; trop haut, il dépasse à chaque coup de manche. Dans l'outil [Réponse indicielle](otb:tool/step),
   le pic monte avec le feedforward : c'est normal.
3. **Atténuation dynamique (D max)** : à votre goût, pour plus de D dans les mouvements rapides et le
   propwash.
4. Un **vol libre** comme d'habitude, et un vol dans le propwash (descentes dans sa propre turbulence,
   split-S, gaz coupés puis remis).

## Vérifier le résultat

- Ouvrez le dernier log dans l'[Assistant](otb:tool/advisor) : il ne doit plus signaler de bruit,
  d'oscillation ou de moteurs saturés.
- Comparez le premier et le dernier log : Configuration (bouton en bas du panneau de droite du
  [visualiseur](otb:tool/viewer)) montre tout ce qui a changé.
- Moteurs tièdes après un vol normal.
- Gardez un `diff all` de ce réglage final (onglet **Ligne de commande (CLI)**).

Les conseils sont des points de départ : votre ressenti en vol a le dernier mot.

En cas de souci : [Dépannage](otb:step/depannage).
