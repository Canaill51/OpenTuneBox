"""Réponse indicielle par déconvolution de Wiener, et métriques de la boucle de régulation.

Méthode (docs/spec-methodes.md, d'après la description publique de PIDtoolbox) :
1. fenêtres de 2 s (réglable), recouvrement selon la durée analysée ;
   pas de 62,5 ms (fenêtre ÷ 32) : même nombre de segments que PIDtoolbox Pro V0.94 ;
2. on garde les fenêtres où la consigne dépasse 20 °/s ;
3. fenêtre de Hann + 100 zéros de chaque côté ;
4. réponse impulsionnelle h = IFFT( G·H* / (|H|² + λ) ), G = FFT(gyro)/N, H = FFT(consigne)/N ;
5. réponse indicielle = somme cumulée de h, sur 500 ms ;
6. contrôle qualité : régime établi (200–500 ms) entre 0,5 et 3.

Les mêmes spectres croisés donnent la fonction de transfert en boucle fermée T(f) = consigne → gyro,
d'où : bande passante (−3 dB), sensibilité S = 1 − T (pic en dB), boucle ouverte L = T/(1 − T)
(fréquence de croisement |L| = 1 et marge de phase).
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
from scipy import signal


PAD = 100


@dataclass
class StepMetrics:
    peak: float = float("nan")  # valeur maximale (1 = pas de dépassement)
    peak_ms: float = float("nan")
    latency_ms: float = float("nan")  # temps pour atteindre 50 %
    rise_ms: float = float("nan")  # 10 % → 90 %
    settling_ms: float = float("nan")  # entrée définitive dans ±5 %
    overshoot_pct: float = float("nan")


@dataclass
class LoopMetrics:
    bandwidth_hz: float = float("nan")  # |T| = −3 dB
    sensitivity_peak_db: float = float("nan")  # max |1 − T| en dB
    sensitivity_peak_hz: float = float("nan")
    crossover_hz: float = float("nan")  # |L| = 1
    phase_margin_deg: float = float("nan")
    freqs: np.ndarray = field(default_factory=lambda: np.zeros(0))
    T: np.ndarray = field(default_factory=lambda: np.zeros(0, dtype=complex))
    coherence: np.ndarray = field(default_factory=lambda: np.zeros(0))


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
) -> StepResult:
    """Réponse indicielle d'un axe. `max_input` permet de séparer petits / grands mouvements
    (ex. < 500 °/s et ≥ 500 °/s comme PIDtoolbox et PID-Analyzer)."""
    sp = np.asarray(sp, dtype=np.float64)
    gy = np.asarray(gy, dtype=np.float64)
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
    hop = max(1, int(round(hop_ms * fs / 1000.0)))
    starts = np.arange(0, n - L + 1, hop)
    w = signal.windows.hann(L)
    nfft = L + 2 * PAD

    # toutes les fenêtres d'un coup (par paquets) : FFT vectorisées
    starts = starts[[_seg_max(sp, s0, L) >= min_input for s0 in starts]] if starts.size else starts
    if max_input is not None and starts.size:
        starts = starts[[_seg_max(sp, s0, L) < max_input for s0 in starts]]
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
    return StepResult(t_ms, R, mean, std, R.shape[0], step_metrics(t_ms, mean),
                      loop_metrics(freqs, T, coh), maxin)


def _seg_max(x: np.ndarray, s0: int, L: int) -> float:
    return float(np.max(np.abs(x[s0 : s0 + L])))


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


def loop_metrics(freqs: np.ndarray, T: np.ndarray, coh: np.ndarray, f_max: float = 150.0,
                 min_coh: float = 0.5) -> LoopMetrics:
    """Métriques fréquentielles de la boucle à partir de T(f) (consigne → gyro)."""
    lm = LoopMetrics(freqs=freqs, T=T, coherence=coh)
    valid = (freqs >= 2.0) & (freqs <= f_max) & np.isfinite(T) & (coh >= min_coh)
    if valid.sum() < 5:
        return lm
    f = freqs[valid]
    Tv = _smooth_complex(T[valid], 5)
    mag = np.abs(Tv)
    below = np.nonzero(mag < 1 / np.sqrt(2))[0]
    if below.size:
        lm.bandwidth_hz = float(f[below[0]])
    S = np.abs(1.0 - Tv)
    k = int(np.argmax(S))
    lm.sensitivity_peak_db = float(20 * np.log10(S[k]))
    lm.sensitivity_peak_hz = float(f[k])
    with np.errstate(invalid="ignore", divide="ignore"):
        Lo = Tv / (1.0 - Tv)
    lmag = np.abs(Lo)
    cross = np.nonzero((lmag[:-1] >= 1.0) & (lmag[1:] < 1.0))[0]
    if cross.size:
        c = int(cross[0])
        lm.crossover_hz = float(f[c])
        lm.phase_margin_deg = float(180.0 + np.degrees(np.angle(Lo[c])))
    return lm


def _smooth_complex(z: np.ndarray, width: int) -> np.ndarray:
    """Moyenne glissante sur parties réelle et imaginaire, bords prolongés (pas d'écrasement)."""
    if width <= 1 or z.size < width:
        return z
    from scipy.ndimage import uniform_filter1d

    return uniform_filter1d(z.real, width, mode="nearest") + 1j * uniform_filter1d(z.imag, width, mode="nearest")
