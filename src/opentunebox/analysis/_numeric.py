"""Primitives numériques en numpy pur, à la place de scipy (D-031, phase 2a).

Chaque fonction reproduit celle de scipy qu'elle remplace, conventions comprises (largeur paire décalée,
bord « nearest », fenêtre de Hann symétrique ou périodique) : `tests/test_numeric.py` les compare à scipy.
"""

from __future__ import annotations

import numpy as np


def hann(length: int, sym: bool = True) -> np.ndarray:
    """Fenêtre de Hann (= `scipy.signal.windows.hann`) : symétrique, ou périodique avec `sym=False`."""
    if length < 1:
        return np.zeros(0)
    if length == 1:
        return np.ones(1)
    m = length if sym else length + 1
    w = 0.5 - 0.5 * np.cos(2.0 * np.pi * np.arange(m) / (m - 1))
    return w if sym else w[:-1]


def _float_out(x: np.ndarray, like: np.ndarray) -> np.ndarray:
    """Rend le dtype flottant de l'entrée, comme scipy (float64 pour tout le reste)."""
    return x.astype(like.dtype, copy=False) if like.dtype.kind == "f" else x


def uniform_filter1d(x, size: int, axis: int = -1) -> np.ndarray:
    """Moyenne glissante de largeur `size`, bord répété (= `scipy.ndimage.uniform_filter1d(mode="nearest")`).

    Pour une largeur paire, la fenêtre de la sortie i couvre [i − size//2, i + size//2 − 1] (origine 0 de scipy).
    """
    a = np.asarray(x)
    size = int(size)
    if size < 1:
        raise ValueError("size doit être au moins 1")
    work = np.moveaxis(a.astype(np.float64, copy=False), axis, -1)
    n = work.shape[-1]
    if size == 1 or n == 0:
        return _float_out(np.moveaxis(work, -1, axis), a)
    left = size // 2
    right = size - 1 - left
    padded = np.concatenate(
        [np.repeat(work[..., :1], left, axis=-1), work, np.repeat(work[..., -1:], right, axis=-1)], axis=-1)
    csum = np.cumsum(padded, axis=-1)
    csum = np.concatenate([np.zeros_like(csum[..., :1]), csum], axis=-1)
    out = (csum[..., size:size + n] - csum[..., :n]) / size
    return _float_out(np.moveaxis(out, -1, axis), a)


def _gaussian_kernel(sigma: float, truncate: float = 4.0) -> np.ndarray:
    radius = int(truncate * sigma + 0.5)
    x = np.arange(-radius, radius + 1)
    k = np.exp(-0.5 * x * x / (sigma * sigma))
    return k / k.sum()


def _convolve_nearest(a: np.ndarray, kernel: np.ndarray, axis: int) -> np.ndarray:
    radius = len(kernel) // 2
    work = np.moveaxis(a, axis, -1)
    n = work.shape[-1]
    padded = np.concatenate(
        [np.repeat(work[..., :1], radius, axis=-1), work, np.repeat(work[..., -1:], radius, axis=-1)], axis=-1)
    out = np.zeros(work.shape, dtype=np.float64)
    for j, w in enumerate(kernel):
        out += w * padded[..., j:j + n]
    return np.moveaxis(out, -1, axis)


def gaussian_filter(a, sigma) -> np.ndarray:
    """Flou gaussien séparable, bord répété (= `scipy.ndimage.gaussian_filter(mode="nearest")`, `truncate=4`).

    `sigma` : un nombre (tous les axes) ou une séquence (un par axe) ; un sigma nul laisse l'axe intact.
    """
    arr = np.asarray(a)
    sigmas = np.broadcast_to(np.asarray(sigma, dtype=np.float64), (arr.ndim,))
    out = arr.astype(np.float64, copy=True)
    for axis, s in enumerate(sigmas):
        if s > 1e-15:
            out = _convolve_nearest(out, _gaussian_kernel(float(s)), axis)
    return _float_out(out, arr)


def xcorr_full(y, x) -> tuple[np.ndarray, np.ndarray]:
    """Intercorrélation complète par FFT et ses décalages (= `signal.correlate(y, x, "full", "fft")`
    et `signal.correlation_lags`). Le pic est au décalage positif quand `y` retarde sur `x`."""
    y = np.asarray(y, dtype=np.float64)
    x = np.asarray(x, dtype=np.float64)
    if len(x) == 0 or len(y) == 0:
        raise ValueError("xcorr_full : signal vide")
    n = len(y) + len(x) - 1
    nfft = 1 << max(0, (n - 1).bit_length())
    corr = np.fft.irfft(np.fft.rfft(y, nfft) * np.conj(np.fft.rfft(x, nfft)), nfft)
    # irfft donne la corrélation circulaire : les décalages négatifs sont en fin de tableau
    out = np.concatenate([corr[nfft - (len(x) - 1):], corr[:len(y)]]) if len(x) > 1 else corr[:len(y)]
    return out, np.arange(-(len(x) - 1), len(y))
