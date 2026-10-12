"""Réponse indicielle par déconvolution de Wiener, et métriques de la boucle de régulation.

Méthode (docs/spec-methodes.md, d'après la description publique de PIDtoolbox) :
1. fenêtres de 2 s (réglable), recouvrement selon la durée analysée ;
   pas de 62,5 ms (fenêtre ÷ 32) : même nombre de segments que PIDtoolbox Pro V0.94 ;
2. on garde les fenêtres où la consigne dépasse 20 °/s ;
3. fenêtre de Hann + 100 zéros de chaque côté ;
4. réponse impulsionnelle h = IFFT( G·H* / (|H|² + λ) ), G = FFT(gyro)/N, H = FFT(consigne)/N ;
5. réponse indicielle = somme cumulée de h, sur 500 ms ;
6. contrôle qualité : régime établi (200–500 ms) entre 0,5 et 3.

Forme de la montée (`shape_metrics`, méthode de Brian White pour le Master) : la cassure (« ripple ») est
l'arrêt de la montée avant la consigne, suivi d'une remontée lente ; les vagues sont l'oscillation de la
courbe sur son palier. Incertitudes par jackknife sur des blocs de fenêtres consécutives (les fenêtres se
chevauchent : une fenêtre seule n'est pas une mesure indépendante).

Les mêmes spectres croisés donnent la fonction de transfert en boucle fermée T(f) = consigne → gyro,
d'où la bande passante (−3 dB) et le pic de suivi max |1 − T|. La boucle ouverte et ses marges viennent
de `loop.py` : mesurée à partir des termes P, I, D, F loggés (`terms`), sinon approchée par T/(1 − T).
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy import signal
from scipy.ndimage import uniform_filter1d

from .loop import LoopMetrics, PidTerms, loop_from_T, loop_from_terms
from .spectral import clear_of_breaks


PAD = 100
STALL_SLOPE = 0.1  # la montée s'arrête quand la pente retombe sous 10 % de son maximum
STALL_SMOOTH_MS = 2.5  # lissage de la courbe avant d'en prendre la pente
WAVES_TREND_MS = 15.0  # tendance retirée avant de mesurer les vagues
WAVES_END_MS = 250.0
JACKKNIFE_BLOCKS = 10


@dataclass
class StepMetrics:
    peak: float = float("nan")  # valeur maximale (1 = pas de dépassement)
    peak_ms: float = float("nan")
    latency_ms: float = float("nan")  # temps pour atteindre 50 %
    rise_ms: float = float("nan")  # 10 % → 90 %
    settling_ms: float = float("nan")  # entrée définitive dans ±5 %
    overshoot_pct: float = float("nan")
    ripple: float = float("nan")  # cassure : ce qui manque pour atteindre min(palier, 1) quand la montée s'arrête
    ripple_std: float = float("nan")  # écart-type (jackknife par blocs)
    ripple_ms: float = float("nan")  # instant de l'arrêt de la montée
    waves: float = float("nan")  # écart quadratique moyen de la courbe autour de sa tendance, sur le palier
    waves_ratio: float = float("nan")  # vagues ÷ ce que le bruit de mesure seul donnerait (≈ 1 : bruit)


@dataclass
class StepResult:
    t_ms: np.ndarray
    responses: np.ndarray  # (k, L) une réponse par fenêtre retenue
    mean: np.ndarray
    std: np.ndarray
    n: int
    metrics: StepMetrics
    loop: LoopMetrics
    max_inputs: np.ndarray  # consigne max de chaque fenêtre retenue (°/s)

    @property
    def ok(self) -> bool:
        return self.n > 0


def step_response(
    sp: np.ndarray,
    gy: np.ndarray,
    fs: float,
    seg_s: float = 2.0,
    min_input: float = 20.0,
    max_input: float | None = None,
    resp_ms: float = 500.0,
    hop_ms: float = 62.5,
    reg: float = 1e-4,
    smooth: int = 0,
    y_correction: bool = False,
    terms: PidTerms | None = None,
    breaks: np.ndarray | None = None,
) -> StepResult:
    """Réponse indicielle d'un axe. `max_input` permet de séparer petits / grands mouvements
    (ex. < 500 °/s et ≥ 500 °/s comme PIDtoolbox et PID-Analyzer). `terms` (P, I, D, F loggés sur la
    même période) : boucle ouverte mesurée ; sans eux, méthode approchée. `breaks` : raccords du log
    (`FlightLog.breaks`), aucune fenêtre ne les chevauche (D-020)."""
    sp = np.asarray(sp, dtype=np.float64)
    gy = np.asarray(gy, dtype=np.float64)
    sp_raw, gy_raw = sp, gy
    if smooth > 1:
        gy = np.convolve(gy, np.ones(smooth) / smooth, mode="same")
    n = min(sp.size, gy.size)
    L = int(round(seg_s * fs))
    Lr = int(round(resp_ms * fs / 1000.0)) + 1
    t_ms = np.arange(Lr) * 1000.0 / fs
    empty = StepResult(t_ms, np.zeros((0, Lr)), np.full(Lr, np.nan), np.full(Lr, np.nan), 0,
                       StepMetrics(), LoopMetrics(), np.zeros(0))
    if n < L:
        return empty
    starts = _window_starts(sp[:n], fs, L, hop_ms, min_input, max_input, breaks)
    w = signal.windows.hann(L)
    nfft = L + 2 * PAD

    # toutes les fenêtres d'un coup (par paquets) : FFT vectorisées
    resp, maxin = [], []
    sum_gh = np.zeros(nfft // 2 + 1, dtype=complex)
    sum_hh = np.zeros(nfft // 2 + 1)
    sum_gg = np.zeros(nfft // 2 + 1)
    steady = (t_ms >= 200) & (t_ms <= 500)
    idx = np.arange(L)
    for k in range(0, starts.size, 256):
        st0 = starts[k : k + 256]
        X = sp[st0[:, None] + idx] * w
        Y = gy[st0[:, None] + idx] * w
        X = np.pad(X, ((0, 0), (PAD, PAD)))
        Y = np.pad(Y, ((0, 0), (PAD, PAD)))
        H = np.fft.fft(X, axis=1) / nfft
        G = np.fft.fft(Y, axis=1) / nfft
        imp = np.real(np.fft.ifft(G * np.conj(H) / (H * np.conj(H) + reg), axis=1))
        st = np.cumsum(imp, axis=1)[:, :Lr]
        ss = st[:, steady]
        good = (ss.min(axis=1) > 0.5) & (ss.max(axis=1) < 3.0) if ss.size else np.ones(len(st0), bool)
        if not good.any():
            continue
        st = st[good]
        if y_correction and ss.size:
            st = st * (2.0 - ss[good].mean(axis=1, keepdims=True))  # « Y corr. » de PIDtoolbox
        resp.append(st)
        maxin.append(np.max(np.abs(sp[st0[good][:, None] + idx]), axis=1))
        Hr = np.fft.rfft(X[good], axis=1)
        Gr = np.fft.rfft(Y[good], axis=1)
        sum_gh += np.sum(Gr * np.conj(Hr), axis=0)
        sum_hh += np.sum(np.abs(Hr) ** 2, axis=0)
        sum_gg += np.sum(np.abs(Gr) ** 2, axis=0)

    if not resp:
        return empty
    R = np.vstack(resp)
    maxin = np.concatenate(maxin)
    mean = R.mean(axis=0)
    std = R.std(axis=0)
    freqs = np.fft.rfftfreq(nfft, 1.0 / fs)
    with np.errstate(invalid="ignore", divide="ignore"):
        T = sum_gh / sum_hh
        coh = np.abs(sum_gh) ** 2 / (sum_gg * sum_hh)
    loop = loop_from_T(freqs, T, coh)
    if terms is None:
        loop.notes.append("termes PID non fournis")
    elif terms.note:
        loop.notes.append(terms.note)
    else:
        loop = loop_from_terms(sp_raw, gy_raw, terms, fs, loop, seg_s, min_input, max_input, breaks)
    metrics = step_metrics(t_ms, mean)
    shape_metrics(t_ms, R, metrics)
    return StepResult(t_ms, R, mean, std, R.shape[0], metrics, loop, maxin)


def _window_starts(sp: np.ndarray, fs: float, L: int, hop_ms: float, min_input: float,
                   max_input: float | None = None, breaks: np.ndarray | None = None) -> np.ndarray:
    """Débuts des fenêtres de L échantillons (pas `hop_ms`) où |consigne| atteint `min_input` (et reste
    sous `max_input`)."""
    if sp.size < L:
        return np.zeros(0, dtype=int)
    hop = max(1, int(round(hop_ms * fs / 1000.0)))
    starts = np.arange(0, sp.size - L + 1, hop)
    starts = starts[clear_of_breaks(starts, L, breaks)]
    if starts.size == 0:
        return starts
    peak = np.lib.stride_tricks.sliding_window_view(np.abs(sp), L)[starts].max(axis=1)
    keep = peak >= min_input
    if max_input is not None:
        keep &= peak < max_input
    return starts[keep]


def input_windows(sp: np.ndarray, fs: float, seg_s: float = 2.0, min_input: float = 20.0,
                  hop_ms: float = 62.5, breaks: np.ndarray | None = None) -> int:
    """Nombre de fenêtres où la réponse indicielle a assez de consigne : calcul rapide, sans la déconvolution
    (le contrôle qualité de `step_response` en écarte ensuite quelques-unes)."""
    return int(_window_starts(np.asarray(sp, dtype=np.float64), fs, int(round(seg_s * fs)), hop_ms, min_input,
                                  breaks=breaks).size)


def step_metrics(t_ms: np.ndarray, r: np.ndarray) -> StepMetrics:
    m = StepMetrics()
    if r.size == 0 or not np.all(np.isfinite(r)):
        return m
    m.latency_ms = _crossing(t_ms, r, 0.5)
    # pic cherché dans la montée (150 ms comme PIDtoolbox, plus long pour les drones lents),
    # pour ne pas confondre dépassement et lente dérive du terme I
    horizon = max(150.0, 3.0 * m.latency_ms) if np.isfinite(m.latency_ms) else t_ms[-1]
    w = t_ms <= horizon
    i = int(np.argmax(r[w]))
    m.peak = float(r[i])
    m.peak_ms = float(t_ms[i])
    m.overshoot_pct = max(0.0, (m.peak - 1.0) * 100.0)
    t10, t90 = _crossing(t_ms, r, 0.1), _crossing(t_ms, r, 0.9)
    m.rise_ms = t90 - t10
    out = np.abs(r - 1.0) > 0.05
    if not out.any():
        m.settling_ms = 0.0
    elif not out[-1]:
        m.settling_ms = float(t_ms[np.nonzero(out)[0][-1] + 1])
    return m


def _smooth(x: np.ndarray, n: int) -> np.ndarray:
    return uniform_filter1d(x, max(1, n), axis=-1, mode="nearest")


def _ripple(t_ms: np.ndarray, r: np.ndarray, latency_ms: float) -> tuple[float, float]:
    """(cassure, instant de l'arrêt de la montée) d'une réponse moyenne."""
    dt = t_ms[1] - t_ms[0]
    rs = _smooth(r, int(round(STALL_SMOOTH_MS / dt)))
    d = np.gradient(rs, t_ms)
    rise = t_ms <= max(100.0, 3.0 * latency_ms)
    k0 = int(np.argmax(d[rise]))
    below = np.nonzero(d[k0:] < STALL_SLOPE * d[k0])[0]
    if below.size == 0:
        return float("nan"), float("nan")
    k = k0 + int(below[0])
    flat = (t_ms >= max(50.0, 3.0 * latency_ms)) & (t_ms <= 200.0)
    if not flat.any():
        return float("nan"), float("nan")
    plateau = float(np.median(r[flat]))
    return min(plateau, 1.0) - float(rs[k]), float(t_ms[k])


def _detrended(t_ms: np.ndarray, r: np.ndarray, w: np.ndarray) -> np.ndarray:
    dt = t_ms[1] - t_ms[0]
    return (r - _smooth(r, int(round(WAVES_TREND_MS / dt))))[..., w]


def shape_metrics(t_ms: np.ndarray, R: np.ndarray, m: StepMetrics) -> None:
    """Cassure et vagues de la réponse moyenne de `R` (une réponse par fenêtre, dans l'ordre du vol),
    avec leur incertitude ; complète `m` (latence déjà calculée)."""
    if R.shape[0] == 0 or t_ms.size < 3 or not np.isfinite(m.latency_ms):
        return
    mean = R.mean(axis=0)
    m.ripple, m.ripple_ms = _ripple(t_ms, mean, m.latency_ms)
    w = (t_ms >= 3.0 * m.latency_ms) & (t_ms <= WAVES_END_MS)
    if not w.any():
        return
    m.waves = float(np.sqrt(np.mean(_detrended(t_ms, mean, w) ** 2)))
    k = min(JACKKNIFE_BLOCKS, R.shape[0] // 4)
    if k < 4:
        return
    blocks = np.array_split(np.arange(R.shape[0]), k)
    sums = np.array([R[b].sum(axis=0) for b in blocks])
    sizes = np.array([b.size for b in blocks])
    loo = (sums.sum(axis=0) - sums) / (R.shape[0] - sizes)[:, None]  # moyennes sans chaque bloc
    rip = np.array([_ripple(t_ms, x, m.latency_ms)[0] for x in loo])
    if np.all(np.isfinite(rip)):
        m.ripple_std = float(np.sqrt((k - 1) / k * np.sum((rip - rip.mean()) ** 2)))
    det = _detrended(t_ms, loo, w)
    noise = np.sqrt((k - 1) / k * np.sum((det - det.mean(axis=0)) ** 2, axis=0))
    floor = float(np.sqrt(np.mean(noise ** 2)))
    if floor > 0:
        m.waves_ratio = m.waves / floor


def _crossing(t: np.ndarray, r: np.ndarray, level: float) -> float:
    """Premier instant (interpolé) où r atteint `level`."""
    above = np.nonzero(r >= level)[0]
    if above.size == 0:
        return float("nan")
    k = int(above[0])
    if k == 0:
        return float(t[0])
    r0, r1 = r[k - 1], r[k]
    frac = (level - r0) / (r1 - r0) if r1 != r0 else 0.0
    return float(t[k - 1] + frac * (t[k] - t[k - 1]))
