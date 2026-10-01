# Assistant de réglage — fonctionnement et calibration

L'assistant (`src/opentunebox/advisor/`) transforme les mesures d'un log en constats argumentés.
Chaque constat cite la mesure et le seuil qui le déclenchent ; les actions passent d'abord par les
curseurs du réglage simplifié de Betaflight, par petits pas (0,1), une chose à la fois.

## Taille du drone

Détectée par la fréquence de rotation des moteurs en vol (eRPM), sinon par le nom du drone :
≥ 420 Hz whoop · 290–420 Hz 2–3,5" · 175–290 Hz 4–5" · 115–175 Hz 6–7" · < 115 Hz 8"+.
Le profil peut être forcé dans la fenêtre de l'assistant ou avec `opentunebox report --profil`.

## Calibration (2026-10-01)

`scripts/calibrate_advisor.py 600 stats.json` sur le corpus `~/Documents/Drones/Logs` : 708 sessions
exploitables (366 × 5", 180 × 7", 78 whoops, 64 × 2–3,5", 20 × 8"+), période automatique, roll + pitch.
Les seuils « élevé » sont le **90e percentile de chaque taille** : un drone est signalé quand il fait
partie des 10 % les plus bruités / lents de sa catégorie.

| Mesure (p10 / p50 / p90) | whoop | 2–3,5" | 5" | 7" |
|---|---|---|---|---|
| Gyro filtré 80–500 Hz (dB) | −34,1 / −31,4 / −28,4 | −37,3 / −33,2 / −27,9 | −37,8 / −33,3 / −28,5 | −39,4 / −36,9 / −30,2 |
| Gyro non filtré 80–500 Hz (dB) | −24,7 / −22,6 / −16,4 | −25,5 / −20,8 / −11,4 | −28,4 / −19,9 / −13,3 | −29,4 / −24,1 / −17,3 |
| Terme D 80–500 Hz (dB) | −23,9 / −16,7 / −11,7 | −26,8 / −21,1 / −13,0 | −27,7 / −20,4 / −13,6 | −33,4 / −24,6 / −16,2 |
| Gyro filtré 20–80 Hz (dB) | −24,1 / −20,5 / −15,6 | −27,5 / −24,1 / −12,3 | −30,0 / −24,9 / −15,3 | −32,5 / −28,5 / −21,2 |
| 1re harmonique moteur restante (dB) | −41,3 / −34,4 / −29,1 | −42,1 / −37,8 / −32,3 | −40,5 / −35,3 / −28,8 | −41,2 / −38,0 / −31,7 |
| Pic de réponse indicielle | 1,0 / 1,1 / 1,3 | 1,0 / 1,1 / 1,3 | 1,0 / 1,1 / 1,3 | 1,0 / 1,1 / 1,2 |
| Latence 50 % (ms) | 11,6 / 13,9 / 17,0 | 10,2 / 16,6 / 19,7 | 10,5 / 15,2 / 20,9 | 14,4 / 19,0 / 23,5 |
| Pic de sensibilité (dB) | 2,9 / 4,0 / 6,9 | 2,0 / 3,5 / 7,5 | 0,0 / 3,1 / 5,4 | 1,7 / 3,0 / 5,0 |
| Marge de phase (°) | 50 / 64 / 74 | 37 / 64 / 72 | 50 / 68 / 77 | 51 / 65 / 74 |
| Retard filtres gyro (ms) | 1,0 / 1,2 / 1,4 | 1,0 / 1,3 / 1,7 | 1,0 / 1,3 / 2,0 | 1,2 / 1,8 / 2,5 |
| Retard filtres D (ms) | 1,7 / 2,0 / 2,4 | 1,8 / 2,3 / 2,5 | 1,9 / 2,3 / 2,7 | 2,1 / 2,5 / 2,8 |

Unités : densité spectrale de puissance en dB de (°/s)²/Hz pour le gyro, d'unités PID²/Hz pour le D.

## Règles principales

- **Dépassement** : pic > 1,20 (attention), > 1,10 (conseil). Si la latence est plus courte que le
  p10 de la taille et que le feedforward est actif, le dépassement est attribué au feedforward.
- **Réponse molle** : pic < 0,98 et latence au-delà du p90.
- **Stabilité** : pic de sensibilité > 6 dB, ou marge de phase < 40°.
- **Bruit** : gyro filtré, terme D, bande 20–80 Hz, harmonique moteur restante > p90 de la taille.
- **Filtrage trop prudent** : retard gyro / D > p90 de la taille alors que le bruit est faible.
- **Résonance** : raie fixe ≥ 6 dB au-dessus de son voisinage, présente à tous les gaz et ne
  correspondant pas à une harmonique moteur.
- **Mécanique** : déséquilibre moteurs > 12 % (interprété avant/arrière, gauche/droite ou diagonale),
  terme I moyen décentré, saturation moteur > 5 % du temps.
