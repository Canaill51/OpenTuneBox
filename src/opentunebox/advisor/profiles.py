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


PROFILES: dict[str, CraftProfile] = {
    # clé, libellé, f moteur stationnaire, résonances, latence p10–p90, retard gyro / D max (p90),
    # bruit gyro, D, 20–80 Hz, harmonique moteur (p90) — calibrés sur 708 sessions (2026-10-01)
    "whoop": CraftProfile("whoop", "Tiny whoop / micro (≤ 75 mm, 1–2S)", (380, 900), (250, 600), (11, 17),
                          1.6, 2.5, -28.4, -11.7, -15.6, -29.1),
    "micro": CraftProfile("micro", "2 à 3,5 pouces", (280, 480), (180, 450), (10, 20),
                          1.8, 2.6, -27.9, -13.0, -12.3, -32.3),
    "five": CraftProfile("five", "4 à 5 pouces", (170, 300), (120, 350), (10, 21),
                         2.1, 2.8, -28.5, -13.6, -15.3, -28.8),
    "seven": CraftProfile("seven", "6 à 7 pouces", (110, 190), (80, 220), (14, 24),
                          2.6, 3.0, -30.2, -16.2, -21.2, -31.7),
    "large": CraftProfile("large", "8 pouces et plus", (50, 125), (40, 160), (18, 30),
                          3.2, 3.5, -31.0, -17.0, -22.0, -32.0),
}


def detect_profile(log: FlightLog) -> tuple[CraftProfile, str]:
    """(profil, explication). Fréquence moteur en stationnaire en priorité, puis nom du drone."""
    hz = hover_motor_hz(log)
    if hz is not None:
        if hz >= 420:
            key = "whoop"
        elif hz >= 290:
            key = "micro"
        elif hz >= 175:
            key = "five"
        elif hz >= 115:
            key = "seven"
        else:
            key = "large"
        return PROFILES[key], f"moteurs à {hz:.0f} Hz en vol (≈ {hz * 60:.0f} tr/min)"
    name = (log.settings.craft_name or "").lower()
    if re.search(r"whoop|65|75|air ?65|meteor", name):
        return PROFILES["whoop"], f"nom du drone « {log.settings.craft_name} »"
    m = re.search(r"(\d(?:[.,]\d)?)\s*(?:\"|in|pouces?|inch)", name)
    if m:
        size = float(m.group(1).replace(",", "."))
        key = "micro" if size < 3.8 else "five" if size < 5.8 else "seven" if size < 7.8 else "large"
        return PROFILES[key], f"nom du drone « {log.settings.craft_name} »"
    return PROFILES["five"], "taille inconnue : profil 5 pouces par défaut"
