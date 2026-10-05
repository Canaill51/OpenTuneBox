"""Assistant de réglage : mesures → constats argumentés → suggestions.

Principes :
- chaque constat cite la mesure qui le justifie (valeur, seuil, axe) ;
- les suggestions passent de préférence par les curseurs du réglage simplifié de Betaflight
  (une seule chose à la fois, par petits pas de 0,1), puis par des réglages précis si besoin ;
- les seuils dépendent de la taille du drone (profiles.py) et sont calibrés sur 708 sessions réelles
  (90e percentile de chaque taille, voir docs/assistant.md).
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from ..analysis import spectral
from ..analysis.delay import compute_delays
from ..analysis.motors import harmonic_prominence, hover_motor_hz, motor_noise, rpm_weight_needed
from ..analysis.period import crash_time
from ..analysis.sliders import slider_view
from ..analysis.stats import flight_stats, pid_error_stats
from ..analysis.step import StepResult, step_response
from ..log.flightlog import AXES, FlightLog
from .profiles import CraftProfile, detect_profile, filter_delay_target_ms

# Stabilité (boucle ouverte mesurée), en conseil : Ms ≤ ~2 (6 dB) et marge 30–60° dans les manuels ;
# 7 dB et 35° retenus sur le corpus du 2026-10-05 (20 % des sessions au lieu de 33 %, docs/assistant.md)
SENSITIVITY_HIGH_DB = 7.0
PHASE_MARGIN_LOW_DEG = 35.0
# Repères de Brian White sur l'échelle des spectres de PIDtoolbox (docs/spec-methodes.md, [S20]–[S22]) :
# terme D au plus −10 dB (déclenche le constat, avec le p90 du corpus), gyro filtré sous −30 dB (affiché)
DTERM_PTB_MAX_DB = -10.0
GYRO_PTB_MAX_DB = -30.0
# Filtre RPM (doc Betaflight 4.5, [S27]) : monter Q par pas jusqu'à 1000 au plus pour gagner du retard ;
# poids (4.5 et plus) juste suffisant pour que la raie ne se voie plus, avec une marge de 6 dB, proposé
# s'il fait gagner au moins 0,5 (8 % des sessions du corpus) ; raie « visible » à plus de 3 dB du bruit voisin
RPM_Q_STEP, RPM_Q_MAX = 250, 1000
RPM_WEIGHT_MARGIN_DB = 6.0
RPM_WEIGHT_GAIN_MIN = 0.5
HARMONIC_VISIBLE_DB = 3.0

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


def _band_db(x: np.ndarray, fs: float, lo: float, hi: float) -> float:
    s = spectral.spectrum(x, fs, resolution_hz=2.0)
    hi = min(hi, 0.95 * fs / 2)
    m = (s.freqs >= lo) & (s.freqs <= hi)
    return float(np.mean(s.values[m])) if m.any() else float("nan")


def band_peak_ptb_db(x: np.ndarray, fs: float, lo: float, hi: float) -> float:
    """Sommet du spectre lissé dans la bande, sur l'échelle de PIDtoolbox (ce que l'œil lit sur sa courbe)."""
    s = spectral.spectrum(x, fs, resolution_hz=2.0, smoothing=2)
    hi = min(hi, 0.95 * fs / 2)
    m = (s.freqs >= lo) & (s.freqs <= hi)
    return float(np.max(s.values[m])) - spectral.PTB_PSD_OFFSET_DB if m.any() else float("nan")


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

    # ------------------------------------------------------------ 1. qualité de l'enregistrement
    hz = hover_motor_hz(log)
    meas["hover_motor_hz"] = hz
    if fs < 900:
        F.append(Finding("Enregistrement", ATTENTION, "Fréquence d'enregistrement trop basse",
                         f"Le log est enregistré à {fs:.0f} Hz : on ne voit rien au-dessus de {fs / 2:.0f} Hz.",
                         ["Réglez blackbox_sample_rate pour enregistrer à 2 kHz (1/2 ou 1/4 selon la boucle PID)."]))
    if hz is not None and 3 * hz > fs / 2:
        F.append(Finding("Enregistrement", CONSEIL, "Harmoniques moteur au-delà de la plage visible",
                         f"Les moteurs tournent à ~{hz:.0f} Hz en vol : la 3e harmonique ({3 * hz:.0f} Hz) dépasse "
                         f"la moitié de la fréquence d'enregistrement ({fs / 2:.0f} Hz).",
                         ["Pour étudier le filtrage, enregistrez plus vite (4 kHz) sur un vol court."]))
    if log.gyro_unfilt is None:
        F.append(Finding("Enregistrement", CONSEIL, "Gyro non filtré absent",
                         "Sans gyro non filtré, impossible de mesurer l'efficacité des filtres ni leur retard.",
                         ["En 4.4 : debug_mode = GYRO_SCALED. En 4.5 et plus : ne désactivez pas le champ gyroUnfilt."]))
    if log.motor_hz is None:
        F.append(Finding("Enregistrement", CONSEIL, "Pas de télémétrie RPM",
                         "Sans eRPM, le filtre RPM est inactif et les harmoniques moteur ne peuvent qu'être estimées.",
                         ["Activez dshot_bidir si vos ESC le permettent (BLHeli_32, Bluejay, AM32)."]))
    if log.stats.get("corrupt_frames", 0) > 20:
        F.append(Finding("Enregistrement", INFO, "Trames corrompues",
                         f"{log.stats['corrupt_frames']} trames corrompues (pauses d'écriture de la carte SD ou de la "
                         "mémoire flash) : quelques trous dans le log, sans conséquence pour l'analyse.",
                         ["Si c'est fréquent : carte SD plus rapide, ou fréquence d'enregistrement plus basse."]))
    ct = crash_time(log)
    if ct is not None:
        F.append(Finding("Vol", INFO, "Crash détecté",
                         f"Crash vers {ct:.1f} s : la période d'analyse s'arrête avant."))

    # ------------------------------------------------------------ 2. réponse indicielle
    steps = []
    for ax in range(3):
        r = step_response(log.setpoint[ax, sel], log.gyro[ax, sel], fs, terms=log.pid_terms(ax, sel))
        steps.append(AxisStep(AXES[ax], r))
    meas["steps"] = {st.axis: st.result.metrics for st in steps}
    for ax, st in enumerate(steps[:2]):
        _step_rules(F, st, profile, s.pids[ax].F)
    if steps[0].result.n < 20 and steps[1].result.n < 20:
        F.append(Finding("Réponse", ATTENTION, "Pas assez de mouvements de manche",
                         f"Seulement {steps[0].result.n} fenêtres exploitables en roll et {steps[1].result.n} en pitch "
                         "(il faut des mouvements de plus de 20 °/s).",
                         ["Volez 20 à 30 s avec des mouvements francs et des flips/rolls rapides sur chaque axe."]))
    r0, r1 = steps[0].result, steps[1].result
    if r0.n >= 20 and r1.n >= 20 and np.isfinite(r0.metrics.peak) and np.isfinite(r1.metrics.peak):
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
    nyq = fs / 2
    hi = min(500.0, 0.95 * nyq)
    g_db = [_band_db(log.gyro[a, sel], fs, 80, hi) for a in range(2)]
    d_db = [_band_db(log.d_term[a, sel], fs, 80, hi) for a in range(2) if np.any(log.d_term[a])]
    low_db = [_band_db(log.gyro[a, sel], fs, 20, 80) for a in range(2)]
    g_ptb = max(band_peak_ptb_db(log.gyro[a, sel], fs, 80, hi) for a in range(2))
    d_ptb = max((band_peak_ptb_db(log.d_term[a, sel], fs, 80, hi) for a in range(2) if np.any(log.d_term[a])),
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
                          "Curseur « filtre gyro » : −0,1 à −0,2."]))
    elif np.isfinite(g_ptb):
        F.append(Finding("Bruit", OK if g_ptb <= GYRO_PTB_MAX_DB else INFO, "Bruit du gyro filtré",
                         f"Sommet du spectre à {g_ptb:.0f} dB sur l'échelle de PIDtoolbox (repère : sous "
                         f"{GYRO_PTB_MAX_DB:.0f} dB) ; bruit moyen {max(g_db):.1f} dB, dans les 90 % du corpus "
                         f"(seuil {profile.gyro_noise_high_db:.1f} dB)."))
    if not dterm_noisy and np.isfinite(d_ptb):
        F.append(Finding("Bruit", OK, "Bruit du terme D",
                         f"Sommet du spectre à {d_ptb:.0f} dB sur l'échelle de PIDtoolbox (repère : au plus "
                         f"{DTERM_PTB_MAX_DB:.0f} dB, idéal sous −20 dB)."))
    target = filter_delay_target_ms(profile.nominal_inch)
    g_goal = (f" Objectif de Brian White pour un {profile.nominal_inch:g} pouces : environ {target[0]:.2f} ms."
              if target else "")
    if np.isfinite(delays.gyro_ms):
        if delays.gyro_ms > profile.gyro_delay_max_ms and not gyro_noisy and not dterm_noisy:
            actions = ["Curseur « filtre gyro » : +0,1 à +0,2 (moins de retard → meilleure réactivité, moins de propwash)."]
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
    if np.isfinite(delays.dterm_ms):
        d_goal = (f" Objectif de Brian White pour un {profile.nominal_inch:g} pouces : environ {target[1]:.2f} ms "
                  "lus par PIDtoolbox, qui mesure le retard du D environ 0,5 ms plus court qu'ici." if target else "")
        if delays.dterm_ms > profile.dterm_delay_max_ms and not dterm_noisy:
            F.append(Finding("Filtres", CONSEIL, "Filtrage du D peut-être trop fort",
                             f"Retard des filtres D {delays.dterm_ms:.2f} ms (repère {profile.dterm_delay_max_ms:.1f} ms) "
                             "avec un D peu bruité." + d_goal,
                             ["Curseur « filtre D » : +0,1."]))
        elif delays.dterm_ms <= profile.dterm_delay_max_ms:
            F.append(Finding("Filtres", OK, "Retard des filtres D",
                             f"{delays.dterm_ms:.2f} ms (repère ≤ {profile.dterm_delay_max_ms:.1f} ms pour {profile.label})." + d_goal))
    if log.motor_hz is not None and log.gyro_unfilt is not None and s.rpm_harmonics > 0:
        mn = [motor_noise(log.gyro[a, sel], log.motor_hz[:, sel], fs) for a in range(2)]
        mp = [motor_noise(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], fs) for a in range(2)]
        left = [float(np.nanmax([m.mean_db[0] for m in mn]))]
        meas["motor_harmonic_left_db"] = left[0]
        meas["motor_harmonic_raw_db"] = float(np.nanmax([m.mean_db[0] for m in mp]))
        if left[0] > profile.motor_left_high_db:
            F.append(Finding("Filtres", CONSEIL, "Bruit moteur mal éliminé",
                             f"Il reste {left[0]:.1f} dB sur la fréquence de rotation des moteurs après filtrage "
                             f"(avant filtrage : {meas['motor_harmonic_raw_db']:.1f} dB).",
                             ["Vérifiez motor_poles (14 pour la plupart des moteurs, 12 pour certains micro-moteurs).",
                              "Filtre RPM : baisser rpm_filter_q (ex. 500 → 350) élargit les notchs."]))
        F.extend(_rpm_weight_rules(log, sel, s, lighten=left[0] <= profile.motor_left_high_db))
    if max(low_db) > profile.low_band_high_db:
        F.append(Finding("Réponse", CONSEIL, "Oscillations basses fréquences (20–80 Hz)",
                         f"Énergie du gyro filtré entre 20 et 80 Hz : {max(low_db):.1f} dB (seuil {profile.low_band_high_db:.1f} dB, "
                         "90 % des drones de cette taille font mieux). C'est la zone du propwash "
                         "et des oscillations de réglage.",
                         ["Si cela arrive en descente/gaz coupés : propwash → plus de D (Damping +0,1) ou D max.",
                          "Si cela arrive en vol stable : oscillation → baisser le Master ou P (Tracking −0,1).",
                          "Regardez l'outil Fréquence × gaz pour voir à quel gaz le phénomène apparaît."]))
    res = _frame_resonance(log, sel, profile)
    if res is not None:
        F.append(res)

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

    return Report(profile, reason, (t0, t1), F, steps, meas)


def _step_rules(F: list[Finding], st: AxisStep, profile: CraftProfile, ff_gain: int = 0) -> None:
    r = st.result
    ax = st.axis
    if r.n < 20:
        return
    m = r.metrics
    lp = r.loop
    pk, lat = m.peak, m.latency_ms
    if pk > 1.10 and ff_gain > 0 and np.isfinite(lat) and lat < profile.latency_ms[0]:
        # Réponse plus rapide que ce que P et D permettent seuls, puis dépassement : c'est le
        # feedforward qui pousse (il agit à plein sur les mouvements de manche vifs).
        F.append(Finding("Réponse", CONSEIL if pk <= 1.20 else ATTENTION,
                         f"Dépassement dû au feedforward en {ax.lower()}",
                         f"Pic {pk:.2f} atteint après une montée très rapide (latence {lat:.1f} ms, plus courte que "
                         f"les {profile.latency_ms[0]:.0f}–{profile.latency_ms[1]:.0f} ms habituels pour {profile.label}). "
                         f"Le feedforward (F = {ff_gain}) anticipe le mouvement et le drone dépasse la consigne.",
                         ["Curseur Stick response (FF) : −0,1 ; ou feedforward_boost plus bas.",
                          "Si vous aimez cette vivacité (freestyle, course), ce n'est pas un défaut : "
                          "vérifiez seulement qu'il n'y a pas de rebond à la fin des flips.",
                          "Le D n'est pas en cause tant que les vols calmes ne dépassent pas."], ax))
        return
    if pk > 1.20:
        F.append(Finding("Réponse", ATTENTION, f"Fort dépassement en {ax.lower()}", f"Pic {pk:.2f} (> 1,20) : le drone "
                         "dépasse la consigne puis revient, signe d'un manque d'amortissement.",
                         ["Curseur Damping (D) : +0,1 à +0,2.", "Ou Tracking (P et I) : −0,1."], ax))
    elif pk > 1.10:
        F.append(Finding("Réponse", CONSEIL, f"Léger dépassement en {ax.lower()}",
                         f"Pic {pk:.2f} (objectif 1,02–1,08).", ["Curseur Damping (D) : +0,1."], ax))
    elif pk < 0.98 and np.isfinite(lat) and lat > profile.latency_ms[1]:
        F.append(Finding("Réponse", CONSEIL, f"Réponse molle en {ax.lower()}",
                         f"Pic {pk:.2f} et latence {lat:.1f} ms (repère {profile.latency_ms[0]:.0f}–{profile.latency_ms[1]:.0f} ms "
                         f"pour {profile.label}) : trop amorti.",
                         ["Curseur Damping : −0,1, ou Tracking : +0,1.", "Pour la réactivité au manche : Stick response (FF) +0,1."], ax))
    else:
        F.append(Finding("Réponse", OK, f"Réponse indicielle {ax.lower()}",
                         f"Pic {pk:.2f}, latence {lat:.1f} ms, {r.n} fenêtres analysées.", [], ax))
    if np.isfinite(lp.sensitivity_peak_db) and lp.sensitivity_peak_db > SENSITIVITY_HIGH_DB:
        at_least = "au moins " if lp.sensitivity_peak_at_edge else ""
        F.append(Finding("Réponse", CONSEIL, f"Marge de stabilité faible en {ax.lower()}",
                         f"Pic de sensibilité {at_least}{lp.sensitivity_peak_db:.1f} dB à {lp.sensitivity_peak_hz:.0f} Hz "
                         f"(> {SENSITIVITY_HIGH_DB:.0f} dB) : le drone amplifie les perturbations à cette fréquence et risque d'osciller."
                         + _loop_caveat(lp),
                         [*_margin_actions(lp), "Réduire le retard des filtres si le bruit le permet."], ax))
    elif np.isfinite(lp.phase_margin_deg) and lp.phase_margin_deg < PHASE_MARGIN_LOW_DEG:
        F.append(Finding("Réponse", CONSEIL, f"Marge de phase réduite en {ax.lower()}",
                         f"Marge de phase {lp.phase_margin_deg:.0f}° (< {PHASE_MARGIN_LOW_DEG:.0f}°) au croisement de {lp.crossover_hz:.0f} Hz."
                         + _loop_caveat(lp), _margin_actions(lp), ax))


def rpm_harmonic_levels(log: FlightLog, sel: slice) -> tuple[np.ndarray, np.ndarray]:
    """Hauteur de chaque harmonique moteur au-dessus du bruit voisin (dB, maximum du roll et du pitch),
    avant filtres (gyro non filtré) et après (gyro filtré)."""
    raw = [harmonic_prominence(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], log.fs) for a in range(2)]
    left = [harmonic_prominence(log.gyro[a, sel], log.motor_hz[:, sel], log.fs) for a in range(2)]
    with np.errstate(all="ignore"):
        return np.fmax(*raw), np.fmax(*left)


def _rpm_weight_rules(log: FlightLog, sel: slice, s, lighten: bool = True) -> list[Finding]:
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


def _loop_caveat(lp) -> str:
    """Précision ajoutée au constat quand la boucle ouverte n'a pas pu être mesurée."""
    if lp.measured:
        return ""
    why = f" ({'; '.join(lp.notes)})" if lp.notes else ""
    return (f" Valeur approchée{why} : calculée sans séparer le D (sur le gyro) et le feedforward (sur la consigne), "
            "elle peut s'écarter de plusieurs dB ou dizaines de degrés.")


def _margin_actions(lp) -> list[str]:
    """Que faire d'une marge faible. Quand le D domine déjà le P et le I au croisement (|Cd/Ce| > 1),
    plus de D fait surtout monter le croisement là où les filtres retardent : il apporte peu de marge
    et peut augmenter le pic de sensibilité (docs/assistant.md, « Boucle ouverte mesurée »)."""
    if not lp.measured:
        return ["Master −0,1.", "Pour une mesure exacte, enregistrez les termes P, I, D et F (champs PID du blackbox)."]
    if np.isfinite(lp.d_ratio) and lp.d_ratio > 1.0:
        return [f"Master −0,1. Le D domine déjà au croisement ({lp.d_ratio:.1f} × P et I) : "
                "plus de D apporterait peu de marge et peut augmenter le pic de sensibilité."]
    return ["Damping (D) +0,1, ou Master −0,1."]


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


def frame_resonance(log: FlightLog, sel: slice) -> tuple[float, float, bool] | None:
    """Raie la plus marquée au-dessus du bruit voisin, quel que soit le gaz : (fréquence en Hz, excès en dB,
    proche d'une harmonique moteur). None si le log ne couvre pas assez de tranches de gaz."""
    x = log.gyro_unfilt if log.gyro_unfilt is not None else log.gyro
    lo, hi = 60.0, min(0.9 * log.fs / 2, 900.0)
    prof = []
    for a in range(2):
        ft = spectral.freq_throttle(x[a, sel], log.throttle[sel], log.fs, mode="amp")
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
    k = int(np.argmax(excess))
    f0 = float(fm[k])
    # une raie moteur se déplace avec le gaz : si f0 correspond à la vitesse moteur en stationnaire,
    # ce n'est probablement pas une résonance
    hz = hover_motor_hz(log)
    near_motor = hz is not None and any(abs(f0 - h * hz) < 0.08 * h * hz for h in (1, 2, 3))
    return f0, float(excess[k]), near_motor


def _frame_resonance(log: FlightLog, sel: slice, profile: CraftProfile) -> Finding | None:
    """Raie de fréquence fixe quel que soit le gaz (résonance du châssis ou pièce desserrée)."""
    res = frame_resonance(log, sel)
    if res is None:
        return None
    f0, excess, near_motor = res
    if excess < profile.resonance_high_db or near_motor:
        return None
    return Finding("Mécanique", CONSEIL, f"Raie fixe vers {f0:.0f} Hz",
                   f"Une vibration à ~{f0:.0f} Hz reste présente quel que soit le gaz (+{excess:.0f} dB au-dessus du "
                   "bruit voisin) : résonance du châssis, d'une antenne, d'une caméra ou pièce desserrée.",
                   ["Resserrez les vis, fixez antennes et câbles, vérifiez la caméra et la pile de cartes.",
                    f"Si elle persiste : notch statique gyro à {f0:.0f} Hz (gyro_notch1_hz = {f0:.0f}, "
                    f"gyro_notch1_cutoff = {0.7 * f0:.0f})."])
