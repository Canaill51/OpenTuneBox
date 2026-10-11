"""Vérification rapide des logs ouverts : à quoi chaque log peut servir, et quoi corriger avant le prochain vol.

Utilisée par l'étape « Vérifier mes logs » du guide intégré. Calculs rapides seulement (pas de spectre, pas
de réponse indicielle complète) : quelques dizaines de millisecondes par log. Les critères et les seuils
sont ceux de l'assistant (`rules.py`, `step_tuning.py`) : aucun seuil propre à ce module.

Chaque correction donne d'abord l'endroit où la faire dans le configurateur Betaflight (onglet, curseur,
champ), puis la commande CLI équivalente, en repli.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from ..analysis.delay import MOVING_SETPOINT_DPS, stick_p95_dps
from ..analysis.motors import hover_motor_hz
from ..analysis.period import crash_time
from ..analysis.sliders import PTB_CATEGORIES, slider_view
from ..analysis.step import input_windows
from ..analysis.step_series import SERIES_KEYS, filter_signature, flight_conditions
from ..log.flightlog import FlightLog
from .rules import log_rate_needed, sample_rate_cli
from .step_tuning import I_SLIDER_RANGE, MIN_WINDOWS

OK, WARN, BLOCK, INFO = "ok", "à revoir", "bloquant", "info"

MIN_VERSION = (4, 4)  # tables de débogage et réglages normalisés à partir de la 4.4 (log/debug_tables.py)
MIN_DURATION_S = 15.0  # vols de test conseillés : 20 à 40 s (guide) ; en dessous de 15 s, trop peu de fenêtres

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
    """Une correction : où la faire dans le configurateur, et la commande CLI équivalente (repli)."""
    where: str
    cli: str = ""


@dataclass
class CheckItem:
    title: str
    status: str  # OK, WARN, BLOCK, INFO
    detail: str
    fixes: list[Fix] = field(default_factory=list)


@dataclass
class Usage:
    """Ce que le log permet de faire (régler les filtres, lire une série PID…)."""
    label: str
    ok: bool
    reason: str


@dataclass
class LogCheck:
    label: str
    moving: bool  # vol avec mouvements (sinon : stationnaire)
    items: list[CheckItem]
    usages: list[Usage]

    def cli_commands(self) -> list[str]:
        return [f.cli for it in self.items if it.status in (WARN, BLOCK) for f in it.fixes if f.cli]


def _n(x: float, d: int = 1) -> str:
    return f"{x:.{d}f}".replace(".", ",")


def check_log(log: FlightLog, sel: slice | None = None, label: str = "") -> LogCheck:
    sel = sel or slice(0, log.n)
    s = log.settings
    items: list[CheckItem] = []
    fs = log.fs

    # version de Betaflight
    version_ok = s.firmware_name.lower().startswith("betaflight") and tuple(s.version[:2]) >= MIN_VERSION
    vtxt = ".".join(str(v) for v in s.version) or "inconnue"
    items.append(CheckItem("Version du firmware", OK if version_ok else BLOCK,
                           f"{s.firmware_name} {vtxt}." + ("" if version_ok else
                           " OpenTuneBox lit Betaflight 4.4 et plus récent."),
                           [] if version_ok else [Fix("Mettre à jour le firmware (onglet « Flasher le Firmware »), "
                                                      "après une sauvegarde `diff all`.")]))

    # durée de la période d'analyse
    i0, i1 = sel.start or 0, min(sel.stop or log.n, log.n)
    dur = float(log.t[i1 - 1] - log.t[i0]) if i1 > i0 else 0.0
    items.append(CheckItem("Durée analysée", OK if dur >= MIN_DURATION_S else WARN,
                           f"{_n(dur)} s de vol dans la période d'analyse"
                           + ("." if dur >= MIN_DURATION_S else " : trop court, visez 20 à 40 s par vol de test.")))

    # fréquence d'enregistrement (mêmes critères que rules.analyze)
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
        items.append(CheckItem("Fréquence d'enregistrement", BLOCK,
                               f"{fs:.0f} Hz ({loops}) : il faut au moins {base_hz:.0f} Hz. Le bruit au-dessus de "
                               f"{fs / 2:.0f} Hz se replie dans le spectre (fausses raies) : les filtres ne peuvent "
                               "pas être jugés.", rate_fix))
    elif not harm_ok:
        items.append(CheckItem("Fréquence d'enregistrement", WARN,
                               f"{fs:.0f} Hz : les moteurs tournent à ~{hz:.0f} Hz, leur 3e harmonique "
                               f"({3 * hz:.0f} Hz) se replie dans le spectre (petit drone, moteurs rapides).", rate_fix))
    else:
        items.append(CheckItem("Fréquence d'enregistrement", OK, f"{fs:.0f} Hz ({loops})."))

    # gyro non filtré : indispensable pour juger les filtres
    unfilt = log.gyro_unfilt is not None
    if unfilt:
        items.append(CheckItem("Gyro non filtré", OK, "Présent : l'effet des filtres et leur retard se mesurent."))
    elif s.family == "4.4":
        items.append(CheckItem("Gyro non filtré", BLOCK,
                               "Absent : en 4.4, seul le mode de débogage GYRO_SCALED l'enregistre.",
                               [Fix("Onglet « Blackbox » → « Mode débug de la Blackbox » : GYRO_SCALED.",
                                    "set debug_mode = GYRO_SCALED")]))
    else:
        items.append(CheckItem("Gyro non filtré", BLOCK, "Absent : le champ a été désactivé dans l'enregistrement.",
                               [Fix("Onglet « Blackbox » → « Champs de débogage inclus » : cocher « Gyro (Unfiltered) ».",
                                    "set blackbox_disable_gyrounfilt = OFF")]))

    # mode de débogage conseillé pour les filtres (centres des notchs dynamiques)
    if s.family != "4.4" and s.dyn_notch_count > 0 and s.debug_mode != "FFT_FREQ":
        items.append(CheckItem("Mode de débogage", WARN,
                               f"{s.debug_mode} : avec FFT_FREQ, le log contient aussi les fréquences suivies par "
                               "le notch dynamique (courbes du Spectre et diagnostic du bruit plus complets).",
                               [Fix("Onglet « Blackbox » → « Mode débug de la Blackbox » : FFT_FREQ.",
                                    "set debug_mode = FFT_FREQ")]))

    # télémétrie RPM
    if log.motor_hz is None:
        items.append(CheckItem("Télémétrie RPM", WARN,
                               "Absente : le filtre RPM est inactif et les harmoniques moteur ne sont qu'estimées.",
                               [Fix("Onglet « Moteurs » → « DShot bi-directionnel » activé (ESC BLHeli_32, Bluejay "
                                    "ou AM32), et « Pôles moteur » juste (aimants de la cloche, souvent 14).",
                                    "set dshot_bidir = ON")]))
    else:
        items.append(CheckItem("Télémétrie RPM", OK, f"Présente ({s.motor_poles} pôles moteur dans les réglages)."))

    # mouvements : stationnaire (bruit) ou avec mouvements (retard des filtres, réponse indicielle)
    p95 = stick_p95_dps(log, sel) if i1 > i0 else 0.0
    moving = p95 >= MOVING_SETPOINT_DPS
    wins = [input_windows(log.setpoint[a, sel], fs) for a in range(2)]
    enough_steps = max(wins) >= MIN_WINDOWS
    items.append(CheckItem("Type de vol", INFO,
                           ("Vol avec mouvements" if moving else "Vol stationnaire")
                           + f" (consigne roll/pitch au 95e centile : {p95:.0f} °/s, repère {MOVING_SETPOINT_DPS:.0f} °/s)."))

    # conditions d'une série PID (méthode de Brian White)
    cond = flight_conditions(log, sel)
    sv = slider_view(s)
    if cond.ff_active:
        items.append(CheckItem("Feedforward", WARN,
                               f"Actif (F roll {cond.ff[0]}, pitch {cond.ff[1]}) hors mode Angle : il fausse la "
                               "lecture d'une série PID.",
                               [Fix(f"Pour les séries : curseur « {CONFIGURATOR_SLIDERS['Stick Resp.']} » à 0, "
                                    "ou vols en mode Angle (le feedforward y est coupé).",
                                    "set simplified_feedforward_gain = 0")]))
    lo, hi = I_SLIDER_RANGE
    i_ok = lo <= cond.i_slider <= hi
    if not i_ok:
        items.append(CheckItem("Curseur I", WARN,
                               f"{_n(cond.i_slider, 2)} : pendant les séries PID, le terme I change la forme de la "
                               "réponse ; le garder entre 0,2 et 0,3, puis le remonter à la fin.",
                               [Fix(f"Curseur « {CONFIGURATOR_SLIDERS['Drift-Wobble']} » : 0,25 (Mode Expert).",
                                    "set simplified_i_gain = 25")]))
    if not sv.from_header:
        items.append(CheckItem("Réglage simplifié", WARN,
                               "Désactivé : les curseurs du configurateur ne pilotent pas les PID ; OpenTuneBox "
                               "estime leurs positions à partir des PID.",
                               [Fix("Onglet « Réglages de PID » → curseurs, « Mode » : RPY. Attention : les PID "
                                    "sont alors recalculés à partir des curseurs (notez-les avant).",
                                    "set simplified_pids_mode = RPY")]))

    crash = crash_time(log)
    if crash is not None:
        items.append(CheckItem("Crash", INFO, f"Crash vers {_n(crash)} s : la période d'analyse s'arrête avant."))
    if log.stats.get("corrupt_frames", 0) > 20:
        items.append(CheckItem("Trames corrompues", INFO,
                               f"{log.stats['corrupt_frames']} trames (pauses d'écriture de la mémoire) : sans "
                               "conséquence pour l'analyse."))

    # verdict par usage
    base_ok = version_ok and rate_ok and unfilt and dur >= MIN_DURATION_S
    base_why = _first_problem(items, ("Version du firmware", "Durée analysée", "Fréquence d'enregistrement",
                                      "Gyro non filtré"))
    aliased = "" if harm_ok else " Attention : 3e harmonique moteur repliée (voir la fréquence d'enregistrement)."
    usages = [
        Usage("Identifier le bruit (filtres)", base_ok,
              base_why if not base_ok else
              ("vol stationnaire : le meilleur pour repérer les raies." if not moving else
               "utilisable ; un vol stationnaire montre encore mieux les raies.") + aliased),
        Usage("Juger le retard des filtres", base_ok and moving,
              base_why if not base_ok else
              ("vol avec mouvements." if moving else
               "vol stationnaire : les filtres dynamiques ne travaillent qu'à un régime. "
               "Faites aussi un vol avec des mouvements francs.")),
    ]
    series_ok = version_ok and dur >= MIN_DURATION_S and enough_steps and not cond.ff_active and i_ok
    series_why = _first_problem(items, ("Version du firmware", "Durée analysée", "Feedforward", "Curseur I"))
    usages.append(Usage("Vol d'une série PID", series_ok,
                        "assez de mouvements, conditions respectées." if series_ok else series_why or
                        f"pas assez de mouvements (moins de {MIN_WINDOWS} fenêtres de plus de 20 °/s en roll et en "
                        "pitch) : volez 20 à 30 s de flips et rolls francs."))
    return LogCheck(label, moving, items, usages)


def _first_problem(items: list[CheckItem], titles: tuple[str, ...]) -> str:
    for it in items:
        if it.title in titles and it.status in (WARN, BLOCK):
            return f"« {it.title} » à corriger (voir ci-dessus)."
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
