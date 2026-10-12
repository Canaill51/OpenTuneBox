"""Vérification des logs ouverts pour le guide intégré : à quoi sert chaque log, et quoi corriger avant le
prochain vol.

Le vol de réglage du guide a deux parties, découpées automatiquement (`analysis/period.py:flight_parts`) :
une partie stationnaire (bruit, filtres) et une partie avec mouvements (retard des filtres, vols de série
PID). La partie avec mouvements suit le protocole de Brian White : mode Angle, profil de rates « LOG »
(ACTUAL 200 / 200 / 0), limite d'angle 30°, wobbles roll et pitch mêlés, Dynamic Damping à 0, curseur I à
0,2–0,3, au moins 200 fenêtres.

Calculs rapides (quelques dizaines de ms par log), sauf `exact=True` : nombre réel de fenêtres de la réponse
indicielle (`step_response`, jusqu'à 1–2 s sur un long vol, à lancer en arrière-plan). Seuils : ceux de
l'assistant et de `step_tuning.py`, plus ceux du protocole ci-dessous (origine dans `docs/assistant.md`).

Chaque correction donne d'abord l'endroit où la faire dans le configurateur Betaflight (onglet, curseur,
champ), puis la commande CLI équivalente, en repli. Chaque point a une clé (`CheckItem.key`) : le guide
coche ses cases d'après elle.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from ..analysis.delay import MOVING_SETPOINT_DPS, compute_delays, stick_p95_dps
from ..analysis.motors import hover_motor_hz
from ..analysis.period import crash_time, flight_parts
from ..analysis.sliders import PTB_CATEGORIES, slider_view
from ..analysis.step import input_windows, step_response
from ..analysis.step_series import SERIES_KEYS, filter_signature, flight_conditions
from ..log.flightlog import FlightLog
from .profiles import filter_delay_target_ms
from .rules import DTERM_PTB_MAX_DB, GYRO_PTB_MAX_DB, band_peak_ptb_db, log_rate_needed, sample_rate_cli
from .step_tuning import I_SLIDER_RANGE, MIN_WINDOWS, SURE_WINDOWS

OK, WARN, BLOCK, INFO = "ok", "à revoir", "bloquant", "info"

MIN_VERSION = (4, 4)  # tables de débogage et réglages normalisés à partir de la 4.4 (log/debug_tables.py)
MIN_DURATION_S = 15.0  # vols de test conseillés : 20 à 40 s (guide) ; en dessous de 15 s, trop peu de fenêtres

# Protocole des vols avec mouvements (choix de Cédric du 2026-10-12, d'après Brian White [S24, S33])
RATES_TYPE_ACTUAL = 3  # rates_type : BETAFLIGHT 0, RACEFLIGHT 1, KISS 2, ACTUAL 3, QUICK 4 (firmware)
LOG_RATE_DPS = 200  # rates linéaires : sensibilité au centre = maximum = 200 °/s, expo 0, sur les trois axes
LOG_ANGLE_LIMIT = 30  # degrés : limite d'angle conseillée par Brian White pour le wobble [S33]
ANGLE_SHARE_MIN = 0.9  # vol « en mode Angle » : au moins 90 % de la partie (comme step_series)
AXES_MIXED_MIN = 0.4  # part du temps en mouvement où roll et pitch bougent ensemble (corpus, 2026-10-12)
DYN_DAMPING_MAX = 0.05  # curseur Dynamic Damping « à 0 » pendant les séries [S34]
D_DELAY_PTB_OFFSET_MS = 0.5  # PIDtoolbox mesure le retard du D ~0,5 ms plus court que nous (profiles.py)

# Curseurs de l'onglet « Réglages de PID » du configurateur : catégorie PTB → libellé français (anglais)
CONFIGURATOR_SLIDERS = {
    "Damping": "Atténuation (Damping)",
    "Tracking": "Suivi (Tracking)",
    "Stick Resp.": "Réponse des sticks (Stick Response)",
    "Dynamic D.": "Atténuation dynamique (Dynamic Damping)",
    "Drift-Wobble": "Dérive - Oscillations (Drift - Wobble)",
    "Pitch D.": "Atténuation du Pitch (Pitch Damping)",
    "Pitch T.": "Suivi du Pitch (Pitch Tracking)",
    "Master": "Multiplicateur Maître (Master Multiplier)",
}


@dataclass
class Fix:
    """Une correction : où la faire dans le configurateur, et les commandes CLI équivalentes (repli)."""
    where: str
    cli: str = ""  # une ou plusieurs commandes, séparées par des retours à la ligne


@dataclass
class CheckItem:
    key: str  # clé stable (cases du guide)
    title: str
    status: str  # OK, WARN, BLOCK, INFO
    detail: str
    fixes: list[Fix] = field(default_factory=list)


@dataclass
class Usage:
    """Ce que le log permet de faire (régler les filtres, lire une série PID…)."""
    key: str  # "noise", "delay", "series"
    label: str
    ok: bool
    reason: str


@dataclass
class LogCheck:
    label: str
    moving: bool  # le log contient une partie avec mouvements
    parts: dict[str, tuple[float, float] | None]  # "hover" / "moving" : (début, fin) en s
    items: list[CheckItem]
    usages: list[Usage]
    windows: list[int] = field(default_factory=list)  # fenêtres roll, pitch de la partie avec mouvements
    exact: bool = False  # windows = vrai N de la réponse indicielle (sinon estimation rapide)
    sliders: dict[str, float] = field(default_factory=dict)  # curseurs (catégorie PTB → fraction)

    def item(self, key: str) -> CheckItem | None:
        return next((it for it in self.items if it.key == key), None)

    def usage(self, key: str) -> Usage | None:
        return next((u for u in self.usages if u.key == key), None)

    def cli_commands(self) -> list[str]:
        out = []
        for it in self.items:
            if it.status in (WARN, BLOCK):
                for f in it.fixes:
                    out += [c for c in f.cli.splitlines() if c and c not in out]
        return out


def _n(x: float, d: int = 1) -> str:
    return f"{x:.{d}f}".replace(".", ",")


def _db(x: float) -> str:
    return f"{x:.0f} dB".replace("-", "−")


def _span(p: tuple[float, float]) -> str:
    return f"{_n(p[0])} à {_n(p[1])} s"


def log_rates(s) -> tuple[list[int], list[int], list[int]] | None:
    """Rates ACTUAL du profil enregistré : (sensibilité au centre °/s, maximum °/s, expo %) par axe ; None pour
    un autre type de rates (dont les nombres n'ont pas le même sens). En-tête : `rates_type`, `rc_rates`
    (centre ÷ 10), `rates` (maximum ÷ 10), `rc_expo` (%)."""
    if s.get_int("rates_type", -1) != RATES_TYPE_ACTUAL:
        return None
    rc, mx, ex = s.get_ints("rc_rates"), s.get_ints("rates"), s.get_ints("rc_expo")
    if min(len(rc), len(mx), len(ex)) < 3:
        return None
    return [10 * v for v in rc[:3]], [10 * v for v in mx[:3]], ex[:3]


def axes_mixed_share(log: FlightLog, sel: slice, min_dps: float = 20.0) -> float:
    """Part du temps en mouvement (roll ou pitch au-delà de `min_dps`) où les deux axes bougent ensemble."""
    mv = np.abs(log.setpoint[:2, sel]) > min_dps
    anym = int(mv.any(axis=0).sum())
    return float((mv[0] & mv[1]).sum() / anym) if anym else 0.0


def log_rates_fix() -> Fix:
    """Mise en place du profil de rates « LOG » des vols de réglage."""
    cli = ["set rates_type = ACTUAL"]
    for ax in ("roll", "pitch", "yaw"):
        cli += [f"set {ax}_rc_rate = {LOG_RATE_DPS // 10}", f"set {ax}_srate = {LOG_RATE_DPS // 10}",
                f"set {ax}_expo = 0"]
    return Fix(f"Onglet « Réglages de PID », profil de rates « LOG » : « Type de rates » ACTUAL, « Sensibilité au "
               f"centre » {LOG_RATE_DPS}, « Max Vel » {LOG_RATE_DPS}, Expo 0, sur les trois axes.", "\n".join(cli))


def check_log(log: FlightLog, sel: slice | None = None, label: str = "", exact: bool = False) -> LogCheck:
    sel = sel or slice(0, log.n)
    s = log.settings
    items: list[CheckItem] = []
    fs = log.fs
    add = lambda *a: items.append(CheckItem(*a))  # noqa: E731

    # ------------------------------------------------------------ enregistrement
    version_ok = s.firmware_name.lower().startswith("betaflight") and tuple(s.version[:2]) >= MIN_VERSION
    vtxt = ".".join(str(v) for v in s.version) or "inconnue"
    add("version", "Version du firmware", OK if version_ok else BLOCK,
        f"{s.firmware_name} {vtxt}." + ("" if version_ok else " OpenTuneBox lit Betaflight 4.4 et plus récent."),
        [] if version_ok else [Fix("Mettre à jour le firmware (onglet « Flasher le Firmware »), après une "
                                   "sauvegarde `diff all`.")])

    i0, i1 = sel.start or 0, min(sel.stop or log.n, log.n)
    t0, t1 = (float(log.t[i0]), float(log.t[i1 - 1])) if i1 > i0 else (0.0, 0.0)
    dur = t1 - t0
    add("duration", "Durée analysée", OK if dur >= MIN_DURATION_S else WARN,
        f"{_n(dur)} s de vol dans la période d'analyse"
        + ("." if dur >= MIN_DURATION_S else " : trop court, visez 20 à 40 s pour chaque partie du vol."))

    hz = hover_motor_hz(log)
    base_hz = log_rate_needed(s.pid_rate_hz)
    harm_hz = log_rate_needed(s.pid_rate_hz, hz)
    need = max(base_hz, harm_hz)
    rate_ok = fs >= 0.95 * base_hz
    harm_ok = hz is None or 3 * hz <= fs / 2 or harm_hz <= fs * 1.05
    loops = f"gyro {_n(s.gyro_rate_hz / 1000)} kHz, boucle PID {_n(s.pid_rate_hz / 1000)} kHz"
    frac = sample_rate_cli(s.pid_rate_hz, need)
    rate_fix = [Fix(f"Onglet « Blackbox » → « Fréquence d'enregistrement de la Blackbox » : {need:.0f} Hz ({frac}).",
                    f"set blackbox_sample_rate = {frac}")]
    if not rate_ok:
        add("log_rate", "Fréquence d'enregistrement", BLOCK,
            f"{fs:.0f} Hz ({loops}) : il faut au moins {base_hz:.0f} Hz. Le bruit au-dessus de {fs / 2:.0f} Hz se "
            "replie dans le spectre (fausses raies) : les filtres ne peuvent pas être jugés.", rate_fix)
    elif not harm_ok:
        add("log_rate", "Fréquence d'enregistrement", WARN,
            f"{fs:.0f} Hz : les moteurs tournent à ~{hz:.0f} Hz, leur 3e harmonique ({3 * hz:.0f} Hz) se replie "
            "dans le spectre (petit drone, moteurs rapides).", rate_fix)
    else:
        add("log_rate", "Fréquence d'enregistrement", OK, f"{fs:.0f} Hz ({loops}).")

    unfilt = log.gyro_unfilt is not None
    if unfilt:
        add("gyro_unfilt", "Gyro non filtré", OK, "Présent : l'effet des filtres et leur retard se mesurent.")
    elif s.family == "4.4":
        add("gyro_unfilt", "Gyro non filtré", BLOCK, "Absent : en 4.4, seul le mode de débogage GYRO_SCALED l'enregistre.",
            [Fix("Onglet « Blackbox » → « Mode débug de la Blackbox » : GYRO_SCALED.", "set debug_mode = GYRO_SCALED")])
    else:
        add("gyro_unfilt", "Gyro non filtré", BLOCK, "Absent : le champ a été désactivé dans l'enregistrement.",
            [Fix("Onglet « Blackbox » → « Champs de débogage inclus » : cocher « Gyro (Unfiltered) ».",
                 "set blackbox_disable_gyrounfilt = OFF")])

    if s.family != "4.4" and s.dyn_notch_count > 0 and s.debug_mode != "FFT_FREQ":
        add("debug_mode", "Mode de débogage", WARN,
            f"{s.debug_mode} : avec FFT_FREQ, le log contient aussi les fréquences suivies par le notch dynamique "
            "(courbes du Spectre et diagnostic du bruit plus complets).",
            [Fix("Onglet « Blackbox » → « Mode débug de la Blackbox » : FFT_FREQ.", "set debug_mode = FFT_FREQ")])

    if log.motor_hz is None:
        add("rpm", "Télémétrie RPM", WARN, "Absente : le filtre RPM est inactif et les harmoniques moteur ne sont "
            "qu'estimées.",
            [Fix("Onglet « Moteurs » → « DShot bi-directionnel » activé (ESC BLHeli_32, Bluejay ou AM32), et « Pôles "
                 "moteur » juste (aimants de la cloche, souvent 14).", "set dshot_bidir = ON")])
    else:
        add("rpm", "Télémétrie RPM", OK, f"Présente ({s.motor_poles} pôles moteur dans les réglages).")

    # ------------------------------------------------------------ parties du vol
    parts = flight_parts(log, t0, t1) if dur > 0 else {"hover": None, "moving": None}
    if parts["hover"] is None and parts["moving"] is None and dur > 0:  # vol trop court pour être découpé
        moving_all = stick_p95_dps(log, sel) >= MOVING_SETPOINT_DPS
        parts["moving" if moving_all else "hover"] = (t0, t1)
    desc = [f"stationnaire de {_span(parts['hover'])}" if parts["hover"] else "pas de partie stationnaire",
            f"mouvements de {_span(parts['moving'])}" if parts["moving"] else "pas de partie avec mouvements"]
    add("parts", "Parties du vol", INFO, " ; ".join(desc).capitalize() + ".")

    # ------------------------------------------------------------ protocole de la partie avec mouvements
    cond = flight_conditions(log, sel)
    sv = slider_view(s)
    windows: list[int] = []
    i_ok, ff_ok, enough = True, True, False
    if parts["moving"]:
        msel = log.slice_time(*parts["moving"])
        cond = flight_conditions(log, msel)
        if exact:
            windows = [step_response(log.setpoint[a, msel], log.gyro[a, msel], fs).n for a in range(2)]
        else:
            windows = [input_windows(log.setpoint[a, msel], fs) for a in range(2)]
        n = min(windows)
        enough = n >= MIN_WINDOWS
        approx = "" if exact else " (estimation rapide ; le vrai nombre est un peu plus faible)"
        if n >= SURE_WINDOWS:
            add("windows", "Fenêtres de mesure", OK, f"N = {windows[0]} en roll, {windows[1]} en pitch{approx}.")
        else:
            add("windows", "Fenêtres de mesure", WARN if enough else BLOCK,
                f"N = {windows[0]} en roll, {windows[1]} en pitch{approx} : il en faut au moins {SURE_WINDOWS} pour "
                f"une mesure sûre" + ("." if enough else f", et {MIN_WINDOWS} pour être lu."),
                [Fix("Au moins 20 s de wobbles roll et pitch mêlés (environ 12 fenêtres par seconde de mouvement).")])

        rates = log_rates(s)
        if rates is None:
            add("rates", "Rates de log", WARN, "Le profil de rates n'est pas de type ACTUAL.", [log_rates_fix()])
        else:
            center, mx, expo = rates
            good = all(c == LOG_RATE_DPS for c in center) and all(m == LOG_RATE_DPS for m in mx) and not any(expo)
            txt = (f"ACTUAL, centre {'/'.join(map(str, center))}, maximum {'/'.join(map(str, mx))} °/s, expo "
                   f"{'/'.join(map(str, expo))} (roll/pitch/yaw)")
            add("rates", "Rates de log", OK if good else WARN,
                txt + ("." if good else f" : le protocole demande {LOG_RATE_DPS} / {LOG_RATE_DPS} / 0 sur les trois axes."),
                [] if good else [log_rates_fix()])

        lim = s.get_int("angle_limit", -1)
        lim_fix = [Fix(f"Onglet « Réglages de PID » → « Limite de l'Angle » : {LOG_ANGLE_LIMIT}.",
                       f"set {'level_limit' if s.family == '4.4' else 'angle_limit'} = {LOG_ANGLE_LIMIT}")]
        if lim < 0:
            add("angle_limit", "Limite d'angle", INFO,
                f"Non enregistrée dans le log (Betaflight 4.4) : vérifiez qu'elle vaut {LOG_ANGLE_LIMIT}°.", lim_fix)
        else:
            add("angle_limit", "Limite d'angle", OK if lim == LOG_ANGLE_LIMIT else WARN,
                f"{lim}°" + ("." if lim == LOG_ANGLE_LIMIT else f" : le protocole demande {LOG_ANGLE_LIMIT}°."),
                [] if lim == LOG_ANGLE_LIMIT else lim_fix)

        share = cond.angle_share
        if share != share:  # modes non enregistrés
            add("angle_mode", "Mode Angle", INFO, "Modes de vol absents du log : impossible de vérifier.")
        else:
            add("angle_mode", "Mode Angle", OK if share >= ANGLE_SHARE_MIN else WARN,
                f"{share:.0%} de la partie avec mouvements en mode Angle"
                + ("." if share >= ANGLE_SHARE_MIN else f" : le protocole demande au moins {ANGLE_SHARE_MIN:.0%}."),
                [] if share >= ANGLE_SHARE_MIN else
                [Fix("Onglet « Modes » : mode ANGLE sur un interrupteur, activé pendant les mouvements.")])

        mixed = axes_mixed_share(log, msel)
        add("axes_mixed", "Roll et pitch mêlés", OK if mixed >= AXES_MIXED_MIN else WARN,
            f"Les deux axes bougent ensemble {mixed:.0%} du temps en mouvement"
            + ("." if mixed >= AXES_MIXED_MIN else
               f" (repère {AXES_MIXED_MIN:.0%}) : bougez le roll et le pitch en même temps, au hasard, plutôt "
               "qu'un axe puis l'autre."))

        ff_ok = not cond.ff_active
        if not ff_ok:
            add("feedforward", "Feedforward", WARN,
                f"Actif (F roll {cond.ff[0]}, pitch {cond.ff[1]}) hors mode Angle : il fausse la lecture d'une série PID.",
                [Fix("Voler en mode Angle (le feedforward y est coupé), ou curseur « "
                     f"{CONFIGURATOR_SLIDERS['Stick Resp.']} » à 0.", "set simplified_feedforward_gain = 0")])
        lo, hi = I_SLIDER_RANGE
        i_ok = lo <= cond.i_slider <= hi
        if not i_ok:
            add("i_slider", "Curseur I", WARN,
                f"{_n(cond.i_slider, 2)} : pendant les séries PID, le terme I change la forme de la réponse ; le garder "
                "entre 0,2 et 0,3, puis le remonter en finitions.",
                [Fix(f"Curseur « {CONFIGURATOR_SLIDERS['Drift-Wobble']} » : 0,25 (Mode Expert).",
                     "set simplified_i_gain = 25")])
        dd = sv.values["Dynamic D."]
        if sv.from_header and dd > DYN_DAMPING_MAX:
            dd_key = "simplified_d_max_gain" if "simplified_d_max_gain" in s.raw else "simplified_dmax_gain"
            add("dyn_damping", "Atténuation dynamique", WARN,
                f"{_n(dd, 2)} : pendant les séries, le D max change la réponse selon la vitesse du mouvement ; le "
                "mettre à 0, puis le régler en finitions.",
                [Fix(f"Curseur « {CONFIGURATOR_SLIDERS['Dynamic D.']} » : 0 (Mode Expert).", f"set {dd_key} = 0")])

    if not sv.from_header:
        add("simplified", "Réglage simplifié", WARN,
            "Désactivé : les curseurs du configurateur ne pilotent pas les PID ; OpenTuneBox estime leurs positions à "
            "partir des PID.",
            [Fix("Onglet « Réglages de PID » → curseurs, « Mode » : RPY. Attention : les PID sont alors recalculés à "
                 "partir des curseurs (notez-les avant).", "set simplified_pids_mode = RPY")])

    crash = crash_time(log)
    if crash is not None:
        add("crash", "Crash", INFO, f"Crash vers {_n(crash)} s : la période d'analyse s'arrête avant.")
    if log.stats.get("corrupt_frames", 0) > 20:
        add("corrupt", "Trames corrompues", INFO,
            f"{log.stats['corrupt_frames']} trames (pauses d'écriture de la mémoire) : sans conséquence pour l'analyse.")

    # ------------------------------------------------------------ verdict par usage
    base_ok = version_ok and rate_ok and unfilt
    base_why = _first_problem(items, ("version", "log_rate", "gyro_unfilt"))
    aliased = "" if harm_ok else " Attention : 3e harmonique moteur repliée (voir la fréquence d'enregistrement)."
    hover, moving = parts["hover"], parts["moving"]
    usages = [
        Usage("noise", "Identifier le bruit (filtres)", base_ok and hover is not None,
              base_why if not base_ok else
              (f"partie stationnaire de {_span(hover)}." + aliased if hover else
               "pas de partie stationnaire : commencez le vol par 20 s de stationnaire avec des montées de gaz.")),
        Usage("delay", "Juger le retard des filtres", base_ok and moving is not None,
              base_why if not base_ok else
              (f"partie avec mouvements de {_span(moving)}." if moving else
               "pas de partie avec mouvements : terminez le vol par 20 s de wobbles en mode Angle.")),
    ]
    series_ok = version_ok and moving is not None and enough and ff_ok and i_ok
    protocol = [it.title.lower() for it in items
                if it.key in ("windows", "rates", "angle_limit", "angle_mode", "axes_mixed", "dyn_damping")
                and it.status == WARN]
    if series_ok:
        reason = "conditions respectées." if not protocol else f"lisible, mais à améliorer : {', '.join(protocol)}."
    elif moving is None:
        reason = "pas de partie avec mouvements."
    else:
        blocking = [title for title, bad in (("Version du firmware", not version_ok), ("Fenêtres de mesure", not enough),
                                             ("Feedforward", not ff_ok), ("Curseur I", not i_ok)) if bad]
        reason = f"à corriger : {', '.join(f'« {b} »' for b in blocking)} (voir ci-dessous)."
    usages.append(Usage("series", "Vol d'une série PID", series_ok, reason))
    return LogCheck(label, moving is not None, parts, items, usages, windows, exact, dict(sv.values))


@dataclass
class FilterTargets:
    """Repères de fin de la phase Filtres : bruit sur la partie stationnaire (échelle de PIDtoolbox), retard des
    filtres sur la partie avec mouvements, objectif de retard de Brian White pour la taille."""
    gyro_ptb_db: float
    dterm_ptb_db: float
    gyro_delay_ms: float
    dterm_delay_ms: float
    delay_goal_ms: tuple[float, float] | None  # (gyro, D) tels que lus par PIDtoolbox

    def checks(self) -> list[tuple[str, bool | None, str]]:
        """(repère, atteint ? (None : non mesurable), texte)."""
        def fin(x):
            return np.isfinite(x)
        out = [("Gyro filtré", self.gyro_ptb_db <= GYRO_PTB_MAX_DB if fin(self.gyro_ptb_db) else None, f"{_db(self.gyro_ptb_db)} (repère : sous {_db(GYRO_PTB_MAX_DB)})"),
               ("Terme D", self.dterm_ptb_db <= DTERM_PTB_MAX_DB if fin(self.dterm_ptb_db) else None,
                f"{_db(self.dterm_ptb_db)} (repère : au plus {_db(DTERM_PTB_MAX_DB)})")]
        if self.delay_goal_ms:
            g, d = self.delay_goal_ms
            out.append(("Retard gyro", self.gyro_delay_ms <= g if fin(self.gyro_delay_ms) else None,
                        f"{_n(self.gyro_delay_ms, 2)} ms (objectif : {_n(g, 2)} ms)"))
            out.append(("Retard D", self.dterm_delay_ms <= d + D_DELAY_PTB_OFFSET_MS if fin(self.dterm_delay_ms) else None,
                        f"{_n(self.dterm_delay_ms, 2)} ms (objectif : {_n(d, 2)} ms lus par PIDtoolbox, soit "
                        f"{_n(d + D_DELAY_PTB_OFFSET_MS, 2)} ms ici)"))
        return out

    def reached(self) -> bool | None:
        vals = [ok for _n_, ok, _t in self.checks()]
        if any(v is False for v in vals):
            return False
        return None if any(v is None for v in vals) else True


def filter_targets(log: FlightLog, parts: dict, inch: float | None) -> FilterTargets | None:
    """Repères de la phase Filtres (calcul de quelques secondes : à lancer en arrière-plan)."""
    if parts.get("hover") is None:
        return None
    hsel = log.slice_time(*parts["hover"])
    fs = log.fs
    hi = min(500.0, 0.95 * fs / 2)
    g = max(band_peak_ptb_db(log.gyro[a, hsel], fs, 80, hi) for a in range(2))
    d = max((band_peak_ptb_db(log.d_term[a, hsel], fs, 80, hi) for a in range(2) if np.any(log.d_term[a])),
            default=float("nan"))
    gd = dd = float("nan")
    if parts.get("moving") is not None and log.gyro_unfilt is not None:
        dl = compute_delays(log, log.slice_time(*parts["moving"]))
        gd, dd = dl.gyro_ms, dl.dterm_ms
    return FilterTargets(g, d, gd, dd, filter_delay_target_ms(inch))


def _first_problem(items: list[CheckItem], keys: tuple[str, ...]) -> str:
    for it in items:
        if it.key in keys and it.status in (WARN, BLOCK):
            return f"« {it.title} » à corriger (voir ci-dessous)."
    return ""


@dataclass
class GroupCheck:
    """Contrôle d'un groupe de logs du même drone : peut-on les lire comme une série ?"""
    craft: str
    labels: list[str]
    status: str
    detail: str


def check_logs(logs: list[tuple[str, FlightLog]]) -> list[GroupCheck]:
    """Logs du même drone (nom et carte) : un seul curseur doit changer entre eux, et pas les filtres."""
    groups: dict[tuple[str, str], list[tuple[str, FlightLog]]] = {}
    for label, lg in logs:
        groups.setdefault((lg.settings.craft_name, lg.settings.board), []).append((label, lg))
    out = []
    for (craft, board), members in groups.items():
        if len(members) < 2:
            continue
        name = craft or board or "drone sans nom"
        labels = [m[0] for m in members]
        values = [slider_view(lg.settings).values for _, lg in members]
        changed = [k for k in PTB_CATEGORIES if len({round(v[k], 2) for v in values}) > 1]
        same_filters = len({filter_signature(lg.settings) for _, lg in members}) == 1
        names = ", ".join(f"« {CONFIGURATOR_SLIDERS[k]} »" for k in changed)
        if not same_filters:
            out.append(GroupCheck(name, labels, WARN,
                                  "Les filtres changent entre ces logs : pour une série PID, gardez les mêmes filtres "
                                  "et ne changez qu'un curseur."))
        if len(changed) > 1:
            out.append(GroupCheck(name, labels, WARN,
                                  f"Plusieurs curseurs changent entre ces logs ({names}) : une série ne doit en changer "
                                  "qu'un seul, par pas de 0,2."))
        elif len(changed) == 1 and same_filters:
            k = changed[0]
            vals = sorted({round(v[k], 2) for v in values})
            ok = k in SERIES_KEYS
            out.append(GroupCheck(name, labels, OK if ok else INFO,
                                  f"Série du curseur « {CONFIGURATOR_SLIDERS[k]} » : "
                                  + " → ".join(_n(v, 2) for v in vals)
                                  + (" ; à lire dans « Réponse indicielle » → Diagnostic du réglage." if ok else
                                     " ; ce curseur ne se règle pas par série (à votre goût).")))
        elif not changed and same_filters:
            out.append(GroupCheck(name, labels, INFO, "Mêmes curseurs et mêmes filtres : vols comparables entre eux."))
    return out
