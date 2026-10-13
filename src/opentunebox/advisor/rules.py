"""Assistant de réglage : mesures → constats argumentés → suggestions.

Principes :
- chaque constat cite la mesure qui le justifie (valeur, seuil, axe) ;
- les suggestions passent de préférence par les curseurs du réglage simplifié de Betaflight
  (une seule chose à la fois ; pas choisis par Cédric : 0,2 pour dégrossir, 0,1 pour le réglage fin,
  séries de 0,5 pour le feedforward ; docs/assistant.md « Pas des curseurs »), puis par des réglages précis ;
- les seuils dépendent de la taille du drone (profiles.py) et sont calibrés sur 681 sessions réelles
  (90e percentile de chaque taille, voir docs/assistant.md « Calibration »).
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, field

import numpy as np

from ..analysis import bf_filters, spectral
from ..analysis.delay import MOVING_SETPOINT_DPS, compute_delays
from ..analysis.motors import (dyn_notch_targets, harmonic_prominence, hover_motor_hz, line_heights, line_shapes, motor_noise,
                               rpm_weight_needed, spectrum_lines)
from ..analysis.period import crash_time
from ..analysis.sliders import slider_view
from ..analysis.stats import flight_stats, pid_error_stats
from ..analysis.step import StepResult, step_response
from ..analysis.step_series import FF_KEY, flight_conditions
from ..log.flightlog import AXES, FlightLog
from .profiles import CraftProfile, detect_profile, filter_delay_target_ms

# Stabilité (boucle ouverte mesurée), en conseil : Ms ≤ ~2 (6 dB) et marge 30–60° dans les manuels ;
# 7 dB et 35° retenus sur le corpus du 2026-10-05 (20 % des sessions au lieu de 33 %, docs/assistant.md)
SENSITIVITY_HIGH_DB = 7.0
# Pic de réponse indicielle (conseil, attention), communs à toutes les tailles (moins de 20 drones par taille
# et par groupe) : p90 (une voix par drone) et p95 (par axe) du corpus du 2026-10-05, séparés selon le
# feedforward, qui gonfle le pic (docs/assistant.md). Repère sans dépassement visible : série BlackSummer.
STEP_PEAK_HIGH = (1.15, 1.25)
STEP_PEAK_HIGH_FF = (1.40, 1.55)
STEP_PEAK_FLAT = 1.03
# Réponse molle : pic sous le p10 des axes du corpus (1,008 sans feedforward, 1,012 avec, 2026-10-12) et latence
# au-dessus du p90 de la taille
STEP_PEAK_SOFT = 1.01
# Fenêtres de la réponse indicielle : lue dès 20 par axe, « mesure moins sûre » sous 200 [S35]. Corpus du
# 2026-10-12 (tranches de n fenêtres de 646 axes, comparées au vol entier) : écart du pic p90 0,20 à 20
# fenêtres, 0,07 à 200 (docs/assistant.md, « Pic de réponse indicielle »)
STEP_MIN_WINDOWS, STEP_SURE_WINDOWS = 20, 200
# Feedforward : pas de pas chiffré dans un conseil à un seul vol, renvoi à la série du guide, par pas de 0,5
# depuis 0 (choix de Cédric, 2026-10-12 ; docs/assistant.md « Pas des curseurs »)
FF_SERIES_ADVICE = ("série Feedforward du guide (curseur « Réponse des sticks » : 0, 0,5, 1… en Acro), en comparant "
                    "le retard consigne → gyro d'un vol à l'autre")
PHASE_MARGIN_LOW_DEG = 35.0
# D dominant au croisement (|Ĉd/Ĉe|) : au-delà, la marge faible ne se corrige plus par du D. Séries Damping du
# corpus (2026-10-12, 130 crans de +0,2, 25 drones, une voix par drone) : dès 1,0, un cran de D fait monter le
# pic de sensibilité (+0,14 dB, intervalle à 90 % +0,03 ; +0,26) ; la marge de phase ne gagne plus que 1 à 3°
D_DOMINANT_RATIO = 1.0
# Repères de Brian White sur l'échelle des spectres de PIDtoolbox (docs/spec-methodes.md, [S20]–[S22]) :
# terme D au plus −10 dB (déclenche le constat, avec le p90 du corpus), gyro filtré sous −30 dB (affiché)
DTERM_PTB_MAX_DB = -10.0
# Terme D « idéalement sous −20 dB » [S20] : en dessous, le simulateur cherche jusqu'où relever ses filtres
# (décision de Cédric, 2026-10-10) ; jusqu'à 1,2, borne du curseur « filtre D » hors mode expert du
# configurateur (useTuningSliders.ts), ou +0,2 au plus au-delà ; sans curseur, coupures × 1,1 par pas (un cran
# de 0,1 depuis 1,0), deux pas au plus. Limite de prudence (2026-10-13, choix de Cédric) : simulé, le D resterait
# sous les repères jusqu'à 6 crans, mais la chauffe des moteurs ne se voit pas dans le log
DTERM_PTB_IDEAL_DB = -20.0
DTERM_SLIDER_BASIC_MAX = 120
DTERM_LIFT_MAX_STEPS = 2
GYRO_PTB_MAX_DB = -30.0
# Filtre RPM (doc Betaflight 4.5, [S27]) : monter Q par pas jusqu'à 1000 au plus pour gagner du retard,
# pas de 100 dans les deux sens (décision de Cédric, 2026-10-10) ;
# poids (4.5 et plus) juste suffisant pour que la raie finisse 3 dB sous le bruit voisin à la sortie du filtre RPM,
# proposé s'il baisse d'au moins 0,5 ; raie « visible » à plus de 4 dB du bruit voisin. Calibration du 2026-10-13
# (choix de Cédric) : sur 1 186 harmoniques dont le pilote a baissé le poids, la raie reste invisible après tous
# les filtres dans plus de 99 % des cas jusqu'à 0 dB prévu (4,4 % de +3 à +6 dB) ; 3 dB garde un coussin pour les
# écarts d'un vol à l'autre. Baisse de 0,5 = environ 0,17 ms de retard en moins sur la 2e harmonique, 0,11 ms sur
# la 3e (4 moteurs, modèle des filtres du firmware) (docs/assistant.md, « Filtre RPM : poids et Q »)
RPM_Q_STEP, RPM_Q_MAX = 100, 1000
RPM_Q_MIN = 250  # borne basse de rpm_filter_q dans le firmware (cli/settings.c)
RPM_WEIGHT_MARGIN_DB = 3.0
RPM_WEIGHT_GAIN_MIN = 0.5
# Calibration du 2026-10-12 (704 sessions) : la même mesure prise là où il n'y a aucune raie (0,45 et 0,6 ×
# la fréquence moteur, gyro filtré, maximum du roll et du pitch) dépasse 3 dB dans 1,4 à 2,8 % des sessions,
# 4 dB dans 0,3 à 0,9 % : 4 dB garde les fausses alertes sous 1 % (docs/assistant.md, « Filtre RPM : poids et Q »)
HARMONIC_VISIBLE_DB = 4.0
# Notch dynamique (notes de réglage Betaflight 4.3, [S19]) : plage qui englobe la résonance à 20 Hz près, le moins
# de notchs possible. Pas de relèvement de dyn_notch_min_hz : en vol, les notchs ne sont presque jamais près de leur
# minimum (debug FFT_FREQ, 598 sessions, 2026-10-13) ; +30 Hz ne fait gagner que 0,002 ms de retard en médiane
# (choix de Cédric ; docs/assistant.md, « Notch dynamique »)
DYN_NOTCH_MARGIN_HZ = 20
DYN_NOTCH_MIN_HZ = 20  # borne basse de dyn_notch_min_hz dans le firmware (cli/settings.c)
DYN_NOTCH_MAX_MIN_HZ, DYN_NOTCH_MAX_MAX_HZ = 200, 1000  # bornes de dyn_notch_max_hz
# Q du notch dynamique avec filtre RPM : 300–450 [S19] ; pas de 50 (décision de Cédric, 2026-10-10).
# Q calculé d'après la forme des raies à suivre (notch SVF de Betaflight, `common/filter.c`, largeur à −3 dB
# = fréquence / Q [S31], Q × 100) : le plus haut Q qui atténue chaque raie de sa hauteur moins 4 dB
# (HARMONIC_VISIBLE_DB) sur sa demi-largeur à mi-puissance ; le firmware vise le pic par interpolation
# parabolique (`flight/dyn_notch_filter.c`). Arrondi à 50 vers le bas, 250 au moins (2026-10-13, choix de Cédric :
# plus bas Q vu avec filtre RPM, presets officiels 250 et corpus 285 ; le calcul ne descend jamais sous 112 dans
# le corpus) ; hausse jusqu'à 450 au plus [S19]
DYN_NOTCH_Q_RPM = (300, 450)
DYN_NOTCH_Q_STEP = 50
DYN_NOTCH_Q_LOW = 250
# Notch statique : même calcul, Q de 1 au moins (anciens défauts de Betaflight 3.2.0 : Q 0,67 à 1,71)
STATIC_NOTCH_Q_LOW = 100
# Vol stationnaire : raies hors moteurs listées dès 6 dB au-dessus du bruit voisin (2026-10-12 : du bruit sans
# raie simulé sur les vols stationnaires du corpus ne dépasse jamais 3 dB ; 6 dB fixe le nombre de conseils,
# choix de Cédric) ; sous 100 Hz, prudence avant de les filtrer (bande de travail du PID, propwash ~20–100 Hz [S1])
HOVER_LINE_MIN_DB = 6.0
# Notch statique conseillé (gyro_notch1) : coupure tirée du Q nécessaire pour la raie, même calcul et mêmes bornes
# que le Q du notch dynamique (dyn_notch_q_need, STATIC_NOTCH_Q_LOW à DYN_NOTCH_Q_RPM[1], soit 0,62 à 0,89 × la
# fréquence) ; firmware : Q = f·c / (f² − c²) (`filterGetNotchQ`, common/filter.c). Sans mesure de la raie (pas de
# gyro non filtré ou d'eRPM, raie sous 4 dB dans le spectre moyen) : 0,7, entre les anciens défauts de Betaflight 3.2.0 (gyro 400 / 300 et
# 200 / 100 Hz, D 260 / 160 Hz : 0,5 à 0,75) ; choix de Cédric, 2026-10-12
STATIC_NOTCH_FALLBACK = 0.7
HOVER_LOW_LINE_HZ = 100.0

# Niveaux de gravité
INFO, OK, CONSEIL, ATTENTION, ALERTE = "info", "ok", "conseil", "attention", "alerte"
SEVERITY_ORDER = {ALERTE: 0, ATTENTION: 1, CONSEIL: 2, INFO: 3, OK: 4}

@dataclass
class Finding:
    category: str  # Enregistrement, Réponse, Bruit, Filtres, Moteurs, Mécanique…
    severity: str
    title: str
    detail: str
    actions: list[str] = field(default_factory=list)
    axis: str | None = None


@dataclass
class AxisStep:
    axis: str
    result: StepResult


@dataclass
class Report:
    profile: CraftProfile
    profile_reason: str
    period: tuple[float, float]
    findings: list[Finding]
    steps: list[AxisStep]
    measures: dict

    def sorted(self) -> list[Finding]:
        return sorted(self.findings, key=lambda f: SEVERITY_ORDER.get(f.severity, 9))


def gaps_text(n_gaps: int, lost_s: float, span_s: float) -> str:
    """« 3 trous dans la période analysée (0,4 s perdues, 1,2 %) »."""
    pct = 100 * lost_s / (span_s + lost_s) if span_s + lost_s > 0 else 0.0
    return (f"{n_gaps} trou{'s' if n_gaps > 1 else ''} dans la période analysée "
            f"({lost_s:.1f} s perdues, {pct:.1f} %)").replace(".", ",")


def _band_db(x: np.ndarray, fs: float, lo: float, hi: float, breaks: np.ndarray | None = None) -> float:
    s = spectral.spectrum(x, fs, resolution_hz=2.0, breaks=breaks)
    hi = min(hi, 0.95 * fs / 2)
    m = (s.freqs >= lo) & (s.freqs <= hi)
    return float(np.mean(s.values[m])) if m.any() else float("nan")


def band_peak_ptb(x: np.ndarray, fs: float, lo: float, hi: float,
                  breaks: np.ndarray | None = None) -> tuple[float, float]:
    """Sommet du spectre lissé dans la bande : (fréquence en Hz, niveau sur l'échelle de PIDtoolbox)."""
    s = spectral.spectrum(x, fs, resolution_hz=2.0, smoothing=2, breaks=breaks)
    hi = min(hi, 0.95 * fs / 2)
    m = (s.freqs >= lo) & (s.freqs <= hi)
    if not m.any():
        return float("nan"), float("nan")
    k = int(np.argmax(s.values[m]))
    return float(s.freqs[m][k]), float(s.values[m][k]) - spectral.PTB_PSD_OFFSET_DB


def band_peak_ptb_db(x: np.ndarray, fs: float, lo: float, hi: float, breaks: np.ndarray | None = None) -> float:
    """Sommet du spectre lissé dans la bande, sur l'échelle de PIDtoolbox (ce que l'œil lit sur sa courbe)."""
    return band_peak_ptb(x, fs, lo, hi, breaks)[1]


def analyze(log: FlightLog, sel: slice | None = None, profile: CraftProfile | None = None) -> Report:
    sel = sel or slice(0, log.n)
    reason = "choisi par l'utilisateur"
    if profile is None:
        profile, reason = detect_profile(log)
    F: list[Finding] = []
    if profile.provisional:
        F.append(Finding("Réglage", INFO, "Seuils provisoires pour cette taille",
                         f"Le corpus de calibration compte trop peu de drones « {profile.label} » : les seuils de "
                         "bruit et de mécanique sont indicatifs."))
    meas: dict = {}
    fs = log.fs
    s = log.settings
    t0, t1 = float(log.t[sel.start]), float(log.t[min(sel.stop, log.n) - 1])
    br = log.breaks(sel)  # raccords : aucune fenêtre d'analyse ne les chevauche (D-020)

    # ------------------------------------------------------------ 1. qualité de l'enregistrement
    hz = hover_motor_hz(log)
    meas["hover_motor_hz"] = hz
    base_hz = log_rate_needed(s.pid_rate_hz)
    harm_hz = log_rate_needed(s.pid_rate_hz, hz)
    meas["log_rate_needed_hz"] = (base_hz, harm_hz)
    aliased = fs < 0.95 * base_hz  # spectre replié : on n'allège aucun filtre sur cette base
    loops = f"gyro à {s.gyro_rate_hz / 1000:.1f} kHz, boucle PID à {s.pid_rate_hz / 1000:.1f} kHz"
    filter_fix = [f"Pour un vol de réglage des filtres : blackbox_sample_rate = "
                  f"{sample_rate_cli(s.pid_rate_hz, max(base_hz, harm_hz))} ({max(base_hz, harm_hz):.0f} Hz)."]
    pid_hz = pid_log_rate_needed(s.pid_rate_hz)
    if fs < 0.95 * PID_LOG_RATE_MIN_HZ:
        F.append(Finding("Enregistrement", ATTENTION, "Fréquence d'enregistrement trop basse",
                         f"Le log est enregistré à {fs:.0f} Hz ({loops}) : il faut au moins {PID_LOG_RATE_MIN_HZ:.0f} Hz "
                         f"pour régler les PID et {base_hz:.0f} Hz pour juger les filtres. Le blackbox garde un "
                         f"échantillon sur N sans filtre anti-repliement : tout bruit au-dessus de {fs / 2:.0f} Hz se "
                         "replie dans le spectre (fausses raies).",
                         [f"Pour un vol de réglage des PID : blackbox_sample_rate = "
                          f"{sample_rate_cli(s.pid_rate_hz, pid_hz)} ({pid_hz:.0f} Hz).", *filter_fix]))
    elif fs < 0.95 * base_hz:
        F.append(Finding("Enregistrement", CONSEIL, "Fréquence d'enregistrement trop basse pour les filtres",
                         f"Le log est enregistré à {fs:.0f} Hz ({loops}) : assez pour régler les PID (au moins "
                         f"{PID_LOG_RATE_MIN_HZ:.0f} Hz), pas pour juger les filtres (au moins {base_hz:.0f} Hz). Tout "
                         f"bruit au-dessus de {fs / 2:.0f} Hz se replie dans le spectre (fausses raies) : aucun filtre "
                         "n'est allégé d'après ce log.", filter_fix))
    if hz is not None and 3 * hz > fs / 2:
        F.append(Finding("Enregistrement", CONSEIL, "Harmoniques moteur au-delà de la plage visible",
                         f"Les moteurs tournent à ~{hz:.0f} Hz en vol : la 3e harmonique ({3 * hz:.0f} Hz) dépasse "
                         f"la moitié de la fréquence d'enregistrement ({fs / 2:.0f} Hz) et se replie dans le spectre "
                         "(petit drone : moteurs rapides).",
                         [f"blackbox_sample_rate = {sample_rate_cli(s.pid_rate_hz, harm_hz)} ({harm_hz:.0f} Hz), au moins "
                          "sur un vol court pour régler les filtres." if harm_hz > fs * 1.05 else
                          "Boucle PID trop lente pour enregistrer plus vite : les harmoniques au-delà restent repliées."]))
    if log.gyro_unfilt is None:
        F.append(Finding("Enregistrement", CONSEIL, "Gyro non filtré absent",
                         "Sans gyro non filtré, impossible de mesurer l'efficacité des filtres ni leur retard.",
                         ["En 4.4 : debug_mode = GYRO_SCALED. En 4.5 et plus : ne désactivez pas le champ gyroUnfilt."]))
    if log.motor_hz is None:
        F.append(Finding("Enregistrement", CONSEIL, "Pas de télémétrie RPM",
                         "Sans eRPM, le filtre RPM est inactif et les harmoniques moteur ne peuvent qu'être estimées.",
                         ["Activez dshot_bidir si vos ESC le permettent (BLHeli_32, Bluejay, AM32)."]))
    n_gaps, lost_s = log.recording_gaps(sel)
    if n_gaps:  # les fenêtres d'analyse qui chevauchent un trou sont écartées (D-020)
        F.append(Finding("Enregistrement", INFO, "Trous dans l'enregistrement",
                         f"{gaps_text(n_gaps, lost_s, t1 - t0)}, dus à des pauses d'écriture de la carte SD ou de la "
                         "mémoire flash. Les fenêtres d'analyse qui chevauchent un trou sont écartées : il reste "
                         "moins de données à analyser.",
                         ["Si c'est fréquent : carte SD plus rapide, ou fréquence d'enregistrement plus basse."]))
    ct = crash_time(log)
    if ct is not None:
        F.append(Finding("Vol", INFO, "Crash détecté",
                         f"Crash vers {ct:.1f} s : la période d'analyse s'arrête avant."))

    # ------------------------------------------------------------ 2. réponse indicielle
    steps = []
    for ax in range(3):
        r = step_response(log.setpoint[ax, sel], log.gyro[ax, sel], fs, terms=log.pid_terms(ax, sel), breaks=br)
        steps.append(AxisStep(AXES[ax], r))
    meas["steps"] = {st.axis: st.result.metrics for st in steps}
    for ax, st in enumerate(steps[:2]):
        _step_rules(F, st, profile, s.pids[ax].F)
    if steps[0].result.n < STEP_MIN_WINDOWS and steps[1].result.n < STEP_MIN_WINDOWS:
        F.append(Finding("Réponse", ATTENTION, "Pas assez de mouvements de manche",
                         f"Seulement {steps[0].result.n} fenêtres exploitables en roll et {steps[1].result.n} en pitch "
                         "(il faut des mouvements de plus de 20 °/s).",
                         ["Volez 20 à 30 s avec des mouvements francs et des flips/rolls rapides sur chaque axe."]))
    r0, r1 = steps[0].result, steps[1].result
    if r0.n >= STEP_MIN_WINDOWS and r1.n >= STEP_MIN_WINDOWS and np.isfinite(r0.metrics.peak) and np.isfinite(r1.metrics.peak):
        d = r1.metrics.peak - r0.metrics.peak
        if abs(d) > profile.roll_pitch_gap_high:
            more = "pitch" if d > 0 else "roll"
            F.append(Finding("Réponse", CONSEIL, "Roll et pitch se comportent différemment",
                             f"Pic {r0.metrics.peak:.2f} en roll contre {r1.metrics.peak:.2f} en pitch : le {more} dépasse davantage "
                             f"(écart {abs(d):.2f}, seuil {profile.roll_pitch_gap_high:.2f}).",
                             [f"Curseur « D pitch » (pitch_d) : {'+' if d > 0 else '−'}0,1 pour équilibrer."]))

    # ------------------------------------------------------------ 3. bruit et filtres
    delays = compute_delays(log, sel)
    meas["delays"] = delays
    F.extend(_setpoint_delay(log, sel, delays.sp_gyro_ms))
    nyq = fs / 2
    # Bandes vérifiées sur le corpus (docs/assistant.md, « Bandes de bruit ») : 500 Hz = plus haute borne
    # commune aux logs à 1 kHz ; changer une borne oblige à recalibrer les seuils p90 des profils
    hi = min(500.0, 0.95 * nyq)
    g_db = [_band_db(log.gyro[a, sel], fs, 80, hi, br) for a in range(2)]
    d_db = [_band_db(log.d_term[a, sel], fs, 80, hi, br) for a in range(2) if np.any(log.d_term[a])]
    low_db = [_band_db(log.gyro[a, sel], fs, 20, 80, br) for a in range(2)]
    g_ptb = max(band_peak_ptb_db(log.gyro[a, sel], fs, 80, hi, br) for a in range(2))
    d_ptb = max((band_peak_ptb_db(log.d_term[a, sel], fs, 80, hi, br) for a in range(2) if np.any(log.d_term[a])),
                default=float("nan"))
    meas.update(gyro_noise_db=g_db, dterm_noise_db=d_db, low_band_db=low_db, gyro_peak_ptb_db=g_ptb,
                dterm_peak_ptb_db=d_ptb)
    gyro_noisy = max(g_db) > profile.gyro_noise_high_db
    dterm_corpus = bool(d_db) and max(d_db) > profile.dterm_noise_high_db
    dterm_ptb = bool(np.isfinite(d_ptb) and d_ptb > DTERM_PTB_MAX_DB)
    dterm_noisy = dterm_corpus or dterm_ptb
    if dterm_noisy:
        why = []
        if dterm_corpus:
            why.append(f"bruit moyen {max(d_db):.1f} dB (80–{hi:.0f} Hz), plus que 90 % des drones de cette taille "
                       f"(seuil {profile.dterm_noise_high_db:.1f} dB)")
        if dterm_ptb:
            why.append(f"sommet du spectre à {d_ptb:.0f} dB sur l'échelle de PIDtoolbox, au-dessus du repère de "
                       f"{DTERM_PTB_MAX_DB:.0f} dB (idéal : sous −20 dB)")
        F.append(Finding("Bruit", ATTENTION, "Terme D bruité",
                         "Terme D : " + " ; ".join(why) + ". Un D bruité fait chauffer les moteurs et use les ESC.",
                         ["Vérifiez d'abord la mécanique (hélices, roulements, vis du châssis).",
                          "Curseur « filtre D » : −0,1 à −0,2 (filtrage plus fort), ou baisser le D (curseur Damping) si la réponse le permet.",
                          "Après le vol : moteurs tièdes = OK ; chauds = réduire D ou filtrer plus."]))
    if gyro_noisy:
        F.append(Finding("Bruit", ATTENTION, "Gyro filtré bruité",
                         f"Bruit du gyro filtré {max(g_db):.1f} dB (80–{hi:.0f} Hz), plus que 90 % des drones de cette taille (seuil {profile.gyro_noise_high_db:.1f} dB). "
                         f"Sommet du spectre à {g_ptb:.0f} dB sur l'échelle de PIDtoolbox (repère : sous {GYRO_PTB_MAX_DB:.0f} dB).",
                         ["Contrôlez hélices et moteurs (vibrations), la fixation du contrôleur de vol (silentblocs).",
                          "Curseur « filtre gyro » : −0,2."]))
    elif np.isfinite(g_ptb):
        F.append(Finding("Bruit", OK if g_ptb <= GYRO_PTB_MAX_DB else INFO, "Bruit du gyro filtré",
                         f"Sommet du spectre à {g_ptb:.0f} dB sur l'échelle de PIDtoolbox (repère : sous "
                         f"{GYRO_PTB_MAX_DB:.0f} dB) ; bruit moyen {max(g_db):.1f} dB, dans les 90 % du corpus "
                         f"(seuil {profile.gyro_noise_high_db:.1f} dB)."))
    if not dterm_noisy and np.isfinite(d_ptb):
        F.append(Finding("Bruit", OK, "Bruit du terme D",
                         f"Sommet du spectre à {d_ptb:.0f} dB sur l'échelle de PIDtoolbox (repère : au plus "
                         f"{DTERM_PTB_MAX_DB:.0f} dB, idéal sous −20 dB)."))
    fr = frame_resonance(log, sel)
    fixed = fr[0] if fr is not None and fr[1] >= profile.resonance_high_db and not fr[2] else None
    dn = dyn_notch_view(log, sel, fixed)
    meas.update(frame_resonance=fr, dyn_notch=dn, aliased=aliased)
    target = filter_delay_target_ms(profile.nominal_inch)
    g_goal = (f" Objectif de Brian White pour un {profile.nominal_inch:g} pouces : environ {target[0]:.2f} ms."
              if target else "")
    if not delays.moving and (np.isfinite(delays.gyro_ms) or np.isfinite(delays.dterm_ms)):
        vals = " ; ".join(f"{name} {v:.2f} ms" for name, v in (("gyro", delays.gyro_ms), ("D", delays.dterm_ms))
                          if np.isfinite(v))
        F.append(Finding("Filtres", INFO, "Retard des filtres peu représentatif",
                         f"Retards mesurés : {vals}. Le vol contient trop peu de mouvements (consigne au-delà de "
                         f"{MOVING_SETPOINT_DPS:.0f} °/s moins de 5 % du temps, 95e centile {delays.stick_p95_dps:.0f} °/s) : "
                         "en vol stationnaire, les filtres dynamiques (LPF dynamiques, notchs RPM et dynamiques) ne "
                         "travaillent qu'à un régime, et le retard ne se mesure pas bien. Aucun conseil fondé sur le "
                         "retard mesuré ; le vol stationnaire sert en revanche à identifier le bruit (harmoniques, "
                         "résonances, bruit résiduel)." + g_goal,
                         ["Pour juger le retard des filtres : log avec des mouvements francs (flips, rolls, wobbles), "
                          "pas seulement du vol stationnaire."]))
    if delays.moving and np.isfinite(delays.gyro_ms):
        if delays.gyro_ms > profile.gyro_delay_max_ms and not gyro_noisy and not dterm_noisy:
            actions = ["Curseur « filtre gyro » : +0,2 (moins de retard → meilleure réactivité, moins de propwash)."]
            if s.rpm_harmonics > 0 and s.rpm_q < RPM_Q_MAX:
                actions.append(f"Filtre RPM : rpm_filter_q {s.rpm_q} → {min(s.rpm_q + RPM_Q_STEP, RPM_Q_MAX)} (notchs "
                               f"plus étroits, moins de retard), puis par pas de {RPM_Q_STEP} jusqu'à {RPM_Q_MAX} au plus, "
                               "en vérifiant à chaque pas que les raies moteur ne réapparaissent pas.")
            actions.append("Contrôlez la température des moteurs après chaque changement.")
            F.append(Finding("Filtres", CONSEIL, "Filtrage gyro peut-être trop prudent",
                             f"Retard des filtres gyro {delays.gyro_ms:.2f} ms (repère {profile.gyro_delay_max_ms:.1f} ms pour "
                             f"{profile.label}, plus que 90 % des drones du corpus) alors que le bruit est faible." + g_goal,
                             actions))
        elif delays.gyro_ms > profile.gyro_delay_max_ms:
            F.append(Finding("Filtres", INFO, "Retard des filtres gyro",
                             f"{delays.gyro_ms:.2f} ms, au-dessus du repère ({profile.gyro_delay_max_ms:.1f} ms pour "
                             f"{profile.label}), mais le bruit ne permet pas de filtrer moins." + g_goal))
        else:
            F.append(Finding("Filtres", OK, "Retard des filtres gyro",
                             f"{delays.gyro_ms:.2f} ms (repère ≤ {profile.gyro_delay_max_ms:.1f} ms pour {profile.label})." + g_goal))
    if delays.moving and np.isfinite(delays.dterm_ms):
        d_goal = (f" Objectif de Brian White pour un {profile.nominal_inch:g} pouces : environ {target[1]:.2f} ms "
                  "lus par PIDtoolbox, qui mesure le retard du D environ 0,5 ms plus court qu'ici." if target else "")
        if delays.dterm_ms > profile.dterm_delay_max_ms and not dterm_noisy:
            F.append(_dterm_lift(log, sel, hi, profile, delays.dterm_ms, d_db, d_ptb, d_goal) if not aliased else
                     Finding("Filtres", INFO, "Retard des filtres D",
                             f"{delays.dterm_ms:.2f} ms, au-dessus du repère ({profile.dterm_delay_max_ms:.1f} ms), mais le log "
                             "est trop lent pour juger le bruit : pas de relèvement proposé." + d_goal))
        elif delays.dterm_ms <= profile.dterm_delay_max_ms:
            F.append(Finding("Filtres", OK, "Retard des filtres D",
                             f"{delays.dterm_ms:.2f} ms (repère ≤ {profile.dterm_delay_max_ms:.1f} ms pour {profile.label})." + d_goal))
    if (not aliased and not dterm_noisy and not gyro_noisy and np.isfinite(d_ptb) and d_ptb < DTERM_PTB_IDEAL_DB
            and np.any(log.d_term[0])):
        f = _dterm_lift_noise(log, sel, hi, profile, d_db, d_ptb, hover=not delays.moving,
                              delay_ms=delays.dterm_ms if delays.moving else float("nan"))
        if f is not None:  # remplace le conseil +0,1 fondé sur le retard, qui en dit moins
            F[:] = [x for x in F if not (x.title == "Filtrage du D peut-être trop fort" and x.severity == CONSEIL)]
            F.append(f)
    F.extend(_lowpass_setup_rules(s))
    if (not aliased and s.gyro_lpf1.enabled and log.gyro_unfilt is not None and log.motor_hz is not None and s.rpm_harmonics > 0
            and not gyro_noisy and not dterm_noisy):
        f = _gyro_lpf1_off(log, sel, hi, profile, g_db, d_db, d_ptb, hover=not delays.moving)
        if f is not None:
            F.append(f)
    if log.motor_hz is not None and log.gyro_unfilt is not None and s.rpm_harmonics > 0:
        mn = [motor_noise(log.gyro[a, sel], log.motor_hz[:, sel], fs, breaks=br) for a in range(2)]
        mp = [motor_noise(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], fs, breaks=br) for a in range(2)]
        left = [float(np.nanmax([m.mean_db[0] for m in mn]))]
        meas["motor_harmonic_left_db"] = left[0]
        meas["motor_harmonic_raw_db"] = float(np.nanmax([m.mean_db[0] for m in mp]))
        if left[0] > profile.motor_left_high_db:
            F.append(Finding("Filtres", CONSEIL, "Bruit moteur mal éliminé",
                             f"Il reste {left[0]:.1f} dB sur la fréquence de rotation des moteurs après filtrage "
                             f"(avant filtrage : {meas['motor_harmonic_raw_db']:.1f} dB).",
                             ["Vérifiez motor_poles (14 pour la plupart des moteurs, 12 pour certains micro-moteurs).",
                              f"Filtre RPM : rpm_filter_q {s.rpm_q} → {max(RPM_Q_MIN, s.rpm_q - RPM_Q_STEP)} (notchs plus "
                              f"larges), puis par pas de {RPM_Q_STEP} tant que la raie reste visible."]))
        F.extend(_rpm_weight_rules(log, sel, s, lighten=left[0] <= profile.motor_left_high_db and not aliased,
                                   h1_flagged=left[0] > profile.motor_left_high_db))
        q_said = any("rpm_filter_q" in a for f in F for a in f.actions)  # retard, raie visible : déjà conseillé
        if not aliased and not gyro_noisy and not dterm_noisy and s.rpm_q < RPM_Q_MAX and not q_said:
            f = _rpm_q_raise(log, sel, s, F, hover=not delays.moving)
            if f is not None:
                F.append(f)
    if max(low_db) > profile.low_band_high_db:
        F.append(Finding("Réponse", CONSEIL, "Oscillations basses fréquences (20–80 Hz)",
                         f"Énergie du gyro filtré entre 20 et 80 Hz : {max(low_db):.1f} dB (seuil {profile.low_band_high_db:.1f} dB, "
                         "90 % des drones de cette taille font mieux). C'est la zone du propwash "
                         "et des oscillations de réglage.",
                         ["Si cela arrive en descente/gaz coupés : propwash → plus de D (Damping +0,1) ou D max.",
                          "Si cela arrive en vol stable : oscillation → Master −0,1 à −0,2.",
                          "Regardez l'outil Fréquence × gaz pour voir à quel gaz le phénomène apparaît."]))
    res = _frame_resonance(log, fr, profile, sel)
    if res is not None:
        F.append(res)
    if not delays.moving and log.motor_hz is not None and log.gyro_unfilt is not None:
        F.extend(_hover_noise(log, sel, profile, g_ptb, d_ptb, fr[0] if res is not None else None, meas, aliased))
    lines_pass = any(f.title.startswith(("Raie fixe", "Raie à ")) for f in F)
    if not aliased and dn is not None:
        F.extend(_dyn_notch_q_n_rules(s, dn, lines_pass, gyro_noisy, dterm_noisy, F, hover=not delays.moving))
    if not aliased and dn is not None and dn["count"] > max(1, len(dn["peaks"])):
        n = max(1, len(dn["peaks"]))
        found = (f"{len(dn['peaks'])} pic(s) à suivre ({', '.join(f'{f:.0f}' for f in dn['peaks'])} Hz)" if dn["peaks"]
                 else "aucun pic à suivre")
        F.append(Finding("Filtres", INFO, "Notch dynamique : moins de notchs suffiraient",
                         f"Entre {dn['min_hz']} et {dn['max_hz']} Hz, après le filtre RPM, {found} pour "
                         f"{dn['count']} notchs. Chaque notch filtre en permanence, même sans pic à suivre (il reste à "
                         "sa dernière fréquence), et ajoute du retard.",
                         [f"dyn_notch_count = {n} (actuellement {dn['count']}), puis vérifiez sur le log suivant que "
                          "les raies de résonance ne réapparaissent pas dans le gyro filtré."]))

    # ------------------------------------------------------------ 4. moteurs et mécanique
    fst = flight_stats(log, sel)
    meas["stats"] = fst
    if fst.motor_saturation_pct > profile.saturation_high_pct:
        F.append(Finding("Moteurs", ATTENTION, "Moteurs souvent saturés",
                         f"Au moins un moteur est à fond {fst.motor_saturation_pct:.1f} % du temps (seuil "
                         f"{profile.saturation_high_pct:.1f} %) : le contrôleur n'a plus de marge pour corriger.",
                         ["Si c'est pendant les manœuvres : baisser les rates ou le Master.",
                          "Sinon : drone trop lourd pour ses moteurs/hélices, ou batterie faible."]))
    if np.isfinite(fst.motor_imbalance_pct) and fst.motor_imbalance_pct > profile.imbalance_high_pct:
        F.append(_imbalance_finding([mm.mean for mm in fst.motors], fst.motor_imbalance_pct))
    pe = pid_error_stats(log, sel)
    meas["pid_error_sd"] = pe.sd
    i_mean = [float(np.mean(log.i_term[a, sel])) for a in range(3)]
    meas["iterm_mean"] = i_mean
    for a in range(2):
        if abs(i_mean[a]) > profile.iterm_offset_high:
            F.append(Finding("Mécanique", CONSEIL, f"Terme I décentré en {AXES[a].lower()}",
                             f"Le terme I {AXES[a].lower()} reste en moyenne à {i_mean[a]:.0f} (seuil ±{profile.iterm_offset_high:.0f}) : le drone "
                             "compense en permanence un déséquilibre.",
                             ["Centrez la batterie / la caméra (centre de gravité).", "Vérifiez qu'aucun moteur ne tourne moins vite."]))

    # ------------------------------------------------------------ 5. réglage simplifié
    sv = slider_view(s)
    meas["sliders"] = sv
    if not sv.from_header:
        F.append(Finding("Réglage", INFO, "Réglage simplifié désactivé",
                         "Les PID ont été saisis à la main : les conseils « curseur » sont donnés à titre indicatif.",
                         ["Positions estimées : " + ", ".join(f"{k} {v:.2f}" for k, v in sv.values.items())]))

    strategy = filter_strategy(F, moving=delays.moving)
    if strategy is not None:
        F.insert(0, strategy)
    return Report(profile, reason, (t0, t1), F, steps, meas)


def _fr(x: float) -> str:
    """Nombre à deux décimales, virgule à la française (paliers cités dans les textes)."""
    return f"{x:.2f}".replace(".", ",")


def _step_rules(F: list[Finding], st: AxisStep, profile: CraftProfile, ff_gain: int = 0) -> None:
    r = st.result
    if r.n < STEP_MIN_WINDOWS:
        return
    first = len(F)
    _step_findings(F, st, profile, ff_gain)
    if r.n < STEP_SURE_WINDOWS:
        for f in F[first:]:
            if f.title.startswith("Marge"):
                continue  # boucle ouverte : précision non mesurée ici, elle a sa propre réserve (_loop_caveat)
            f.detail += (f" Mesure moins sûre : {r.n} fenêtres ({STEP_SURE_WINDOWS} conseillées, soit environ 20 s de "
                         "wobbles roll et pitch mêlés).")


def _step_findings(F: list[Finding], st: AxisStep, profile: CraftProfile, ff_gain: int) -> None:
    r = st.result
    ax = st.axis
    m = r.metrics
    lp = r.loop
    pk, lat = m.peak, m.latency_ms
    advice, strong = STEP_PEAK_HIGH_FF if ff_gain > 0 else STEP_PEAK_HIGH
    with_ff = f" avec feedforward (F = {ff_gain})" if ff_gain > 0 else " sans feedforward"
    if pk > advice and ff_gain > 0 and np.isfinite(lat) and lat < profile.latency_ms[0]:
        # Réponse plus rapide que ce que P et D permettent seuls, puis dépassement : c'est le
        # feedforward qui pousse (il agit à plein sur les mouvements de manche vifs).
        F.append(Finding("Réponse", CONSEIL if pk <= strong else ATTENTION,
                         f"Dépassement dû au feedforward en {ax.lower()}",
                         f"Pic {pk:.2f} atteint après une montée très rapide (latence {lat:.1f} ms, plus courte que "
                         f"les {profile.latency_ms[0]:.0f}–{profile.latency_ms[1]:.0f} ms habituels pour {profile.label}). "
                         f"Le feedforward (F = {ff_gain}) anticipe le mouvement et le drone dépasse la consigne.",
                         [f"Refaites la {FF_SERIES_ADVICE} ; ou feedforward_boost plus bas.",
                          "Si vous aimez cette vivacité (freestyle, course), ce n'est pas un défaut : "
                          "vérifiez seulement qu'il n'y a pas de rebond à la fin des flips.",
                          "Le D n'est pas en cause tant que les vols calmes ne dépassent pas."], ax))
        return
    if pk > strong:
        F.append(Finding("Réponse", ATTENTION, f"Fort dépassement en {ax.lower()}",
                         f"Pic {pk:.2f} (> {_fr(strong)}{with_ff}, plus que 95 % des axes du corpus) : le drone "
                         "dépasse la consigne puis revient, signe d'un manque d'amortissement.",
                         ["Curseur Damping (D) : +0,2."], ax))
    elif pk > advice:
        F.append(Finding("Réponse", CONSEIL, f"Léger dépassement en {ax.lower()}",
                         f"Pic {pk:.2f} (> {_fr(advice)}{with_ff}, plus que 90 % des drones du corpus). Sans "
                         f"feedforward, une réponse sans dépassement visible donne environ {_fr(STEP_PEAK_FLAT)} ; "
                         "le feedforward fait monter le pic.", ["Curseur Damping (D) : +0,1."], ax))
    elif pk < STEP_PEAK_SOFT and np.isfinite(lat) and lat > profile.latency_ms[1]:
        F.append(Finding("Réponse", CONSEIL, f"Réponse molle en {ax.lower()}",
                         f"Pic {pk:.2f} et latence {lat:.1f} ms (repère {profile.latency_ms[0]:.0f}–{profile.latency_ms[1]:.0f} ms "
                         f"pour {profile.label}) : trop amorti.",
                         ["Curseur Damping (D) : −0,1.",
                          f"Pour la réactivité au manche : {FF_SERIES_ADVICE}."], ax))
    else:
        F.append(Finding("Réponse", OK, f"Réponse indicielle {ax.lower()}",
                         f"Pic {pk:.2f}, latence {lat:.1f} ms, {r.n} fenêtres analysées.", [], ax))
    if np.isfinite(lp.sensitivity_peak_db) and lp.sensitivity_peak_db > SENSITIVITY_HIGH_DB:
        at_least = "au moins " if lp.sensitivity_peak_at_edge else ""
        F.append(Finding("Réponse", CONSEIL, f"Marge de stabilité faible en {ax.lower()}",
                         f"Pic de sensibilité {at_least}{lp.sensitivity_peak_db:.1f} dB à {lp.sensitivity_peak_hz:.0f} Hz "
                         f"(> {SENSITIVITY_HIGH_DB:.0f} dB) : le drone amplifie les perturbations à cette fréquence et risque d'osciller."
                         + _loop_caveat(lp) + _whoop_margin_note(profile),
                         [*_margin_actions(lp), "Réduire le retard des filtres si le bruit le permet.",
                          *_edge_actions(lp)], ax))
    elif np.isfinite(lp.phase_margin_deg) and lp.phase_margin_deg < PHASE_MARGIN_LOW_DEG:
        F.append(Finding("Réponse", CONSEIL, f"Marge de phase réduite en {ax.lower()}",
                         f"Marge de phase {lp.phase_margin_deg:.0f}° (< {PHASE_MARGIN_LOW_DEG:.0f}°) au croisement de {lp.crossover_hz:.0f} Hz."
                         + _loop_caveat(lp) + _whoop_margin_note(profile), _margin_actions(lp), ax))


def _setpoint_delay(log: FlightLog, sel: slice, sp_ms: list[float]) -> list[Finding]:
    """Retard consigne → gyro (intercorrélation) roll / pitch : la mesure de la série Feedforward des finitions
    (pas de 0,5, en Acro ; choix de Cédric, 2026-10-12), lue ici plutôt que sur la réponse indicielle, que le
    feedforward fausse."""
    if not any(np.isfinite(sp_ms[:2])):
        return []
    cond = flight_conditions(log, sel)
    ff = slider_view(log.settings).values[FF_KEY]
    mode = ("mode Angle : le firmware y coupe le feedforward, volez en Acro pour la série" if cond.angle else "Acro")
    txt = ", ".join(f"{AXES[a].lower()} {v:.1f} ms" for a, v in enumerate(sp_ms[:2]) if np.isfinite(v))
    txt = f"{txt[0].upper()}{txt[1:]} (feedforward {ff:.1f}".replace(".", ",")
    return [Finding("Réponse", INFO, "Retard consigne → gyro", f"{txt}, {mode})."
                    + " Série Feedforward (0, 0,5, 1… en Acro) : comparez ce retard d'un vol à l'autre ; plus il est "
                    "court, plus le drone suit le manche tôt.", [])]


# Fréquence d'enregistrement minimale (Cédric, 2026-10-05) : 2 kHz pour un gyro à 8 kHz, 1,6 kHz pour un gyro
# à 3,2 kHz, c'est-à-dire la plus petite fraction 1/2^n de la boucle PID qui atteint 1,6 kHz ; plus pour voir
# la 3e harmonique moteur des petits drones (≥ 6 × la fréquence moteur). Le blackbox ne filtre pas avant de
# sous-échantillonner : tout ce qui dépasse la moitié de la fréquence du log se replie.
LOG_RATE_MIN_HZ = 1600.0
LOG_RATE_FRACTIONS = (1, 2, 4, 8, 16)  # blackbox_sample_rate : 1/1 … 1/16 de la boucle PID


def log_rate_needed(pid_hz: float, motor_hz: float | None = None) -> float:
    """Fréquence d'enregistrement à viser : la plus lente des fractions de la boucle PID qui atteint 1,6 kHz
    (et 6 × la fréquence moteur si elle est connue) ; la boucle PID elle-même si aucune n'y suffit."""
    need = LOG_RATE_MIN_HZ if motor_hz is None else max(LOG_RATE_MIN_HZ, 6.0 * motor_hz)
    ok = [pid_hz / d for d in LOG_RATE_FRACTIONS if pid_hz / d >= 0.95 * need]
    return min(ok) if ok else pid_hz


# Fréquence d'enregistrement minimale pour régler les PID (Cédric, 2026-10-12) : 1 kHz, latence et réponse
# indicielle à la milliseconde près ; sous 500 Hz, la réponse indicielle n'est plus lisible (bande analysée
# jusqu'à 0,4 × la fréquence du log, soit 200 Hz). Le bruit au-delà de la moitié de la fréquence du log se
# replie aussi, mais il pèse peu sur la réponse aux mouvements du manche (quelques dizaines de hertz).
PID_LOG_RATE_MIN_HZ = 1000.0
PID_LOG_RATE_FLOOR_HZ = 500.0


def pid_log_rate_needed(pid_hz: float) -> float:
    """Fréquence d'enregistrement à viser pour un vol de réglage des PID : la plus lente des fractions de la
    boucle PID qui atteint 1 kHz ; la boucle PID elle-même si aucune n'y suffit."""
    ok = [pid_hz / d for d in LOG_RATE_FRACTIONS if pid_hz / d >= 0.95 * PID_LOG_RATE_MIN_HZ]
    return min(ok) if ok else pid_hz


def sample_rate_cli(pid_hz: float, rate_hz: float) -> str:
    """Valeur de blackbox_sample_rate pour enregistrer à `rate_hz`."""
    d = min(LOG_RATE_FRACTIONS, key=lambda x: abs(pid_hz / x - rate_hz))
    return f"1/{d}"


def predict_filter_change(log: FlightLog, sel: slice, mod, hi: float) -> dict:
    """Effet de réglages de filtres `mod` (simulateur `bf_filters.whatif_spectrum`, exact quand rien ne
    change) : variation du bruit moyen et du sommet du spectre (dB, par axe roll / pitch) du gyro filtré et
    du terme D, et des retards des chaînes gyro et D (ms, au gaz médian)."""
    s = log.settings
    out = {"gyro_mean": [], "gyro_peak": [], "d_mean": [], "d_peak": []}

    def deltas(w):
        m = (w.freqs >= 80) & (w.freqs <= hi)
        cur, new = w.current_db[m], w.modified_db[m]
        sm = lambda v: np.convolve(v, np.ones(5) / 5, mode="same")  # noqa: E731
        return float(np.mean(new) - np.mean(cur)), float(np.max(sm(new)) - np.max(sm(cur)))

    for a in range(2):
        if log.gyro_unfilt is not None:
            g = deltas(bf_filters.whatif_spectrum(log.gyro[a], log.gyro_unfilt[a], log, sel, s, mod, "gyro"))
            out["gyro_mean"].append(g[0])
            out["gyro_peak"].append(g[1])
        if np.any(log.d_term[a]):
            d = deltas(bf_filters.whatif_spectrum(log.d_term[a], log.d_term_prefilter[a], log, sel, s, mod, "dterm"))
            out["d_mean"].append(d[0])
            out["d_peak"].append(d[1])
    thr = float(np.median(log.throttle[sel])) / 100.0
    delay = lambda chain: bf_filters.chain_delay_ms(chain).phase_ms  # noqa: E731
    out["gyro_delay_gain_ms"] = delay(bf_filters.gyro_chain(s, thr, dyn_notch_hz=[])) - delay(
        bf_filters.gyro_chain(mod, thr, dyn_notch_hz=[]))
    out["d_delay_gain_ms"] = delay(bf_filters.dterm_chain(s, thr)) - delay(bf_filters.dterm_chain(mod, thr))
    return out


def _noise_after(pred: dict, d_db: list[float], d_ptb: float, g_db: list[float] | None = None):
    """Bruits prévus (maximum roll / pitch) : gyro moyen, D moyen, sommet du D (échelle PIDtoolbox)."""
    g = max(x + dx for x, dx in zip(g_db, pred["gyro_mean"])) if g_db and pred["gyro_mean"] else None
    d = max(x + dx for x, dx in zip(d_db, pred["d_mean"])) if d_db and pred["d_mean"] else None
    dp = d_ptb + max(pred["d_peak"]) if pred["d_peak"] and np.isfinite(d_ptb) else None
    return g, d, dp


def _gyro_lpf1_off(log, sel, hi, profile, g_db, d_db, d_ptb, hover: bool = False) -> Finding | None:
    """LPF1 du gyro superflu avec le filtre RPM, si le simulateur prévoit un bruit encore sous les repères.
    Fondé sur le bruit (le retard gagné vient du modèle des filtres, pas de la mesure) : valable aussi en vol
    stationnaire, qui ne montre toutefois le bruit qu'à un régime."""
    s = log.settings
    mod = copy.deepcopy(s)
    mod.gyro_lpf1 = bf_filters.lowpass_off(s.gyro_lpf1)
    pred = predict_filter_change(log, sel, mod, hi)
    g, d, dp = _noise_after(pred, d_db, d_ptb, g_db)
    if None in (g, d, dp) or g > profile.gyro_noise_high_db or d > profile.dterm_noise_high_db or dp > DTERM_PTB_MAX_DB:
        return None
    lp = s.gyro_lpf1
    now = f"{lp.dyn_min_hz}–{lp.dyn_max_hz} Hz dynamique" if lp.dynamic else f"{lp.static_hz} Hz"
    return Finding("Filtres", CONSEIL, "Le LPF1 du gyro peut être coupé",
                   f"Avec le filtre RPM, le LPF1 du gyro ({lp.type} {now}) est souvent superflu. Sans lui, le simulateur "
                   f"prévoit {pred['gyro_delay_gain_ms']:.2f} ms de retard en moins, un gyro filtré à {g:.1f} dB "
                   f"(seuil {profile.gyro_noise_high_db:.1f}), un terme D à {d:.1f} dB (seuil {profile.dterm_noise_high_db:.1f}) "
                   f"et un sommet du D à {dp:.0f} dB PIDtoolbox (repère {DTERM_PTB_MAX_DB:.0f}). Retard calculé "
                   "d'après les réglages des filtres." + (" Vol stationnaire : bruit vu à un seul régime, à confirmer sur un "
                                                          "vol avec des gaz plus hauts." if hover else ""),
                   ["gyro_lpf1_static_hz = 0 et gyro_lpf1_dyn_min_hz = 0 (le curseur « filtre gyro » ne rallume pas un "
                    "filtre coupé).",
                    "Vol d'essai court, puis vérifiez la température des moteurs et le spectre du log suivant."])


def _dterm_lift(log, sel, hi, profile, delay_ms, d_db, d_ptb, d_goal) -> Finding:
    """Retard du D au-dessus du repère avec un D peu bruité : relever ses filtres seulement si le simulateur
    prévoit un D encore sous les repères avec le curseur « filtre D » +0,1."""
    s = log.settings
    sl = s.sliders()
    mult = sl.get("dterm_mult", -1) if sl.get("dterm_filter", 0) > 0 else -1
    factor = (mult + 10) / mult if mult > 0 else 1.1  # curseur +0,1 = multiplicateur +10 (simplified_tuning.c)
    mod = copy.deepcopy(s)
    mod.dterm_lpf1 = bf_filters.scale_lowpass(s.dterm_lpf1, factor)
    mod.dterm_lpf2 = bf_filters.scale_lowpass(s.dterm_lpf2, factor)
    pred = predict_filter_change(log, sel, mod, hi) if np.any(log.d_term[0]) else None
    head = f"Retard des filtres D {delay_ms:.2f} ms (repère {profile.dterm_delay_max_ms:.1f} ms) avec un D peu bruité."
    if pred is None:
        return Finding("Filtres", CONSEIL, "Filtrage du D peut-être trop fort", head + d_goal, ["Curseur « filtre D » : +0,1."])
    _, d, dp = _noise_after(pred, d_db, d_ptb)
    forecast = (f" Avec le curseur « filtre D » +0,1 (coupures ×{factor:.2f}), le simulateur prévoit "
                f"{pred['d_delay_gain_ms']:.2f} ms de retard en moins, un terme D à {d:.1f} dB (seuil "
                f"{profile.dterm_noise_high_db:.1f}) et un sommet à {dp:.0f} dB PIDtoolbox (repère {DTERM_PTB_MAX_DB:.0f}).")
    if d is not None and dp is not None and d <= profile.dterm_noise_high_db and dp <= DTERM_PTB_MAX_DB:
        return Finding("Filtres", CONSEIL, "Filtrage du D peut-être trop fort", head + forecast + d_goal,
                       ["Curseur « filtre D » : +0,1.",
                        "Vol d'essai court, puis vérifiez la température des moteurs : moins de filtrage du D les fait "
                        "chauffer plus vite que tout autre réglage."])
    return Finding("Filtres", INFO, "Retard des filtres D", head + forecast +
                   " Le D dépasserait les repères : mieux vaut ne pas relever ses filtres." + d_goal)


def _dterm_lift_noise(log, sel, hi, profile, d_db, d_ptb, hover: bool, delay_ms: float) -> Finding | None:
    """Terme D sous −20 dB PIDtoolbox : plus grand relèvement de ses filtres (pas de +0,1 du curseur « filtre
    D », ou coupures × 1,1 sans curseur) pour lequel le simulateur prévoit un D encore sous les repères
    (−10 dB PIDtoolbox et p90 de la taille). Fondé sur le bruit : valable aussi en vol stationnaire."""
    s = log.settings
    sl = s.sliders()
    mult = sl.get("dterm_mult", -1) if sl.get("dterm_filter", 0) > 0 else -1
    if mult > 0:  # curseur +0,1 = multiplicateur +10 (simplified_tuning.c)
        k_max = (max(DTERM_SLIDER_BASIC_MAX - mult, 0) if mult < DTERM_SLIDER_BASIC_MAX
                 else 10 * DTERM_LIFT_MAX_STEPS) // 10
    else:
        k_max = DTERM_LIFT_MAX_STEPS
    best = None
    for k in range(k_max, 0, -1):  # du plus grand pas au plus petit : le plus souvent une seule simulation
        factor = (mult + 10 * k) / mult if mult > 0 else 1.1 ** k
        mod = copy.deepcopy(s)
        mod.dterm_lpf1 = bf_filters.scale_lowpass(s.dterm_lpf1, factor)
        mod.dterm_lpf2 = bf_filters.scale_lowpass(s.dterm_lpf2, factor)
        if (mod.dterm_lpf1, mod.dterm_lpf2) == (s.dterm_lpf1, s.dterm_lpf2):
            continue  # coupures plafonnées à 1000 Hz : rien ne change
        pred = predict_filter_change(log, sel, mod, hi)
        _, d, dp = _noise_after(pred, d_db, d_ptb)
        if d is not None and dp is not None and d <= profile.dterm_noise_high_db and dp <= DTERM_PTB_MAX_DB:
            best = {"k": k, "lpf": (mod.dterm_lpf1, mod.dterm_lpf2), "d": d, "dp": dp, "gain": pred["d_delay_gain_ms"]}
            break
    if best is None:
        return None
    k = best["k"]
    if mult > 0:
        now, new = mult / 100, (mult + 10 * k) / 100
        f1 = lambda x: f"{x:.1f}".replace(".", ",")  # noqa: E731
        action = (f"Curseur « filtre D » : {f1(now)} → {f1(new)}" +
                  (f", en passant par {', '.join(f1(now + 0.1 * i) for i in range(1, k))} avec un vol d'essai à chaque "
                   "pas." if k > 1 else "."))
        if mult + 10 * k > DTERM_SLIDER_BASIC_MAX:
            action += " Au-delà de 1,2, le configurateur demande le mode expert."
    else:
        lp1, lp2 = best["lpf"]
        parts = [f"dterm_lpf1_dyn_min_hz = {lp1.dyn_min_hz}, dterm_lpf1_dyn_max_hz = {lp1.dyn_max_hz}" if lp1.dynamic else
                 f"dterm_lpf1_static_hz = {lp1.static_hz}" if lp1.enabled else "",
                 f"dterm_lpf2_static_hz = {lp2.static_hz}" if lp2.enabled else ""]
        action = "Coupures du D (sans curseur) : " + " ; ".join(p for p in parts if p) + " (× 1,1 par pas)."
    delay = f" Retard mesuré du D : {delay_ms:.2f} ms." if np.isfinite(delay_ms) else ""
    return Finding("Filtres", CONSEIL, "Terme D propre : ses filtres peuvent être relevés",
                   f"Sommet du terme D à {d_ptb:.1f} dB sur l'échelle de PIDtoolbox, sous l'idéal de {DTERM_PTB_IDEAL_DB:.0f} dB "
                   f"de Brian White.{delay} En relevant ses filtres de {k} pas, le simulateur prévoit "
                   f"{best['gain']:.2f} ms de retard en moins, un terme D à {best['d']:.1f} dB (seuil "
                   f"{profile.dterm_noise_high_db:.1f}) et un sommet à {best['dp']:.0f} dB PIDtoolbox (repère "
                   f"{DTERM_PTB_MAX_DB:.0f})." +
                   (" Vol stationnaire : bruit vu à un seul régime, à confirmer sur un vol avec des gaz plus hauts."
                    if hover else ""),
                   [action, "Après chaque vol d'essai, touchez les moteurs : moins de filtrage du D les fait chauffer plus "
                    "vite que tout autre réglage ; tièdes = OK, chauds = revenir au pas précédent."])


# Stratégie de filtrage : ordre des étapes (doc Betaflight [S30] : un filtre à la fois, en vérifiant la
# température des moteurs). D'abord ce qui protège (repliement, D sous-filtré, bruit trop fort), puis la
# mécanique, le filtre RPM, le notch dynamique, enfin les passe-bas du gyro et du D.
FILTER_STAGES = [
    ("Enregistrement", ("Fréquence d'enregistrement trop basse", "Harmoniques moteur au-delà")),
    ("À corriger d'abord", ("LPF2 du gyro coupé", "Filtrage du D insuffisant", "Gyro filtré bruité", "Terme D bruité")),
    ("Mécanique", ("Raie fixe",)),
    ("Filtre RPM", ("Filtre RPM : Q", "Harmonique moteur hors du filtre RPM", "Poids du filtre RPM trop bas", "Raie moteur encore visible", "Bruit moteur mal éliminé",
                    "Filtre RPM plus fort que nécessaire")),
    ("Notch dynamique", ("Raie à ", "Notch dynamique")),
    ("Passe-bas du gyro", ("Le LPF1 du gyro", "Filtrage gyro peut-être trop prudent", "Biquad sur le")),
    ("Passe-bas du D", ("LPF1 du D en mode statique", "Filtrage du D peut-être trop fort", "Terme D propre")),
]

# Stratégie affichée à partir de 2 étapes (décision de Cédric, 2026-10-05) : 37 % des sessions du corpus,
# contre 79 % dès une étape (scripts/advisor_rates.py).
STRATEGY_MIN_STAGES = 2


def main_action(f: Finding) -> str:
    """L'action qui change un réglage, sinon la première."""
    return next((a for a in f.actions if "=" in a or "→" in a or "Curseur" in a), f.actions[0])


def filter_strategy(findings: list[Finding], moving: bool = True) -> Finding | None:
    """Synthèse ordonnée des constats de filtrage : quelles étapes (filtre RPM, notch dynamique, passe-bas du
    gyro et du D) modifier, dans quel ordre. None sous STRATEGY_MIN_STAGES étapes : une seule étape, le
    constat seul dit déjà quoi faire."""
    steps = []
    for stage, prefixes in FILTER_STAGES:
        hits = [f for f in findings if f.actions and f.severity in (ATTENTION, CONSEIL)
                and any(f.title.startswith(p) for p in prefixes)]
        if hits:
            steps.append((stage, hits))
    if len(steps) < STRATEGY_MIN_STAGES:
        return None
    detail = " ; ".join(f"{i + 1}. {stage} : " + ", ".join(f"« {f.title} »" for f in hits)
                        for i, (stage, hits) in enumerate(steps))
    actions = [f"{i + 1}. {stage} — {main_action(hits[0])}" + (f" (+ {len(hits) - 1} autre(s) constat(s))" if len(hits) > 1 else "")
               for i, (stage, hits) in enumerate(steps)]
    actions.append("Un seul changement à la fois, vol d'essai court, puis température des moteurs ; nouveau log "
                   + ("avec mouvements pour juger le retard." if not moving else "après chaque étape."))
    return Finding("Filtres", CONSEIL, "Stratégie de filtrage",
                   "Ordre conseillé des réglages de filtres d'après les constats ci-dessous : " + detail + ".", actions)


def _hover_noise(log, sel, profile, g_ptb, d_ptb, fixed_hz, meas, aliased: bool = False) -> list[Finding]:
    """Vol stationnaire : régime stable, le meilleur log pour identifier le bruit. Bilan des harmoniques
    moteur, des raies qui n'en viennent pas (résonances, bruit résiduel) et du bruit restant ; conseil pour
    chaque raie encore visible après filtrage."""
    s = log.settings
    hz = hover_motor_hz(log)
    lines = []
    for a in range(2):
        for ln in spectrum_lines(log.gyro_unfilt[a, sel], log.gyro[a, sel], log.motor_hz[:, sel], log.fs,
                                 (20.0, 900.0), min_db=HOVER_LINE_MIN_DB, breaks=log.breaks(sel)):
            near = [o for o in lines if abs(o.freq_hz - ln.freq_hz) <= 0.1 * o.freq_hz]
            if not near:
                lines.append(ln)
            elif ln.raw_db > near[0].raw_db:
                lines[lines.index(near[0])] = ln
    lines.sort(key=lambda o: -o.raw_db)
    meas["hover_lines"] = lines
    parts = []
    if hz is not None:
        parts.append(f"moteurs à {hz:.0f} Hz (harmoniques {', '.join(f'{h * hz:.0f}' for h in (1, 2, 3))} Hz)")
    if s.rpm_harmonics > 0:
        raw, left = rpm_harmonic_levels(log, sel)
        harm = [f"{k + 1}e {raw[k]:.0f} → {left[k]:.0f} dB" for k in range(3) if np.isfinite(raw[k]) and np.isfinite(left[k])]
        if harm:
            parts.append("hauteur des harmoniques avant → après filtres : " + ", ".join(harm))
    parts.append("autres raies : " + (", ".join(f"{o.freq_hz:.0f} Hz ({o.raw_db:.0f} → {o.left_db:.0f} dB)" for o in lines[:5])
                                      if lines else "aucune au-dessus de " f"{HOVER_LINE_MIN_DB:.0f} dB"))
    parts.append(f"bruit restant : gyro {g_ptb:.0f} dB (repère {GYRO_PTB_MAX_DB:.0f}), D {d_ptb:.0f} dB "
                 f"(repère {DTERM_PTB_MAX_DB:.0f}) sur l'échelle de PIDtoolbox" if np.isfinite(d_ptb) else
                 f"bruit restant : gyro {g_ptb:.0f} dB (repère {GYRO_PTB_MAX_DB:.0f}) sur l'échelle de PIDtoolbox")
    out = [Finding("Filtres", INFO, "Vol stationnaire : bruit identifié",
                   "Régime stable, raies moteur repérées par l'eRPM : " + " ; ".join(parts) + "."
                   + (" Log trop lent : certaines raies peuvent être des repliements." if aliased else ""))]
    if aliased:
        return out
    for ln in lines:
        if not ln.left_db > HARMONIC_VISIBLE_DB:
            continue
        if fixed_hz is not None and abs(ln.freq_hz - fixed_hz) <= 0.1 * fixed_hz:
            continue  # déjà traitée par le constat « Raie fixe »
        out.append(_residual_line(s, ln, static_notch_cutoff(ln.freq_hz, *_line_shape(log, sel, ln.freq_hz))))
    return out


def static_notch_cutoff(f0: float, width: float, height: float) -> tuple[int, bool]:
    """Coupure (Hz) de gyro_notch1 pour une raie de largeur à mi-puissance `width` et de hauteur `height` (dB) :
    Q nécessaire (dyn_notch_q_need, arrondi comme le notch dynamique, de STATIC_NOTCH_Q_LOW à 450), puis coupure c telle que
    Q = f0·c / (f0² − c²), soit c = f0·(√(1 + 4Q²) − 1) / (2Q). Arrondie vers le bas (notch un peu plus large).
    Renvoie aussi si la raie a été mesurée (sinon STATIC_NOTCH_FALLBACK)."""
    q = dyn_notch_q_need([f0], [width], [height])
    if q is None or not np.isfinite(q):
        return int(f0 * STATIC_NOTCH_FALLBACK), False
    Q = min(_q_round(q, STATIC_NOTCH_Q_LOW), DYN_NOTCH_Q_RPM[1]) / 100
    return int(f0 * (np.sqrt(1 + 4 * Q * Q) - 1) / (2 * Q)), True


def _line_shape(log: FlightLog, sel: slice, f0: float) -> tuple[float, float]:
    """Largeur et hauteur d'une raie dans le gyro non filtré, la plus large et la plus haute du roll et du pitch
    (comme le Q du notch dynamique) ; NaN sans gyro non filtré ou sans eRPM."""
    if log.gyro_unfilt is None or log.motor_hz is None:
        return float("nan"), float("nan")
    sh = [line_shapes(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], log.fs, [f0], log.settings.rpm_harmonics,
                      breaks=log.breaks(sel), rpm_settings=log.settings)[0] for a in range(2)]
    def worst(vals):
        return float(np.nanmax(vals)) if np.any(np.isfinite(vals)) else float("nan")
    return worst([x[0] for x in sh]), worst([x[1] for x in sh])


def _static_notch_text(f0: float, cutoff: tuple[int, bool]) -> str:
    how = "calculée d'après la largeur de la raie" if cutoff[1] else "0,7 × la fréquence : raie trop faible ou absente dans le spectre moyen du gyro non filtré"
    return f"gyro_notch1_hz = {f0:.0f}, gyro_notch1_cutoff = {cutoff[0]} (coupure {how})"


def _residual_line(s, ln, cutoff: tuple[int, bool] | None = None) -> Finding:
    """Raie hors moteurs encore visible après filtrage : où la traiter."""
    f0 = ln.freq_hz
    actions = ["Cherchez d'abord une cause mécanique : vis, antennes, câbles, caméra, pile de cartes, hélices."]
    if dyn_notch_active(s):
        lo, hi = f0 - DYN_NOTCH_MARGIN_HZ, f0 + DYN_NOTCH_MARGIN_HZ
        if lo < s.dyn_notch_min_hz or hi > s.dyn_notch_max_hz:
            new_lo = max(DYN_NOTCH_MIN_HZ, min(s.dyn_notch_min_hz, int(lo // 10 * 10)))
            new_hi = min(DYN_NOTCH_MAX_MAX_HZ, max(s.dyn_notch_max_hz, int(-(-hi // 10) * 10)))
            actions.append(f"Notch dynamique : plage {s.dyn_notch_min_hz}–{s.dyn_notch_max_hz} Hz → dyn_notch_min_hz = "
                           f"{new_lo}, dyn_notch_max_hz = {new_hi} (20 Hz de marge autour de la raie).")
        else:
            actions.append(f"La raie est dans la plage du notch dynamique mais passe quand même : dyn_notch_count "
                           f"{s.dyn_notch_count} → {min(s.dyn_notch_count + 1, 7)} (si le Q est trop haut pour elle, le "
                           "constat « Notch dynamique : Q trop haut » le dit).")
    if cutoff is None:
        cutoff = (int(f0 * STATIC_NOTCH_FALLBACK), False)
    actions.append(f"Ou notch statique : {_static_notch_text(f0, cutoff)}.")
    caveat = ""
    if f0 < HOVER_LOW_LINE_HZ:
        caveat = (f" Sous {HOVER_LOW_LINE_HZ:.0f} Hz, un notch ajoute du retard là où le PID travaille, et la raie peut venir "
                  "d'une oscillation de réglage (P ou D) plutôt que de la mécanique : vérifiez la réponse sur un vol avec "
                  "mouvements avant de filtrer.")
    return Finding("Filtres", CONSEIL, f"Raie à {f0:.0f} Hz encore visible après filtrage",
                   f"En vol stationnaire, une raie qui ne vient pas des moteurs dépasse le bruit voisin de {ln.raw_db:.0f} dB "
                   f"avant filtres et de {ln.left_db:.0f} dB après : les filtres actuels ne la traitent pas." + caveat, actions)


def _lowpass_setup_rules(s) -> list[Finding]:
    """Réglages de passe-bas déconseillés par Betaflight (notes de réglage 4.2 / 4.3, aide du configurateur)."""
    out = []
    order = {"PT1": 1, "PT2": 2, "PT3": 3, "BIQUAD": 2}
    if s.pid_rate_hz < 0.99 * s.gyro_rate_hz and not s.gyro_lpf2.enabled:
        slow = s.pid_rate_hz < 3000
        out.append(Finding("Filtres", ATTENTION if slow else CONSEIL, "LPF2 du gyro coupé : risque de repliement",
                           f"La boucle PID ({s.pid_rate_hz / 1000:.1f} kHz) est plus lente que le gyro "
                           f"({s.gyro_rate_hz / 1000:.1f} kHz) : le LPF2 du gyro sert de filtre anti-repliement avant "
                           "ce sous-échantillonnage. Sans lui, du bruit haute fréquence se replie dans la bande utile.",
                           [f"gyro_lpf2_type = PT1, gyro_lpf2_static_hz = {500 if slow else 1000} "
                            f"({'indispensable à 2 kHz' if slow else 'à 4 kHz : de 500 à 1000 Hz'})."]))
    d_order = sum(order.get(lp.type, 1) for lp in (s.dterm_lpf1, s.dterm_lpf2) if lp.enabled)
    if d_order < 2:
        out.append(Finding("Filtres", ATTENTION, "Filtrage du D insuffisant",
                           "Le terme D n'a qu'un filtre du premier ordre (ou aucun) : Betaflight demande au moins deux PT1 "
                           "ou un filtre du second ordre. Risque de moteurs brûlants ou de départ incontrôlé à l'armement.",
                           ["Réactivez le LPF2 du D (dterm_lpf2_type = PT1, dterm_lpf2_static_hz = valeur haute de la plage "
                            "dynamique du LPF1), ou passez le LPF1 du D en PT2."]))
    if s.dterm_lpf1.static_hz and not s.dterm_lpf1.dynamic:
        f = s.dterm_lpf1.static_hz
        out.append(Finding("Filtres", INFO, "LPF1 du D en mode statique",
                           f"Le LPF1 du D est fixe à {f} Hz. Betaflight déconseille le mode statique pour le D : en mode "
                           "dynamique, il filtre plus à bas régime (grondement moteur, armement) et moins en vol (moins de "
                           "retard, moins de propwash).",
                           [f"dterm_lpf1_dyn_min_hz = {f}, dterm_lpf1_dyn_max_hz = {2 * f} (rapport des valeurs par défaut "
                            "75–150 Hz, gardé par le curseur « filtre D »)."]))
    for name, lp in (("LPF1", s.gyro_lpf1), ("LPF2", s.gyro_lpf2)):
        if lp.enabled and lp.type == "BIQUAD":
            out.append(Finding("Filtres", INFO, f"Biquad sur le {name} du gyro",
                               "Le biquad peut osciller sur les variations brusques et dépasser près de sa coupure ; "
                               "Betaflight recommande le PT2, qui filtre presque autant avec moins de retard.",
                               [f"gyro_{name.lower()}_type = PT2."]))
    return out


def rpm_harmonic_levels(log: FlightLog, sel: slice) -> tuple[np.ndarray, np.ndarray]:
    """Hauteur de chaque harmonique moteur au-dessus du bruit voisin (dB, maximum du roll et du pitch),
    avant filtres (gyro non filtré) et après (gyro filtré)."""
    raw = [harmonic_prominence(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], log.fs, breaks=log.breaks(sel))
           for a in range(2)]
    left = [harmonic_prominence(log.gyro[a, sel], log.motor_hz[:, sel], log.fs, breaks=log.breaks(sel))
            for a in range(2)]
    with np.errstate(all="ignore"):
        return np.fmax(*raw), np.fmax(*left)


def _rpm_weight_rules(log: FlightLog, sel: slice, s, lighten: bool = True, h1_flagged: bool = False) -> list[Finding]:
    """Poids du filtre RPM : raie restée visible avec un poids réduit, ou poids plus fort que nécessaire
    (seulement si `lighten`, c'est-à-dire si le bruit moteur est bien éliminé)."""
    raw, left = rpm_harmonic_levels(log, sel)
    n = min(s.rpm_harmonics, 3)
    cur = [w / 100 for w in (list(s.rpm_weights) + [100] * 3)[:3]]
    out = []
    low = [k for k in range(n) if left[k] > HARMONIC_VISIBLE_DB and cur[k] < 1.0]
    if low:
        out.append(Finding("Filtres", CONSEIL, "Poids du filtre RPM trop bas",
                           "Raie moteur encore visible après filtrage : " + " ; ".join(
                               f"{k + 1}e harmonique {left[k]:.1f} dB au-dessus du bruit voisin, poids {cur[k] * 100:.0f}"
                               for k in low) + ".",
                           ["rpm_filter_weights : remontez le poids de cette harmonique (vers 100) avant de toucher à "
                            "rpm_filter_q."]))
    missing = [k for k in range(n, 3) if np.isfinite(left[k]) and left[k] > HARMONIC_VISIBLE_DB]
    if missing:
        out.append(Finding("Filtres", CONSEIL, "Harmonique moteur hors du filtre RPM",
                           f"Le filtre RPM ne traite que {n} harmonique(s) ; " + " ; ".join(
                               f"la {k + 1}e reste {left[k]:.0f} dB au-dessus du bruit voisin après filtrage" for k in missing)
                           + ".",
                           [f"rpm_filter_harmonics = {max(missing) + 1} (actuellement {n}), en réglant son poids avec "
                            "rpm_filter_weights (4.5 et plus)."]))
    # 1re harmonique déjà signalée par « Bruit moteur mal éliminé » : pas de doublon
    full = [k for k in range(n) if left[k] > HARMONIC_VISIBLE_DB and cur[k] >= 1.0 and not (k == 0 and h1_flagged)]
    if full:
        q_new = max(RPM_Q_MIN, s.rpm_q - RPM_Q_STEP)  # comme « Bruit moteur mal éliminé »
        out.append(Finding("Filtres", CONSEIL, "Raie moteur encore visible après le filtre RPM",
                           "Malgré un poids de 100 : " + " ; ".join(
                               f"{k + 1}e harmonique {raw[k]:.0f} dB avant filtres, {left[k]:.0f} dB après (au-dessus du "
                               "bruit voisin)" for k in full) + ". Les notchs RPM sont trop étroits pour la raie, ou mal placés.",
                           ["Vérifiez motor_poles (14 pour la plupart des moteurs, 12 pour certains micro-moteurs) : un "
                            "mauvais nombre de pôles décale tous les notchs.",
                            f"rpm_filter_q {s.rpm_q} → {q_new} (notchs plus larges, un peu plus de retard), puis par "
                            f"pas de {RPM_Q_STEP} tant que la raie reste visible."]))
    if not lighten or (s.version and s.version < (4, 5)):
        return out  # rpm_filter_weights n'existe qu'à partir de Betaflight 4.5
    new = list(cur)
    for k in range(1, n):  # 2e et 3e harmoniques, comme la documentation de Betaflight
        if not (np.isfinite(raw[k]) and left[k] <= HARMONIC_VISIBLE_DB):
            continue
        need = np.ceil(10 * rpm_weight_needed(raw[k] + RPM_WEIGHT_MARGIN_DB) - 1e-9) / 10
        if cur[k] - need >= RPM_WEIGHT_GAIN_MIN - 1e-9:
            new[k] = need
    if new != cur:
        fmt = lambda w: ", ".join(f"{x * 100:.0f}" for x in w[:max(n, 1)])  # noqa: E731
        changed = [k for k in range(n) if new[k] != cur[k]]
        out.append(Finding("Filtres", CONSEIL, "Filtre RPM plus fort que nécessaire",
                           "Avant filtrage, " + " ; ".join(
                               f"la {k + 1}e harmonique ne dépasse le bruit voisin que de {raw[k]:.1f} dB" if raw[k] > 0
                               else f"la {k + 1}e harmonique ne dépasse pas le bruit voisin" for k in changed) +
                           f" : un poids de {', '.join(f'{new[k] * 100:.0f}' for k in changed)} suffit à l'effacer "
                           f"(marge de {RPM_WEIGHT_MARGIN_DB:.0f} dB). Moins de filtrage = moins de retard.",
                           [f"rpm_filter_weights = {fmt(new)} (actuellement {fmt(cur)}).",
                            "Vérifiez sur le log suivant que les raies moteur restent invisibles dans le spectre du gyro "
                            "filtré et du terme D."]))
    return out


def _rpm_q_raise(log: FlightLog, sel: slice, s, F: list[Finding], hover: bool) -> Finding | None:
    """Toutes les harmoniques traitées effacées avec le Q actuel et bruit sous les repères : des notchs plus
    étroits suffiraient (moins de retard). Pas de 100 jusqu'à 1000 [S27] ; décision de Cédric (2026-10-10)."""
    _raw, left = rpm_harmonic_levels(log, sel)
    treated = [k for k in range(min(s.rpm_harmonics, 3)) if np.isfinite(left[k])]
    if not treated or any(left[k] > HARMONIC_VISIBLE_DB for k in treated):
        return None
    levels = ", ".join(f"{k + 1}e {left[k]:.0f} dB" for k in treated)
    then = (" Un réglage à la fois : d'abord les poids (« Filtre RPM plus fort que nécessaire »), puis Q."
            if any(f.title == "Filtre RPM plus fort que nécessaire" for f in F) else "")
    return Finding("Filtres", CONSEIL, "Filtre RPM : Q peut monter",
                   f"Les harmoniques traitées sont effacées après filtres (au-dessus du bruit voisin : {levels} ; "
                   f"visible au-delà de {HARMONIC_VISIBLE_DB:.0f} dB) et le bruit du gyro et du D est sous les repères : "
                   "des notchs plus étroits suffiraient, avec moins de retard." + then +
                   (" Vol stationnaire : bruit vu à un seul régime, à confirmer sur un vol avec des gaz plus hauts."
                    if hover else ""),
                   [f"rpm_filter_q {s.rpm_q} → {min(s.rpm_q + RPM_Q_STEP, RPM_Q_MAX)}, puis par pas de {RPM_Q_STEP} "
                    f"jusqu'à {RPM_Q_MAX} au plus, en vérifiant à chaque pas que les raies moteur ne réapparaissent pas "
                    "dans le gyro filtré.",
                    "Température des moteurs après chaque vol d'essai."])


def _dyn_notch_q_n_rules(s, dn: dict, lines_pass: bool, gyro_noisy: bool, dterm_noisy: bool, F: list[Finding],
                         hover: bool) -> list[Finding]:
    """Q et nombre de notchs du notch dynamique (avec filtre RPM) d'après les raies à suivre (décision de
    Cédric, 2026-10-10) : Q baissé si une raie est trop large ou trop haute pour le Q actuel et qu'une raie à
    suivre dépasse encore HARMONIC_VISIBLE_DB dans le gyro filtré (choix de Cédric, 2026-10-13 : sans ce
    contrôle, la règle se trompait une fois sur deux, docs/assistant.md « Notch dynamique »), monté (450 au
    plus) si rien ne passe et que le bruit est faible ; un notch de plus par pic à suivre quand une raie passe
    ou que le gyro est bruité."""
    out = []
    q, need = s.dyn_notch_q, dn["q_need"]
    shapes = ", ".join(f"{f0:.0f} Hz ({w:.0f} Hz de large, {h:+.0f} dB)" for f0, w, h
                       in zip(dn["peaks"], dn["widths"], dn["heights"]) if np.isfinite(w) and np.isfinite(h))
    passing = [(f0, h) for f0, h in zip(dn["peaks"], dn.get("left", [])) if h > HARMONIC_VISIBLE_DB]
    if need is not None and np.isfinite(need) and _q_round(need) < q and passing:
        new = _q_round(need)
        seen = ", ".join(f"{f0:.0f} Hz ({h:+.0f} dB)" for f0, h in passing)
        floor = need < DYN_NOTCH_Q_LOW  # raie trop large ou trop haute pour un notch dynamique raisonnable
        out.append(Finding("Filtres", CONSEIL, "Notch dynamique : Q trop haut pour les raies à suivre",
                           f"Raies à suivre après le filtre RPM (largeur à mi-puissance, hauteur au-dessus du bruit "
                           f"voisin) : {shapes}. Encore visible dans le gyro filtré : {seen}. Avec dyn_notch_q {q}, le "
                           f"notch est trop étroit pour ramener la plus exigeante à {HARMONIC_VISIBLE_DB:.0f} dB du "
                           "bruit voisin sur toute sa largeur ; " + (
                               f"le calcul (forme du notch de Betaflight) demanderait un Q de {need:.0f}, sous "
                               f"{DYN_NOTCH_Q_LOW}, le plus bas Q utilisé avec filtre RPM : un notch aussi large "
                               "ajouterait beaucoup de retard." if floor else
                               f"il faut un Q de {new} au plus (calculé d'après la forme du notch de Betaflight)."),
                           [f"dyn_notch_q {q} → {new} (notchs plus larges, un peu plus de retard)."] + (
                               ["Si la raie reste visible : cherchez sa cause mécanique (vis, hélices, roulements, "
                                "antenne, caméra, pile de cartes) plutôt que d'élargir encore le notch."] if floor else [])))
    elif not (lines_pass or gyro_noisy or dterm_noisy) and q < DYN_NOTCH_Q_RPM[1]:
        if need is None:  # aucune raie mesurable : pas de calcul, petit pas
            new = min(q + DYN_NOTCH_Q_STEP, DYN_NOTCH_Q_RPM[1])
            why = "aucune raie à suivre n'est mesurable"
        else:
            new = DYN_NOTCH_Q_RPM[1] if not np.isfinite(need) else min(_q_round(need), DYN_NOTCH_Q_RPM[1])
            why = f"raies à suivre : {shapes} ; un Q de {new} suffit encore à les effacer" if shapes else \
                "les raies à suivre dépassent à peine le bruit voisin"
        if new > q:
            seen = ("aucune raie ne passe les filtres en vol stationnaire" if hover else
                    "aucune raie fixe n'est signalée")
            out.append(Finding("Filtres", CONSEIL, "Notch dynamique : Q peut monter",
                               f"Avec le filtre RPM, le notch dynamique ne traite que les résonances qui ne viennent pas "
                               f"des moteurs ; {seen} et le bruit du gyro et du D est sous les repères ({why}). Un Q "
                               "plus haut donne des notchs plus étroits, avec moins de retard.",
                               [f"dyn_notch_q {q} → {new} ({DYN_NOTCH_Q_RPM[1]} au plus avec filtre RPM), puis vérifiez "
                                "que les raies de résonance ne réapparaissent pas dans le gyro filtré."]))
    n_peaks = len(dn["peaks"])
    said = any("dyn_notch_count" in a for f in F for a in f.actions)
    if n_peaks > dn["count"] and (lines_pass or gyro_noisy) and not said:
        n = min(n_peaks, 7)
        out.append(Finding("Filtres", CONSEIL, "Notch dynamique : plus de notchs",
                           f"{n_peaks} pics à suivre après le filtre RPM ({', '.join(f'{f:.0f}' for f in dn['peaks'])} Hz) "
                           f"pour {dn['count']} notch(s), et " + ("une raie passe les filtres." if lines_pass else
                                                                 "le gyro filtré est bruité."),
                           [f"dyn_notch_count {dn['count']} → {n} (un notch par pic à suivre)."]))
    return out


def _loop_caveat(lp) -> str:
    """Précision ajoutée au constat quand la boucle ouverte n'a pas pu être mesurée."""
    if lp.measured:
        return ""
    why = f" ({'; '.join(lp.notes)})" if lp.notes else ""
    return (f" Valeur approchée{why} : calculée sans séparer le D (sur le gyro) et le feedforward (sur la consigne), "
            "elle peut s'écarter de plusieurs dB ou dizaines de degrés.")


def _whoop_margin_note(profile: CraftProfile) -> str:
    """Précision pour un whoop : marge faible fréquente et réelle (vérifiée sur la simulation et les vols
    chirp de l'AIR65, docs/validation.md), seuil théorique et non percentile du corpus."""
    if profile.key != "whoop":
        return ""
    return (" Fréquent sur un whoop : la boucle est plus rapide (croisement vers 20 Hz) et le D y domine, "
            "si bien que les filtres du D retardent beaucoup la boucle au croisement. Le seuil vient de la "
            "théorie (pas d'une comparaison avec les autres whoops) : la marge est vraiment faible.")


def _edge_actions(lp) -> list[str]:
    """Pic au bord haut de la bande mesurée : la mesure s'arrête où s'arrête l'énergie des mouvements (W-004).
    Des wobbles plus vifs (protocole de Brian White [S33]) la font aller plus loin ; pas de vol chirp (D-019)."""
    if not lp.sensitivity_peak_at_edge:
        return []
    return [f"Mesure arrêtée à {lp.sensitivity_peak_hz:.0f} Hz, au bord de ce que vos mouvements excitent : le vrai "
            "pic peut être plus haut. Des wobbles plus rapides et plus secs, au hasard sur le roll et le pitch en "
            "même temps, la font aller plus loin."]


def _margin_actions(lp) -> list[str]:
    """Que faire d'une marge faible. Quand le D domine déjà le P et le I au croisement (|Cd/Ce| > 1),
    plus de D fait surtout monter le croisement là où les filtres retardent : il apporte peu de marge
    et peut augmenter le pic de sensibilité (docs/assistant.md, « Boucle ouverte mesurée »)."""
    if not lp.measured:
        return ["Master −0,1 à −0,2.", "Pour une mesure exacte, enregistrez les termes P, I, D et F (champs PID du blackbox)."]
    if np.isfinite(lp.d_ratio) and lp.d_ratio > D_DOMINANT_RATIO:
        return [f"Master −0,1 à −0,2. Le D domine déjà au croisement ({lp.d_ratio:.1f} × P et I) : "
                "plus de D apporterait peu de marge et peut augmenter le pic de sensibilité."]
    return ["Damping (D) +0,1, ou Master −0,1 à −0,2."]


def _imbalance_finding(m: list[float], imbalance: float) -> Finding:
    """Interprète le déséquilibre pour un quad en X (ordre Betaflight : 1 AR-D, 2 AV-D, 3 AR-G, 4 AV-G)."""
    values = ", ".join(f"M{i + 1} {v:.0f} %" for i, v in enumerate(m))
    actions = ["Un moteur qui force seul peut aussi être un moteur abîmé, une hélice tordue ou un ESC fatigué."]
    cause = f"Le moteur {int(np.argmax(m)) + 1} travaille {imbalance:.0f} % de plus que la moyenne."
    if len(m) == 4:
        mean = float(np.mean(m))
        front, rear = (m[1] + m[3]) / 2, (m[0] + m[2]) / 2
        left, right = (m[2] + m[3]) / 2, (m[0] + m[1]) / 2
        diag_a, diag_b = (m[0] + m[3]) / 2, (m[1] + m[2]) / 2
        gaps = {"avant": front - rear, "arrière": rear - front, "gauche": left - right, "droite": right - left}
        side, gap = max(gaps.items(), key=lambda kv: kv[1])
        dgap = abs(diag_a - diag_b)
        if gap / mean > 0.12 and gap >= dgap:
            cause = f"Les moteurs côté {side} travaillent {100 * gap / mean:.0f} % de plus : centre de gravité décalé vers l'{side}" \
                if side in ("avant", "arrière") else \
                f"Les moteurs côté {side} travaillent {100 * gap / mean:.0f} % de plus : centre de gravité décalé à {side}."
            actions.insert(0, f"Déplacez la batterie (ou la caméra) pour recentrer le poids, à l'opposé du côté {side}.")
        elif dgap / mean > 0.12:
            pair = "M1/M4" if diag_a > diag_b else "M2/M3"
            cause = (f"Les moteurs en diagonale {pair} travaillent {100 * dgap / mean:.0f} % de plus : le drone compense "
                     "un couple en lacet (yaw).")
            actions.insert(0, "Vérifiez le sens et le modèle des hélices, et qu'un moteur ne frotte pas ; un châssis "
                              "vrillé ou un moteur penché donne le même symptôme.")
    return Finding("Mécanique", CONSEIL, "Moteurs déséquilibrés", f"{cause} ({values}).", actions)


def frame_resonance(log: FlightLog, sel: slice, lo_hz: float = 20.0) -> tuple[float, float, bool] | None:
    """Raie la plus marquée au-dessus du bruit voisin, quel que soit le gaz : (fréquence en Hz, excès en dB,
    proche d'une harmonique moteur). None si le log ne couvre pas assez de tranches de gaz."""
    x = log.gyro_unfilt if log.gyro_unfilt is not None else log.gyro
    lo, hi = lo_hz, min(0.9 * log.fs / 2, 900.0)
    prof = []
    for a in range(2):
        ft = spectral.freq_throttle(x[a, sel], log.throttle[sel], log.fs, mode="amp", breaks=log.breaks(sel))
        ok = ft.counts > 3
        if ok.sum() < 8:
            return None
        prof.append(np.nanmedian(ft.z[:, ok], axis=1))
    f = ft.freqs
    p = 20 * np.log10(np.maximum(np.mean(prof, axis=0), 1e-9))
    m = (f >= lo) & (f <= hi)
    fm, pm = f[m], p[m]
    if pm.size < 10:
        return None
    base = np.convolve(pm, np.ones(15) / 15, mode="same")
    excess = pm - base
    # dès 20 Hz (dyn_notch_min_hz descend jusque-là), sans les 5 premiers Hz : une raie collée au bord vient
    # des mouvements du drone ou d'un test d'excitation (wobble, chirp), pas d'une résonance
    excess[fm < lo + 5] = -np.inf
    k = int(np.argmax(excess))
    f0 = float(fm[k])
    # une raie moteur se déplace avec le gaz : si f0 correspond à la vitesse moteur en stationnaire,
    # ce n'est probablement pas une résonance
    hz = hover_motor_hz(log)
    near_motor = hz is not None and any(abs(f0 - h * hz) < 0.08 * h * hz for h in (1, 2, 3))
    return f0, float(excess[k]), near_motor


def _frame_resonance(log: FlightLog, res, profile: CraftProfile, sel: slice | None = None) -> Finding | None:
    """Raie de fréquence fixe quel que soit le gaz (résonance du châssis ou pièce desserrée) ; `res` vient
    de frame_resonance."""
    if res is None:
        return None
    f0, excess, near_motor = res
    if excess < profile.resonance_high_db or near_motor:
        return None
    actions = ["Resserrez les vis, fixez antennes et câbles, vérifiez la caméra et la pile de cartes."]
    s = log.settings
    if dyn_notch_active(s):
        lo, hi = f0 - DYN_NOTCH_MARGIN_HZ, f0 + DYN_NOTCH_MARGIN_HZ
        if lo < s.dyn_notch_min_hz or hi > s.dyn_notch_max_hz:
            new_lo = max(DYN_NOTCH_MIN_HZ, min(s.dyn_notch_min_hz, int(lo // 10 * 10)))
            new_hi = min(DYN_NOTCH_MAX_MAX_HZ, max(s.dyn_notch_max_hz, int(-(-hi // 10) * 10)))
            actions.append(f"Le notch dynamique ({s.dyn_notch_min_hz}–{s.dyn_notch_max_hz} Hz) ne couvre pas cette raie à "
                           f"{DYN_NOTCH_MARGIN_HZ} Hz près : dyn_notch_min_hz = {new_lo}, dyn_notch_max_hz = {new_hi}.")
        else:
            actions.append(f"Elle est dans la plage du notch dynamique ({s.dyn_notch_min_hz}–{s.dyn_notch_max_hz} Hz), qui "
                           "peut la suivre si un notch est libre.")
    cutoff = static_notch_cutoff(f0, *_line_shape(log, sel if sel is not None else slice(0, log.n), f0))
    actions.append(f"Si elle persiste : notch statique gyro à {f0:.0f} Hz, {_static_notch_text(f0, cutoff)}.")
    return Finding("Mécanique", CONSEIL, f"Raie fixe vers {f0:.0f} Hz",
                   f"Une vibration à ~{f0:.0f} Hz reste présente quel que soit le gaz (+{excess:.0f} dB au-dessus du "
                   "bruit voisin) : résonance du châssis, d'une antenne, d'une caméra ou pièce desserrée.", actions)


def dyn_notch_active(s) -> bool:
    """Notch dynamique en service : au moins un notch et boucle PID ≥ 2 kHz (`DYN_NOTCH_UPDATE_MIN_HZ`)."""
    return s.dyn_notch_count > 0 and s.dyn_notch_max_hz > 0 and s.pid_rate_hz >= 2000


def dyn_notch_view(log: FlightLog, sel: slice, fixed_hz: float | None = None) -> dict | None:
    """Pics que le notch dynamique a à suivre (roll ou pitch, celui qui en compte le plus, plus la raie fixe
    signalée `fixed_hz`), avec filtre RPM actif et gyro non filtré ; None sinon (sans filtre RPM, il doit
    aussi suivre les raies moteur). La recherche déborde de deux cases SDFT de chaque côté de la plage, pour
    ne pas manquer un pic collé à une limite."""
    s = log.settings
    if not dyn_notch_active(s) or log.gyro_unfilt is None or log.motor_hz is None or s.rpm_harmonics == 0:
        return None
    band = (max(s.dyn_notch_min_hz - 37, DYN_NOTCH_MIN_HZ), s.dyn_notch_max_hz + 37)
    t = [dyn_notch_targets(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], log.fs, band, s,
                           breaks=log.breaks(sel)) for a in range(2)]
    peaks = [float(f) for f in max(t, key=lambda x: x.freqs.size).freqs]
    if fixed_hz is not None and all(abs(f - fixed_hz) > 37 for f in peaks):
        peaks.append(float(fixed_hz))
    peaks = sorted(peaks)
    # forme de chaque raie : la plus large et la plus haute du roll et du pitch (côté prudent)
    sh = [line_shapes(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], log.fs, peaks, s.rpm_harmonics,
                      breaks=log.breaks(sel), rpm_settings=s) for a in range(2)]
    def worst(vals):
        return float(np.nanmax(vals)) if np.any(np.isfinite(vals)) else float("nan")
    widths = [worst([x[i][0] for x in sh]) for i in range(len(peaks))]
    heights = [worst([x[i][1] for x in sh]) for i in range(len(peaks))]
    # hauteur de chaque raie dans le gyro filtré (après tous les filtres, notch dynamique compris)
    left = [worst(v) for v in zip(*(line_heights(log.gyro[a, sel], log.fs, peaks, breaks=log.breaks(sel))
                                    for a in range(2)))] if peaks else []
    return {"count": s.dyn_notch_count, "min_hz": s.dyn_notch_min_hz, "max_hz": s.dyn_notch_max_hz,
            "peaks": peaks, "widths": widths, "heights": heights, "left": left,
            "q_need": dyn_notch_q_need(peaks, widths, heights)}


def dyn_notch_q_need(peaks: list[float], widths: list[float], heights: list[float]) -> float | None:
    """Plus haut Q du notch dynamique (× 100, comme Betaflight) qui ramène chaque raie à HARMONIC_VISIBLE_DB au plus du
    bruit voisin sur sa demi-largeur. Notch centré en f0, à l'écart d : |H| ≈ 2d / √((2d)² + (f0/Q)²), d'où
    Q = f0·h / (2d·√(1 − h²)) pour une atténuation h. None sans raie mesurable ; inf si aucune n'en a besoin."""
    qs = []
    for f0, w, ht in zip(peaks, widths, heights):
        if not (np.isfinite(w) and w > 0 and np.isfinite(ht)):
            continue
        need_db = ht - HARMONIC_VISIBLE_DB
        if need_db <= 0:
            qs.append(float("inf"))
            continue
        h = 10 ** (-need_db / 20)
        qs.append(100 * f0 * h / (w * np.sqrt(1 - h * h)))  # 2d = w
    return min(qs) if qs else None


def _q_round(q: float, low: int = DYN_NOTCH_Q_LOW) -> int:
    return int(max(low, q // DYN_NOTCH_Q_STEP * DYN_NOTCH_Q_STEP))
