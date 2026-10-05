"""Marges de stabilité d'une boucle ouverte L(f) mesurée (docs/spec-methodes.md).

- phase déroulée (`np.unwrap`), ramenée dans (−360°, 0] au premier point : une phase sous −180°
  donne une marge négative, au lieu de repartir vers +180° comme avec `np.angle` seul ;
- croisement : premier passage de |L| au-dessous de 1, interpolé en (log f, log |L|) ;
  marge de phase = 180° + ∠L à ce croisement. Les croisements suivants sont ignorés : sur les vrais
  logs, ils viennent presque toujours du bruit (|L| qui repasse 1 vers 100 Hz, docs/validation.md) ;
- marge de gain = −20·log10 |L| au premier passage de la phase par −180° (− k·360°) après le croisement ;
- pic de sensibilité Ms = max |1/(1 + L)| ; s'il tombe au bord de la bande, ce n'est qu'un minorant.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


@dataclass
class Margins:
    crossover_hz: float = float("nan")  # premier passage de |L| au-dessous de 1
    phase_margin_deg: float = float("nan")  # négative si la phase y est sous −180°
    gain_margin_db: float = float("nan")
    phase_crossover_hz: float = float("nan")  # ∠L = −180° (− k·360°) après le croisement
    sensitivity_peak_db: float = float("nan")  # max |1/(1 + L)| en dB
    sensitivity_peak_hz: float = float("nan")
    sensitivity_peak_at_edge: bool = False  # pic au bord de la bande : Ms réel ≥ valeur donnée


def margins(freqs: np.ndarray, L: np.ndarray, valid: np.ndarray | None = None) -> Margins:
    """Marges de phase et de gain, et pic de sensibilité, de la boucle ouverte L sur les points `valid`."""
    freqs = np.asarray(freqs, dtype=np.float64)
    L = np.asarray(L, dtype=complex)
    ok = np.isfinite(L) & (np.abs(L) > 0) & (freqs > 0)
    if valid is not None:
        ok &= np.asarray(valid, dtype=bool)
    m = Margins()
    if ok.sum() < 2:
        return m
    f, Lv = freqs[ok], L[ok]
    logf = np.log(f)
    logmag = np.log(np.abs(Lv))
    phase = np.unwrap(np.angle(Lv))
    phase -= 2 * np.pi * np.ceil(phase[0] / (2 * np.pi))  # premier point dans (−360°, 0]

    # premier passage de |L| au-dessous de 1 : marge de phase
    down = np.nonzero((logmag[:-1] >= 0) & (logmag[1:] < 0))[0]
    if down.size:
        x, _, ph = _interp(logf, logmag, phase, logmag, int(down[0]))
        m.crossover_hz, m.phase_margin_deg = float(np.exp(x)), float(180.0 + np.degrees(ph))

    # premier passage de la phase par −180° (− k·360°) après le croisement, |L| < 1 : marge de gain
    level = (phase + np.pi) / (2 * np.pi)  # entier aux phases −180° + k·360°
    start = int(down[0]) + 1 if down.size else 0
    for i in range(start, logf.size - 1):
        k = np.floor(max(level[i], level[i + 1]))
        if min(level[i], level[i + 1]) <= k:  # un entier entre les deux points
            x, lm, _ = _interp(logf, logmag, phase, level - k, i)
            if lm < 0:
                m.gain_margin_db, m.phase_crossover_hz = float(-20.0 * lm / np.log(10)), float(np.exp(x))
                break

    S = np.abs(1.0 / (1.0 + Lv))
    k = int(np.argmax(S))
    m.sensitivity_peak_db = float(20 * np.log10(S[k]))
    m.sensitivity_peak_hz = float(f[k])
    m.sensitivity_peak_at_edge = k in (0, S.size - 1)
    return m


def _interp(logf, logmag, phase, g, i):
    """Passage de g par zéro entre les points i et i + 1 : (log f, log |L|, phase) interpolés linéairement."""
    t = g[i] / (g[i] - g[i + 1]) if g[i] != g[i + 1] else 0.0
    return (logf[i] + t * (logf[i + 1] - logf[i]),
            logmag[i] + t * (logmag[i + 1] - logmag[i]),
            phase[i] + t * (phase[i + 1] - phase[i]))
