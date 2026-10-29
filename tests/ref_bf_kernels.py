"""Noyaux numba d'origine de `analysis/bf_filters.py`, gardés TELS QUELS comme référence des tests de parité
avec leur port Rust (`rust/otb-dsp`, D-031 phase 2a). Ne pas les modifier : ils servent d'étalon."""

import math

import numpy as np
from numba import njit

_K_PT1, _K_LPF, _K_NOTCH = 0, 1, 2
_MIN_STAGE_GAIN = 1e-15


@njit(cache=True)
def _biquad(kind: int, f: float, q: float, w: float, fs: float):
    """Coefficients (b0, b1, b2, a1, a2) d'un étage ; f ≤ 0 → étage transparent."""
    if f <= 0.0:
        return 1.0, 0.0, 0.0, 0.0, 0.0
    if kind == _K_PT1:
        omega = 2.0 * math.pi * f / fs
        k = omega / (omega + 1.0)
        return k, 0.0, 0.0, -(1.0 - k), 0.0
    omega = 2.0 * math.pi * f / fs
    sn = math.sin(omega)
    cs = math.cos(omega)
    alpha = sn / (2.0 * q)
    a0 = 1.0 + alpha
    a1 = -2.0 * cs / a0
    a2 = (1.0 - alpha) / a0
    if kind == _K_LPF:
        b1 = (1.0 - cs) / a0
        return b1 * 0.5, b1, b1 * 0.5, a1, a2
    # notch pondéré : b = w·b_notch + (1−w)·a
    b0 = 1.0 / a0
    b1 = -2.0 * cs / a0
    b2 = 1.0 / a0
    return (w * b0 + (1.0 - w), w * b1 + (1.0 - w) * a1, w * b2 + (1.0 - w) * a2, a1, a2)


@njit(cache=True)
def _sections_response(kinds, fcs, qs, ws, rates, freqs):
    """Réponse complexe du produit des étages biquad (kinds, fcs, qs, ws, rates : un élément par étage).

    Chaque étage est évalué seul, H = (b0 + b1·z⁻¹ + b2·z⁻²)/(1 + a1·z⁻¹ + a2·z⁻²), puis multiplié :
    même résultat que le produit des polynômes de `coefficients`, sans en construire aucun.
    ⚠️ Gain d'un étage jamais sous `_MIN_STAGE_GAIN` : au centre exact d'un notch (harmonique moteur sur une
    case FFT), un vrai zéro rendrait nul le rapport |H_mod|²/|H_actuel|² du simulateur (L-024).
    """
    n = freqs.shape[0]
    h = np.ones(n, dtype=np.complex128)
    z = np.empty(n, dtype=np.complex128)  # z⁻¹, recalculé seulement quand la cadence change
    rate = -1.0
    for s in range(kinds.shape[0]):
        if rates[s] != rate:
            rate = rates[s]
            for i in range(n):
                z[i] = np.exp(-2j * np.pi * freqs[i] / rate)
        b0, b1, b2, a1, a2 = _biquad(kinds[s], fcs[s], qs[s], ws[s], rate)
        for i in range(n):
            g = (b0 + z[i] * (b1 + z[i] * b2)) / (1.0 + z[i] * (a1 + z[i] * a2))
            if abs(g) < _MIN_STAGE_GAIN:
                g = _MIN_STAGE_GAIN  # creux exact d'un notch : même plancher dans les deux chaînes comparées
            h[i] *= g
    return h


@njit(cache=True)
def _run_section(x, kind, freq, q, weight, fs, f_limit):
    """Applique un étage biquad (forme directe I) à x (axes, n), coefficients variables.

    freq / weight : tableaux (n,) ; une fréquence ≤ 0 ou ≥ f_limit rend l'étage transparent.
    L'état initial est l'état stationnaire pour la première valeur (gain statique 1).
    """
    n_ax, n = x.shape
    y = np.empty_like(x)
    for ax in range(n_ax):
        x1 = x[ax, 0]
        x2 = x1
        y1 = x1
        y2 = x1
        pf = -1.0
        pw = -1.0
        b0, b1, b2, a1, a2 = 1.0, 0.0, 0.0, 0.0, 0.0
        for i in range(n):
            f = freq[i]
            w = weight[i]
            if f != pf or w != pw:
                fe = f if f < f_limit else 0.0
                b0, b1, b2, a1, a2 = _biquad(kind, fe, q, w, fs)
                pf = f
                pw = w
            xi = x[ax, i]
            yi = b0 * xi + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
            x2 = x1
            x1 = xi
            y2 = y1
            y1 = yi
            y[ax, i] = yi
    return y


@njit(cache=True)
def _pt1_track(x, k):
    """PT1 simple sur un signal 1D (lissage de la fréquence moteur)."""
    y = np.empty_like(x)
    s = x[0]
    for i in range(x.shape[0]):
        s += k * (x[i] - s)
        y[i] = s
    return y
