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


PROFILES: dict[str, CraftProfile] = {
    "whoop": CraftProfile("whoop", "Tiny whoop / micro (≤ 75 mm, 1–2S)", (380, 900), (250, 600), (14, 30), 2.5, 3.5),
    "micro": CraftProfile("micro", "2 à 3,5 pouces", (280, 480), (180, 450), (12, 24), 2.5, 3.5),
    "five": CraftProfile("five", "4 à 5 pouces", (170, 300), (120, 350), (10, 20), 2.8, 3.8),
    "seven": CraftProfile("seven", "6 à 7 pouces", (110, 190), (80, 220), (14, 26), 3.5, 4.5),
    "large": CraftProfile("large", "8 pouces et plus", (50, 125), (40, 160), (18, 35), 4.5, 6.0),
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
