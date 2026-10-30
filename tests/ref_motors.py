"""Code d'origine (numpy) de `analysis/motors.py:harmonic_prominence`, `rpm_attenuation`, `rpm_filtered_spectrum` et
`_masked_spectrum`, gardé TEL QUEL comme étalon des tests de parité avec leur port Rust (`rust/otb-analysis`) :
D-031, phase 3b. Ne pas le modifier. Les fonctions sont seulement renommées (suffixe `_ref`)."""

import warnings

import numpy as np

from opentunebox.analysis.bf_filters import _K_NOTCH, _sections_response, rpm_notch_freq_weight
from opentunebox.analysis.spectral import _seg_spectra, _segments, to_db

MOTOR_LINE_MARGIN = 0.05


def harmonic_prominence_ref(x: np.ndarray, mhz: np.ndarray, fs: float, n_harm: int = 3, seg_ms: float = 300.0,
                        min_hz: float = 20.0, breaks: np.ndarray | None = None) -> np.ndarray:
    """Hauteur (dB) de chaque harmonique moteur au-dessus du bruit voisin, médiane des fenêtres de 300 ms.

    Raie = DSP moyenne entre h × (moteur le plus lent) et h × (moteur le plus rapide), ± une case ;
    bruit voisin = DSP moyenne de 15 à 30 % de part et d'autre de h × (fréquence moyenne). Sans raie,
    le résultat vaut environ 0 dB. NaN quand l'harmonique sort de la plage mesurable.
    """
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop, breaks)
    if segs.shape[0] == 0:
        return np.full(n_harm, np.nan)
    f, P = _seg_spectra(segs, fs, "psd")
    df = f[1] - f[0]
    lo_m = _segments(mhz.min(axis=0), L, hop, breaks)[0].mean(axis=1)
    hi_m = _segments(mhz.max(axis=0), L, hop, breaks)[0].mean(axis=1)
    mean_m = _segments(mhz.mean(axis=0), L, hop, breaks)[0].mean(axis=1)
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


def rpm_attenuation_ref(f: np.ndarray, motor_hz: np.ndarray, settings) -> np.ndarray:
    """|H| des notchs RPM du firmware aux fréquences f, forme (fenêtres, len(f)) ; motor_hz (moteurs, fenêtres)."""
    q, rate = settings.rpm_q / 100.0, settings.pid_rate_hz
    f = np.ascontiguousarray(f, dtype=np.float64)
    out = np.ones((motor_hz.shape[1], len(f)))
    # fréquence et poids de chaque notch, (harmoniques, moteurs, fenêtres), calculés en une fois
    fw = [rpm_notch_freq_weight(motor_hz, h, settings, rate) for h in range(settings.rpm_harmonics)]
    fr = np.stack([x[0] for x in fw])
    w = np.broadcast_to(np.stack([x[1] for x in fw]), fr.shape)
    for k in range(motor_hz.shape[1]):
        # même ordre d'étages que l'ancien produit (moteur puis harmonique) ; poids nul = étage omis
        fk, wk = fr[:, :, k].T.ravel(), w[:, :, k].T.ravel()
        on = wk > 0
        n = int(on.sum())
        if n:
            out[k] = np.abs(_sections_response(np.full(n, _K_NOTCH, np.int64), fk[on], np.full(n, q), wk[on],
                                               np.full(n, float(rate)), f))
    return out


def rpm_filtered_spectrum_ref(x: np.ndarray, mhz: np.ndarray | None, fs: float, rpm_settings=None, seg_ms: float = 300.0,
                          breaks: np.ndarray | None = None) -> tuple[np.ndarray, np.ndarray]:
    """DSP moyenne (linéaire) des fenêtres de `seg_ms` de x, après le filtre RPM du firmware : chaque fenêtre est
    multipliée par |H|² de ses notchs (fréquence moyenne de chaque moteur sur la fenêtre, Q, poids et fondu de
    chaque harmonique, à la cadence PID) ; images repliées des harmoniques retirées (log plus lent que la
    boucle). Sans `rpm_settings` ou sans filtre RPM : DSP moyenne de x."""
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop, breaks)
    if segs.shape[0] == 0:
        return np.zeros(0), np.zeros(0)
    f, P = _seg_spectra(segs, fs, "psd")
    if mhz is not None and rpm_settings is not None and rpm_settings.rpm_harmonics > 0:
        mean_m = np.vstack([_segments(np.asarray(m, dtype=np.float64), L, hop, breaks)[0].mean(axis=1) for m in mhz])
        # ⚠️ réponse du filtre RPM, pas un masque : un poids réduit laisse un reste que le firmware suit (L-028)
        P = P * rpm_attenuation_ref(f, mean_m, rpm_settings) ** 2
        lo_m = _segments(mhz.min(axis=0), L, hop, breaks)[0].min(axis=1)  # étendue parcourue pendant la fenêtre
        hi_m = _segments(mhz.max(axis=0), L, hop, breaks)[0].max(axis=1)
        lobe = 2 * fs / L  # demi-largeur du lobe principal de la fenêtre de Hann
        for k in range(P.shape[0]):
            for h in range(1, rpm_settings.rpm_harmonics + 1):
                a, b = h * lo_m[k] - lobe, h * hi_m[k] + lobe
                P[k, (f >= fs - b) & (f <= fs - a)] = np.nan  # image repliée si le log est trop lent
    with np.errstate(all="ignore"), warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        return f, np.nanmean(P, axis=0)


def _masked_spectrum_ref(x: np.ndarray, mhz: np.ndarray, fs: float, n_harm: int, seg_ms: float,
                     breaks: np.ndarray | None = None):
    """DSP moyenne (linéaire) des fenêtres de 300 ms, bandes des harmoniques moteur 1…n retirées
    (h × [moteur le plus lent ; le plus rapide] pendant la fenêtre, ± 5 % de la fréquence moteur, et leur
    image repliée : sur un whoop loggé à 4 kHz, la 8e harmonique retombe vers 200 Hz)."""
    L = max(16, int(round(fs * seg_ms / 1000.0)))
    hop = L // 2
    segs, _ = _segments(np.asarray(x, dtype=np.float64), L, hop, breaks)
    if segs.shape[0] == 0:
        return np.zeros(0), np.zeros(0)
    f, P = _seg_spectra(segs, fs, "psd")
    lo_m = _segments(mhz.min(axis=0), L, hop, breaks)[0].min(axis=1)
    hi_m = _segments(mhz.max(axis=0), L, hop, breaks)[0].max(axis=1)
    P = P.copy()
    for k in range(P.shape[0]):
        for h in range(1, n_harm + 1):
            # marge : 5 % de la fréquence moteur pour toutes les harmoniques. Raies ≥ 6 dB de 119 vols stationnaires
            # du corpus comparées au hasard (2026-10-13) : flancs de la fondamentale jusqu'à 3 % au moins, de la 2e
            # jusqu'à 1 %, aucun pour la 3e ; l'ancienne marge (5 % de l'harmonique : 10 et 15 % pour la 2e et
            # la 3e) cachait des raies hors moteurs (docs/assistant.md, « Stratégie de filtrage et vol stationnaire »)
            a, b = h * lo_m[k] - MOTOR_LINE_MARGIN * lo_m[k], h * hi_m[k] + MOTOR_LINE_MARGIN * hi_m[k]
            P[k, (f >= a) & (f <= b)] = np.nan
            for j in range(1, int(b // fs) + 2):  # images repliées |h·f − j·fs| (log plus lent que la boucle)
                P[k, (f >= j * fs - b) & (f <= j * fs - a)] = np.nan
                P[k, (f >= a - j * fs) & (f <= b - j * fs)] = np.nan
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        return f, np.nanmean(P, axis=0)
