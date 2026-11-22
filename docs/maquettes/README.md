# Maquettes de l'interface

**La référence du dessin de l'interface** (D-061) : mesures, couleurs et textes de chaque écran s'y relèvent. Copie
du 2026-11-22 du canevas de Cédric « OpenTuneBox — refonte navigation et mise en page », créé le 2026-11-13, versée
dans le dépôt à sa demande (W-070). Il pourra la retirer quand l'interface sera faite.

| Fichier | Planche | Statut |
|---|---|---|
| `Application.dc.html` | Prototype complet — toute l'application | **fait foi** (D-061) |
| `Main.dc.html` | A · Modifier un fragment (extraction depuis la blackbox) | essai |
| `OngletsReponse.dc.html` | B · Onglets à deux niveaux — Réponse indicielle | essai |
| `DroneAssistant.dc.html` | C · Espace par drone — Assistant | essai |
| `TableauDeBord.dc.html` | Tableau de bord — drones et fragments | essai |
| `Spectre.dc.html` | Spectre — analyse spectrale | essai |
| `BruitFiltres.dc.html` | Bruit et filtres — espace fusionné (Spectre, × gaz, × temps, Simulateur) | essai |
| `JaugesReponse.dc.html` | Réponse indicielle — mesures en jauges | essai |
| `JaugesReponseB.dc.html` | Réponse indicielle — variante : un repère par fragment | essai |
| `canvas.json` | Index du canevas : titres, emplacement des planches, notes | — |

En cas de désaccord entre deux planches, **le prototype complet gagne** (plan, lot 30).

**Comment les lire.** Ce sont les sources des planches, pas des pages autonomes : elles ont besoin de l'outil de dessin
qui les affiche (`support.js`, absent ici). On les lit comme du texte : styles en ligne (`style="…"`) pour les
mesures et les couleurs, données d'exemple dans le bloc `<script>` de chaque planche. `Application.dc.html` est gros
(570 Ko) : y chercher un écran par son titre plutôt que de le lire en entier.

**Ce qui n'est pas copié** : les 44 images du canevas (3,8 Mo), des cartes de chaleur d'exemple des vues Fréq. × gaz
et Fréq. × temps, sans règle de dessin. Les planches y renvoient par des adresses `/_blob/…` qui ne s'ouvrent pas
hors du canevas.

Les valeurs et les noms de logs affichés sont des exemples : ils ne valent pas mesure.
