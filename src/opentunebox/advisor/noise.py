"""Diagnostic du bruit et de la stratégie de filtrage, pour la fenêtre Analyse spectrale.

Reprend les constats de l'assistant (`rules.analyze`, mêmes seuils, mêmes conseils) et les range en
quatre parties : conditions du log, gyro, terme D, stratégie. Ajoute les niveaux de bruit avant et après
filtres, axe par axe, pour lire le rapport à côté des courbes.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from ..log.flightlog import AXES, FlightLog
from .profiles import CraftProfile
from .rules import (
    ATTENTION,
    CONSEIL,
    DTERM_PTB_MAX_DB,
    GYRO_PTB_MAX_DB,
    OK,
    SEVERITY_ORDER,
    Finding,
    Report,
    _band_db,
    analyze,
    band_peak_ptb,
    main_action,
    rpm_harmonic_levels,
)

# Rangement des constats par début de titre (comme rules.FILTER_STAGES). Un constat de bruit ou de
# filtre absent de ces listes tombe dans la partie gyro (test_noise_report_classifies_every_title).
CONDITION_TITLES = ("Seuils provisoires", "Retard des filtres peu représentatif")
DTERM_TITLES = ("Terme D bruité", "Bruit du terme D", "Filtrage du D insuffisant", "LPF1 du D en mode statique",
                "Filtrage du D peut-être trop fort", "Retard des filtres D")
GYRO_TITLES = ("Gyro filtré bruité", "Bruit du gyro filtré", "Retard des filtres gyro",
               "Filtrage gyro peut-être trop prudent", "Le LPF1 du gyro", "LPF2 du gyro coupé", "Biquad sur le",
               "Bruit moteur mal éliminé", "Poids du filtre RPM trop bas", "Harmonique moteur hors du filtre RPM",
               "Raie moteur encore visible", "Filtre RPM plus fort", "Raie fixe", "Raie à ", "Notch dynamique",
               "Vol stationnaire : bruit identifié", "Oscillations basses fréquences")
STRATEGY_TITLE = "Stratégie de filtrage"
NOISE_CATEGORIES = ("Bruit", "Filtres")  # en mécanique, seule la « Raie fixe » (GYRO_TITLES) concerne le bruit


@dataclass
class AxisNoise:
    """Bruit d'un axe dans la bande 80 Hz → `hi`, sur l'échelle de PIDtoolbox (dB)."""
    axis: str
    before_db: float  # sommet du spectre lissé avant filtres
    after_db: float  # même chose après filtres
    peak_hz: float  # fréquence du sommet avant filtres
    reduction_db: float  # baisse du niveau moyen dans la bande (les sommets ne sont pas à la même fréquence)


@dataclass
class NoiseReport:
    report: Report  # rapport complet de l'assistant (profil, période, mesures)
    band: tuple[float, float]
    moving: bool  # assez de mouvements pour juger le retard des filtres
    has_unfiltered: bool  # gyro non filtré dans le log
    gyro: list[AxisNoise] = field(default_factory=list)
    dterm: list[AxisNoise] = field(default_factory=list)
    harmonics: tuple[np.ndarray, np.ndarray] | None = None  # hauteur des harmoniques 1–3 avant, après filtres
    conditions: list[Finding] = field(default_factory=list)
    gyro_findings: list[Finding] = field(default_factory=list)
    dterm_findings: list[Finding] = field(default_factory=list)
    strategy: list[Finding] = field(default_factory=list)


def section_of(f: Finding) -> str | None:
    """Partie du diagnostic où ranger un constat de l'assistant (None : hors sujet)."""
    if f.title == STRATEGY_TITLE:
        return "strategy"
    if f.category == "Enregistrement" or f.title.startswith(CONDITION_TITLES):
        return "conditions"
    if f.title.startswith(DTERM_TITLES):
        return "dterm"
    if f.title.startswith(GYRO_TITLES):
        return "gyro"
    if f.category in NOISE_CATEGORIES:
        return "gyro"
    return None


def _axis_noise(raw: np.ndarray | None, filt: np.ndarray, fs: float, band, axes) -> list[AxisNoise]:
    out = []
    for a in axes:
        if not np.any(filt[a]):
            continue
        nan = float("nan")
        hz, before = band_peak_ptb(raw[a], fs, *band) if raw is not None else (nan, nan)
        drop = _band_db(raw[a], fs, *band) - _band_db(filt[a], fs, *band) if raw is not None else nan
        out.append(AxisNoise(AXES[a], before, band_peak_ptb(filt[a], fs, *band)[1], hz, drop))
    return out


def _strategy_fallback(actionable: list[Finding], moving: bool) -> Finding:
    """Moins de deux étapes de filtrage : l'assistant n'écrit pas de stratégie, on dit quoi faire quand même."""
    next_log = ("Log suivant : vol avec des mouvements francs (flips, rolls) pour juger le retard des filtres."
                if not moving else "Log suivant : même type de vol, pour comparer.")
    if not actionable:
        return Finding("Filtres", OK, "Aucun changement de filtre conseillé",
                       "Aucun constat de bruit ou de filtrage ne demande de modifier un réglage sur cette période.",
                       [next_log])
    f = actionable[0]
    return Finding("Filtres", CONSEIL, "Un seul réglage de filtre à faire",
                   f"Commencez par « {f.title} » (détail ci-dessus), puis vérifiez la température des moteurs.",
                   [main_action(f), next_log])


def noise_report(log: FlightLog, sel: slice | None = None, profile: CraftProfile | None = None) -> NoiseReport:
    sel = sel or slice(0, log.n)
    rep = analyze(log, sel, profile)
    meas = rep.measures
    hi = min(500.0, 0.95 * log.fs / 2)
    band = (80.0, hi)
    moving = bool(meas["delays"].moving)
    raw_gyro = log.gyro_unfilt[:, sel] if log.gyro_unfilt is not None else None
    nr = NoiseReport(rep, band, moving, log.gyro_unfilt is not None)
    nr.gyro = _axis_noise(raw_gyro, log.gyro[:, sel], log.fs, band, range(3))
    nr.dterm = _axis_noise(log.d_term_prefilter[:, sel], log.d_term[:, sel], log.fs, band, range(2))
    if log.gyro_unfilt is not None and log.motor_hz is not None:
        nr.harmonics = rpm_harmonic_levels(log, sel)
    parts = {"conditions": nr.conditions, "gyro": nr.gyro_findings, "dterm": nr.dterm_findings,
             "strategy": nr.strategy}
    for f in rep.sorted():
        key = section_of(f)
        if key is not None:
            parts[key].append(f)
    if not nr.strategy:
        actionable = sorted((f for f in nr.gyro_findings + nr.dterm_findings
                             if f.actions and f.severity in (ATTENTION, CONSEIL) and f.category != "Réponse"),
                            key=lambda f: SEVERITY_ORDER[f.severity])
        nr.strategy.append(_strategy_fallback(actionable, moving))
    return nr


def level_note(kind: str) -> str:
    """Repère de Brian White affiché sous le tableau des niveaux (échelle de PIDtoolbox)."""
    if kind == "gyro":
        return f"Repère après filtres : sous {GYRO_PTB_MAX_DB:.0f} dB."
    return f"Repère après filtres : au plus {DTERM_PTB_MAX_DB:.0f} dB, idéal sous −20 dB."

