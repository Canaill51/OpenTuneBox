"""Analyses spectrales : spectre 2D, fréquence × gaz, fréquence × temps.

Méthodes (docs/spec-methodes.md) :
- spectre 2D : moyenne de Welch (fenêtres de Hann, recouvrement 50 %), en densité spectrale de
  puissance (dB) ou en amplitude ; lissage optionnel en fréquence ;
- fréquence × gaz (méthode PIDtoolbox) : fenêtres de 300 ms, gaz moyen par fenêtre, 100 tranches
  de gaz de 1 % avec recouvrement ±1 %, spectres moyennés par tranche (résolution 3,33 Hz) ;
- fréquence × temps : spectrogramme en fenêtres de 300 ms centrées, pas réglable.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy import ndimage, signal

EPS = 1e-12

# Nos DSP en dB (densité physique, identique à scipy.signal.welch) sont plus hautes que celles de PIDtoolbox
# de 6,5 dB, quelle que soit la fréquence (5 sessions de 4 drones avec captures PTB, docs/validation.md) :
# l'interface affiche ses DSP sur l'échelle de PTB (`to_ptb_db`) ; les calculs gardent la densité physique.
PTB_PSD_OFFSET_DB = 6.5


# ---------------------------------------------------------------- outils


def clear_of_breaks(starts: np.ndarray, length: int, breaks: np.ndarray | None) -> np.ndarray:
    """Masque des fenêtres [début, début + length) qui ne chevauchent aucun raccord (trou du log, D-020)."""
    starts = np.asarray(starts)
    if breaks is None or len(breaks) == 0 or starts.size == 0:
        return np.ones(starts.size, dtype=bool)
    b = np.asarray(breaks)
    # premier raccord strictement après le début : la fenêtre le contient s'il tombe avant sa fin
    k = np.searchsorted(b, starts, side="right")
    nxt = np.where(k < b.size, b[np.minimum(k, b.size - 1)], np.iinfo(np.int64).max)
    return nxt >= starts + length


def _segments(x: np.ndarray, length: int, hop: int,
              breaks: np.ndarray | None = None) -> tuple[np.ndarray, np.ndarray]:
    """Vue (n_seg, length) des fenêtres successives, et indices de début ; `breaks` (raccords du log,
    `FlightLog.breaks`) : les fenêtres qui en chevauchent un sont écartées."""
    n = x.shape[-1]
    if n < length:
        return np.zeros((0, length)), np.zeros(0, dtype=int)
    starts = np.arange(0, n - length + 1, hop)
    view = np.lib.stride_tricks.sliding_window_view(x, length)[::hop][: len(starts)]
    keep = clear_of_breaks(starts, length, breaks)
    if keep.all():
        return view, starts
    return view[keep], starts[keep]


def ptb_subsample(duration_s: float, short: int, mid: int, long: int) -> int:
    """Facteur de recouvrement de PIDtoolbox selon la durée analysée (≤ 20 s, 20–60 s, > 60 s)."""
    if duration_s <= 20:
        return short
    if duration_s <= 60:
        return mid
    return long


def _seg_spectra(segs: np.ndarray, fs: float, mode: str) -> tuple[np.ndarray, np.ndarray]:
    """Spectres unilatéraux de fenêtres déjà découpées (Hann). mode = 'psd' (dB) ou 'amp'."""
    L = segs.shape[1]
    w = signal.windows.hann(L, sym=False)
    segs = segs - segs.mean(axis=1, keepdims=True)
    X = np.fft.rfft(segs * w, axis=1)
    f = np.fft.rfftfreq(L, 1.0 / fs)
    if mode == "amp":
        # amplitude d'un sinus (correction de la fenêtre de Hann : somme(w) = L/2)
        A = np.abs(X) * 2.0 / w.sum()
        return f, A
    P = (np.abs(X) ** 2) / (fs * np.sum(w**2))
    # spectre unilatéral : cases doublées sauf le continu et Nyquist (absent si L est impair), comme scipy
    P[:, 1:(-1 if L % 2 == 0 else None)] *= 2.0
    return f, P


def to_db(p: np.ndarray) -> np.ndarray:
    return 10.0 * np.log10(np.maximum(p, EPS))


def to_ptb_db(db: np.ndarray) -> np.ndarray:
    """DSP en dB → échelle de PIDtoolbox, celle qu'affiche l'interface (les NaN restent NaN)."""
    return db - PTB_PSD_OFFSET_DB


def smooth_freq(y: np.ndarray, level: int) -> np.ndarray:
    """Lissage le long des fréquences (0 = aucun ; 1…5 de plus en plus fort)."""
    if level <= 0:
        return y
    width = 1 + 2 * level
    return ndimage.uniform_filter1d(y, size=width, axis=-1, mode="nearest")


# ---------------------------------------------------------------- spectre 2D


@dataclass
class Spectrum:
    freqs: np.ndarray
    values: np.ndarray  # dB si psd, sinon amplitude
    mode: str  # "psd" ou "amp"
    resolution_hz: float


def spectrum(x: np.ndarray, fs: float, mode: str = "psd", resolution_hz: float = 0.5,
             smoothing: int = 0, breaks: np.ndarray | None = None) -> Spectrum:
    """Spectre d'un signal 1D par la méthode de Welch.

    `resolution_hz` fixe la longueur des fenêtres (0,5 Hz → 2 s), ce qui permet de lire le détail
    sous 100 Hz ; plus la fenêtre est courte, plus la moyenne est lisse.
    """
    x = np.asarray(x, dtype=np.float64)
    n = x.shape[0]
    L = int(min(n, max(64, round(fs / resolution_hz))))
    hop = max(1, L // 2)
    segs, _ = _segments(x, L, hop, breaks)
    if segs.shape[0] == 0:
        return Spectrum(np.zeros(0), np.zeros(0), mode, resolution_hz)
    f, S = _seg_spectra(segs, fs, mode)
    v = S.mean(axis=0)
    if mode == "psd":
        v = to_db(v)
    v = smooth_freq(v, smoothing)
    return Spectrum(f, v, mode, fs / L)


# ---------------------------------------------------------------- fréquence × gaz


@dataclass
class FreqThrottle:
    freqs: np.ndarray  # (nf,)
    bins: np.ndarray  # (100,) centre des tranches (1 … 100 %)
    z: np.ndarray  # (nf, 100) : amplitude ou PSD en dB ; NaN si tranche vide
    counts: np.ndarray  # (100,) nombre de fenêtres par tranche
    mode: str

    def band_stats(self, f_lo: float, f_hi: float) -> tuple[float, float]:
        """Moyenne et pic dans une bande de fréquences (sur les tranches non vides)."""
        m = (self.freqs >= f_lo) & (self.freqs <= f_hi)
        z = self.z[m][:, self.counts > 0]
        if z.size == 0:
            return float("nan"), float("nan")
        return float(np.nanmean(z)), float(np.nanmax(z))


def freq_throttle(x: np.ndarray, xaxis: np.ndarray, fs: float, mode: str = "amp", seg_ms: float = 300.0,
                  nbins: int = 100, x_range: tuple[float, float] = (0.0, 100.0),
                  breaks: np.ndarray | None = None) -> FreqThrottle:
    """Carte fréquence × gaz (ou × % moteur, × Hz moteur… selon `xaxis`).

    Chaque fenêtre de 300 ms est rangée selon la valeur moyenne de `xaxis` ; une fenêtre compte
    pour les tranches voisines (±1 tranche) comme dans PIDtoolbox, ce qui lisse la carte.
    """
    x = np.asarray(x, dtype=np.float64)
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    duration = x.shape[0] / fs
    hop = max(1, L // ptb_subsample(duration, 5, 3, 1))
    segs, starts = _segments(x, L, hop, breaks)
    lo, hi = x_range
    edges = np.linspace(lo, hi, nbins + 1)
    centers = 0.5 * (edges[:-1] + edges[1:])
    if segs.shape[0] == 0:
        f = np.fft.rfftfreq(L, 1.0 / fs)
        return FreqThrottle(f, centers, np.full((f.size, nbins), np.nan), np.zeros(nbins, int), mode)
    f, S = _seg_spectra(segs, fs, mode)
    xs, _ = _segments(np.asarray(xaxis, dtype=np.float64), L, hop, breaks)
    xm = xs.mean(axis=1)
    k = np.clip(np.floor((xm - lo) / (hi - lo) * nbins).astype(int), 0, nbins - 1)
    z = np.zeros((f.size, nbins))
    counts = np.zeros(nbins, dtype=int)
    # accumulation de chaque fenêtre dans sa tranche et ses deux voisines
    for off in (-1, 0, 1):
        kk = k + off
        ok = (kk >= 0) & (kk < nbins)
        np.add.at(z.T, kk[ok], S[ok])
        np.add.at(counts, kk[ok], 1)
    with np.errstate(invalid="ignore", divide="ignore"):
        z = z / counts[None, :]
    z[:, counts == 0] = np.nan
    if mode == "psd":
        z = to_db(z)
        z[:, counts == 0] = np.nan
    return FreqThrottle(f, centers, z, counts, mode)


def smooth_map(z: np.ndarray, level: int) -> np.ndarray:
    """Lissage gaussien d'une carte (fréquence × tranche), en ignorant les tranches vides."""
    if level <= 0:
        return z
    mask = ~np.isnan(z)
    zz = np.where(mask, z, 0.0)
    sig = (1.25 * level, 0.5 * level)
    num = ndimage.gaussian_filter(zz, sig, mode="nearest")
    den = ndimage.gaussian_filter(mask.astype(float), sig, mode="nearest")
    with np.errstate(invalid="ignore", divide="ignore"):
        out = num / den
    out[~mask] = np.nan
    return out


# ---------------------------------------------------------------- fréquence × temps


@dataclass
class Spectrogram:
    times: np.ndarray  # centre des fenêtres (s, temps du log)
    freqs: np.ndarray
    z: np.ndarray  # (nf, nt) PSD en dB


def freq_time(x: np.ndarray, t: np.ndarray, fs: float, win_ms: float = 300.0, hop_div: int = 2,
              freq_smoothing: int = 0, breaks: np.ndarray | None = None) -> Spectrogram:
    x = np.asarray(x, dtype=np.float64)
    L = max(16, int(round(fs * win_ms / 1000.0)))
    hop = max(1, L // max(1, hop_div))
    segs, starts = _segments(x, L, hop)
    if segs.shape[0] == 0:
        return Spectrogram(np.zeros(0), np.zeros(0), np.zeros((0, 0)))
    f, P = _seg_spectra(segs, fs, "psd")
    z = smooth_freq(to_db(P), freq_smoothing).T
    # colonnes gardées pour rester à leur place dans le temps ; vides sur un trou du log (D-020)
    z[:, ~clear_of_breaks(starts, L, breaks)] = np.nan
    times = t[np.minimum(starts + L // 2, t.shape[0] - 1)]
    return Spectrogram(times, f, z)
