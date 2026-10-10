"""Fréquences moteur, harmoniques, notchs dynamiques et bruit moteur."""

from __future__ import annotations

import warnings
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


def harmonic_prominence(x: np.ndarray, mhz: np.ndarray, fs: float, n_harm: int = 3, seg_ms: float = 300.0,
                        min_hz: float = 20.0) -> np.ndarray:
    """Hauteur (dB) de chaque harmonique moteur au-dessus du bruit voisin, médiane des fenêtres de 300 ms.

    Raie = DSP moyenne entre h × (moteur le plus lent) et h × (moteur le plus rapide), ± une case ;
    bruit voisin = DSP moyenne de 15 à 30 % de part et d'autre de h × (fréquence moyenne). Sans raie,
    le résultat vaut environ 0 dB. NaN quand l'harmonique sort de la plage mesurable.
    """
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop)
    if segs.shape[0] == 0:
        return np.full(n_harm, np.nan)
    f, P = _seg_spectra(segs, fs, "psd")
    df = f[1] - f[0]
    lo_m = _segments(mhz.min(axis=0), L, hop)[0].mean(axis=1)
    hi_m = _segments(mhz.max(axis=0), L, hop)[0].mean(axis=1)
    mean_m = _segments(mhz.mean(axis=0), L, hop)[0].mean(axis=1)
    out = []
    for h in range(1, n_harm + 1):
        vals = []
        for k in range(P.shape[0]):
            fc = h * mean_m[k]
            if fc < min_hz or 1.3 * fc > 0.95 * fs / 2:
                continue
            line = (f >= h * lo_m[k] - df) & (f <= h * hi_m[k] + df)
            floor = ((f >= 0.7 * fc) & (f <= 0.85 * fc)) | ((f >= 1.15 * fc) & (f <= 1.3 * fc))
            if line.any() and floor.sum() >= 2:
                vals.append(to_db(P[k, line].mean()) - to_db(P[k, floor].mean()))
        out.append(float(np.median(vals)) if vals else np.nan)
    return np.array(out)


def rpm_weight_needed(prominence_db: float) -> float:
    """Poids RPM (0–1) juste suffisant pour ramener une raie au niveau du bruit voisin.

    Firmware (`rpmNotchUpdate`, `common/filter.c`) : le notch retranche poids × passe-bande, il reste
    (1 − poids) de la raie à sa fréquence centrale, soit 20·log10(1 − poids) dB.
    """
    if not np.isfinite(prominence_db) or prominence_db <= 0:
        return 0.0
    return float(1.0 - 10 ** (-prominence_db / 20.0))


@dataclass
class NotchTargets:
    freqs: np.ndarray  # fréquences des pics que le notch dynamique suivrait (Hz, du plus fort au plus faible)
    ratios_db: np.ndarray  # hauteur de chaque pic au-dessus du bruit moyen de la plage (dB)
    band: tuple[float, float]  # plage examinée (Hz)


def dyn_notch_targets(x: np.ndarray, mhz: np.ndarray | None, fs: float, band: tuple[float, float],
                      rpm_harmonics: int = 0, seg_ms: float = 300.0, bin_hz: float = 18.5) -> NotchTargets:
    """Pics que le notch dynamique de Betaflight suivrait dans `band`, d'après le gyro non filtré.

    Comme le firmware (`flight/dyn_notch_filter.c`) : spectre de puissance à la résolution de sa SDFT
    (≈ 18,5 Hz), bruit = puissance moyenne de la plage hors pics, pic suivi s'il dépasse 2 fois ce bruit
    (+3 dB). Le notch dynamique est placé après le filtre RPM : les bandes des harmoniques traitées par
    celui-ci (h × [moteur le plus lent ; le plus rapide] pendant la fenêtre ± 5 %, et leur image repliée quand le log est plus
    lent que la boucle) sont retirées fenêtre par fenêtre.
    """
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop)
    lo, hi = band[0], min(band[1], 0.95 * fs / 2)
    empty = NotchTargets(np.zeros(0), np.zeros(0), (lo, hi))
    if segs.shape[0] == 0 or hi <= lo:
        return empty
    f, P = _seg_spectra(segs, fs, "psd")
    if mhz is not None and rpm_harmonics > 0:
        lo_m = _segments(mhz.min(axis=0), L, hop)[0].min(axis=1)  # étendue parcourue pendant la fenêtre
        hi_m = _segments(mhz.max(axis=0), L, hop)[0].max(axis=1)
        P = P.copy()
        for k in range(P.shape[0]):
            for h in range(1, rpm_harmonics + 1):
                a, b = 0.95 * h * lo_m[k], 1.05 * h * hi_m[k]
                P[k, (f >= a) & (f <= b)] = np.nan
                P[k, (f >= fs - b) & (f <= fs - a)] = np.nan  # image repliée si le log est trop lent
    with np.errstate(all="ignore"), warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        S = np.nanmean(P, axis=0)
    # regroupement en cases de la largeur de celles de la SDFT
    edges = np.arange(lo, hi + bin_hz, bin_hz)
    centres, power = [], []
    for a, b in zip(edges[:-1], edges[1:]):
        v = S[(f >= a) & (f < b)]
        v = v[np.isfinite(v)]
        if v.size:
            centres.append(0.5 * (a + b))
            power.append(v.mean())
    c, p = np.array(centres), np.array(power)
    if p.size < 5:
        return empty
    peaks = [i for i in range(1, p.size - 1) if p[i] > p[i - 1] and p[i] >= p[i + 1]]
    # bruit moyen hors pics, au-dessus de 60 Hz comme le firmware (DYN_NOTCH_FLOOR_MIN_HZ : les mouvements
    # du drone sont plus bas), qui retire chaque pic et ses deux voisines ; les pics sont cherchés partout
    above = c >= 60
    if not above.any():
        above = np.ones(p.size, bool)
    floor = p[above].mean()
    for _ in range(3):
        keep = above.copy()
        for i in peaks:
            if p[i] > 2 * floor:
                keep[max(0, i - 1):i + 2] = False
        floor = p[keep].mean() if keep.any() else floor
    kept = sorted((i for i in peaks if p[i] > 2 * floor), key=lambda i: -p[i])
    return NotchTargets(c[kept], 10 * np.log10(p[kept] / floor), (lo, hi))


@dataclass
class SpectrumLine:
    freq_hz: float
    raw_db: float  # hauteur au-dessus du bruit voisin avant filtres (dB)
    left_db: float  # même chose après filtres (NaN si non mesurable)


def _masked_spectrum(x: np.ndarray, mhz: np.ndarray, fs: float, n_harm: int, seg_ms: float):
    """DSP moyenne (linéaire) des fenêtres de 300 ms, bandes des harmoniques moteur 1…n retirées
    (h × [moteur le plus lent ; le plus rapide] pendant la fenêtre, ± 5 % de l'harmonique pour les trois
    premières et de la fréquence moteur ensuite, et leur
    image repliée : sur un whoop loggé à 4 kHz, la 8e harmonique retombe vers 200 Hz)."""
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop)
    if segs.shape[0] == 0:
        return np.zeros(0), np.zeros(0)
    f, P = _seg_spectra(segs, fs, "psd")
    lo_m = _segments(mhz.min(axis=0), L, hop)[0].min(axis=1)
    hi_m = _segments(mhz.max(axis=0), L, hop)[0].max(axis=1)
    P = P.copy()
    for k in range(P.shape[0]):
        for h in range(1, n_harm + 1):
            # marge : 5 % de l'harmonique pour les trois premières (raies fortes, flancs larges), 5 % de la
            # fréquence moteur ensuite (raies faibles ; 5 % de la 8e, ±80 Hz, masquerait tout le spectre)
            m = 0.05 * (h if h <= 3 else 1)
            a, b = h * lo_m[k] - m * lo_m[k], h * hi_m[k] + m * hi_m[k]
            P[k, (f >= a) & (f <= b)] = np.nan
            for j in range(1, int(b // fs) + 2):  # images repliées |h·f − j·fs| (log plus lent que la boucle)
                P[k, (f >= j * fs - b) & (f <= j * fs - a)] = np.nan
                P[k, (f >= a - j * fs) & (f <= b - j * fs)] = np.nan
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        return f, np.nanmean(P, axis=0)


def _line_height(f: np.ndarray, S: np.ndarray, f0: float) -> float:
    """Hauteur (dB) de la DSP à f0 (± une case) au-dessus du bruit voisin (10 à 20 % de part et d'autre)."""
    df = f[1] - f[0]
    line = S[(f >= f0 - df) & (f <= f0 + df)]
    floor = S[((f >= 0.8 * f0) & (f <= 0.9 * f0)) | ((f >= 1.1 * f0) & (f <= 1.2 * f0))]
    line, floor = line[np.isfinite(line)], floor[np.isfinite(floor)]
    if line.size == 0 or floor.size < 2 or floor.mean() <= 0 or line.mean() <= 0:
        return float("nan")
    return float(10 * np.log10(line.mean() / floor.mean()))


def _half_power_width(f: np.ndarray, S: np.ndarray, f0: float) -> float:
    """Largeur (Hz) d'une raie à mi-puissance de son excès sur le bruit voisin (10 à 20 % de part et d'autre),
    interpolée entre cases ; NaN si la raie ne dépasse pas le bruit ou touche une bande retirée."""
    floor = S[((f >= 0.8 * f0) & (f <= 0.9 * f0)) | ((f >= 1.1 * f0) & (f <= 1.2 * f0))]
    floor = floor[np.isfinite(floor)]
    near = np.flatnonzero(np.abs(f - f0) <= 10.0)  # sommet le plus proche (pic trouvé à 18,5 Hz près)
    if floor.size < 2 or near.size == 0 or not np.any(np.isfinite(S[near])):
        return float("nan")
    k = int(near[np.nanargmax(S[near])])
    level = floor.mean() + 0.5 * (S[k] - floor.mean())
    if not S[k] > floor.mean():
        return float("nan")
    edges = []
    for step in (-1, 1):
        j = k
        while 0 <= j + step < f.size and np.isfinite(S[j + step]) and S[j + step] > level:
            j += step
        n = j + step
        if not (0 <= n < f.size and np.isfinite(S[n])):
            return float("nan")
        edges.append(f[j] + (f[n] - f[j]) * (S[j] - level) / (S[j] - S[n]))
    return float(edges[1] - edges[0])


def line_shapes(x_raw: np.ndarray, mhz: np.ndarray, fs: float, freqs, n_harm: int,
                seg_ms: float = 300.0) -> list[tuple[float, float]]:
    """(largeur à mi-puissance en Hz, hauteur au-dessus du bruit voisin en dB) des raies `freqs` du gyro non
    filtré, bandes des harmoniques moteur traitées par le filtre RPM retirées (comme pour le notch
    dynamique, placé après lui)."""
    f, S = _masked_spectrum(x_raw, mhz, fs, max(n_harm, 1), seg_ms)
    if f.size < 8:
        return [(float("nan"), float("nan"))] * len(freqs)
    return [(_half_power_width(f, S, float(f0)), _line_height(f, S, float(f0))) for f0 in freqs]


def spectrum_lines(x_raw: np.ndarray, x_filt: np.ndarray, mhz: np.ndarray, fs: float, band: tuple[float, float],
                   min_db: float = 6.0, n_harm: int = 8, seg_ms: float = 300.0) -> list[SpectrumLine]:
    """Raies qui ne viennent pas des moteurs (résonances, bruit résiduel), avant et après filtres.

    Pensé pour le vol stationnaire : régime stable, raies moteur connues par l'eRPM et retirées jusqu'à
    la 8e harmonique ; toute raie restante dépassant de `min_db` le bruit voisin est listée, la plus haute
    d'abord (une seule par groupe de ±10 %)."""
    f, S = _masked_spectrum(x_raw, mhz, fs, n_harm, seg_ms)
    if f.size < 8:
        return []
    _, Sf = _masked_spectrum(x_filt, mhz, fs, n_harm, seg_ms)
    lo, hi = band[0], min(band[1], 0.9 * fs / 2 / 1.2)
    cands = []
    for i in range(1, f.size - 1):
        if lo <= f[i] <= hi and np.isfinite(S[i]) and S[i] >= np.nanmax(S[i - 1:i + 2]):
            h = _line_height(f, S, f[i])
            if np.isfinite(h) and h >= min_db:
                cands.append((h, float(f[i])))
    out: list[SpectrumLine] = []
    for h, f0 in sorted(cands, reverse=True):
        if all(abs(f0 - o.freq_hz) > 0.1 * o.freq_hz for o in out):
            out.append(SpectrumLine(f0, h, _line_height(f, Sf, f0)))
    return out


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
