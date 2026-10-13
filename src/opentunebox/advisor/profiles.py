"""Profils de taille de drone et détection automatique.

La taille se devine surtout par la fréquence de rotation des moteurs en vol stationnaire : plus l'hélice
est petite, plus le moteur tourne vite. Le nombre de cellules départage les petits drones (2 à 3,5") qui
tournent aussi vite qu'un 5" (docs/assistant.md, « Taille du drone »).
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from ..analysis.motors import hover_motor_hz
from ..log.flightlog import FlightLog


@dataclass(frozen=True)
class CraftProfile:
    key: str
    label: str
    hover_hz: tuple[float, float]  # fréquence moteur typique en stationnaire
    resonance_hz: tuple[float, float]  # zone habituelle des résonances de châssis
    latency_ms: tuple[float, float]  # latence (50 %) de réponse indicielle « normale »
    gyro_delay_max_ms: float  # au-delà, le filtrage gyro est probablement trop lourd
    dterm_delay_max_ms: float
    # seuils de bruit (dB) = 90e percentile du corpus pour cette taille (docs/assistant.md)
    gyro_noise_high_db: float = -28.5  # gyro filtré, 80–500 Hz
    dterm_noise_high_db: float = -13.5  # terme D, 80–500 Hz
    low_band_high_db: float = -15.5  # gyro filtré, 20–80 Hz
    motor_left_high_db: float = -29.5  # 1re harmonique moteur restante après filtrage
    # seuils « mécaniques » = 90e percentile, une voix par drone ; par taille si elle compte au moins
    # 20 drones, sinon commun à tout le corpus ; saturation toujours commune (docs/assistant.md)
    imbalance_high_pct: float = 13.5  # déséquilibre des moteurs
    iterm_offset_high: float = 33.0  # terme I moyen (valeur absolue, roll ou pitch)
    saturation_high_pct: float = 1.5  # temps passé avec un moteur ≥ 98 %
    roll_pitch_gap_high: float = 0.12  # écart de pic de réponse entre roll et pitch
    resonance_high_db: float = 7.9  # raie fixe au-dessus du bruit voisin
    provisional: bool = False  # trop peu de drones dans le corpus : seuils peu fiables
    # taille d'hélice représentative, pour les objectifs de retard des filtres : médiane des drones de taille connue
    # du corpus (2–3,5" : 3, sur 9 drones ; 8"+ : 8, sur 3 drones ; docs/assistant.md, 2026-10-12)
    nominal_inch: float | None = None


# Recalibration du 2026-10-05 sur 681 sessions, 94 drones, tout le corpus (docs/assistant.md, « Calibration »),
# avec la détection de taille par fréquence moteur + cellules :
# - bruit : p90 par session ; latence : p10 arrondi vers le bas, p90 vers le haut ; retards : p90 arrondi
#   au dixième supérieur ;
# - 2–3,5", 5", 7" recalculés ; whoop (11 drones) et 8"+ (6 drones) inchangés (calibration précédente :
#   un seul drone de plus y déplace le seuil de plusieurs dB) ;
# - seuils mécaniques : une voix par drone ; communs (valeurs par défaut de CraftProfile) sauf 5" et 7".
# Retards des filtres recalculés le 2026-10-05 (décision de Cédric) sur les seuls vols avec mouvements
# (consigne > 50 °/s, 573 sessions sur 681), pour toutes les tailles, whoop et 8"+ compris. Gardés quand le
# seuil est passé à 30 °/s (D-026, choix de Cédric du 2026-10-13) : le p90 par session y monte de 0,1 ms en 5"
# et en 2–3,5" à cause de 2 à 7 vols de quelques drones ; une voix par drone, le 2–3,5" ne bouge pas.
PROFILES: dict[str, CraftProfile] = {
    "whoop": CraftProfile(
        "whoop", "Tiny whoop / micro (≤ 75 mm, 1–2S)", hover_hz=(345, 900), resonance_hz=(250, 600),
        latency_ms=(13, 20), gyro_delay_max_ms=1.5, dterm_delay_max_ms=2.5,
        gyro_noise_high_db=-28.0, dterm_noise_high_db=-14.7, low_band_high_db=-13.9, motor_left_high_db=-32.2,
        provisional=True),
    "micro": CraftProfile(
        "micro", "2 à 3,5 pouces", hover_hz=(200, 345), resonance_hz=(180, 450), nominal_inch=3,
        latency_ms=(10, 21), gyro_delay_max_ms=1.8, dterm_delay_max_ms=2.6,
        gyro_noise_high_db=-29.4, dterm_noise_high_db=-14.8, low_band_high_db=-13.9, motor_left_high_db=-32.5,
        provisional=True),
    "five": CraftProfile(
        "five", "4 à 5 pouces", hover_hz=(160, 290), resonance_hz=(120, 350), nominal_inch=5,
        latency_ms=(10, 22), gyro_delay_max_ms=1.9, dterm_delay_max_ms=2.7,
        gyro_noise_high_db=-28.3, dterm_noise_high_db=-13.1, low_band_high_db=-15.7, motor_left_high_db=-28.4,
        imbalance_high_pct=14.0, iterm_offset_high=32, roll_pitch_gap_high=0.11, resonance_high_db=6.8),
    "seven": CraftProfile(
        "seven", "6 à 7 pouces", hover_hz=(115, 160), resonance_hz=(80, 220), nominal_inch=7,
        latency_ms=(15, 24), gyro_delay_max_ms=2.6, dterm_delay_max_ms=2.8,
        gyro_noise_high_db=-31.4, dterm_noise_high_db=-17.5, low_band_high_db=-21.7, motor_left_high_db=-31.8,
        imbalance_high_pct=11.2, iterm_offset_high=31, roll_pitch_gap_high=0.08, resonance_high_db=8.2),
    "large": CraftProfile(
        "large", "8 pouces et plus", hover_hz=(50, 125), resonance_hz=(40, 160), nominal_inch=8,
        latency_ms=(18, 30), gyro_delay_max_ms=3.5, dterm_delay_max_ms=3.1,
        gyro_noise_high_db=-31.0, dterm_noise_high_db=-17.0, low_band_high_db=-22.0, motor_left_high_db=-32.0,
        provisional=True),
}


def filter_delay_target_ms(inch: float | None) -> tuple[float, float] | None:
    """Objectifs de retard des filtres (gyro, D) selon la taille, graphique « Rough guide to ideal filter
    delay » de Brian White : droites de 1 ms à 2 pouces jusqu'à 4 ms (gyro) et 3 ms (D) à 14 pouces.
    Valeurs lues par PIDtoolbox, qui mesure le retard du D environ 0,5 ms plus court que nous (docs/assistant.md)."""
    if inch is None or inch < 2:
        return None
    return 1.0 + (inch - 2) * 3.0 / 12, 1.0 + (inch - 2) * 2.0 / 12


# Limites de taille, vérifiées sur 41 drones de taille connue (docs/assistant.md, « Taille du drone »)
WHOOP_HZ, MICRO_HZ, FIVE_HZ, SEVEN_HZ = 345, 290, 160, 115
SMALL_PACK_HZ = 200  # 2 à 4S au-dessus de 200 Hz : petit drone (un 5" en 4S tourne plus lentement)
# Pas de règle pour le 6S : les 3,5" en 6S (~255 Hz) recouvrent les 5" en vol freestyle (jusqu'à 275 Hz)


def profile_key(hz: float, cells: int | None = None) -> str:
    """Taille d'après la fréquence moteur médiane en vol et, si connu, le nombre de cellules."""
    if cells == 1:
        return "whoop"
    if hz >= WHOOP_HZ:
        return "whoop"
    if hz >= MICRO_HZ:
        return "micro"
    if hz >= FIVE_HZ:
        if cells is not None and cells <= 4 and hz >= SMALL_PACK_HZ:
            return "micro"
        return "five"
    if hz >= SEVEN_HZ:
        return "seven"
    return "large"


def detect_profile(log: FlightLog) -> tuple[CraftProfile, str]:
    """(profil, explication). Fréquence moteur en vol et nombre de cellules en priorité, puis nom du drone."""
    hz = hover_motor_hz(log)
    cells = log.cells
    if hz is not None:
        why = f"moteurs à {hz:.0f} Hz en vol (≈ {hz * 60:.0f} tr/min)" + (f", batterie {cells}S" if cells else "")
        return PROFILES[profile_key(hz, cells)], why
    if cells == 1:
        return PROFILES["whoop"], "batterie 1S"
    name = (log.settings.craft_name or "").lower()
    # 65 / 75 (mm) seulement comme nombre entier : « 1750 » (une capacité) n'est pas un whoop
    if re.search(r"whoop|meteor|(?<!\d)(65|75)(?!\d)", name):
        return PROFILES["whoop"], f"nom du drone « {log.settings.craft_name} »"
    # taille en pouces : 1 ou 2 chiffres (« 10in »), décimale facultative (« 3.5" »)
    m = re.search(r"(?<!\d)(\d{1,2}(?:[.,]\d)?)\s*(?:\"|in(?:ch(?:es)?)?\b|pouces?\b)", name)
    if m:
        size = float(m.group(1).replace(",", "."))
        key = "micro" if size < 3.8 else "five" if size < 5.8 else "seven" if size < 7.8 else "large"
        return PROFILES[key], f"nom du drone « {log.settings.craft_name} »"
    return PROFILES["five"], "taille inconnue : profil 5 pouces par défaut"
