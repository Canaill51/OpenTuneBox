"""Fréquences moteur, harmoniques, notchs dynamiques et bruit moteur."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from ..log.flightlog import FlightLog
from .spectral import _seg_spectra, _segments, to_db


def motor_hz(log: FlightLog, multiplier: float | None = None) -> tuple[np.ndarray | None, bool]:
    """Fréquence de rotation de chaque moteur (Hz) et indicateur « estimée ».

    Avec la télémétrie DShot bidirectionnelle (eRPM), la valeur est mesurée. Sinon on l'estime :
    - si le KV moteur est connu (BF 2025.12+) : tr/min ≈ KV × tension × commande × 0,85 ;
    - sinon : Hz ≈ `multiplier` × % moteur (à caler par l'utilisateur, comme « RPM est. » de PTB).
    """
    if log.motor_hz is not None:
        return log.motor_hz, False
    if not log.n_motors:
        return None, True
    frac = np.clip(log.motors / 100.0, 0, 1)
    kv = log.settings.motor_kv
    if multiplier is None and kv > 0 and log.vbat is not None:
        rpm = kv * log.vbat[None, :] * frac * 0.85
        return rpm / 60.0, True
    m = 2.1 if multiplier is None else multiplier
    return m * log.motors * 4.0, True  # 2,1 × 4 ≈ 8,4 Hz par % (ordre de grandeur 5")


def dyn_notch_centres(log: FlightLog) -> np.ndarray | None:
    """Centres des notchs dynamiques (count, n) en Hz, si le log est en debug FFT_FREQ."""
    s = log.settings
    if s.debug_mode != "FFT_FREQ" or s.dyn_notch_count <= 0 or log.debug.shape[0] == 0:
        return None
    count = s.dyn_notch_count
    if s.family in ("2025.12", "2026.6"):
        rows = list(range(1, min(1 + count, log.debug.shape[0])))
    else:
        # 4.x : debug[3] est écrasé par le gyro avant notch si count ≥ 4
        rows = [p for p in range(min(count, log.debug.shape[0])) if p != 3]
    if not rows:
        return None
    return log.debug[rows].astype(np.float64)


def distribution(values: np.ndarray, freqs: np.ndarray) -> np.ndarray:
    """Fraction du temps où la fréquence est inférieure à chaque valeur de `freqs` (0 → 1).

    Sert aux courbes en « S » superposées aux spectres (où se situent les harmoniques pendant le vol).
    """
    v = np.sort(values[np.isfinite(values) & (values > 0)])
    if v.size == 0:
        return np.zeros_like(freqs, dtype=float)
    return np.searchsorted(v, freqs, side="right") / v.size


@dataclass
class MotorNoise:
    harmonics: np.ndarray  # 1, 2, 3
    mean_db: np.ndarray  # (n_harm,)
    std_db: np.ndarray


def motor_noise(x: np.ndarray, mhz: np.ndarray, fs: float, n_harm: int = 3, seg_ms: float = 300.0,
                min_hz: float = 20.0) -> MotorNoise:
    """Niveau de bruit (PSD, dB) aux fréquences des harmoniques moteur, fenêtre par fenêtre.

    Pour chaque fenêtre de 300 ms, on lit la PSD à h × fréquence moyenne des moteurs, h = 1…n.
    Le résultat (moyenne ± écart-type) montre l'efficacité du filtre RPM sur chaque harmonique
    lorsqu'on compare le gyro préfiltre et le gyro filtré.
    """
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, starts = _segments(np.asarray(x, dtype=np.float64), L, hop)
    harm = np.arange(1, n_harm + 1)
    if segs.shape[0] == 0:
        return MotorNoise(harm, np.full(n_harm, np.nan), np.full(n_harm, np.nan))
    f, P = _seg_spectra(segs, fs, "psd")
    P = to_db(P)
    mean_motor = mhz.mean(axis=0)
    mseg, _ = _segments(mean_motor, L, hop)
    fm = mseg.mean(axis=1)
    df = f[1] - f[0]
    means, stds = [], []
    for h in harm:
        fh = h * fm
        ok = (fh >= min_hz) & (fh < f[-1])
        idx = np.round(fh[ok] / df).astype(int)
        vals = P[np.nonzero(ok)[0], idx]
        means.append(np.mean(vals) if vals.size else np.nan)
        stds.append(np.std(vals) if vals.size else np.nan)
    return MotorNoise(harm, np.array(means), np.array(stds))


def hover_motor_hz(log: FlightLog) -> float | None:
    """Fréquence moteur typique en vol stationnaire (médiane quand le gaz est stable), pour
    reconnaître la taille du drone."""
    mhz, estimated = motor_hz(log)
    if mhz is None or estimated:
        return None
    m = mhz.mean(axis=0)
    flying = log.armed & (log.throttle > 5) & (m > 10)
    if flying.sum() < 100:
        return None
    return float(np.median(m[flying]))
