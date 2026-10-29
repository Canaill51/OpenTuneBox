"""Boucle numpy d'origine de `analysis/step.py:step_response`, gardée TELLE QUELLE comme étalon des tests de parité
avec son port Rust (`rust/otb-dsp`, `_native.step_windows`) : D-031, phase 2b (pilote). Ne pas la modifier.

Même signature et mêmes résultats que `_native.step_windows` ; `steady_lo:steady_hi` est la plage d'indices de
la réponse (200–500 ms) où l'on contrôle la qualité d'une fenêtre (vide : toutes les fenêtres passent).
"""

import numpy as np


def step_windows_numpy(sp, gy, w, starts, L, Lr, pad, reg, y_correction, steady_lo, steady_hi):
    """→ (R (k, Lr), maxin (k,), sum_gh complexe (nfft//2+1,), sum_hh, sum_gg), fenêtres retenues dans l'ordre."""
    sp = np.asarray(sp, dtype=np.float64)
    gy = np.asarray(gy, dtype=np.float64)
    nfft = L + 2 * pad
    resp, maxin = [], []
    sum_gh = np.zeros(nfft // 2 + 1, dtype=complex)
    sum_hh = np.zeros(nfft // 2 + 1)
    sum_gg = np.zeros(nfft // 2 + 1)
    steady = np.zeros(Lr, dtype=bool)
    steady[steady_lo:steady_hi] = True
    idx = np.arange(L)
    for k in range(0, starts.size, 256):
        st0 = starts[k : k + 256]
        X = sp[st0[:, None] + idx] * w
        Y = gy[st0[:, None] + idx] * w
        X = np.pad(X, ((0, 0), (pad, pad)))
        Y = np.pad(Y, ((0, 0), (pad, pad)))
        Hr = np.fft.rfft(X, axis=1)
        Gr = np.fft.rfft(Y, axis=1)
        H, G = Hr / nfft, Gr / nfft
        imp = np.fft.irfft(G * np.conj(H) / (H * np.conj(H) + reg), n=nfft, axis=1)
        st = np.cumsum(imp, axis=1)[:, :Lr]
        ss = st[:, steady]
        good = (ss.min(axis=1) > 0.5) & (ss.max(axis=1) < 3.0) if ss.size else np.ones(len(st0), bool)
        if not good.any():
            continue
        st = st[good]
        if y_correction and ss.size:
            st = st * (2.0 - ss[good].mean(axis=1, keepdims=True))
        resp.append(st)
        maxin.append(np.max(np.abs(sp[st0[good][:, None] + idx]), axis=1))
        Hr, Gr = Hr[good], Gr[good]
        sum_gh += np.sum(Gr * np.conj(Hr), axis=0)
        sum_hh += np.sum(np.abs(Hr) ** 2, axis=0)
        sum_gg += np.sum(np.abs(Gr) ** 2, axis=0)
    if not resp:
        return np.zeros((0, Lr)), np.zeros(0), sum_gh, sum_hh, sum_gg
    return np.vstack(resp), np.concatenate(maxin), sum_gh, sum_hh, sum_gg
