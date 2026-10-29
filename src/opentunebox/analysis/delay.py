"""Retards par intercorrélation : filtres gyro et D, consigne → gyro.

Méthode PIDtoolbox : signaux lissés (moyenne glissante), retard = maximum de l'intercorrélation
dans ±30 ms. On ajoute une interpolation parabolique pour une précision sous l'échantillon.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from . import _numeric

from ..log.flightlog import FlightLog
from .spectral import _segments


def xcorr_delay_ms(x: np.ndarray, y: np.ndarray, fs: float, max_ms: float = 30.0,
                   smooth: int = 0) -> float:
    """Retard (ms) de `y` par rapport à `x` (positif si y est en retard)."""
    x = np.asarray(x, dtype=np.float64)
    y = np.asarray(y, dtype=np.float64)
    n = min(x.size, y.size)
    if n < 32:
        return float("nan")
    x, y = x[:n], y[:n]
    if smooth > 1:
        k = np.ones(smooth) / smooth
        x = np.convolve(x, k, mode="same")
        y = np.convolve(y, k, mode="same")
    x = x - x.mean()
    y = y - y.mean()
    if not np.any(x) or not np.any(y):
        return float("nan")
    max_lag = max(1, int(round(max_ms * fs / 1000.0)))
    c, lags = _numeric.xcorr_full(y, x)
    m = (lags >= -max_lag) & (lags <= max_lag)
    c, lags = c[m], lags[m]
    i = int(np.argmax(c))
    shift = float(lags[i])
    if 0 < i < c.size - 1:
        a, b, d = c[i - 1], c[i], c[i + 1]
        den = a - 2 * b + d
        if den != 0:
            shift += 0.5 * (a - d) / den
    return shift * 1000.0 / fs


def phase_delay_ms(x: np.ndarray, y: np.ndarray, fs: float, band: tuple[float, float] = (10.0, 100.0),
                   min_coherence: float = 0.6, breaks: np.ndarray | None = None) -> float:
    """Retard de `y` par rapport à `x` déduit de la phase de la fonction de transfert.

    H(f) = Sxy/Sxx (Welch, fenêtres de 0,5 s) ; retard(f) = −phase/(2πf) ; moyenne pondérée par la
    cohérence dans la bande utile au pilotage. Contrairement à l'intercorrélation, le résultat ne
    dépend pas du bruit haute fréquence qui domine souvent les signaux en vol stationnaire.
    Mêmes fenêtres que `scipy.signal.csd` (Hann, recouvrement de moitié, moyenne retirée), sans celles
    qui chevauchent un raccord `breaks` du log (D-020).
    """
    x = np.asarray(x, dtype=np.float64)
    y = np.asarray(y, dtype=np.float64)
    n = min(x.size, y.size)
    nper = int(min(n, 2 ** int(np.ceil(np.log2(fs * 0.5)))))
    if nper < 64:
        return float("nan")
    sx, _ = _segments(x[:n], nper, nper // 2, breaks)
    sy, _ = _segments(y[:n], nper, nper // 2, breaks)
    if sx.shape[0] == 0:
        return float("nan")
    w = _numeric.hann(nper, sym=False)
    X = np.fft.rfft((sx - sx.mean(axis=1, keepdims=True)) * w, axis=1)
    Y = np.fft.rfft((sy - sy.mean(axis=1, keepdims=True)) * w, axis=1)
    f = np.fft.rfftfreq(nper, 1.0 / fs)
    sxy = np.mean(np.conj(X) * Y, axis=0)  # échelle commune sans effet : seuls des rapports servent
    sxx = np.mean(np.abs(X) ** 2, axis=0)
    syy = np.mean(np.abs(Y) ** 2, axis=0)
    with np.errstate(divide="ignore", invalid="ignore"):
        coh = np.where(sxx * syy > 0, np.abs(sxy) ** 2 / (sxx * syy), 0.0)
    m = (f >= band[0]) & (f <= band[1]) & (coh >= min_coherence) & (sxx > 0)
    if m.sum() < 3:
        return float("nan")
    h = sxy[m] / sxx[m]
    ph = np.unwrap(np.angle(h))
    d = -ph / (2 * np.pi * f[m])
    return float(np.average(d, weights=coh[m]) * 1000.0)


# Le retard des filtres n'est jugé que si le vol contient des mouvements : en vol stationnaire, les filtres
# dynamiques (LPF dynamiques, notchs RPM et dynamiques) ne travaillent qu'à un régime. Critère : la consigne
# dépasse 30 °/s au moins 5 % du temps en roll ou en pitch (D-026) ; les vols stationnaires du corpus restent
# sous 25 °/s. ⚠️ Pas 50 °/s : les wobbles en mode Angle limité à 30° (protocole du guide) plafonnent vers
# 47 °/s et seraient pris pour des vols stationnaires (L-007).
MOVING_SETPOINT_DPS = 30.0


def stick_p95_dps(log, sel: slice) -> float:
    """95e centile de |consigne| (°/s), maximum du roll et du pitch : ampleur des mouvements de manche."""
    return float(max(np.percentile(np.abs(log.setpoint[a, sel]), 95) for a in range(2)))


@dataclass
class Delays:
    gyro_ms: float  # filtres gyro (gyro non filtré → gyro filtré)
    dterm_ms: float  # filtres du D (D préfiltre → D)
    sp_gyro_ms: list[float]  # consigne → gyro, par axe (latence de la boucle)
    stick_p95_dps: float = float("inf")  # ampleur des mouvements de manche (stick_p95_dps)

    @property
    def moving(self) -> bool:
        """Assez de mouvements pour que les retards des filtres soient représentatifs."""
        return self.stick_p95_dps >= MOVING_SETPOINT_DPS


def compute_delays(log: FlightLog, sel: slice | None = None) -> Delays:
    """Retards des filtres (méthode de phase, roll + pitch) et latence consigne → gyro (intercorrélation)."""
    sel = sel or slice(None)
    fs = log.fs
    gyro_ms = float("nan")
    br = log.breaks(sel)
    if log.gyro_unfilt is not None:
        vals = [phase_delay_ms(log.gyro_unfilt[a, sel], log.gyro[a, sel], fs, breaks=br) for a in range(2)]
        gyro_ms = float(np.nanmean(vals)) if np.any(np.isfinite(vals)) else float("nan")
    dterm_ms = float("nan")
    if np.any(log.d_term[0]):
        vals = [phase_delay_ms(log.d_term_prefilter[a, sel], log.d_term[a, sel], fs, breaks=br) for a in range(2)]
        dterm_ms = float(np.nanmean(vals)) if np.any(np.isfinite(vals)) else float("nan")
    sp = []
    for a in range(3):
        x = log.setpoint[a, sel]
        if np.std(x) < 5.0:  # pas assez de mouvements de manche sur cet axe
            sp.append(float("nan"))
        else:
            sp.append(xcorr_delay_ms(x, log.gyro[a, sel], fs, 100.0))
    return Delays(gyro_ms, dterm_ms, sp, stick_p95_dps(log, sel))
