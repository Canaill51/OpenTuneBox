"""Diagnostic du réglage par la réponse indicielle, pour la fenêtre Réponse indicielle.

Méthode de Brian White (docs/spec-methodes.md, [S32]) : séries de vols où un seul curseur change par pas
de 0,2, feedforward à 0 (ou mode Angle), curseur I à 0,2–0,3. Trois blocs, plus un quatrième en finitions :
1. équilibre P/D (série Damping) : première valeur sans dépassement, ou au-delà de laquelle le pic ne
   baisse plus ;
2. niveau des gains (série Master) : on monte tant que la latence baisse sans cassure (« ripple ») ni
   vagues ; au premier signe, on garde la valeur d'avant ;
3. équilibre roll / pitch : latences des deux axes rapprochées.
4. série Feedforward (Réponse des sticks, en Acro, pas de 0,5 : 0, 0,5, 1…) : reconnue mais pas jugée, la
   réponse indicielle étant faussée par le feedforward ; ses retards consigne → gyro sont dans l'Assistant
   (choix de Cédric, 2026-10-12).
Seuils et origine : docs/assistant.md (« Diagnostic du réglage »).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

from ..analysis.step_series import FF_KEY, FlightStep, Series, find_series
from .rules import ATTENTION, CONSEIL, INFO, OK, STEP_MIN_WINDOWS, STEP_SURE_WINDOWS, Finding

SERIES_STEP = 0.2  # pas conseillé entre deux vols [S32]
STEP_TOO_FINE = 0.15  # pas plus fin : écarts trop faibles pour se lire sur la réponse [S32]
I_SLIDER_RANGE = (0.15, 0.35)  # curseur I conseillé 0,2–0,3 [S32], à 0,05 près
MIN_WINDOWS = STEP_MIN_WINDOWS  # fenêtres exploitables par axe : en dessous, le vol n'est pas lu
SURE_WINDOWS = STEP_SURE_WINDOWS  # en dessous, mesure moins sûre (avertissement) : consigne de Brian White [S35]
# équilibre P/D : séries Damping du corpus (8 drones dont le choix final est connu)
PEAK_FLAT_MAX = 1.05  # pic « sans dépassement » (repère 1,03 de rules.STEP_PEAK_FLAT, plus le bruit d'un vol)
PEAK_PLATEAU = 0.02  # le cran suivant ne baisse plus le pic que de moins de 0,02 : D suffisant
PEAK_PLATEAU_MAX = 1.10  # … à condition que le pic soit déjà modéré
PITCH_PEAK_GAP = 0.02  # pitch qui dépasse plus que le roll au Damping choisi : Pitch Damping à monter
# niveau des gains : séries Master du corpus (10 drones dont le choix final est connu)
RIPPLE_HIGH = 0.065  # cassure nette (gardés ≤ 0,067, rejetés ≥ 0,064)
RIPPLE_EARLY = 0.03  # début de cassure : près de l'optimum
RIPPLE_PLATEAU = 0.05  # cassure qui, avec une latence en plateau, suffit à arrêter
RIPPLE_STRONG = 0.10  # au-delà, pas d'affinage entre les deux crans
RIPPLE_GROWTH = 0.02  # cassure qui grandit d'un cran à l'autre
LATENCY_PLATEAU_PCT = 4.0  # gain de latence d'un cran de +0,2 sous lequel la latence ne baisse plus
# équilibre roll / pitch [S32]
LATENCY_GAP_OK_MS = 1.0  # écart de latence pitch − roll : équilibré
LATENCY_GAP_MAX_MS = 2.0  # acceptable selon le drone (inertie, moteurs) ; au-delà, à corriger
WAVES_RATIO_HIGH = 2.5  # vagues nettement au-dessus du bruit (Estimation : bruit ≈ 1–1,5, essais synthétiques)

AXES2 = ("Roll", "Pitch")
# noms des curseurs dans le configurateur Betaflight (les séries gardent les clés de PIDtoolbox)
SLIDER_NAMES = {"Damping": "Damping", "Master": "Master", "Pitch T.": "Pitch Tracking", "Pitch D.": "Pitch Damping",
                FF_KEY: "Feedforward"}


@dataclass
class TuningBlock:
    title: str
    series: Series | None = None
    lines: list[str] = field(default_factory=list)  # mesures vol par vol
    finding: Finding | None = None
    best: int | None = None  # indice du vol conseillé


@dataclass
class TuningReport:
    conditions: list[Finding] = field(default_factory=list)
    blocks: list[TuningBlock] = field(default_factory=list)


def _fmt(v: float) -> str:
    """1,2 / 1,15 / 0,065 : virgule, zéros inutiles retirés (au moins un chiffre après la virgule)."""
    t = f"{v:.3f}".rstrip("0")
    return (t + "0" if t.endswith(".") else t).replace(".", ",")


def _n(x: float, d: int = 2, sign: bool = False) -> str:
    """Nombre à la française : virgule décimale, « – » s'il manque."""
    if not math.isfinite(x):
        return "–"
    return f"{x:{'+' if sign else ''}.{d}f}".replace(".", ",")


def _axes(f: FlightStep) -> list[int]:
    return [a for a in range(2) if a < len(f.n) and f.n[a] >= MIN_WINDOWS]


def _pick(flights: list[FlightStep], s: Series) -> tuple[list[float], list[int]]:
    """Un vol par valeur du curseur : celui qui a le plus de fenêtres."""
    best: dict[float, int] = {}
    for i, v in zip(s.flights, s.values):
        if v not in best or sum(flights[i].n) > sum(flights[best[v]].n):
            best[v] = i
    vals = sorted(best)
    return vals, [best[v] for v in vals]


def _conditions(flights: list[FlightStep], ff_series: set[int] = frozenset()) -> list[Finding]:
    """Conditions du protocole ; les vols d'une série Feedforward (`ff_series`) ont le feedforward voulu."""
    out = []
    ff = [f.label for i, f in enumerate(flights) if f.conditions.ff_active and i not in ff_series]
    if ff:
        out.append(Finding("Réglage", CONSEIL, "Feedforward actif",
                           f"{', '.join(ff)} : feedforward > 0 hors mode Angle. Il gonfle le pic et raccourcit la "
                           "latence : la réponse ne montre plus seulement le P et le D.",
                           ["Pour les séries : feedforward à 0 en Acro, ou vols en mode Angle (le firmware y coupe "
                            "le feedforward de la boucle de vitesse)."]))
    angle = [flights[i].label for i in sorted(ff_series) if flights[i].conditions.angle]
    if angle:
        out.append(Finding("Réglage", ATTENTION, "Série Feedforward en mode Angle",
                           f"{', '.join(angle)} : le firmware coupe le feedforward en mode Angle, ces vols ne "
                           "mesurent pas son effet.", ["Série Feedforward : vols en Acro uniquement."]))
    lo, hi = I_SLIDER_RANGE
    bad_i = [f"{f.label} ({_fmt(f.conditions.i_slider)})" for f in flights if not lo <= f.conditions.i_slider <= hi]
    if bad_i:
        out.append(Finding("Réglage", CONSEIL, "Curseur I hors de 0,2–0,3",
                           f"{', '.join(bad_i)}. Le terme I change la forme de la réponse et fausse la lecture.",
                           ["Pour les séries : curseur I (Drift-Wobble) entre 0,2 et 0,3, remonté une fois le "
                            "réglage terminé."]))
    few = [f.label for f in flights if not _axes(f)]
    if few:
        out.append(Finding("Réglage", ATTENTION, "Pas assez de mouvements",
                           f"{', '.join(few)} : moins de {MIN_WINDOWS} fenêtres exploitables en roll et en pitch.",
                           ["Volez 20 à 30 s avec des flips et rolls francs sur chaque axe."]))
    thin = [f"{f.label} (N = {min(f.n[:2])})" for f in flights
            if _axes(f) and len(f.n) >= 2 and min(f.n[:2]) < SURE_WINDOWS]
    if thin:
        out.append(Finding("Réglage", CONSEIL, "Mesure moins sûre",
                           f"{', '.join(thin)} : moins de {SURE_WINDOWS} fenêtres en roll ou en pitch. La réponse "
                           "moyenne bouge encore d'un vol à l'autre ; le choix peut se jouer à un cran près.",
                           ["Wobbles roll et pitch mêlés, au moins 20 s (environ 12 fenêtres par seconde de "
                            "mouvement)."]))
    return out


def _step_note(s: Series) -> str:
    fine = [d for d in s.steps if d < STEP_TOO_FINE]
    if not fine:
        return ""
    return (f" Pas de {_fmt(min(fine))} entre deux vols : trop fin pour se lire sur la réponse, "
            f"préférez {_fmt(SERIES_STEP)}.")


def _peak(f: FlightStep, ax: int) -> float:
    """Pic arrondi à 0,01 : la valeur lue à l'écran (1,052 s'affiche et se juge 1,05)."""
    return round(f.metrics[ax].peak, 2) if ax in _axes(f) else float("nan")


def damping_block(flights: list[FlightStep], s: Series) -> TuningBlock:
    """Série Damping (jugée sur le roll : Damping règle le D des deux axes, le pitch se corrige ensuite par
    Pitch D.) ou série Pitch D. (jugée sur le pitch)."""
    ax = 1 if s.key == "Pitch D." else 0
    name = SLIDER_NAMES[s.key]
    title = f"Équilibre roll / pitch (série {name})" if ax else "Équilibre P/D (série Damping)"
    b = TuningBlock(title, s)
    vals, idx = _pick(flights, s)
    peaks = [_peak(flights[i], ax) for i in idx]
    for v, i in zip(vals, idx):
        f = flights[i]
        b.lines.append(f"{name} {_fmt(v)} : pic " + " / ".join(
            f"{AXES2[a].lower()} {_n(f.metrics[a].peak, 2)}" for a in _axes(f)) + f" ({f.label})")
    ok = [k for k, p in enumerate(peaks) if math.isfinite(p)]
    if len(ok) < 2:
        b.finding = Finding("Réponse", INFO, f"Série {name} incomplète",
                            "Il faut au moins deux vols exploitables.", [])
        return b
    axis = AXES2[ax].lower()
    chosen, why, flat = None, "", False
    for j, k in enumerate(ok):
        if peaks[k] <= PEAK_FLAT_MAX:
            chosen, flat = k, True
            why = f"premier pic {axis} sans dépassement visible (≤ {_fmt(PEAK_FLAT_MAX)})"
            break
        if j + 1 < len(ok) and peaks[k] <= PEAK_PLATEAU_MAX and peaks[k] - peaks[ok[j + 1]] < PEAK_PLATEAU - 1e-9:
            chosen, why = k, f"le cran suivant ne baisse plus le pic {axis} (moins de {_fmt(PEAK_PLATEAU)})"
            break
    note = _step_note(s)
    if chosen is None:
        nxt = vals[ok[-1]] + SERIES_STEP
        b.finding = Finding("Réponse", CONSEIL, f"Encore du dépassement en {axis}",
                            f"Le pic {axis} reste au-dessus de {_fmt(PEAK_FLAT_MAX)} jusqu'à {name} "
                            f"{_fmt(vals[ok[-1]])}." + note, [f"Volez avec {name} {_fmt(nxt)}, puis comparez."])
        return b
    b.best = idx[chosen]
    if chosen == ok[0] and flat:
        prv = vals[chosen] - SERIES_STEP
        b.finding = Finding("Réponse", CONSEIL, f"{name} {_fmt(vals[chosen])} : déjà sans dépassement",
                            f"Pic {axis} {_n(peaks[chosen], 2)} dès la plus petite valeur testée : une valeur "
                            "plus basse suffit peut-être." + note,
                            [f"Volez avec {name} {_fmt(prv)} pour trouver la limite."])
        return b
    detail = f"Pic {axis} {_n(peaks[chosen], 2)} : {why}." + note
    actions = [f"Gardez {name} {_fmt(vals[chosen])}" + (" pour la série Master." if not ax else ".")]
    sev = OK
    pitch = _peak(flights[idx[chosen]], 1)
    if not ax and pitch > PEAK_FLAT_MAX and pitch - peaks[chosen] >= PITCH_PEAK_GAP:
        sev = CONSEIL
        detail += f" Le pitch dépasse encore ({_n(pitch, 2)}) : il a plus d'inertie, son D se règle à part."
        actions.append(f"Pitch Damping (simplified_pitch_d_gain) +{_fmt(SERIES_STEP)} pour amortir le pitch, "
                       "puis comparez son pic.")
    b.finding = Finding("Réponse", sev, f"{name} conseillé : {_fmt(vals[chosen])}", detail, actions)
    return b


def _ripple(f: FlightStep) -> tuple[float, float]:
    """Plus forte cassure des deux axes et son écart-type."""
    best = (float("nan"), float("nan"))
    for a in _axes(f):
        m = f.metrics[a]
        if math.isfinite(m.ripple) and not (m.ripple <= best[0]):
            best = (m.ripple, m.ripple_std)
    return best


def _latency(f: FlightStep) -> float:
    v = [f.metrics[a].latency_ms for a in _axes(f)]
    return sum(v) / len(v) if v else float("nan")


def _master_signs(rip: tuple[float, float], prev_rip: float, gain: float, waves: float) -> tuple[list[str], list[str]]:
    """(signes de gains trop hauts, premiers signes) d'un cran de la série Master."""
    strong, early = [], []
    if rip[0] >= RIPPLE_HIGH:
        strong.append(f"cassure {_n(rip[0], 3)} (≥ {_n(RIPPLE_HIGH, 3)})")
    elif rip[0] >= RIPPLE_EARLY:
        early.append(f"début de cassure ({_n(rip[0], 3)})")
    if math.isfinite(gain) and gain < LATENCY_PLATEAU_PCT:
        text = f"latence presque plus améliorée ({_n(gain, 1)} %)"
        (strong if rip[0] >= RIPPLE_PLATEAU and rip[0] - prev_rip >= RIPPLE_GROWTH else early).append(text)
    if waves >= WAVES_RATIO_HIGH:
        strong.append(f"vagues sur le palier (× {_n(waves, 1)} le bruit)")
    return strong, early


def master_block(flights: list[FlightStep], s: Series) -> TuningBlock:
    b = TuningBlock("Niveau des gains (série Master)", s)
    vals, idx = _pick(flights, s)
    rip = [_ripple(flights[i]) for i in idx]
    lat = [_latency(flights[i]) for i in idx]
    waves = [max((flights[i].metrics[a].waves_ratio for a in _axes(flights[i])), default=float("nan")) for i in idx]
    gains = [float("nan")] + [100 * (1 - lat[k] / lat[k - 1]) if lat[k - 1] > 0 else float("nan")
                              for k in range(1, len(idx))]
    signs = [([], [])] + [_master_signs(rip[k], rip[k - 1][0], gains[k], waves[k]) for k in range(1, len(idx))]
    for k, (v, i) in enumerate(zip(vals, idx)):
        gain = f", gain {_n(gains[k], 1)} %" if k else ""
        unc = f" ± {_n(rip[k][1], 3)}" if math.isfinite(rip[k][1]) else ""
        state = "trop haut" if signs[k][0] else "premiers signes" if signs[k][1] else "propre"
        b.lines.append((f"Master {_fmt(v)} : latence {_n(lat[k], 1)} ms{gain}, cassure {_n(rip[k][0], 3, True)}{unc}, "
                        f"vagues × {_n(waves[k], 1)} — {state} ({flights[i].label})"))
    note = _step_note(s)
    stop = next((k for k in range(1, len(idx)) if signs[k][0]), None)
    if stop is None:
        last = len(idx) - 1
        b.best = idx[last]
        nxt = _fmt(vals[last] + SERIES_STEP)
        if signs[last][1]:
            b.finding = Finding("Réponse", CONSEIL, f"Master {_fmt(vals[last])} : proche de l'optimum",
                                f"Premiers signes : {', '.join(signs[last][1])}." + note,
                                [f"Vous pouvez garder Master {_fmt(vals[last])}, ou voler à {nxt} pour confirmer : "
                                 "si la cassure devient nette, revenez à la valeur d'avant.",
                                 "Surveillez aussi la chaleur des moteurs et le bruit (fenêtre Spectre)."])
        else:
            b.finding = Finding("Réponse", CONSEIL, f"Master {_fmt(vals[last])} sans signe de gains trop hauts",
                                "Latence encore en baisse, sans cassure ni vagues." + note,
                                [f"Volez avec Master {nxt}, puis comparez.",
                                 "Surveillez aussi la chaleur des moteurs et le bruit (fenêtre Spectre)."])
        return b
    keep = stop - 1
    b.best = idx[keep]
    actions = [f"Gardez Master {_fmt(vals[keep])}."]
    strong = rip[stop][0] >= RIPPLE_STRONG or waves[stop] >= WAVES_RATIO_HIGH
    if not strong and vals[stop] - vals[keep] > 0.1 + 1e-9:
        actions.append(f"Affinage possible : Master {_fmt((vals[keep] + vals[stop]) / 2)}.")
    if math.isfinite(rip[stop][1]) and rip[stop][1] > 0.03:
        actions.append("Cassure mesurée avec une grande incertitude : un vol de plus à cette valeur la confirmerait.")
    b.finding = Finding("Réponse", OK, f"Master conseillé : {_fmt(vals[keep])}",
                        f"À Master {_fmt(vals[stop])} : {', '.join(signs[stop][0])}." + note, actions)
    return b


def _gap(f: FlightStep) -> float:
    """Latence pitch − roll (ms) ; nan si un axe manque."""
    if _axes(f) != [0, 1]:
        return float("nan")
    return f.metrics[1].latency_ms - f.metrics[0].latency_ms


def _gap_line(f: FlightStep) -> str:
    return (f"latence roll {_n(f.metrics[0].latency_ms, 1)} ms, pitch {_n(f.metrics[1].latency_ms, 1)} ms "
            f"(écart {_n(_gap(f), 1, True)} ms), cassure pitch {_n(f.metrics[1].ripple, 3, True)}")


def roll_pitch_block(flights: list[FlightStep], s: Series | None, ref: int | None) -> TuningBlock | None:
    """Série Pitch T. (latences) si elle existe, sinon le vol `ref` (conseillé par la série Master, ou le
    vol le mieux mesuré)."""
    if s is not None:
        name = SLIDER_NAMES[s.key]
        b = TuningBlock(f"Équilibre roll / pitch (série {name})", s)
        vals, idx = _pick(flights, s)
        for v, i in zip(vals, idx):
            b.lines.append(f"{name} {_fmt(v)} : {_gap_line(flights[i])} ({flights[i].label})")
        ok = [k for k, i in enumerate(idx) if math.isfinite(_gap(flights[i]))
              and not flights[i].metrics[1].ripple >= RIPPLE_HIGH]
        if not ok:
            b.finding = Finding("Réponse", INFO, "Série incomplète ou cassure sur le pitch",
                                "Aucun vol avec les deux axes mesurés et sans cassure nette sur le pitch.", [])
            return b
        k = min(ok, key=lambda k: abs(_gap(flights[idx[k]])))
        b.best = idx[k]
        g = _gap(flights[idx[k]])
        sev = OK if abs(g) <= LATENCY_GAP_OK_MS else CONSEIL if abs(g) <= LATENCY_GAP_MAX_MS else ATTENTION
        b.finding = Finding("Réponse", sev, f"{name} conseillé : {_fmt(vals[k])}",
                            f"Écart de latence pitch − roll le plus faible sans cassure nette : {_n(g, 1, True)} ms "
                            f"(visé : {_n(LATENCY_GAP_OK_MS, 0)} à {_n(LATENCY_GAP_MAX_MS, 0)} ms au plus)."
                            + _step_note(s), [] if sev == OK else [_gap_action(g)])
        return b
    if ref is None:
        return None
    f = flights[ref]
    g = _gap(f)
    if not math.isfinite(g):
        return None
    b = TuningBlock("Équilibre roll / pitch", None, [f"{_gap_line(f)} ({f.label})"], best=ref)
    if abs(g) <= LATENCY_GAP_OK_MS:
        b.finding = Finding("Réponse", OK, "Roll et pitch équilibrés",
                            f"Écart de latence {_n(g, 1, True)} ms (≤ {_n(LATENCY_GAP_OK_MS, 0)} ms).", [])
    elif abs(g) <= LATENCY_GAP_MAX_MS:
        b.finding = Finding("Réponse", INFO, "Roll et pitch presque équilibrés",
                            f"Écart de latence {_n(g, 1, True)} ms : acceptable (1 à 2 ms selon le drone).",
                            [_gap_action(g) + " (facultatif)"])
    else:
        b.finding = Finding("Réponse", CONSEIL, f"Le {'pitch' if g > 0 else 'roll'} répond plus lentement",
                            f"Écart de latence {_n(g, 1, True)} ms (> {_n(LATENCY_GAP_MAX_MS, 0)} ms).",
                            [_gap_action(g)])
    return b


def _gap_action(g: float) -> str:
    sign = "+" if g > 0 else "−"
    return (f"Pitch Tracking (simplified_pitch_pi_gain) {sign}0,2 pour rapprocher les latences ; vérifiez "
            "qu'aucune cassure n'apparaît sur le pitch.")


def ff_block(flights: list[FlightStep], s: Series) -> TuningBlock:
    """Série Feedforward (Acro) reconnue : la réponse indicielle, faussée par le feedforward, ne la juge pas ; les
    retards consigne → gyro se lisent dans l'Assistant (`rules._setpoint_delay`) et sous les logs du guide."""
    vals, idx = _pick(flights, s)
    b = TuningBlock("Feedforward (série Réponse des sticks)", s,
                    [f"Feedforward {_fmt(v)} : {flights[i].label}" for v, i in zip(vals, idx)])
    b.finding = Finding("Réponse", INFO, "Série Feedforward : lisez les retards dans l'Assistant",
                        "Le feedforward fausse la réponse indicielle : elle ne sert pas à choisir sa valeur. "
                        "L'Assistant donne, pour chaque vol, le retard consigne → gyro en roll et en pitch.", [])
    return b


def step_tuning(flights: list[FlightStep]) -> TuningReport:
    series = find_series(flights)
    ff_series = {i for s in series if s.key == FF_KEY for i in s.flights}
    rep = TuningReport(_conditions(flights, ff_series))
    master_best = None
    for s in series:
        if s.key == FF_KEY:
            rep.blocks.append(ff_block(flights, s))
        elif s.key in ("Damping", "Pitch D."):
            rep.blocks.append(damping_block(flights, s))
        elif s.key == "Master":
            b = master_block(flights, s)
            rep.blocks.append(b)
            master_best = b.best
    pitch = [s for s in series if s.key == "Pitch T."]
    for s in pitch:
        rep.blocks.append(roll_pitch_block(flights, s, None))
    if not pitch:
        ref = master_best if master_best is not None else max(
            (i for i, f in enumerate(flights) if _axes(f) == [0, 1]), key=lambda i: sum(flights[i].n), default=None)
        b = roll_pitch_block(flights, None, ref)
        if b is not None:
            rep.blocks.append(b)
    return rep
