#!/usr/bin/env python3
"""Cas de test de `analysis/loop.py` (étape 4c-B(a)) pour `rust/otb-analysis/src/open_loop.rs`.

Usage : loop_cases.py SORTIE.json

`margins` : boucles ouvertes classiques (intégrateur + retard, deux pôles, gain faible ou fort, phase qui passe sous −180°,
plusieurs passages par −180°), bruit complexe, |L| exactement égal à 1 sur un point (frontière `>=`), phase exactement à −π
(niveau entier), valeurs égales (t = 0), L = −1 (S infinie), zéros, NaN, infinis, très grands ou très petits modules, masques
`valid`, fréquences nulles ou négatives, moins de 2 points, pic de sensibilité au bord ou à l'intérieur de la bande.
`loop_from_T` : T de premier et deuxième ordre à plusieurs bandes passantes, avec bruit, cohérences et bornes variées (< 5 points,
`f_max` et `min_coh` aux frontières), T = 1 (division par zéro), NaN, grilles de fréquences différentes.
`dterm_model` : réglages tirés de `bf_filters_cases` (filtres du D, notch, cadences PID), grilles de fréquences, gains D, gaz.

Un flottant s'écrit en hexadécimal ; un complexe se range en deux tableaux (parties réelle et imaginaire) ; `"error": true` = Python
a levé une exception. Le test `rust/otb-analysis/tests/open_loop.rs` mesure l'écart à Python (log, atan2, exp, cos : L-048).
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import enc_settings, hx, random_settings
from flight_stats_cases import INPUTS, call, ref

from opentunebox.analysis import loop

NAN = float("nan")


def cx(z):
    z = np.asarray(z, dtype=complex)
    return {"re": ref(z.real), "im": ref(z.imag)}


def enc_margins(m) -> dict:
    return {"crossover": hx(m.crossover_hz), "pm": hx(m.phase_margin_deg), "gm": hx(m.gain_margin_db),
            "pcross": hx(m.phase_crossover_hz), "ms_db": hx(m.sensitivity_peak_db), "ms_hz": hx(m.sensitivity_peak_hz),
            "edge": bool(m.sensitivity_peak_at_edge)}


def enc_lm(lm) -> dict:
    return {"method": lm.method, "notes": list(lm.notes), "band": [hx(lm.band_hz[0]), hx(lm.band_hz[1])],
            "bandwidth": hx(lm.bandwidth_hz), "tracking": hx(lm.tracking_peak_db), "ms_db": hx(lm.sensitivity_peak_db),
            "ms_hz": hx(lm.sensitivity_peak_hz), "edge": bool(lm.sensitivity_peak_at_edge), "crossover": hx(lm.crossover_hz),
            "pm": hx(lm.phase_margin_deg), "gm": hx(lm.gain_margin_db), "L": cx(lm.L), "valid": [bool(v) for v in lm.valid]}


def plant(f, k=1.0, tau=0.0, t1=0.0, t2=0.0, integ=True, delay_s=0.0):
    """L(jω) = k e^{−jωτ} / (jω^integ (1 + jωT1)(1 + jωT2))."""
    s = 2j * np.pi * f
    out = k * np.exp(-s * delay_s) / ((1 + s * t1) * (1 + s * t2) * (s if integ else 1.0))
    return out * (np.exp(-s * tau) if tau else 1.0)


def main():
    rng = np.random.default_rng(20261106)
    out = {"margins": [], "from_t": [], "dterm": [], "settings": []}

    def mg(freqs, L, valid=None, label=""):
        freqs = np.asarray(freqs, float)
        r, e = call(loop.margins, freqs, np.asarray(L, complex), valid)
        out["margins"].append({"label": label, "f": ref(freqs), "L": cx(L), "valid": None if valid is None else [bool(v) for v in valid],
                               "error": e, "out": None if e else enc_margins(r)})

    grid = np.linspace(2.0, 200.0, 100)
    fine = np.geomspace(2.0, 400.0, 160)
    # boucles classiques : gain, retard pur, deuxième pôle
    for k in (3.0, 20.0, 100.0, 400.0, 2000.0):
        for delay_s in (0.0, 0.002, 0.006, 0.02):
            for t2 in (0.0, 0.004):
                mg(grid, plant(grid, k * 2 * np.pi, delay_s=delay_s, t1=0.01, t2=t2), label=f"classique k={k} τ={delay_s} T2={t2}")
    for k in (30.0, 600.0):
        mg(fine, plant(fine, k, delay_s=0.01, t1=0.002), label=f"grille géométrique k={k}")
        mg(fine[::-1], plant(fine, k, delay_s=0.01, t1=0.002)[::-1], label="fréquences décroissantes")
    # phase qui fait plusieurs tours (retard long) : plusieurs passages par −180°
    for tau in (0.03, 0.08, 0.2):
        mg(grid, plant(grid, 80.0, delay_s=tau, t1=0.005), label=f"retard long {tau}")
        mg(grid, 0.4 * plant(grid, 80.0, delay_s=tau, t1=0.005), label=f"retard long {tau}, gain faible")
    # bruit complexe, sans structure
    for n in (2, 3, 5, 20, 100, 400):
        for sc in (0.3, 1.0, 5.0):
            L = sc * (rng.normal(size=n) + 1j * rng.normal(size=n))
            mg(np.linspace(1.0, 150.0, n), L, label=f"bruit n={n} échelle {sc}")
    # frontières : |L| = 1 exactement sur un point, phase exactement à −π, valeurs égales
    f = np.array([2.0, 4.0, 8.0, 16.0, 32.0])
    mg(f, np.array([4.0, 2.0, 1.0, 0.5, 0.25], dtype=complex), label="|L| = 1 pile sur un point, phase nulle")
    mg(f, -np.array([4.0, 2.0, 1.0, 0.5, 0.25], dtype=complex), label="|L| = 1 pile, phase −π")
    mg(f, -np.array([4.0, 3.0, 2.0, 0.5, 0.25], dtype=complex), label="phase −π partout")
    mg(f, np.array([2.0, 1.5, 1.0, 1.0, 0.5], dtype=complex), label="deux |L| égaux à 1")
    mg(f, np.array([0.5, 0.5, 0.5, 0.5, 0.5], dtype=complex), label="|L| constant < 1")
    mg(f, np.array([2.0, 2.0, 2.0, 2.0, 2.0], dtype=complex), label="|L| constant > 1")
    mg(f, np.array([1.0, 1.0, 1.0, 1.0, 1.0], dtype=complex), label="|L| constant = 1")
    mg(f, np.array([2.0, 1.5, -1.0, 0.7, 0.3], dtype=complex), label="L = −1 (S infinie)")
    mg(f[:3], np.array([-1.0, -1.0, -1.0], dtype=complex), label="L = −1 partout")
    mg(f, np.array([2.0, 1.5, 1j, -1j, 0.3j], dtype=complex), label="phases ±π/2")
    # phase autour de −π par l'autre côté : Im = −0.0
    mg(f, np.array([-2.0 - 0.0j, -1.5 - 0.0j, -0.7 - 0.0j, -0.3 - 0.0j, -0.1 - 0.0j]), label="Im = −0 (angle −π)")
    mg(f, np.array([-2.0 + 0.0j, -1.5 + 0.0j, -0.7 + 0.0j, -0.3 + 0.0j, -0.1 + 0.0j]), label="Im = +0 (angle +π)")
    # modules extrêmes, NaN, infinis, zéros, fréquences invalides
    mg(f, np.array([1e300, 1e150, 1.0, 1e-150, 1e-300], dtype=complex), label="modules 1e300 à 1e-300")
    mg(f, np.array([1e200 + 1e200j, 1e100, 1.0, 1e-200, 1e-320], dtype=complex), label="grands et sous-normaux")
    mg(f, np.array([2.0, 0.0, 1.0, 0.5, 0.25], dtype=complex), label="un zéro")
    mg(f, np.array([2.0, complex(NAN, 0), 1.0, 0.5, 0.25]), label="un NaN réel")
    mg(f, np.array([2.0, complex(0, NAN), 1.0, 0.5, 0.25]), label="un NaN imaginaire")
    mg(f, np.array([2.0, complex(np.inf, 0), 1.0, 0.5, 0.25]), label="un infini")
    mg(f, np.array([complex(NAN, NAN)] * 5), label="tout NaN")
    mg(f, np.zeros(5, complex), label="tout zéro")
    mg(np.array([0.0, 4.0, 8.0, 16.0, 32.0]), np.array([4.0, 2.0, 1.0, 0.5, 0.25], dtype=complex), label="fréquence nulle")
    mg(np.array([-2.0, 4.0, 8.0, 16.0, 32.0]), np.array([4.0, 2.0, 1.0, 0.5, 0.25], dtype=complex), label="fréquence négative")
    mg(np.array([0.0, 4.0, 8.0, 16.0, 32.0]), np.array([2.0, 0.5, 0.4, 0.3, 0.2], dtype=complex), label="croisement dès le point de fréquence nulle")
    mg(f, np.array([2.0, complex(1.0, np.inf), 1.0, 0.5, 0.25]), label="partie imaginaire infinie, réelle finie")
    mg(f, np.array([complex(1.0, np.inf), 0.5, 0.4, 0.3, 0.2]), label="Im infinie au premier point, puis |L| < 1")
    mg(np.array([2.0, 4.0, NAN, 16.0, 32.0]), np.array([4.0, 2.0, 1.0, 0.5, 0.25], dtype=complex), label="fréquence NaN")
    for n in (0, 1, 2):
        mg(np.linspace(2.0, 30.0, n), 0.5 * np.exp(-1j * np.linspace(0.1, 3.0, n)), label=f"{n} points")
    # masques `valid`
    L = plant(grid, 200.0, delay_s=0.004, t1=0.01)
    for name, v in (("tout vrai", np.ones(100, bool)), ("tout faux", np.zeros(100, bool)), ("un seul point", np.arange(100) == 7),
                    ("deux points", (np.arange(100) == 7) | (np.arange(100) == 40)), ("première moitié", np.arange(100) < 50),
                    ("seconde moitié", np.arange(100) >= 50), ("trous", np.arange(100) % 3 != 0),
                    ("bande 5–60 Hz", (grid >= 5) & (grid <= 60))):
        mg(grid, L, v, label=f"valid {name}")
    # pic de sensibilité : à l'intérieur, au premier point, au dernier point
    mg(grid, plant(grid, 200.0, delay_s=0.003, t1=0.01), label="pic intérieur probable")
    mg(grid, 0.05 / (1 + 1j * grid / 50), label="pic au bord (|L| petit, décroissant)")
    mg(grid, 30.0 * np.exp(-1j * grid / 40), label="pic au bord haut")
    # points au-dessus de 1 puis passage, avec |L| qui repasse au-dessus de 1 plus loin (seul le premier compte)
    rr = np.where(grid < 60, 5.0 * np.exp(-grid / 20), 0.2 + 4.0 * np.exp(-((grid - 120) / 15) ** 2))
    mg(grid, rr * np.exp(-1j * grid / 25), label="|L| repasse au-dessus de 1")
    # niveaux entiers tombant exactement sur un point (phase = −π − k·2π)
    ph = np.array([-0.5, -2.0, -np.pi, -4.0, -2 * np.pi - np.pi, -8.0, -3 * np.pi * 2 - np.pi])
    fa = np.linspace(3.0, 30.0, ph.size)
    mg(fa, np.exp(1j * ph) * np.linspace(2.0, 0.1, ph.size), label="phases sur les niveaux entiers")
    mg(fa, np.exp(1j * ph) * np.linspace(0.9, 0.1, ph.size), label="phases sur les niveaux entiers, |L| < 1")
    # sauts de phase proches de π (np.unwrap)
    th = np.cumsum(np.array([0.0, 2.9, 3.1, 3.2, -3.0, -3.1, 3.14159, -3.14159, 1.0, 0.1]))
    fb = np.linspace(2.0, 60.0, th.size)
    mg(fb, np.exp(1j * th) * np.linspace(3.0, 0.2, th.size), label="sauts de phase proches de π")
    for _ in range(40):  # tirages libres : modules et phases quelconques
        n = int(rng.integers(2, 60))
        ff = np.sort(rng.uniform(0.5, 300.0, n))
        th = np.cumsum(rng.normal(0, 1.2, n))
        mod = np.exp(np.cumsum(rng.normal(-0.1, 0.5, n)) + rng.normal(0.5))
        mg(ff, mod * np.exp(1j * th), label="tirage libre")

    # ---- loop_from_T
    def ft(freqs, T, coh, f_max=150.0, min_coh=0.5, label=""):
        freqs, T, coh = np.asarray(freqs, float), np.asarray(T, complex), np.asarray(coh, float)
        r, e = call(loop.loop_from_T, freqs, T, coh, f_max, min_coh)
        out["from_t"].append({"label": label, "f": ref(freqs), "T": cx(T), "coh": ref(coh), "f_max": hx(f_max),
                              "min_coh": hx(min_coh), "error": e, "out": None if e else enc_lm(r)})

    fg = np.linspace(0.0, 250.0, 126)
    for bw in (8.0, 20.0, 45.0, 90.0, 400.0):
        T = 1.0 / (1 + 1j * fg / bw)
        for noise in (0.0, 0.03):
            Tn = T + noise * (rng.normal(size=fg.size) + 1j * rng.normal(size=fg.size))
            ft(fg, Tn, np.clip(1.0 - 0.3 * fg / 250.0 + rng.normal(0, 0.05, fg.size), 0, 1), label=f"ordre 1 bw={bw} bruit={noise}")
    for wn in (30.0, 80.0):
        for zeta in (0.2, 0.7):
            s = 1j * fg / wn
            ft(fg, 1.0 / (1 + 2 * zeta * s + s * s), np.full(fg.size, 0.9), label=f"ordre 2 wn={wn} ζ={zeta}")
    T = 1.0 / (1 + 1j * fg / 40.0)
    coh = np.full(fg.size, 0.8)
    for fmax in (150.0, 100.0, 12.0, 11.9999, 2.0, 1.0, NAN, np.inf, 250.0):
        ft(fg, T, coh, f_max=fmax, label=f"f_max={fmax}")
    for mc in (0.0, 0.5, 0.8, 0.8000001, 1.0, 1.5, -1.0, NAN):
        ft(fg, T, coh, min_coh=mc, label=f"min_coh={mc}")
    # moins de 5 points, 5 points pile, cohérence en dents de scie
    for n in (0, 1, 4, 5, 6, 7, 11):
        ff = np.linspace(2.0, 100.0, n)
        ft(ff, 1.0 / (1 + 1j * ff / 30.0), np.full(n, 0.9), label=f"{n} points")
    for lo in (3, 4, 5):
        c2 = np.full(fg.size, 0.9)
        c2[lo + 2:] = 0.1  # seuls quelques points passent la cohérence
        ft(fg, T, c2, label=f"{lo + 2 - 1} points cohérents")
    # T = 1 (L infinie), T proche de 1, T nul, T avec NaN, infinis, cohérence NaN
    ft(fg, np.ones(fg.size, complex), coh, label="T = 1")
    ft(fg, np.full(fg.size, 1.0 + 1e-17j), coh, label="T = 1 + 1e-17j")
    ft(fg, np.zeros(fg.size, complex), coh, label="T = 0")
    ft(fg, np.full(fg.size, 0.99, complex), coh, label="T = 0,99 constant")
    ft(fg, np.full(fg.size, 0.3, complex), coh, label="T = 0,3 constant (|T| < 0,707)")
    Tn = T.copy()
    Tn[20] = complex(NAN, 0)
    ft(fg, Tn, coh, label="un NaN dans T")
    Tn[20] = complex(np.inf, 0)
    ft(fg, Tn, coh, label="un infini dans T")
    Tn[20] = complex(0.3, np.inf)
    ft(fg, Tn, coh, label="T : partie imaginaire infinie, réelle finie")
    cn = coh.copy()
    cn[30] = NAN
    ft(fg, T, cn, label="cohérence NaN")
    # |T| passant exactement par 1/√2 sur un point lissé : tout le tableau identique (le lissage ne change rien)
    ft(fg, np.full(fg.size, complex(1 / np.sqrt(2)), complex), coh, label="|T| = 1/√2 pile partout")
    ft(fg, np.full(fg.size, complex(np.nextafter(1 / np.sqrt(2), 0)), complex), coh, label="|T| juste sous 1/√2")
    # grilles de fréquences : espacement variable, valeurs non triées
    ff = np.sort(rng.uniform(0.0, 250.0, 90))
    ft(ff, 1.0 / (1 + 1j * ff / 35.0), np.full(90, 0.9), label="grille irrégulière")
    ft(fg[::-1], T[::-1], coh, label="grille décroissante")
    for _ in range(20):  # tirages libres
        n = int(rng.integers(5, 140))
        ff = np.linspace(0.0, rng.uniform(60, 400), n)
        Tr = (rng.uniform(0.2, 1.2) * np.exp(-ff / rng.uniform(20, 200))) * np.exp(-1j * ff * rng.uniform(0.001, 0.05))
        Tr = Tr + 0.05 * (rng.normal(size=n) + 1j * rng.normal(size=n))
        ft(ff, Tr, rng.uniform(0.3, 1.0, n), f_max=float(rng.choice([100.0, 150.0, 300.0])), label="tirage libre")

    # ---- dterm_model
    settings = random_settings(rng, 40)
    out["settings"] = [enc_settings(s) for s in settings]
    grids = [np.linspace(0.0, 400.0, 81), np.geomspace(1.0, 1000.0, 60), np.array([0.0]), np.array([]), np.array([37.5]),
             np.linspace(1.0, 3000.0, 40), np.array([1e-9, 1.0, 1e9]), np.array([NAN, 5.0]), np.array([np.inf, 5.0])]
    for i, s in enumerate(settings):
        for g in (grids[0], grids[1], grids[5]) if i % 2 else grids:
            for d_gain, thr in ((30.0, 0.5), (0.0, 0.2), (-5.0, 0.9), (1e6, 0.0)):
                if i % 3 == 0 or (d_gain, thr) == (30.0, 0.5):
                    r, e = call(loop.dterm_model, s, g, d_gain, thr)
                    out["dterm"].append({"s": i, "f": ref(g), "d": hx(d_gain), "thr": hx(thr), "error": e,
                                         "out": None if e else cx(r)})
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
