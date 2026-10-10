"""Diagnostic du bruit et de la stratégie de filtrage, pour la fenêtre Analyse spectrale.

Reprend les constats de l'assistant (`rules.analyze`, mêmes seuils, mêmes conseils) et les range en
quatre parties : conditions du log, gyro, terme D, stratégie. Ajoute les niveaux de bruit avant et après
filtres, axe par axe, et un bloc par filtre (filtre RPM, notch dynamique, passe-bas du gyro et du D) :
réglages, mesures, constats, et un verdict même quand aucune règle ne se déclenche (« rien à changer »
ou « impossible à juger », avec la raison). Les verdicts n'ont aucun seuil à eux : ils reprennent les
repères de rules.py.
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
    FILTER_STAGES,
    GYRO_PTB_MAX_DB,
    HARMONIC_VISIBLE_DB,
    INFO,
    OK,
    RPM_WEIGHT_GAIN_MIN,
    RPM_WEIGHT_MARGIN_DB,
    SEVERITY_ORDER,
    Finding,
    Report,
    _band_db,
    analyze,
    band_peak_ptb,
    dyn_notch_active,
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
# Blocs de filtres de la partie gyro (étapes de rules.FILTER_STAGES ; la raie fixe va au notch, qui la traite)
_STAGES = dict(FILTER_STAGES)
RPM_TITLES = _STAGES["Filtre RPM"]
NOTCH_TITLES = _STAGES["Notch dynamique"] + _STAGES["Mécanique"] + ("Raie fixe",)
GYRO_LOWPASS_TITLES = _STAGES["Passe-bas du gyro"] + ("LPF2 du gyro coupé", "Retard des filtres gyro",
                                                     "Gyro filtré bruité")  # conseil : curseur « filtre gyro »
DYN_NOTCH_UPDATE_MIN_HZ = 2000  # boucle PID minimale du notch dynamique (rules.dyn_notch_active)
RPM_HARMONICS_DEFAULT = 3  # valeur par défaut de rpm_filter_harmonics (betaflight pg/rpm_filter.c)


@dataclass
class AxisNoise:
    """Bruit d'un axe dans la bande 80 Hz → `hi`, sur l'échelle de PIDtoolbox (dB)."""
    axis: str
    before_db: float  # sommet du spectre lissé avant filtres
    after_db: float  # même chose après filtres
    peak_hz: float  # fréquence du sommet avant filtres
    reduction_db: float  # baisse du niveau moyen dans la bande (les sommets ne sont pas à la même fréquence)


@dataclass
class FilterBlock:
    """Un filtre : réglages actuels, ce que montre le log, constats de l'assistant, verdict."""
    title: str
    settings: str
    observations: list[str] = field(default_factory=list)
    findings: list[Finding] = field(default_factory=list)
    verdict: Finding | None = None  # seulement si aucun constat ne demande de changer un réglage


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
    gyro_blocks: list[FilterBlock] = field(default_factory=list)  # filtre RPM, notch dynamique, passe-bas
    gyro_other: list[Finding] = field(default_factory=list)  # constats du gyro hors blocs (niveau de bruit…)
    dterm_block: FilterBlock | None = None


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


def lowpass_text(lp) -> str:
    if not lp.enabled:
        return "coupé"
    if lp.dynamic:
        return f"{lp.type} dynamique {lp.dyn_min_hz}–{lp.dyn_max_hz} Hz"
    return f"{lp.type} {lp.static_hz} Hz"


def _actionable(findings: list[Finding]) -> bool:
    return any(f.actions and f.severity in (ATTENTION, CONSEIL) for f in findings)


def _rpm_block(log: FlightLog, nr: NoiseReport, findings: list[Finding]) -> FilterBlock:
    s = log.settings
    n = s.rpm_harmonics
    w = (list(s.rpm_weights) + [100] * 3)[:max(n, 1)]
    settings = (f"rpm_filter_harmonics = {n}, rpm_filter_weights = {', '.join(map(str, w))}, rpm_filter_q = {s.rpm_q}, "
                f"rpm_filter_min_hz = {s.rpm_min_hz}" if n else "coupé (rpm_filter_harmonics = 0)")
    b = FilterBlock("Filtre RPM", settings, findings=findings)
    if log.motor_hz is None:
        b.verdict = Finding("Filtres", CONSEIL if not s.dshot_bidir else INFO, "Filtre RPM : impossible à juger",
                            "Pas d'eRPM dans le log : sans télémétrie RPM, le filtre RPM ne fonctionne pas et les "
                            "harmoniques moteur ne peuvent pas être repérées.",
                            ["blackbox_disable_rpm = OFF (dshot_bidir est déjà activé)." if s.dshot_bidir else
                             "dshot_bidir = ON si vos ESC le permettent (BLHeli_32, Bluejay, AM32), puis "
                             f"rpm_filter_harmonics = {RPM_HARMONICS_DEFAULT}."])
        return b
    if n == 0:
        b.verdict = Finding("Filtres", CONSEIL, "Filtre RPM coupé",
                            "L'eRPM est enregistré mais le filtre RPM est coupé : c'est le filtre le plus efficace "
                            "contre le bruit moteur, pour très peu de retard.",
                            [f"rpm_filter_harmonics = {RPM_HARMONICS_DEFAULT} (valeur par défaut de Betaflight), puis "
                             "réglez les poids d'après le log suivant."])
        return b
    if nr.harmonics is None:
        b.verdict = Finding("Filtres", INFO, "Filtre RPM : efficacité non mesurable",
                            "Sans gyro non filtré, on ne voit pas les harmoniques moteur avant filtrage.",
                            ["En 4.4 : debug_mode = GYRO_SCALED. En 4.5 et plus : blackbox_disable_gyrounfilt = OFF."])
        return b
    raw, left = nr.harmonics
    for k in range(3):
        if not (np.isfinite(raw[k]) and np.isfinite(left[k])):
            continue
        state = "effacée" if left[k] <= HARMONIC_VISIBLE_DB else "encore visible"
        how = f"poids {w[k]}" if k < n else "non traitée par le filtre RPM"
        b.observations.append(f"{k + 1}e harmonique : {raw[k]:.0f} → {left[k]:.0f} dB au-dessus du bruit voisin "
                              f"({how}) : {state}.")
    if _actionable(findings):
        return b
    why = []
    if nr.report.measures.get("aliased"):
        why.append("le log est trop lent pour proposer d'alléger les poids")
    elif s.version and s.version < (4, 5):
        why.append("rpm_filter_weights n'existe qu'à partir de Betaflight 4.5")
    else:
        why.append(f"aucun poids ne peut baisser d'au moins {RPM_WEIGHT_GAIN_MIN:.1f} en gardant {RPM_WEIGHT_MARGIN_DB:.0f} dB "
                   "de marge sous le bruit voisin")
    q = ("L'assistant ne propose de monter rpm_filter_q (notchs plus étroits, moins de retard) que sur un vol avec "
         "mouvements, quand le retard des filtres gyro dépasse le repère alors que le bruit est faible")
    b.verdict = Finding("Filtres", OK, "Filtre RPM : rien à changer",
                        f"Les harmoniques traitées sont effacées (au plus {HARMONIC_VISIBLE_DB:.0f} dB au-dessus du bruit "
                        f"voisin après filtres) ; {'; '.join(why)}. {q}.")
    return b


def _notch_block(log: FlightLog, nr: NoiseReport, findings: list[Finding]) -> FilterBlock:
    s = log.settings
    m = nr.report.measures
    if s.dyn_notch_count == 0 or s.dyn_notch_max_hz == 0:
        settings = "coupé (dyn_notch_count = 0)"
    else:
        settings = (f"dyn_notch_count = {s.dyn_notch_count}, dyn_notch_q = {s.dyn_notch_q}, "
                    f"dyn_notch_min_hz = {s.dyn_notch_min_hz}, dyn_notch_max_hz = {s.dyn_notch_max_hz}")
        if s.pid_rate_hz < DYN_NOTCH_UPDATE_MIN_HZ:
            settings += f" ; inactif : boucle PID sous {DYN_NOTCH_UPDATE_MIN_HZ / 1000:.0f} kHz"
    b = FilterBlock("Notch dynamique", settings, findings=findings)
    fr = m.get("frame_resonance")
    if fr is not None and not fr[2]:
        b.observations.append(f"Raie la plus marquée quel que soit le gaz : ~{fr[0]:.0f} Hz, {fr[1]:+.0f} dB au-dessus "
                              "du bruit voisin.")
    dn = m.get("dyn_notch")
    if dn is not None:
        peaks = ", ".join(f"{f:.0f}" for f in dn["peaks"])
        b.observations.append(f"Pics à suivre après le filtre RPM : {peaks} Hz." if peaks else
                              "Aucun pic à suivre après le filtre RPM.")
    if _actionable(findings):
        return b
    rpm_on = log.motor_hz is not None and s.rpm_harmonics > 0
    if not dyn_notch_active(s):
        if rpm_on:
            b.verdict = Finding("Filtres", OK, "Notch dynamique : rien à changer",
                                "Il est coupé ; avec le filtre RPM, il ne sert qu'aux résonances qui ne viennent pas "
                                "des moteurs, et aucune n'est signalée.")
        else:
            b.verdict = Finding("Filtres", INFO, "Notch dynamique et filtre RPM coupés",
                                "Seuls les passe-bas filtrent le bruit moteur : activez d'abord le filtre RPM (bloc "
                                "ci-dessus), puis jugez le notch dynamique sur le log suivant.")
        return b
    if dn is None:
        b.verdict = Finding("Filtres", INFO, "Notch dynamique : réglage non jugé",
                            "Il faut le gyro non filtré et le filtre RPM actif : sans filtre RPM, le notch dynamique "
                            "suit aussi les raies moteur et son nombre de notchs ne se juge pas.")
        return b
    out = [f for f in dn["peaks"] if f < dn["min_hz"] or f > dn["max_hz"]]
    detail = (f"{len(dn['peaks'])} pic(s) à suivre pour {dn['count']} notch(s), plage "
              f"{dn['min_hz']}–{dn['max_hz']} Hz.")
    if len(dn["peaks"]) > dn["count"]:
        # fréquent en vol avec mouvements (les pics bougent avec les gaz) : simple remarque, pas un défaut
        detail += (" Plus de pics que de notchs : les notchs suivent les plus forts ; l'assistant propose un notch "
                   "de plus seulement si une raie reste visible après filtrage en vol stationnaire.")
    if out and nr.moving:  # en stationnaire, rules._hover_noise traite déjà chaque raie qui passe
        b.verdict = Finding("Filtres", INFO, "Notch dynamique : pic en dehors de la plage",
                            detail + " Pic à " + ", ".join(f"{f:.0f}" for f in out) + " Hz hors de la plage : l'assistant "
                            "ne propose de l'élargir que pour une raie vue en vol stationnaire ou une raie fixe.",
                            ["Vol stationnaire de 20 à 30 s pour confirmer la raie."])
    else:
        b.verdict = Finding("Filtres", OK, "Notch dynamique : rien à changer",
                            detail + " L'assistant ne propose de relever dyn_notch_min_hz (moins de retard) que sur un "
                            "vol avec mouvements, quand le retard des filtres gyro dépasse le repère alors que le bruit "
                            "est faible.")
    return b


def _lowpass_block(title: str, lps, findings: list[Finding], moving: bool, prefix: str) -> FilterBlock:
    settings = " ; ".join(f"{prefix}_{name.lower()} : {lowpass_text(lp)}" for name, lp in zip(("LPF1", "LPF2"), lps))
    b = FilterBlock(title, settings, findings=findings)
    if not _actionable(findings):
        b.verdict = Finding("Filtres", OK, f"{title} : rien à changer",
                            "Aucun constat ne demande de les modifier." if moving else
                            "Aucun constat de bruit ne demande de les modifier ; leur retard se juge sur un vol avec "
                            "mouvements.")
    return b


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
    s = log.settings
    pick = lambda prefixes: [f for f in nr.gyro_findings if f.title.startswith(prefixes)]  # noqa: E731
    nr.gyro_blocks = [_rpm_block(log, nr, pick(RPM_TITLES)), _notch_block(log, nr, pick(NOTCH_TITLES)),
                      _lowpass_block("Passe-bas du gyro", (s.gyro_lpf1, s.gyro_lpf2), pick(GYRO_LOWPASS_TITLES),
                                     moving, "gyro")]
    in_blocks = {id(f) for b in nr.gyro_blocks for f in b.findings}
    nr.gyro_other = [f for f in nr.gyro_findings if id(f) not in in_blocks]
    nr.dterm_block = _lowpass_block("Passe-bas du D", (s.dterm_lpf1, s.dterm_lpf2), nr.dterm_findings, moving, "dterm")
    verdicts = [b.verdict for b in nr.gyro_blocks if b.verdict is not None]
    if not nr.strategy:
        actionable = sorted((f for f in nr.gyro_findings + nr.dterm_findings + verdicts
                             if f.actions and f.severity in (ATTENTION, CONSEIL) and f.category != "Réponse"),
                            key=lambda f: SEVERITY_ORDER[f.severity])
        nr.strategy.append(_strategy_fallback(actionable, moving))
    return nr


def level_note(kind: str) -> str:
    """Repère de Brian White affiché sous le tableau des niveaux (échelle de PIDtoolbox)."""
    if kind == "gyro":
        return f"Repère après filtres : sous {GYRO_PTB_MAX_DB:.0f} dB."
    return f"Repère après filtres : au plus {DTERM_PTB_MAX_DB:.0f} dB, idéal sous −20 dB."

