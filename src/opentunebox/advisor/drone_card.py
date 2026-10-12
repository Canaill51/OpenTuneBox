"""Fiche drone du guide intégré : ce que l'utilisateur dit de son drone, et les réglages qui en découlent.

Sans Qt : la lecture et l'écriture du fichier des fiches sont dans `gui/guide.py`. Les valeurs conseillées
viennent des mêmes fonctions que l'assistant (`rules.log_rate_needed`, profils de taille).
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field

from .profiles import PROFILES
from .rules import log_rate_needed, sample_rate_cli

# Choix proposés dans la fiche : (valeur enregistrée, libellé)
BF_FAMILIES = [("4.4", "4.4"), ("4.5", "4.5"), ("2025.12", "2025.12 et plus récent")]
PID_RATES_KHZ = [(8.0, "8 kHz"), (4.0, "4 kHz"), (3.2, "3,2 kHz (gyro BMI270)"), (2.0, "2 kHz"), (1.6, "1,6 kHz")]
ESCS = [("BLHeli_32", "BLHeli_32"), ("Bluejay", "BLHeli_S avec Bluejay"), ("AM32", "AM32"),
        ("BLHeli_S", "BLHeli_S d'origine (sans Bluejay)")]
BIDIR_ESCS = {"BLHeli_32", "Bluejay", "AM32"}  # DShot bidirectionnel possible
LARGE_SIZES = {"seven", "large"}  # 6–7" et plus : autres listes et plages (Brian White [S34])

# Séries de vols (méthode de Brian White, choix de Cédric du 2026-10-12) : valeurs du curseur, vol par vol
DAMPING_SERIES = {"small": (0.6, 0.8, 1.0, 1.2, 1.4, 1.6), "large": (1.0, 1.2, 1.4, 1.6)}  # [S34]
MASTER_SERIES = (1.0, 1.2, 1.4, 1.6, 1.8, 2.0)  # arrêt au premier vol « trop haut » [S34]
PITCH_SERIES_LEN = 3  # Pitch Damping / Pitch Tracking : 3 vols dans le sens donné par le diagnostic
# Filtres généraux par défaut (identiques en 4.4, 4.5 et 2025.12+ : firmware, pg/rpm_filter.c et pg/dyn_notch.c) ;
# « Réinitialiser ce profil » du configurateur ne les remet pas
DRONE_FILTER_DEFAULTS = {
    "rpm": {"harmonics": 3, "q": 500, "min_hz": 100, "fade_hz": 50},
    "dyn_notch": {"count": 3, "q": 300, "min_hz": 100, "max_hz": 600},
}
# Finitions : plages de départ (Brian White [S34, S36], arrondies et choisies par Cédric)
I_RANGE = {"small": (0.8, 1.0), "large": (0.6, 0.8)}
FF_RANGE = {"small": (0.8, 1.2), "large": (0.5, 1.0)}


@dataclass
class DroneCard:
    name: str  # = nom du drone dans Betaflight (craft_name) : relie les logs à la fiche
    bf: str = "4.5"
    size: str = "five"  # clé de profiles.PROFILES
    pid_khz: float = 4.0  # fréquence de la boucle PID (onglet Configuration)
    esc: str = "BLHeli_32"
    done: dict[str, bool] = field(default_factory=dict)  # cases cochées à la main

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> DroneCard:
        known = {k: d[k] for k in cls.__dataclass_fields__ if k in d}
        card = cls(**known)
        if card.size not in PROFILES:
            card.size = "five"
        return card

    # ------------------------------------------------------------ valeurs conseillées
    @property
    def bidir(self) -> bool:
        return self.esc in BIDIR_ESCS

    def log_rate(self) -> tuple[float, str]:
        """Fréquence d'enregistrement conseillée (Hz) et valeur `blackbox_sample_rate` (1/N), d'après la boucle
        PID et la vitesse typique des moteurs de cette taille (milieu de `CraftProfile.hover_hz`)."""
        pid_hz = self.pid_khz * 1000.0
        lo, hi = PROFILES[self.size].hover_hz
        hz = log_rate_needed(pid_hz, (lo + hi) / 2)
        return hz, sample_rate_cli(pid_hz, hz)

    @property
    def size_group(self) -> str:
        """« small » (jusqu'à 5") ou « large » (6–7" et plus)."""
        return "large" if self.size in LARGE_SIZES else "small"

    def damping_series(self) -> tuple[float, ...]:
        return DAMPING_SERIES[self.size_group]

    def i_range(self) -> tuple[float, float]:
        return I_RANGE[self.size_group]

    def ff_range(self) -> tuple[float, float]:
        return FF_RANGE[self.size_group]

    def debug_mode(self) -> str:
        """Mode de débogage des vols de réglage : GYRO_SCALED en 4.4 (seul gyro non filtré), FFT_FREQ ensuite."""
        return "GYRO_SCALED" if self.bf == "4.4" else "FFT_FREQ"

    def angle_limit_setting(self) -> str:
        """Nom CLI de la limite d'angle : `level_limit` en 4.4, `angle_limit` ensuite."""
        return "level_limit" if self.bf == "4.4" else "angle_limit"


def card_for_log(cards: list[DroneCard], craft_name: str) -> DroneCard | None:
    """Fiche dont le nom est celui du drone dans le log (casse et espaces ignorés)."""
    key = craft_name.strip().lower()
    if not key:
        return None
    return next((c for c in cards if c.name.strip().lower() == key), None)


def pitch_series(current: float, direction: int, step: float = 0.2) -> list[float]:
    """Pitch Damping / Pitch Tracking : `PITCH_SERIES_LEN` valeurs à partir de la position actuelle, vers le
    haut (`direction` > 0) ou vers le bas, par pas de 0,2, bornées à 0,2–2,0 (plage du configurateur)."""
    sign = 1 if direction > 0 else -1
    vals = [round(current + sign * step * k, 2) for k in range(1, PITCH_SERIES_LEN + 1)]
    return [v for v in vals if 0.2 <= v <= 2.0]
