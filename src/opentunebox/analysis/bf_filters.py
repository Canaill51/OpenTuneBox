"""Modèles exacts des filtres Betaflight (réponse en fréquence, retard, simulation sur log).

Réimplémentation à partir de docs/spec-betaflight.md §2–3 (sources : `common/filter.c`,
`sensors/gyro_init.c`, `flight/rpm_filter.c`, `flight/pid.c`, `flight/mixer.c`).

Conventions :
- un filtre = `FilterSpec` (type, fréquence, Q, poids) ; une « chaîne » = liste de
  `(FilterSpec, cadence_Hz)` car le LPF2 gyro tourne à la cadence gyro, le reste à la cadence PID ;
- coefficients en z sous la forme (b, a) avec a[0] = 1, comme scipy.signal ;
- la simulation temporelle traite tous les étages comme des biquads (forme directe I), ce qui
  permet de changer les coefficients à chaque échantillon (filtres RPM, LPF dynamique, notch dynamique).
"""

from __future__ import annotations

import math
from dataclasses import dataclass, replace
from typing import NamedTuple, Sequence

import numpy as np

from .. import _native

# Corrections de coupure : n PT1 identiques en cascade → −3 dB à fc (filter.c)
CUTOFF_CORRECTION_PT2 = 1.553773974
CUTOFF_CORRECTION_PT3 = 1.961459177
BUTTERWORTH_Q = 1.0 / math.sqrt(2.0)
FILTER_KINDS = ("pt1", "pt2", "pt3", "biquad_lpf", "notch")
# Type d'en-tête (LowpassSetting.type) → type de FilterSpec
_LPF_KIND = {"PT1": "pt1", "BIQUAD": "biquad_lpf", "PT2": "pt2", "PT3": "pt3"}
# Codes internes de la simulation
_K_PT1, _K_LPF, _K_NOTCH = 0, 1, 2


@dataclass(frozen=True)
class FilterSpec:
    """Un filtre élémentaire Betaflight.

    kind   : "pt1", "pt2", "pt3", "biquad_lpf" ou "notch" ;
    f_hz   : coupure (passe-bas, −3 dB) ou centre (notch) ;
    q      : facteur de qualité (notch, biquad) ;
    weight : poids du notch RPM, sortie y = w·notch(x) + (1−w)·x.
    """

    kind: str
    f_hz: float
    q: float = BUTTERWORTH_Q
    weight: float = 1.0

    def __post_init__(self):
        if self.kind not in FILTER_KINDS:
            raise ValueError(f"type de filtre inconnu : {self.kind}")


Chain = list[tuple[FilterSpec, float]]


class ChainDelay(NamedTuple):
    group_ms: float  # retard de groupe −dφ/dω à la fréquence demandée
    phase_ms: float  # retard de phase −φ/ω à la même fréquence (basse fréquence → retard « vu » par le PID)


# ---------------------------------------------------------------- formules élémentaires


def pt1_gain(f_cut: float, fs: float) -> float:
    """Gain k du PT1 Betaflight : ω/(ω+1) avec ω = 2π·fc/fs."""
    omega = 2.0 * math.pi * f_cut / fs
    return omega / (omega + 1.0)


def notch_q(center_hz: float, cutoff_hz: float) -> float:
    """Q d'un notch statique à partir du centre et de la coupure basse (filterGetNotchQ)."""
    return center_hz * cutoff_hz / (center_hz * center_hz - cutoff_hz * cutoff_hz)


def dyn_lpf_cutoff(min_hz, max_hz, expo, throttle_frac):
    """Coupure du passe-bas dynamique en fonction des gaz (0–1), comme le firmware.

    Les gaz sont quantifiés au centième (lrintf(t·100)/100, arrondi au pair comme lrintf).
    expo > 0 : courbe = t(1−t)·expo/10 + t, fc = (max−min)·courbe + min ;
    expo = 0 : ancienne loi fc = max(min, max·t(1 − t²/3)·1,5).
    Accepte un scalaire ou un tableau de gaz.
    """
    t = np.rint(np.clip(np.asarray(throttle_frac, dtype=np.float64), 0.0, 1.0) * 100.0) / 100.0
    if expo > 0:
        curve = t * (1.0 - t) * (expo / 10.0) + t
        fc = (max_hz - min_hz) * curve + min_hz
    else:
        fc = np.maximum(max_hz * (t * (1.0 - t * t / 3.0) * 1.5), min_hz)
    return float(fc) if fc.ndim == 0 else fc


def _sections(spec: FilterSpec) -> list[tuple[int, float, float, float]]:
    """Décompose un filtre en étages biquad : (code, fréquence corrigée, q, poids)."""
    if spec.kind == "pt1":
        return [(_K_PT1, spec.f_hz, 0.0, 1.0)]
    if spec.kind == "pt2":
        return [(_K_PT1, spec.f_hz * CUTOFF_CORRECTION_PT2, 0.0, 1.0)] * 2
    if spec.kind == "pt3":
        return [(_K_PT1, spec.f_hz * CUTOFF_CORRECTION_PT3, 0.0, 1.0)] * 3
    if spec.kind == "biquad_lpf":
        return [(_K_LPF, spec.f_hz, spec.q, 1.0)]
    return [(_K_NOTCH, spec.f_hz, spec.q, spec.weight)]


def coefficients(spec: FilterSpec, fs: float) -> tuple[np.ndarray, np.ndarray]:
    """Fonction de transfert H(z) = b(z⁻¹)/a(z⁻¹) du filtre à la cadence fs (Hz)."""
    b = np.array([1.0])
    a = np.array([1.0])
    for kind, f, q, w in _sections(spec):
        b0, b1, b2, a1, a2 = _native.biquad(kind, float(f), float(q), float(w), float(fs))
        if kind == _K_PT1:
            bs, as_ = np.array([b0]), np.array([1.0, a1])
        else:
            bs, as_ = np.array([b0, b1, b2]), np.array([1.0, a1, a2])
        b = np.convolve(b, bs)
        a = np.convolve(a, as_)
    return b, a


# ---------------------------------------------------------------- réponses en fréquence


def _sections_response(kinds, fcs, qs, ws, rates, freqs) -> np.ndarray:
    """Réponse complexe du produit des étages biquad (un élément par étage dans kinds, fcs, qs, ws, rates).

    Chaque étage est évalué seul, H = (b0 + b1·z⁻¹ + b2·z⁻²)/(1 + a1·z⁻¹ + a2·z⁻²), puis multiplié :
    même résultat que le produit des polynômes de `coefficients`, sans en construire aucun. Calcul en Rust.
    ⚠️ Gain d'un étage jamais sous 1e-15 : au centre exact d'un notch (harmonique moteur sur une case FFT),
    un vrai zéro rendrait nul le rapport |H_mod|²/|H_actuel|² du simulateur (L-024).
    """
    f64 = lambda a: np.ascontiguousarray(a, dtype=np.float64)  # noqa: E731
    return _native.sections_response(np.ascontiguousarray(kinds, dtype=np.int64), f64(fcs), f64(qs), f64(ws),
                                     f64(rates), f64(freqs))


def _chain_arrays(chain):
    """Étages biquad d'une chaîne [(FilterSpec, cadence)] en tableaux pour `_sections_response`."""
    rows = [(kind, f, q, w, fs) for spec, fs in chain for kind, f, q, w in _sections(spec)]
    if not rows:
        return (np.zeros(0, np.int64),) + (np.zeros(0),) * 4
    k, f, q, w, r = zip(*rows)
    return (np.array(k, np.int64), np.array(f, np.float64), np.array(q, np.float64), np.array(w, np.float64),
            np.array(r, np.float64))


def freq_response(specs: Sequence[FilterSpec], freqs, fs: float) -> np.ndarray:
    """Réponse complexe d'une suite de filtres tous à la cadence fs, aux fréquences `freqs` (Hz)."""
    return chain_response([(spec, fs) for spec in specs], freqs)


def chain_response(chain: Chain, freqs) -> np.ndarray:
    """Réponse complexe d'une chaîne à cadences mixtes : chaque étage évalué à sa propre cadence."""
    freqs = np.asarray(freqs, dtype=np.float64)
    h = _sections_response(*_chain_arrays(chain), freqs.ravel())
    return h.reshape(freqs.shape)


def chain_delay_ms(specs, fs: float | None = None, f_hz: float = 10.0) -> ChainDelay:
    """Retard de la chaîne en ms à la fréquence f_hz : (retard de groupe, retard de phase).

    `specs` est soit une liste de FilterSpec (tous à la cadence `fs`), soit une chaîne
    [(FilterSpec, cadence)] comme renvoyée par gyro_chain / dterm_chain.
    Aux basses fréquences (≈ 10 Hz) les deux valeurs se rejoignent : c'est le retard
    utile pour le réglage.
    """
    chain = _as_chain(specs, fs)
    df = max(1e-3, f_hz * 1e-3)
    f = np.array([f_hz - df, f_hz, f_hz + df])
    phase = np.unwrap(np.angle(chain_response(chain, f)))
    group = -(phase[2] - phase[0]) / (2.0 * np.pi * 2.0 * df)
    ph = -phase[1] / (2.0 * np.pi * f_hz)
    return ChainDelay(float(group * 1e3), float(ph * 1e3))


def _as_chain(specs, fs: float | None) -> Chain:
    out = []
    for item in specs:
        if isinstance(item, FilterSpec):
            if fs is None:
                raise ValueError("fs requis pour une liste de FilterSpec")
            out.append((item, float(fs)))
        else:
            out.append((item[0], float(item[1])))
    return out


# ---------------------------------------------------------------- chaînes depuis l'en-tête


LPF_MAX_HZ = 1000  # coupure maximale d'un passe-bas dans le firmware (LPF_MAX_HZ, DYN_LPF_MAX_HZ)


def scale_lowpass(lp, factor: float):
    """Passe-bas aux coupures multipliées par `factor` (comme un curseur de filtre), plafonnées à 1000 Hz ;
    un filtre coupé le reste (le firmware ne recalcule que les coupures non nulles, simplified_tuning.c)."""
    def k(v):
        return int(min(LPF_MAX_HZ, round(v * factor))) if v else 0
    return replace(lp, static_hz=k(lp.static_hz), dyn_min_hz=k(lp.dyn_min_hz), dyn_max_hz=k(lp.dyn_max_hz))


def lowpass_off(lp):
    """Passe-bas désactivé (coupures à 0)."""
    return replace(lp, static_hz=0, dyn_min_hz=0, dyn_max_hz=0)


def _lowpass_spec(lp, throttle_frac: float) -> FilterSpec | None:
    """Passe-bas statique ou dynamique (dyn_min > 0 remplace la valeur statique)."""
    if lp.dynamic:
        f = dyn_lpf_cutoff(lp.dyn_min_hz, lp.dyn_max_hz, lp.expo, throttle_frac)
    elif lp.static_hz > 0:
        f = float(lp.static_hz)
    else:
        return None
    return FilterSpec(_LPF_KIND.get(lp.type, "pt1"), f)


def _static_notch(center: int, cutoff: int, nyquist: float) -> FilterSpec | None:
    """Notch statique, centre ramené sous la limite de Nyquist comme le firmware."""
    if center > nyquist:
        center = int(nyquist) if cutoff < nyquist else 0
    if center <= 0 or cutoff <= 0 or cutoff >= center:
        return None
    return FilterSpec("notch", float(center), notch_q(center, cutoff))


def rpm_notch_freq_weight(motor_hz, harmonic: int, settings, rate_hz: float):
    """Fréquence et poids d'un notch RPM (harmonique h, 0 = fondamentale), scalaires ou tableaux."""
    max_hz = 0.48 * rate_hz
    f = np.clip((harmonic + 1) * np.asarray(motor_hz, dtype=np.float64), settings.rpm_min_hz, max_hz)
    margin = f - settings.rpm_min_hz
    fade = np.where(margin < settings.rpm_fade_hz, margin / max(1e-9, settings.rpm_fade_hz), 1.0)
    weights = settings.rpm_weights or [100]
    hw = weights[harmonic] if harmonic < len(weights) else 100
    w = np.clip(hw / 100.0, 0.0, 1.0) * fade
    return f, w


def dyn_notch_enabled(settings) -> bool:
    """Notch dynamique actif : count > 0 et cadence PID ≥ 2 kHz."""
    return settings.dyn_notch_count > 0 and settings.pid_rate_hz >= 2000 - 1e-6


def dyn_notch_initial_centres(settings) -> list[float]:
    """Centres initiaux du notch dynamique : (p + 0,5)(max − min)/count + min."""
    n = settings.dyn_notch_count
    lo, hi = settings.dyn_notch_min_hz, settings.dyn_notch_max_hz
    return [(p + 0.5) * (hi - lo) / n + lo for p in range(n)]


DYN_NOTCH_BIN_HZ = 18.5  # case de la SDFT du firmware à 8 kHz (dyn_notch_filter.c)
DYN_NOTCH_MIN_LIMIT_HZ = 20.0  # borne basse de dyn_notch_min_hz dans le firmware (cli/settings.c)


def complete_dyn_centres(known, settings, peaks) -> list[float]:
    """Centres du notch dynamique `settings` quand on en ajoute au-delà de ceux enregistrés dans le log (D-041).

    1. centres enregistrés (`known`), ramenés dans la plage de `settings` et limités à son nombre de notchs ;
    2. puis les pics prévus (`peaks`, triés par puissance décroissante) dans la plage, à plus de 1,5 case (27,75 Hz) d'un centre
       déjà pris : le firmware place ses notchs sur les pics les plus hauts, pas à des positions fixes ;
    3. sans pic pour un notch : son centre initial (comme le firmware au départ).
    Mesuré sur 383 sessions en debug FFT_FREQ de 64 drones (rappel 0,45 contre 0,28 pour les centres initiaux seuls)."""
    lo, hi = settings.dyn_notch_min_hz, settings.dyn_notch_max_hz
    c = [min(max(v, lo), hi) for v in known][: settings.dyn_notch_count]
    tol = 1.5 * DYN_NOTCH_BIN_HZ
    for f in peaks:
        if len(c) >= settings.dyn_notch_count:
            break
        if lo <= f <= hi and all(abs(f - v) > tol for v in c):
            c.append(float(f))
    return c + dyn_notch_initial_centres(settings)[len(c):]


def gyro_chain(settings, throttle_frac: float = 0.5, motor_hz=None, dyn_notch_hz=None) -> Chain:
    """Chaîne de filtres gyro complète, dans l'ordre du firmware.

    - LPF2 à la cadence gyro ;
    - à la cadence PID : notches RPM (moteur × harmonique, seulement si `motor_hz` est fourni,
      une fréquence par moteur), notches statiques 1 et 2, LPF1 (statique ou dynamique selon
      `throttle_frac`), notches dynamiques.
    `dyn_notch_hz` : centres des notches dynamiques ; None → centres initiaux du firmware,
    liste vide → notches dynamiques omis.
    """
    s = settings
    gyro_rate, pid_rate = s.gyro_rate_hz, s.pid_rate_hz
    chain: Chain = []
    lpf2 = _lowpass_spec(s.gyro_lpf2, throttle_frac)
    if lpf2 is not None:
        chain.append((lpf2, gyro_rate))
    if motor_hz is not None and s.rpm_harmonics > 0:
        q = s.rpm_q / 100.0
        mhz = np.atleast_1d(np.asarray(motor_hz, dtype=np.float64))
        fw = [rpm_notch_freq_weight(mhz, h, s, pid_rate) for h in range(s.rpm_harmonics)]  # tous les moteurs à la fois
        for m in range(mhz.shape[0]):
            for f, w in fw:
                if w[m] > 0:
                    chain.append((FilterSpec("notch", float(f[m]), q, float(w[m])), pid_rate))
    nyq = 1e6 / 2 / s.looptime_us / s.pid_denom * 0.95
    for center, cutoff in s.gyro_notches[:2]:
        sp = _static_notch(center, cutoff, nyq)
        if sp is not None:
            chain.append((sp, pid_rate))
    lpf1 = _lowpass_spec(s.gyro_lpf1, throttle_frac)
    if lpf1 is not None:
        chain.append((lpf1, pid_rate))
    if dyn_notch_enabled(s):
        centres = dyn_notch_initial_centres(s) if dyn_notch_hz is None else list(dyn_notch_hz)
        q = s.dyn_notch_q / 100.0
        for c in centres[: s.dyn_notch_count]:
            if c > 0 and q > 0:
                chain.append((FilterSpec("notch", float(c), q), pid_rate))
    return chain


def dterm_chain(settings, throttle_frac: float = 0.5) -> Chain:
    """Chaîne de filtres du terme D (cadence PID) : notch D → LPF1 D (statique/dynamique) → LPF2 D."""
    s = settings
    pid_rate = s.pid_rate_hz
    chain: Chain = []
    center, cutoff = s.dterm_notch
    sp = _static_notch(center, cutoff, pid_rate / 2.0)
    if sp is not None:
        chain.append((sp, pid_rate))
    for lp in (s.dterm_lpf1, s.dterm_lpf2):
        spec = _lowpass_spec(lp, throttle_frac)
        if spec is not None:
            chain.append((spec, pid_rate))
    return chain


# ---------------------------------------------------------------- simulation temporelle


def _apply(x, spec_kind: str, freq, fs: float, q: float = BUTTERWORTH_Q, weight=1.0):
    """Applique un filtre (fréquence/poids scalaires ou tableaux) via ses étages biquad."""
    n = x.shape[1]
    freq = np.broadcast_to(np.asarray(freq, dtype=np.float64), (n,))
    weight = np.broadcast_to(np.asarray(weight, dtype=np.float64), (n,))
    corr = {"pt2": CUTOFF_CORRECTION_PT2, "pt3": CUTOFF_CORRECTION_PT3}.get(spec_kind, 1.0)
    reps = {"pt2": 2, "pt3": 3}.get(spec_kind, 1)
    code = {"pt1": _K_PT1, "pt2": _K_PT1, "pt3": _K_PT1, "biquad_lpf": _K_LPF, "notch": _K_NOTCH}[spec_kind]
    f_eff = np.ascontiguousarray(freq * corr)
    w = np.ascontiguousarray(weight)
    limit = 0.48 * fs
    # filtre « au-delà » de la cadence du log : ignoré (la composante est de toute façon repliée)
    f_eff = np.where(freq < limit, f_eff, 0.0)
    for _ in range(reps):
        x = _native.run_section(np.ascontiguousarray(x, dtype=np.float64), code, f_eff, float(q), w, float(fs), np.inf)
    return x


def _lowpass_sim(x, lp, throttle_frac, fs):
    if not lp.enabled:
        return x
    kind = _LPF_KIND.get(lp.type, "pt1")
    if lp.dynamic:
        f = dyn_lpf_cutoff(lp.dyn_min_hz, lp.dyn_max_hz, lp.expo, throttle_frac)
    else:
        f = float(lp.static_hz)
    return _apply(x, kind, f, fs)


def fft_freq_centres(log, settings=None) -> np.ndarray | None:
    """Centres des notches dynamiques lus dans le debug FFT_FREQ, forme (count, n), ou None.

    2025.12+ : debug[1..count] ; 4.x : debug[p] pour p < count, mais debug[3] contient le gyro
    (le centre du 4e notch est perdu → mis à 0, notch ignoré). Valeurs de l'axe de débogage seul.
    """
    s = settings or log.settings
    if s.debug_mode != "FFT_FREQ" or s.dyn_notch_count <= 0:
        return None
    count = min(s.dyn_notch_count, 7)
    new = s.family in ("2025.12", "2026.6")
    rows = []
    for p in range(count):
        idx = p + 1 if new else p
        if (not new and idx == 3) or idx >= log.debug.shape[0]:
            rows.append(np.zeros(log.n))
        else:
            rows.append(np.asarray(log.debug[idx], dtype=np.float64))
    return np.vstack(rows)


def simulate_gyro(log, settings=None) -> np.ndarray:
    """Rejoue la chaîne de filtres gyro sur `log.gyro_unfilt` (3, n) → gyro filtré simulé (3, n).

    Approximations (le log est en général décimé par rapport à la boucle PID) :
    - tous les filtres sont calculés à la cadence du log (coefficients à fs du log), ceux dont la
      fréquence dépasse 0,48·fs du log sont ignorés à chaque échantillon ;
    - notches RPM variables depuis `log.motor_hz` (lissé par le PT1 rpm_filter_lpf_hz du firmware),
      coefficients recalculés à chaque échantillon ;
    - LPF1 dynamique depuis `log.throttle/100` (quantification au centième, sans la limite de 5 ms) ;
    - notches dynamiques depuis le debug FFT_FREQ si présent (centres de l'axe de débogage appliqués
      aux trois axes), sinon omis.
    `settings` permet de simuler d'autres réglages que ceux du log (comparaison avant/après).
    """
    if log.gyro_unfilt is None:
        raise ValueError("pas de gyro non filtré dans ce log")
    s = settings or log.settings
    fs = float(log.fs)
    thr = np.clip(np.asarray(log.throttle, dtype=np.float64) / 100.0, 0.0, 1.0)
    x = np.ascontiguousarray(np.asarray(log.gyro_unfilt, dtype=np.float64))

    # LPF2 (cadence gyro dans le firmware)
    x = _lowpass_sim(x, s.gyro_lpf2, thr, fs)

    # Filtre RPM
    if log.motor_hz is not None and s.rpm_harmonics > 0:
        q = s.rpm_q / 100.0
        k = pt1_gain(s.rpm_lpf_hz, fs) if s.rpm_lpf_hz > 0 else 1.0
        for m in range(log.motor_hz.shape[0]):
            mhz = _native.pt1_track(np.ascontiguousarray(log.motor_hz[m], dtype=np.float64), k)
            for h in range(s.rpm_harmonics):
                f, w = rpm_notch_freq_weight(mhz, h, s, s.pid_rate_hz)
                if not np.any(w > 0):
                    continue
                x = _apply(x, "notch", f, fs, q, w)

    # Notches statiques
    nyq = 1e6 / 2 / s.looptime_us / s.pid_denom * 0.95
    for center, cutoff in s.gyro_notches[:2]:
        sp = _static_notch(center, cutoff, nyq)
        if sp is not None:
            x = _apply(x, "notch", sp.f_hz, fs, sp.q)

    # LPF1 statique / dynamique
    x = _lowpass_sim(x, s.gyro_lpf1, thr, fs)

    # Notches dynamiques (centres du debug FFT_FREQ)
    if dyn_notch_enabled(s):
        centres = fft_freq_centres(log, s)
        if centres is not None:
            q = s.dyn_notch_q / 100.0
            for c in centres:
                if q > 0 and np.any(c > 0):
                    x = _apply(x, "notch", c, fs, q)
    return x


def simulate_dterm(log, settings=None, gyro: np.ndarray | None = None) -> np.ndarray:
    """Dérivée filtrée du gyro telle que vue par le terme D : −dg/dt passée dans la chaîne D (°/s²).

    `gyro` : gyro filtré à utiliser (par défaut `log.gyro`, ou par ex. simulate_gyro(log, réglages)).
    Le résultat est à multiplier par Kd·0,000529·Dmax·TPA pour obtenir le terme D (unités somme PID).
    Mêmes approximations que simulate_gyro (cadence du log, LPF1 dynamique selon les gaz).
    """
    s = settings or log.settings
    fs = float(log.fs)
    g = np.asarray(log.gyro if gyro is None else gyro, dtype=np.float64)
    delta = np.zeros_like(g)
    delta[:, 1:] = -np.diff(g, axis=1) * fs
    delta[:, 0] = delta[:, 1] if g.shape[1] > 1 else 0.0
    x = np.ascontiguousarray(delta)
    thr = np.clip(np.asarray(log.throttle, dtype=np.float64) / 100.0, 0.0, 1.0)
    center, cutoff = s.dterm_notch
    sp = _static_notch(center, cutoff, s.pid_rate_hz / 2.0)
    if sp is not None:
        x = _apply(x, "notch", sp.f_hz, fs, sp.q)
    x = _lowpass_sim(x, s.dterm_lpf1, thr, fs)
    x = _lowpass_sim(x, s.dterm_lpf2, thr, fs)
    return x


# ---------------------------------------------------------------- spectre « et si » (méthode relative)


@dataclass
class WhatIfSpectrum:
    freqs: np.ndarray
    current_db: np.ndarray  # spectre réellement enregistré (moyenne des fenêtres)
    modified_db: np.ndarray  # même vol, filtres modifiés


def whatif_spectrum(x_logged: np.ndarray, x_prefilt: np.ndarray | None, log, sel: slice, base, mod,
                    target: str = "gyro", seg_ms: float = 300.0, cache: dict | None = None) -> WhatIfSpectrum:
    """Spectre qu'aurait eu `x_logged` avec les filtres `mod` au lieu de `base`.

    Plutôt que de rejouer les filtres à la cadence du log (trop basse, repliement), on part du
    spectre réellement enregistré et, fenêtre par fenêtre (300 ms), on le multiplie par
    |H_mod(f)|² / |H_base(f)|², les deux chaînes étant évaluées à la vraie cadence PID avec le gaz,
    la vitesse des moteurs et les centres des notchs dynamiques de cette fenêtre. Avec des
    réglages inchangés, on retrouve exactement le log. Là où les filtres actuels écrasent tout
    (|H_base|² < 1 %), on repart du signal avant filtres × |H_mod|².

    target = "gyro" (chaîne gyro) ou "dterm" (chaîne gyro puis chaîne D).
    cache : dictionnaire partagé entre plusieurs appels sur le même log, la même période et les mêmes
    `base` / `mod` (axes, gyro et D) : les réponses des filtres de chaque fenêtre n'y sont calculées qu'une fois.
    """
    from .spectral import _seg_spectra, _segments, to_db

    fs = log.fs
    L = max(32, int(round(seg_ms * fs / 1000.0)))
    hop = L // 2
    x = np.asarray(x_logged[sel], dtype=np.float64)
    br = log.breaks(sel)  # aucune fenêtre sur un trou du log (D-020)
    segs, starts = _segments(x, L, hop, br)
    if segs.shape[0] == 0:
        return WhatIfSpectrum(np.zeros(0), np.zeros(0), np.zeros(0))
    f, P = _seg_spectra(segs, fs, "psd")
    Ppf = None
    if x_prefilt is not None:
        Ppf = _seg_spectra(_segments(np.asarray(x_prefilt[sel], dtype=np.float64), L, hop, br)[0], fs, "psd")[1]
    thr = np.asarray(log.throttle[sel], dtype=np.float64) / 100.0
    mhz = None if log.motor_hz is None else np.asarray(log.motor_hz[:, sel], dtype=np.float64)
    centres = fft_freq_centres(log)
    centres = None if centres is None else centres[:, sel]

    cache = {} if cache is None else cache
    peaks = ()
    dyn_changed = (mod.dyn_notch_count != base.dyn_notch_count or mod.dyn_notch_min_hz != base.dyn_notch_min_hz
                   or mod.dyn_notch_max_hz != base.dyn_notch_max_hz)
    if centres is not None and dyn_changed and mod.dyn_notch_count > 0:
        pkey = ("peaks", sel.start, sel.stop, sel.step, mod.dyn_notch_min_hz, mod.dyn_notch_max_hz)
        if pkey not in cache:
            cache[pkey] = _dyn_peaks(log, sel, base, mod)
        peaks = cache[pkey]
    # moyennes de chaque fenêtre, calculées en une fois (gaz au centième, moteurs et centres à 5 Hz près)
    idx = np.asarray(starts, dtype=np.int64)
    win_mean = lambda v: np.lib.stride_tricks.sliding_window_view(v, L, axis=-1)[..., idx, :].mean(axis=-1)  # noqa: E731
    thr_w = np.round(win_mean(thr), 2)
    mhz_w = None if mhz is None else np.round(win_mean(mhz) / 5.0) * 5.0
    cen_w = None if centres is None else np.round(win_mean(centres) / 5.0) * 5.0
    out_mod = np.zeros_like(P)
    for k in range(len(starts)):
        t = float(thr_w[k])
        m = None if mhz_w is None else tuple(mhz_w[:, k].tolist())
        c = None if cen_w is None else tuple(cen_w[:, k].tolist())
        key = ("gyro", f.size, t, m, c)
        if key not in cache:
            cache[key] = _gyro_h2(f, base, mod, t, m, c, peaks)
        r_cur, r_mod = cache[key]
        if target == "dterm":
            dkey = ("dterm", f.size, t)
            if dkey not in cache:
                cache[dkey] = tuple(np.abs(chain_response(dterm_chain(x, t), f)) ** 2 for x in (base, mod))
            d_cur, d_mod = cache[dkey]
            r_cur, r_mod = r_cur * d_cur, r_mod * d_mod
        # quotient direct : au creux d'un notch les deux |H|² valent ~1e-30 et un plancher sur le seul
        # dénominateur effondrerait le spectre (−90 dB) même à réglages inchangés (L-024)
        with np.errstate(divide="ignore", invalid="ignore"):
            ratio = np.where(r_cur > 0, np.minimum(r_mod / r_cur, 1e3), 1.0)
        est = P[k] * ratio
        if Ppf is not None:
            # là où les filtres actuels écrasaient le signal (le log n'y montre que le plancher de
            # bruit) et que les nouveaux filtres sont plus faibles, on repart du signal avant filtres
            weaker = (r_cur < 1e-2) & (r_mod > r_cur)
            est = np.where(weaker, np.maximum(est, Ppf[k] * r_mod), est)
        out_mod[k] = est
    return WhatIfSpectrum(f, to_db(P.mean(axis=0)), to_db(out_mod.mean(axis=0)))


def _dyn_peaks(log, sel, base, mod) -> tuple[float, ...]:
    """Pics que le notch dynamique `mod` aurait à suivre sur la période `sel` : roll du gyro non filtré × réponse des notchs RPM
    des réglages actuels (`motors.dyn_notch_targets`, la méthode validée contre FFT_FREQ), triés par puissance décroissante.
    Vide sans gyro non filtré : les notchs ajoutés restent alors à leurs centres initiaux."""
    from .motors import dyn_notch_targets

    unfilt = getattr(log, "gyro_unfilt", None)
    if unfilt is None:
        return ()
    mhz = None if log.motor_hz is None or base.rpm_harmonics <= 0 else np.asarray(log.motor_hz[:, sel], dtype=np.float64)
    band = (max(mod.dyn_notch_min_hz - 2 * DYN_NOTCH_BIN_HZ, DYN_NOTCH_MIN_LIMIT_HZ), mod.dyn_notch_max_hz + 2 * DYN_NOTCH_BIN_HZ)
    t = dyn_notch_targets(np.asarray(unfilt[0][sel], dtype=np.float64), mhz, log.fs, band, None if mhz is None else base,
                          breaks=log.breaks(sel))
    return tuple(float(v) for v in t.freqs)


def _gyro_h2(f, base, mod, thr, motors, centres, peaks=()):
    """|H|² des chaînes gyro actuelle et modifiée pour une fenêtre. `peaks` : pics prévus pour les notchs dynamiques ajoutés."""
    def h2(s):
        dyn = None
        if centres is not None:
            c = [v for v in centres if v > 0]
            if s.dyn_notch_count != base.dyn_notch_count or s.dyn_notch_min_hz != base.dyn_notch_min_hz \
                    or s.dyn_notch_max_hz != base.dyn_notch_max_hz:
                # réglage du notch dynamique modifié : centres enregistrés ramenés dans la nouvelle plage, puis notchs
                # ajoutés sur les pics prévus, sinon à leur centre initial (D-041)
                c = complete_dyn_centres(c, s, peaks)
            dyn = c
        return np.abs(chain_response(gyro_chain(s, thr, motor_hz=motors, dyn_notch_hz=dyn), f)) ** 2

    return h2(base), h2(mod)
