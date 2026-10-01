"""Assistant de réglage : mesures → constats argumentés → suggestions.

Principes :
- chaque constat cite la mesure qui le justifie (valeur, seuil, axe) ;
- les suggestions passent de préférence par les curseurs du réglage simplifié de Betaflight
  (une seule chose à la fois, par petits pas de 0,1), puis par des réglages précis si besoin ;
- les seuils dépendent de la taille du drone (profiles.py) ; ceux de bruit sont calibrés sur un
  corpus de ~800 logs réels (voir docs/assistant.md).
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from ..analysis import spectral
from ..analysis.delay import compute_delays
from ..analysis.motors import hover_motor_hz, motor_noise
from ..analysis.period import crash_time
from ..analysis.sliders import slider_view
from ..analysis.stats import flight_stats, pid_error_stats
from ..analysis.step import StepResult, step_response
from ..log.flightlog import AXES, FlightLog
from .profiles import CraftProfile, detect_profile

# Niveaux de gravité
INFO, OK, CONSEIL, ATTENTION, ALERTE = "info", "ok", "conseil", "attention", "alerte"
SEVERITY_ORDER = {ALERTE: 0, ATTENTION: 1, CONSEIL: 2, INFO: 3, OK: 4}

# Seuils (dB de densité spectrale, (°/s)²/Hz ou (unités PID)²/Hz) — calibrés sur le corpus
GYRO_NOISE_HIGH_DB = -10.0  # gyro filtré, moyenne 80–500 Hz
DTERM_NOISE_HIGH_DB = -2.0  # terme D, moyenne 80–500 Hz
MOTOR_HARMONIC_LEFT_DB = -28.0  # bruit restant sur la 1re harmonique moteur après filtrage
LOW_BAND_HIGH_DB = 5.0  # gyro filtré 20–80 Hz (oscillations, propwash)


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


def analyze(log: FlightLog, sel: slice | None = None, profile: CraftProfile | None = None) -> Report:
    sel = sel or slice(0, log.n)
    reason = "choisi par l'utilisateur"
    if profile is None:
        profile, reason = detect_profile(log)
    F: list[Finding] = []
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
        r = step_response(log.setpoint[ax, sel], log.gyro[ax, sel], fs)
        steps.append(AxisStep(AXES[ax], r))
    meas["steps"] = {st.axis: st.result.metrics for st in steps}
    for ax, st in enumerate(steps[:2]):
        _step_rules(F, st, profile)
    if steps[0].result.n < 20 and steps[1].result.n < 20:
        F.append(Finding("Réponse", ATTENTION, "Pas assez de mouvements de manche",
                         f"Seulement {steps[0].result.n} fenêtres exploitables en roll et {steps[1].result.n} en pitch "
                         "(il faut des mouvements de plus de 20 °/s).",
                         ["Volez 20 à 30 s avec des mouvements francs et des flips/rolls rapides sur chaque axe."]))
    r0, r1 = steps[0].result, steps[1].result
    if r0.n >= 20 and r1.n >= 20 and np.isfinite(r0.metrics.peak) and np.isfinite(r1.metrics.peak):
        d = r1.metrics.peak - r0.metrics.peak
        if abs(d) > 0.08:
            more = "pitch" if d > 0 else "roll"
            F.append(Finding("Réponse", CONSEIL, "Roll et pitch se comportent différemment",
                             f"Pic {r0.metrics.peak:.2f} en roll contre {r1.metrics.peak:.2f} en pitch : le {more} dépasse davantage.",
                             [f"Curseur « D pitch » (pitch_d) : {'+' if d > 0 else '−'}0,1 pour équilibrer."]))

    # ------------------------------------------------------------ 3. bruit et filtres
    delays = compute_delays(log, sel)
    meas["delays"] = delays
    nyq = fs / 2
    hi = min(500.0, 0.95 * nyq)
    g_db = [_band_db(log.gyro[a, sel], fs, 80, hi) for a in range(2)]
    d_db = [_band_db(log.d_term[a, sel], fs, 80, hi) for a in range(2) if np.any(log.d_term[a])]
    low_db = [_band_db(log.gyro[a, sel], fs, 20, 80) for a in range(2)]
    meas.update(gyro_noise_db=g_db, dterm_noise_db=d_db, low_band_db=low_db)
    gyro_noisy = max(g_db) > GYRO_NOISE_HIGH_DB
    dterm_noisy = bool(d_db) and max(d_db) > DTERM_NOISE_HIGH_DB
    if dterm_noisy:
        F.append(Finding("Bruit", ATTENTION, "Terme D bruité",
                         f"Bruit du terme D {max(d_db):.1f} dB (80–{hi:.0f} Hz), au-dessus du seuil {DTERM_NOISE_HIGH_DB:.0f} dB. "
                         "Un D bruité fait chauffer les moteurs et use les ESC.",
                         ["Vérifiez d'abord la mécanique (hélices, roulements, vis du châssis).",
                          "Curseur « filtre D » : −0,1 à −0,2 (filtrage plus fort), ou baisser le D (curseur Damping) si la réponse le permet.",
                          "Après le vol : moteurs tièdes = OK ; chauds = réduire D ou filtrer plus."]))
    if gyro_noisy:
        F.append(Finding("Bruit", ATTENTION, "Gyro filtré bruité",
                         f"Bruit du gyro filtré {max(g_db):.1f} dB (80–{hi:.0f} Hz), seuil {GYRO_NOISE_HIGH_DB:.0f} dB.",
                         ["Contrôlez hélices et moteurs (vibrations), la fixation du contrôleur de vol (silentblocs).",
                          "Curseur « filtre gyro » : −0,1 à −0,2."]))
    if np.isfinite(delays.gyro_ms):
        if delays.gyro_ms > profile.gyro_delay_max_ms and not gyro_noisy and not dterm_noisy:
            F.append(Finding("Filtres", CONSEIL, "Filtrage gyro peut-être trop prudent",
                             f"Retard des filtres gyro {delays.gyro_ms:.2f} ms (repère {profile.gyro_delay_max_ms:.1f} ms pour "
                             f"{profile.label}) alors que le bruit est faible.",
                             ["Curseur « filtre gyro » : +0,1 à +0,2 (moins de retard → meilleure réactivité, moins de propwash).",
                              "Contrôlez la température des moteurs après chaque changement."]))
        else:
            F.append(Finding("Filtres", OK, "Retard des filtres gyro",
                             f"{delays.gyro_ms:.2f} ms (repère ≤ {profile.gyro_delay_max_ms:.1f} ms pour {profile.label})."))
    if np.isfinite(delays.dterm_ms) and delays.dterm_ms > profile.dterm_delay_max_ms and not dterm_noisy:
        F.append(Finding("Filtres", CONSEIL, "Filtrage du D peut-être trop fort",
                         f"Retard des filtres D {delays.dterm_ms:.2f} ms (repère {profile.dterm_delay_max_ms:.1f} ms) "
                         "avec un D peu bruité.",
                         ["Curseur « filtre D » : +0,1."]))
    if log.motor_hz is not None and log.gyro_unfilt is not None and s.rpm_harmonics > 0:
        mn = [motor_noise(log.gyro[a, sel], log.motor_hz[:, sel], fs) for a in range(2)]
        mp = [motor_noise(log.gyro_unfilt[a, sel], log.motor_hz[:, sel], fs) for a in range(2)]
        left = [float(np.nanmax([m.mean_db[0] for m in mn]))]
        meas["motor_harmonic_left_db"] = left[0]
        meas["motor_harmonic_raw_db"] = float(np.nanmax([m.mean_db[0] for m in mp]))
        if left[0] > MOTOR_HARMONIC_LEFT_DB:
            F.append(Finding("Filtres", CONSEIL, "Bruit moteur mal éliminé",
                             f"Il reste {left[0]:.1f} dB sur la fréquence de rotation des moteurs après filtrage "
                             f"(avant filtrage : {meas['motor_harmonic_raw_db']:.1f} dB).",
                             ["Vérifiez motor_poles (14 pour la plupart des moteurs, 12 pour certains micro-moteurs).",
                              "Filtre RPM : baisser rpm_filter_q (ex. 500 → 350) élargit les notchs."]))
    if max(low_db) > LOW_BAND_HIGH_DB:
        F.append(Finding("Réponse", CONSEIL, "Oscillations basses fréquences (20–80 Hz)",
                         f"Énergie du gyro filtré entre 20 et 80 Hz : {max(low_db):.1f} dB. C'est la zone du propwash "
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
    if fst.motor_saturation_pct > 5:
        F.append(Finding("Moteurs", ATTENTION, "Moteurs souvent saturés",
                         f"Au moins un moteur est à fond {fst.motor_saturation_pct:.1f} % du temps : le contrôleur "
                         "n'a plus de marge pour corriger.",
                         ["Si c'est pendant les manœuvres : baisser les rates ou le Master.",
                          "Sinon : drone trop lourd pour ses moteurs/hélices, ou batterie faible."]))
    if np.isfinite(fst.motor_imbalance_pct) and fst.motor_imbalance_pct > 12:
        F.append(_imbalance_finding([mm.mean for mm in fst.motors], fst.motor_imbalance_pct))
    pe = pid_error_stats(log, sel)
    meas["pid_error_sd"] = pe.sd
    i_mean = [float(np.mean(log.i_term[a, sel])) for a in range(3)]
    meas["iterm_mean"] = i_mean
    for a in range(2):
        if abs(i_mean[a]) > 40:
            F.append(Finding("Mécanique", CONSEIL, f"Terme I décentré en {AXES[a].lower()}",
                             f"Le terme I {AXES[a].lower()} reste en moyenne à {i_mean[a]:.0f} : le drone "
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


def _step_rules(F: list[Finding], st: AxisStep, profile: CraftProfile) -> None:
    r = st.result
    ax = st.axis
    if r.n < 20:
        return
    m = r.metrics
    lp = r.loop
    pk, lat = m.peak, m.latency_ms
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
    if np.isfinite(lp.sensitivity_peak_db) and lp.sensitivity_peak_db > 6.0:
        F.append(Finding("Réponse", ATTENTION, f"Marge de stabilité faible en {ax.lower()}",
                         f"Pic de sensibilité {lp.sensitivity_peak_db:.1f} dB à {lp.sensitivity_peak_hz:.0f} Hz (> 6 dB) : "
                         "le drone amplifie les perturbations à cette fréquence et risque d'osciller.",
                         ["Baisser le Master de 0,1, ou augmenter le D.", "Réduire le retard des filtres si le bruit le permet."], ax))
    elif np.isfinite(lp.phase_margin_deg) and lp.phase_margin_deg < 40:
        F.append(Finding("Réponse", CONSEIL, f"Marge de phase réduite en {ax.lower()}",
                         f"Marge de phase {lp.phase_margin_deg:.0f}° (< 40°).", ["Damping +0,1."], ax))


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


def _frame_resonance(log: FlightLog, sel: slice, profile: CraftProfile) -> Finding | None:
    """Raie de fréquence fixe quel que soit le gaz (résonance du châssis ou pièce desserrée)."""
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
    if excess[k] < 6.0:
        return None
    f0 = float(fm[k])
    # une raie moteur se déplace avec le gaz : si f0 correspond à la vitesse moteur en stationnaire,
    # ce n'est probablement pas une résonance
    hz = hover_motor_hz(log)
    if hz is not None and any(abs(f0 - h * hz) < 0.08 * h * hz for h in (1, 2, 3)):
        return None
    return Finding("Mécanique", CONSEIL, f"Raie fixe vers {f0:.0f} Hz",
                   f"Une vibration à ~{f0:.0f} Hz reste présente quel que soit le gaz (+{excess[k]:.0f} dB au-dessus du "
                   "bruit voisin) : résonance du châssis, d'une antenne, d'une caméra ou pièce desserrée.",
                   ["Resserrez les vis, fixez antennes et câbles, vérifiez la caméra et la pile de cartes.",
                    f"Si elle persiste : notch statique gyro à {f0:.0f} Hz (gyro_notch1_hz = {f0:.0f}, "
                    f"gyro_notch1_cutoff = {0.7 * f0:.0f})."])
