"""Boucle ouverte L(f) d'un axe et ses marges de stabilité (docs/spec-methodes.md, « Boucle ouverte »).

Mesure (« mesurée ») : boucle coupée à l'entrée du drone, comme dans les manuels. Le régulateur de
Betaflight est u = P + I + D + F avec P + I = Ce·e sur l'erreur e = consigne − gyro, D = −Cd·gyro (D sur le
gyro) et F sur la consigne ; le drone G va de u au gyro. Les termes sont loggés séparément, d'où :
- Ĉe = S(e, P+I)/S(e, e) et Ĉd = −S(gyro, D)/S(gyro, gyro) : régressions directes (sorties déterministes) ;
- Ĝ = S(r, gyro)/S(r, u) : estimateur instrumental, la consigne r servant d'instrument (l'estimateur
  direct S(u, gyro)/S(u, u) est biaisé en boucle fermée : les perturbations corrèlent u et gyro) ;
- L = Ĝ·(Ĉe + Ĉd), sensibilité S = 1/(1 + L).
En mode angle ou horizon, la consigne dépend de l'attitude : l'instrument est alors la commande radio.
Fenêtres de Hann de 2 s recouvertes de moitié, où la consigne dépasse 20 °/s ; spectres lissés sur
20 % de la fréquence (au moins 5 points) ; bande valide = points contigus depuis 2 Hz, jusqu'à
min(200 Hz ; 0,4 × fréquence du log), tant que l'erreur aléatoire de Ĝ reste ≤ 0,2 et que les
cohérences erreur → P+I et gyro → D restent ≥ 0,8. Erreur aléatoire d'une fonction de transfert
estimée avec la cohérence γ² sur n moyennes : √((1 − γ²)/(2·n·γ²)), cumulée sur instrument → gyro et
instrument → u ; n = fenêtres × largeur du lissage / 2. Incertitude des marges par jackknife (8 blocs
de fenêtres consécutives).

Approchée (« approchée », repli) : L = T/(1 − T), T = consigne → gyro. Juste seulement si tout le
régulateur agit sur l'erreur ; faux pour Betaflight (D sur le gyro, feedforward sur la consigne).

Marges (`margins`) :
- phase déroulée (`np.unwrap`), ramenée dans (−360°, 0] au premier point : une phase sous −180°
  donne une marge négative, au lieu de repartir vers +180° comme avec `np.angle` seul ;
- croisement : premier passage de |L| au-dessous de 1, interpolé en (log f, log |L|) ;
  marge de phase = 180° + ∠L à ce croisement. Les croisements suivants sont ignorés : sur les vrais
  logs, ils viennent presque toujours du bruit (|L| qui repasse 1 vers 100 Hz, docs/validation.md) ;
- marge de gain = −20·log10 |L| au premier passage de la phase par −180° (− k·360°) après le croisement ;
- pic de sensibilité Ms = max |1/(1 + L)| ; s'il tombe au bord de la bande, ce n'est qu'un minorant.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
from scipy import signal
from scipy.ndimage import uniform_filter1d

from .bf_filters import chain_response, dterm_chain
from .spectral import clear_of_breaks

MEASURED, APPROX = "mesurée", "approchée"
F_MIN, F_MAX = 2.0, 200.0  # bande de la mesure (Hz), plafonnée à 0,4 × fréquence du log
MAX_ERR_G = 0.2  # erreur aléatoire relative de Ĝ tolérée (≈ ±1,6 dB, ±11°)
MIN_COH_PID = 0.8  # cohérences erreur → P+I et gyro → D
SMOOTH = 5  # lissage des spectres (points), méthode approchée
SMOOTH_REL = 0.2  # lissage de la mesure : largeur = 20 % de la fréquence (au moins SMOOTH points)
BLOCKS = 8  # jackknife
DTERM_SCALE = 0.000529  # flight/pid.h : Kd = DTERM_SCALE × D


@dataclass
class Margins:
    crossover_hz: float = float("nan")  # premier passage de |L| au-dessous de 1
    phase_margin_deg: float = float("nan")  # négative si la phase y est sous −180°
    gain_margin_db: float = float("nan")
    phase_crossover_hz: float = float("nan")  # ∠L = −180° (− k·360°) après le croisement
    sensitivity_peak_db: float = float("nan")  # max |1/(1 + L)| en dB
    sensitivity_peak_hz: float = float("nan")
    sensitivity_peak_at_edge: bool = False  # pic au bord de la bande : Ms réel ≥ valeur donnée


def margins(freqs: np.ndarray, L: np.ndarray, valid: np.ndarray | None = None) -> Margins:
    """Marges de phase et de gain, et pic de sensibilité, de la boucle ouverte L sur les points `valid`."""
    freqs = np.asarray(freqs, dtype=np.float64)
    L = np.asarray(L, dtype=complex)
    ok = np.isfinite(L) & (np.abs(L) > 0) & (freqs > 0)
    if valid is not None:
        ok &= np.asarray(valid, dtype=bool)
    m = Margins()
    if ok.sum() < 2:
        return m
    f, Lv = freqs[ok], L[ok]
    logf = np.log(f)
    logmag = np.log(np.abs(Lv))
    phase = np.unwrap(np.angle(Lv))
    phase -= 2 * np.pi * np.ceil(phase[0] / (2 * np.pi))  # premier point dans (−360°, 0]

    # premier passage de |L| au-dessous de 1 : marge de phase
    down = np.nonzero((logmag[:-1] >= 0) & (logmag[1:] < 0))[0]
    if down.size:
        x, _, ph = _interp(logf, logmag, phase, logmag, int(down[0]))
        m.crossover_hz, m.phase_margin_deg = float(np.exp(x)), float(180.0 + np.degrees(ph))

    # premier passage de la phase par −180° (− k·360°) après le croisement, |L| < 1 : marge de gain
    level = (phase + np.pi) / (2 * np.pi)  # entier aux phases −180° + k·360°
    start = int(down[0]) + 1 if down.size else 0
    for i in range(start, logf.size - 1):
        k = np.floor(max(level[i], level[i + 1]))
        if min(level[i], level[i + 1]) <= k:  # un entier entre les deux points
            x, lm, _ = _interp(logf, logmag, phase, level - k, i)
            if lm < 0:
                m.gain_margin_db, m.phase_crossover_hz = float(-20.0 * lm / np.log(10)), float(np.exp(x))
                break

    S = np.abs(1.0 / (1.0 + Lv))
    k = int(np.argmax(S))
    m.sensitivity_peak_db = float(20 * np.log10(S[k]))
    m.sensitivity_peak_hz = float(f[k])
    m.sensitivity_peak_at_edge = k in (0, S.size - 1)
    return m


def _interp(logf, logmag, phase, g, i):
    """Passage de g par zéro entre les points i et i + 1 : (log f, log |L|, phase) interpolés linéairement."""
    t = g[i] / (g[i] - g[i + 1]) if g[i] != g[i + 1] else 0.0
    return (logf[i] + t * (logf[i + 1] - logf[i]),
            logmag[i] + t * (logmag[i + 1] - logmag[i]),
            phase[i] + t * (phase[i + 1] - phase[i]))


# ---------------------------------------------------------------- boucle ouverte d'un axe


@dataclass
class PidTerms:
    """Termes PID loggés d'un axe sur la période analysée (unités « somme PID »)."""

    p: np.ndarray
    i: np.ndarray
    d: np.ndarray
    f: np.ndarray
    note: str = ""  # non vide : boucle ouverte non mesurable (raison), repli sur la méthode approchée
    instrument: np.ndarray | None = None  # signal extérieur à la boucle pour estimer G ; défaut : la consigne


@dataclass
class LoopMetrics:
    method: str = APPROX  # MEASURED ou APPROX
    notes: list[str] = field(default_factory=list)  # pourquoi la méthode approchée, avertissements
    band_hz: tuple[float, float] = (float("nan"), float("nan"))  # points valides de la mesure
    bandwidth_hz: float = float("nan")  # |T| = −3 dB
    tracking_peak_db: float = float("nan")  # max |1 − T| en dB (consigne → erreur, ancien « pic de sensibilité »)
    sensitivity_peak_db: float = float("nan")  # max |1/(1 + L)| en dB
    sensitivity_peak_hz: float = float("nan")
    sensitivity_peak_at_edge: bool = False  # pic au bord de la bande analysée : Ms réel ≥ valeur donnée
    crossover_hz: float = float("nan")  # premier passage de |L| au-dessous de 1
    phase_margin_deg: float = float("nan")  # négative possible
    gain_margin_db: float = float("nan")
    d_ratio: float = float("nan")  # |Ĉd/Ĉe| au croisement : > 1, le D domine (mesurée)
    std: dict[str, float] = field(default_factory=dict)  # écarts-types jackknife (mesurée)
    freqs: np.ndarray = field(default_factory=lambda: np.zeros(0))
    T: np.ndarray = field(default_factory=lambda: np.zeros(0, dtype=complex))
    coherence: np.ndarray = field(default_factory=lambda: np.zeros(0))
    L: np.ndarray = field(default_factory=lambda: np.zeros(0, dtype=complex))  # boucle ouverte sur `freqs`
    valid: np.ndarray = field(default_factory=lambda: np.zeros(0, dtype=bool))

    @property
    def measured(self) -> bool:
        return self.method == MEASURED


def loop_from_T(freqs: np.ndarray, T: np.ndarray, coh: np.ndarray, f_max: float = 150.0,
                min_coh: float = 0.5) -> LoopMetrics:
    """Méthode approchée : bande passante, puis L = T/(1 − T) et ses marges, T = consigne → gyro."""
    lm = LoopMetrics(freqs=freqs, T=T, coherence=coh)
    valid = (freqs >= F_MIN) & (freqs <= f_max) & np.isfinite(T) & (coh >= min_coh)
    if valid.sum() < 5:
        return lm
    f = freqs[valid]
    Tv = _smooth_complex(T[valid], SMOOTH)
    mag = np.abs(Tv)
    below = np.nonzero(mag < 1 / np.sqrt(2))[0]
    if below.size:
        lm.bandwidth_hz = float(f[below[0]])
    with np.errstate(invalid="ignore", divide="ignore"):
        Lo = Tv / (1.0 - Tv)
    _fill(lm, margins(f, Lo))
    lm.tracking_peak_db = lm.sensitivity_peak_db  # ici S = 1 − T
    lm.band_hz = (float(f[0]), float(f[-1]))
    lm.L = np.full(freqs.shape, np.nan, dtype=complex)
    lm.L[valid] = Lo
    lm.valid = valid
    return lm


def _fill(lm: LoopMetrics, mg: Margins) -> None:
    lm.sensitivity_peak_db = mg.sensitivity_peak_db
    lm.sensitivity_peak_hz = mg.sensitivity_peak_hz
    lm.sensitivity_peak_at_edge = mg.sensitivity_peak_at_edge
    lm.crossover_hz = mg.crossover_hz
    lm.phase_margin_deg = mg.phase_margin_deg
    lm.gain_margin_db = mg.gain_margin_db


# Spectres croisés S(x, y) = Σ conj(X)·Y accumulés ; r consigne, y gyro, p = P + I, u = P + I + D + F,
# e = r − y, z instrument (consigne, ou commande radio en mode angle)
_PAIRS = ("rr", "yy", "ee", "uu", "pp", "dd", "zz", "ry", "rf", "zy", "zu", "zp", "zd", "ze", "ep", "yd")


@dataclass
class LoopSpectra:
    """Spectres croisés de la mesure, sommés par bloc de fenêtres consécutives (pour le jackknife)."""

    freqs: np.ndarray
    fs: float
    blocks: dict[str, np.ndarray]  # paire → (nb_blocs, nb_fréquences)
    n_windows: int
    has_d: bool

    def total(self, leave_out: int | None = None) -> dict[str, np.ndarray]:
        keep = [b for b in range(self.n_blocks) if b != leave_out]
        return {k: v[keep].sum(axis=0) for k, v in self.blocks.items()}

    @property
    def n_blocks(self) -> int:
        return next(iter(self.blocks.values())).shape[0] if self.blocks else 0


def loop_spectra(r: np.ndarray, y: np.ndarray, terms: PidTerms, fs: float, seg_s: float = 2.0,
                 min_input: float = 20.0, max_input: float | None = None, blocks: int = BLOCKS,
                 breaks: np.ndarray | None = None) -> LoopSpectra | None:
    """Spectres croisés sur les fenêtres où la consigne dépasse `min_input` (et reste sous `max_input`)."""
    r = np.asarray(r, dtype=np.float64)
    y = np.asarray(y, dtype=np.float64)
    pi = np.asarray(terms.p, dtype=np.float64) + np.asarray(terms.i, dtype=np.float64)
    d = np.asarray(terms.d, dtype=np.float64)
    f = np.asarray(terms.f, dtype=np.float64)
    z = r if terms.instrument is None else np.asarray(terms.instrument, dtype=np.float64)
    n = min(r.size, y.size, pi.size, d.size, f.size, z.size)
    L = int(round(seg_s * fs))
    if n < L or L < 8:
        return None
    starts = np.arange(0, n - L + 1, L // 2)
    starts = starts[clear_of_breaks(starts, L, breaks)]  # aucune fenêtre sur un trou du log (D-020)
    if starts.size < 2:
        return None
    idx = np.arange(L)
    peak = np.max(np.abs(r[starts[:, None] + idx]), axis=1)
    ok = peak >= min_input
    if max_input is not None:
        ok &= peak < max_input
    starts = starts[ok]
    if starts.size < 2:
        return None
    w = signal.windows.hann(L)
    freqs = np.fft.rfftfreq(L, 1.0 / fs)
    groups = np.array_split(starts, min(blocks, starts.size))
    out = {k: np.zeros((len(groups), freqs.size), dtype=complex) for k in _PAIRS}
    for b, st in enumerate(groups):
        sig = {}
        for name, x in (("r", r), ("y", y), ("p", pi), ("d", d), ("f", f), ("z", z)):
            seg = x[st[:, None] + idx]
            sig[name] = np.fft.rfft((seg - seg.mean(axis=1, keepdims=True)) * w, axis=1)
        sig["e"] = sig["r"] - sig["y"]
        sig["u"] = sig["p"] + sig["d"] + sig["f"]
        for k in _PAIRS:
            out[k][b] = np.sum(np.conj(sig[k[0]]) * sig[k[1]], axis=0)
    return LoopSpectra(freqs, fs, out, int(starts.size), bool(np.any(d)))


@dataclass
class LoopEstimate:
    """Fonctions de transfert estimées (toutes sur `freqs`) et masque des points valides."""

    freqs: np.ndarray
    Ce: np.ndarray
    Cd: np.ndarray
    Cf: np.ndarray
    G: np.ndarray
    T: np.ndarray
    L: np.ndarray
    coh: dict[str, np.ndarray]  # « zy », « zu » (instrument → gyro, → u), « ep », « yd »
    valid: np.ndarray
    S_a: np.ndarray  # contrôle, variante A : S = 1 − T·(Ce + Cd)/(Ce + Cf)
    L_c: np.ndarray  # contrôle, variante C : Ce et Cd estimés eux aussi par l'instrument
    err_g: np.ndarray  # erreur aléatoire relative de Ĝ


def estimate(spec: LoopSpectra, leave_out: int | None = None) -> LoopEstimate:
    f = spec.freqs
    df = f[1] - f[0] if f.size > 1 else 1.0
    widths = np.maximum(SMOOTH, 2 * np.round(SMOOTH_REL * f / df / 2) + 1).astype(int)
    s = {k: _smooth_variable(v, widths) for k, v in spec.total(leave_out).items()}
    with np.errstate(invalid="ignore", divide="ignore"):
        Ce = s["ep"] / s["ee"]
        Cd = -s["yd"] / s["yy"] if spec.has_d else np.zeros_like(Ce)
        G = s["zy"] / s["zu"]
        T = s["ry"] / s["rr"]
        Cf = s["rf"] / s["rr"]
        L = G * (Ce + Cd)
        coh = {"zy": _coh(s, "zy", "zz", "yy"), "zu": _coh(s, "zu", "zz", "uu"),
               "ep": _coh(s, "ep", "ee", "pp"), "yd": _coh(s, "yd", "yy", "dd")}
        S_a = 1.0 - T * (Ce + Cd) / (Ce + Cf)
        Cd_c = -s["zd"] / s["zy"] if spec.has_d else np.zeros_like(Ce)
        L_c = G * (s["zp"] / s["ze"] + Cd_c)
        n_avg = spec.n_windows * widths / 2.0
        err_g = np.sqrt(_rand_err(coh["zy"], n_avg) ** 2 + _rand_err(coh["zu"], n_avg) ** 2)
    ok = (f >= F_MIN) & (f <= min(F_MAX, 0.4 * spec.fs)) & np.isfinite(L) & (err_g <= MAX_ERR_G)
    ok &= coh["ep"] >= MIN_COH_PID
    if spec.has_d:
        ok &= coh["yd"] >= MIN_COH_PID
    return LoopEstimate(f, Ce, Cd, Cf, G, T, L, coh, _first_run(f, ok), S_a, L_c, err_g)


def _rand_err(coh: np.ndarray, n_avg) -> np.ndarray:
    """Erreur aléatoire relative du module d'une fonction de transfert estimée (cohérence γ², n moyennes)."""
    c = np.clip(np.nan_to_num(coh), 1e-12, 1.0)
    return np.sqrt((1.0 - c) / (2.0 * n_avg * c))


def _first_run(f: np.ndarray, ok: np.ndarray) -> np.ndarray:
    """Points valides contigus à partir du premier point valide (au-dessus de F_MIN)."""
    out = np.zeros_like(ok)
    idx = np.nonzero(ok)[0]
    if idx.size:
        i = idx[0]
        stop = np.nonzero(~ok[i:])[0]
        out[i:i + (stop[0] if stop.size else ok.size - i)] = True
    return out


def _smooth_variable(z: np.ndarray, widths: np.ndarray) -> np.ndarray:
    """Moyenne glissante de largeur variable (impaire, centrée), tronquée aux bords."""
    c = np.concatenate([[0], np.cumsum(z)])
    i = np.arange(z.size)
    lo = np.clip(i - widths // 2, 0, z.size)
    hi = np.clip(i + widths // 2 + 1, 0, z.size)
    return (c[hi] - c[lo]) / (hi - lo)


def _coh(s, xy, xx, yy):
    return np.abs(s[xy]) ** 2 / (np.real(s[xx]) * np.real(s[yy]))


_STD_KEYS = ("sensitivity_peak_db", "crossover_hz", "phase_margin_deg", "gain_margin_db")


def loop_from_terms(r: np.ndarray, y: np.ndarray, terms: PidTerms, fs: float, approx: LoopMetrics,
                    seg_s: float = 2.0, min_input: float = 20.0, max_input: float | None = None,
                    breaks: np.ndarray | None = None) -> LoopMetrics:
    """Boucle ouverte mesurée. `approx` (méthode approchée, même période) fournit la bande passante et le
    pic de suivi, et sert de repli, avec une note, si la mesure est impossible."""
    spec = loop_spectra(r, y, terms, fs, seg_s, min_input, max_input, breaks=breaks)
    if spec is None:
        approx.notes.append("pas assez de fenêtres pour mesurer la boucle ouverte")
        return approx
    est = estimate(spec)
    if est.valid.sum() < 5:
        approx.notes.append("cohérence insuffisante pour mesurer la boucle ouverte")
        return approx
    lm = LoopMetrics(method=MEASURED, bandwidth_hz=approx.bandwidth_hz, tracking_peak_db=approx.tracking_peak_db,
                     freqs=spec.freqs, T=est.T, coherence=est.coh["zy"], L=est.L, valid=est.valid)
    fv = spec.freqs[est.valid]
    lm.band_hz = (float(fv[0]), float(fv[-1]))
    _fill(lm, margins(spec.freqs, est.L, est.valid))
    if np.isfinite(lm.crossover_hz):
        k = int(np.argmin(np.abs(spec.freqs - lm.crossover_hz)))
        lm.d_ratio = float(np.abs(est.Cd[k] / est.Ce[k]))
    if spec.n_blocks >= 3:
        lm.std = _jackknife(spec, lm)
    return lm


def _jackknife(spec: LoopSpectra, lm: LoopMetrics) -> dict[str, float]:
    """Écart-type jackknife de chaque marge (un bloc de fenêtres retiré à chaque fois)."""
    vals = {k: [] for k in _STD_KEYS}
    for b in range(spec.n_blocks):
        est = estimate(spec, leave_out=b)
        mg = margins(spec.freqs, est.L, est.valid)
        for k in _STD_KEYS:
            vals[k].append(getattr(mg, k))
    out = {}
    nb = spec.n_blocks
    for k, v in vals.items():
        v = np.asarray(v, dtype=float)
        if np.isfinite(getattr(lm, k)) and np.all(np.isfinite(v)):
            out[k] = float(np.sqrt((nb - 1) / nb * np.sum((v - v.mean()) ** 2)))
        else:
            out[k] = float("nan")  # la mesure n'existe pas dans tous les sous-ensembles
    return out


def dterm_model(settings, freqs: np.ndarray, d_gain: float, throttle_frac: float = 0.5) -> np.ndarray:
    """Cd attendu d'après l'en-tête : Kd·dérivée discrète·filtres du D (cadence PID), pour recouper Ĉd.

    `d_gain` en unités de l'en-tête (D) ; la phase ne dépend que des filtres, le module du D effectif
    (D max, TPA) peut différer.
    """
    fp = settings.pid_rate_hz
    z1 = np.exp(-2j * np.pi * np.asarray(freqs, dtype=np.float64) / fp)
    return DTERM_SCALE * d_gain * fp * (1 - z1) * chain_response(dterm_chain(settings, throttle_frac), freqs)


def _smooth_complex(z: np.ndarray, width: int) -> np.ndarray:
    """Moyenne glissante sur parties réelle et imaginaire, bords prolongés (pas d'écrasement)."""
    if width <= 1 or z.size < width:
        return z
    return uniform_filter1d(z.real, width, mode="nearest") + 1j * uniform_filter1d(z.imag, width, mode="nearest")
