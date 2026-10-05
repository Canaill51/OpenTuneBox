"""Profils de taille de drone et détection automatique.

La taille se devine surtout par la fréquence de rotation des moteurs en vol stationnaire :
plus l'hélice est petite, plus le moteur tourne vite (docs/spec-methodes.md, table des fréquences).
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


# Calibration du 2026-10-05 sur 707 sessions, 90 drones (docs/assistant.md, « Calibration ») :
# - bruit : p90 par session ; latence : p10 arrondi vers le bas, p90 vers le haut ; retards : p90 arrondi
#   vers le haut ;
# - whoop : une voix par drone (le corpus est dominé par quelques drones) ; 2–3,5" et 8"+ : provisoires ;
# - seuils mécaniques : valeurs par défaut de CraftProfile (communes) sauf 5" et 7" (≥ 20 drones).
PROFILES: dict[str, CraftProfile] = {
    "whoop": CraftProfile(
        "whoop", "Tiny whoop / micro (≤ 75 mm, 1–2S)", hover_hz=(345, 900), resonance_hz=(250, 600),
        latency_ms=(13, 20), gyro_delay_max_ms=2.1, dterm_delay_max_ms=3.5,
        gyro_noise_high_db=-28.0, dterm_noise_high_db=-14.7, low_band_high_db=-13.9, motor_left_high_db=-32.2,
        provisional=True),
    "micro": CraftProfile(
        "micro", "2 à 3,5 pouces", hover_hz=(290, 345), resonance_hz=(180, 450),
        latency_ms=(12, 25), gyro_delay_max_ms=1.8, dterm_delay_max_ms=2.7,
        gyro_noise_high_db=-27.3, dterm_noise_high_db=-11.5, low_band_high_db=-13.6, motor_left_high_db=-32.0,
        provisional=True),
    "five": CraftProfile(
        "five", "4 à 5 pouces", hover_hz=(170, 300), resonance_hz=(120, 350),
        latency_ms=(10, 21), gyro_delay_max_ms=2.1, dterm_delay_max_ms=2.8,
        gyro_noise_high_db=-28.5, dterm_noise_high_db=-13.6, low_band_high_db=-15.3, motor_left_high_db=-28.8,
        imbalance_high_pct=14.1, iterm_offset_high=32, roll_pitch_gap_high=0.12, resonance_high_db=7.4),
    "seven": CraftProfile(
        "seven", "6 à 7 pouces", hover_hz=(110, 190), resonance_hz=(80, 220),
        latency_ms=(14, 24), gyro_delay_max_ms=2.6, dterm_delay_max_ms=3.0,
        gyro_noise_high_db=-30.2, dterm_noise_high_db=-16.2, low_band_high_db=-21.2, motor_left_high_db=-31.7,
        imbalance_high_pct=11.0, iterm_offset_high=29, roll_pitch_gap_high=0.07, resonance_high_db=7.3),
    "large": CraftProfile(
        "large", "8 pouces et plus", hover_hz=(50, 125), resonance_hz=(40, 160),
        latency_ms=(18, 30), gyro_delay_max_ms=3.2, dterm_delay_max_ms=3.5,
        gyro_noise_high_db=-31.0, dterm_noise_high_db=-17.0, low_band_high_db=-22.0, motor_left_high_db=-32.0,
        provisional=True),
}


def profile_key_from_hz(hz: float) -> str:
    """Taille d'après la fréquence moteur médiane en vol (limites vérifiées sur le corpus)."""
    if hz >= 345:
        return "whoop"
    if hz >= 290:
        return "micro"
    if hz >= 175:
        return "five"
    if hz >= 115:
        return "seven"
    return "large"


def detect_profile(log: FlightLog) -> tuple[CraftProfile, str]:
    """(profil, explication). Fréquence moteur en stationnaire en priorité, puis nom du drone."""
    hz = hover_motor_hz(log)
    if hz is not None:
        return PROFILES[profile_key_from_hz(hz)], f"moteurs à {hz:.0f} Hz en vol (≈ {hz * 60:.0f} tr/min)"
    name = (log.settings.craft_name or "").lower()
    if re.search(r"whoop|65|75|air ?65|meteor", name):
        return PROFILES["whoop"], f"nom du drone « {log.settings.craft_name} »"
    m = re.search(r"(\d(?:[.,]\d)?)\s*(?:\"|in|pouces?|inch)", name)
    if m:
        size = float(m.group(1).replace(",", "."))
        key = "micro" if size < 3.8 else "five" if size < 5.8 else "seven" if size < 7.8 else "large"
        return PROFILES[key], f"nom du drone « {log.settings.craft_name} »"
    return PROFILES["five"], "taille inconnue : profil 5 pouces par défaut"
