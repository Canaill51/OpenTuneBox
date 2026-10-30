#!/usr/bin/env python3
"""Cas de test de `analysis/delay.py` pour `rust/otb-analysis/src/delay.rs` (4c-2).

Usage : delay_cases.py SORTIE.json

`xcorr_delay_ms` : signaux retardés (entier, fractionnaire, négatif), bruit seul, marche aléatoire, sinus (pics ambigus), marche,
constantes et signal nul (NaN), NaN et infini, longueurs 0 à 3000 autour de la limite de 32 points, tailles différentes, pic au bord
de la fenêtre des retards (pas d'interpolation), fenêtre d'un seul retard de chaque côté (`max_ms` minuscule), égalités.
`phase_delay_ms` : filtre passe-bas retardé à plusieurs cadences (dont 1 024 Hz, où `log2(fs / 2)` est exact, et 50 Hz, fenêtre
trop courte), raccords supprimant des fenêtres, bandes et cohérences minimales (0, 0,6, 0,99, 1,5, −1 : poids nuls), signaux
constants ou nuls, NaN, tailles différentes. `compute_delays` et `stick_p95_dps` : journaux fabriqués avec ou sans gyro non filtré,
terme D nul, manche faible, trou d'enregistrement, très courts, NaN ; périodes pleine, partielle, vide.

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception. `smooth > 1` n'est pas porté (W-024) : le cas
est marqué `"ported": false` et le test exige une erreur de Rust. Le test `rust/otb-analysis/tests/delay.rs` mesure l'écart à
Python (FFT, `hypot`, `atan2` : L-048, L-051).
"""
import json
import sys
from pathlib import Path

import numpy as np
from flight_stats_cases import INPUTS, call, enc_log, make_log, ref, with_
from scipy.signal import lfilter

from opentunebox.analysis import delay
from bf_filters_cases import hx


def lowpass(x, fc, fs):
    w = 2 * np.pi * fc / fs
    k = w / (w + 1.0)
    return lfilter([k], [1.0, -(1.0 - k)], x)


def delayed(rng, n, fs, d=5.0, fc=60.0, noise=0.05):
    """(x, y) : y = x passé dans un PT1 de fc Hz, retardé de d échantillons (fractionnaire par interpolation), plus du bruit."""
    x = rng.normal(0, 100, n + 80)
    y = lowpass(x, fc, fs)
    i = np.arange(n) + 40.0 - d
    y = np.interp(i, np.arange(n + 80), y)
    return x[40:40 + n], y + rng.normal(0, noise * y.std(), n)


def main():
    rng = np.random.default_rng(20261105)
    out = {"xcorr": [], "phase": [], "logs": [], "names": [], "stick": [], "compute": []}

    # ---- xcorr_delay_ms
    def xc(x, y, fs, max_ms=30.0, smooth=0, label=""):
        r, e = call(delay.xcorr_delay_ms, x, y, fs, max_ms, smooth)
        out["xcorr"].append({"label": label, "x": ref(np.asarray(x, float)), "y": ref(np.asarray(y, float)), "fs": hx(fs),
                             "max_ms": hx(max_ms), "smooth": smooth, "error": e, "ported": smooth <= 1,
                             "out": None if e else hx(r)})

    for fs in (500.0, 1000.0, 2000.0, 4000.0, 8000.0):
        for n in (31, 32, 33, 64, 300, 1500):
            if n in (31, 32, 33, 64) or fs in (1000.0, 4000.0):
                x, y = delayed(rng, n, fs, d=3.0)
                xc(x, y, fs, label=f"retard fs={fs} n={n}")
    for d in (-6.0, 0.0, 0.4, 1.0, 7.5, 13.0):
        x, y = delayed(rng, 2000, 2000.0, d=d)
        xc(x, y, 2000.0, 30.0, label=f"retard {d}")
        xc(x, y, 2000.0, 100.0, label=f"retard {d} (fenêtre 100 ms)")
    x, y = delayed(rng, 2000, 2000.0, d=80.0)
    xc(x, y, 2000.0, 30.0, label="pic hors fenêtre (bord)")
    xc(x, y, 2000.0, 0.1, label="fenêtre d'un seul retard")
    xc(x, y, 2000.0, 0.0, label="max_ms nul")
    xc(x, y, 2000.0, 1e9, label="fenêtre immense")
    xc(x, y, 2000.0, float("nan"), label="max_ms NaN")
    # pic au centre d'une fenêtre de 3 retards (i = 1), à l'avant-dernier rang (i = len − 2) et au premier / dernier rang
    xs, ys = delayed(rng, 2000, 1000.0, d=0.3)
    xc(xs, ys, 1000.0, 1.0, label="fenêtre ±1 retard, pic au centre")
    xs3, ys3 = delayed(rng, 2000, 1000.0, d=2.0)
    xc(xs3, ys3, 1000.0, 3.0, label="fenêtre ±3, pic à l'avant-dernier rang")
    xc(*delayed(rng, 2000, 1000.0, d=-2.0), 1000.0, 3.0, label="fenêtre ±3, pic au deuxième rang")
    xc(*delayed(rng, 2000, 1000.0, d=3.0), 1000.0, 3.0, label="fenêtre ±3, pic au dernier rang")
    xc(*delayed(rng, 2000, 1000.0, d=-3.0), 1000.0, 3.0, label="fenêtre ±3, pic au premier rang")
    n = 1500
    rw = np.cumsum(rng.normal(0, 1, n))
    xc(rw, np.roll(rw, 4), 1000.0, label="marche aléatoire")
    xc(rng.normal(0, 1, n), rng.normal(0, 1, n), 1000.0, label="bruits indépendants")
    t = np.arange(n) / 1000.0
    xc(np.sin(2 * np.pi * 20 * t), np.sin(2 * np.pi * 20 * t - 0.5), 1000.0, 100.0, label="sinus 20 Hz")
    xc(np.sin(2 * np.pi * 20 * t), np.sin(2 * np.pi * 20 * t), 1000.0, 100.0, label="sinus identiques")
    xc(np.sign(np.sin(2 * np.pi * 7 * t)), np.sign(np.sin(2 * np.pi * 7 * t - 0.3)), 1000.0, 100.0, label="créneaux")
    step = np.where(t > 0.5, 100.0, 0.0)
    xc(step, np.where(t > 0.512, 100.0, 0.0), 1000.0, label="marche")
    xc(np.full(n, 3.0), rng.normal(0, 1, n), 1000.0, label="x constant")
    xc(rng.normal(0, 1, n), np.zeros(n), 1000.0, label="y nul")
    xc(np.zeros(n), np.zeros(n), 1000.0, label="tout nul")
    xn = rng.normal(0, 1, n)
    xn[100] = np.nan
    xc(xn, rng.normal(0, 1, n), 1000.0, label="NaN dans x")
    xi = rng.normal(0, 1, n)
    xi[100] = np.inf
    xc(xi, rng.normal(0, 1, n), 1000.0, label="infini dans x")
    a, b = delayed(rng, 1500, 1000.0, d=2.0)
    xc(a, b[:900], 1000.0, label="y plus court")
    xc(a[:700], b, 1000.0, label="x plus court")
    xc(a[:0], b[:0], 1000.0, label="vides")
    ties = np.zeros(200)
    ties[50] = 1.0
    xc(ties, ties, 1000.0, label="impulsions identiques")
    xc(ties, np.roll(ties, 3), 1000.0, label="impulsion retardée")
    for sm in (1, 2, 5, 40, 5000):
        xc(a, b, 1000.0, 30.0, sm, label=f"smooth {sm}")
    for fs in (0.0, -1000.0, 1e-3, 1e12):
        xc(a, b, fs, label=f"fs {fs}")

    # ---- phase_delay_ms
    def ph(x, y, fs, band=(10.0, 100.0), coh=0.6, breaks=None, label=""):
        r, e = call(delay.phase_delay_ms, x, y, fs, band, coh, None if breaks is None else np.asarray(breaks, dtype=np.int64))
        out["phase"].append({"label": label, "x": ref(np.asarray(x, float)), "y": ref(np.asarray(y, float)), "fs": hx(fs),
                             "band": [hx(band[0]), hx(band[1])], "coh": hx(coh),
                             "breaks": None if breaks is None else [int(v) for v in breaks], "error": e,
                             "out": None if e else hx(r)})

    for fs in (100.0, 250.0, 500.0, 1000.0, 1024.0, 2000.0, 2048.0, 3200.0, 4000.0, 8000.0):
        for n in sorted({min(int(fs * 0.4), 3000), min(int(fs * 1.0), 3000), min(int(fs * 2.5), 3000)}):
            x, y = delayed(rng, n, fs, d=fs / 1000.0 * 4.0, fc=70.0)
            ph(x, y, fs, label=f"fs={fs} n={n}")
    ph(*delayed(rng, 200, 50.0), 50.0, label="fs=50 : fenêtre < 64")
    ph(*delayed(rng, 200, 120.0), 120.0, label="fs=120")
    x, y = delayed(rng, 6000, 2000.0, d=6.0, fc=50.0)
    ph(x, y, 2000.0, label="référence 3 s")
    for br in ([], [1500], [500, 1700, 3000], [10, 20, 30], list(range(100, 6000, 400))):
        ph(x, y, 2000.0, breaks=br, label=f"raccords {len(br)}")
    for band in ((10.0, 100.0), (0.0, 1000.0), (50.0, 60.0), (200.0, 100.0), (30.0, 31.0), (-5.0, 5.0)):
        ph(x, y, 2000.0, band=band, label=f"bande {band}")
    for coh in (0.0, 0.6, 0.9, 0.99, 1.0, 1.5, -1.0, float("nan")):
        ph(x, y, 2000.0, coh=coh, label=f"cohérence {coh}")
    ph(x, rng.normal(0, 1, 6000), 2000.0, coh=0.0, label="bruits indépendants, cohérence 0")
    ph(x, rng.normal(0, 1, 6000), 2000.0, label="bruits indépendants")
    ph(x, np.zeros(6000), 2000.0, coh=-1.0, label="y nul, cohérence -1 : poids nuls")
    ph(x, np.zeros(6000), 2000.0, coh=0.0, label="y nul, cohérence 0 : poids nuls")
    ph(np.zeros(6000), y, 2000.0, coh=0.0, label="x nul, cohérence 0 : sxx = 0")
    ph(np.zeros(6000), y, 2000.0, coh=-1.0, label="x nul, cohérence -1")
    # bandes qui contiennent exactement 2, 3 puis 4 cases de fréquence (nper = 1024 à 2 kHz : pas de 1,953 Hz)
    for lo, hi in ((50.0, 53.0), (50.0, 54.9), (50.0, 56.8), (49.9, 50.8), (50.78125, 50.78125)):
        ph(x, y, 2000.0, band=(lo, hi), label=f"bande {lo}-{hi} : peu de cases")
    ph(x, np.zeros(6000), 2000.0, label="y nul")
    ph(np.full(6000, 2.0), y, 2000.0, label="x constant")
    xn = x.copy()
    xn[3000] = np.nan
    ph(xn, y, 2000.0, label="NaN dans x")
    ph(x, y[:4000], 2000.0, label="y plus court")
    ph(x[:63], y[:63], 2000.0, label="63 points")
    for fs in (0.0, -2000.0, float("nan"), float("inf"), 1.0):
        ph(x, y, fs, label=f"fs {fs}")
    ph(-x, y, 2000.0, label="phase inversée")
    ph(x, np.roll(y, 300), 2000.0, label="retard de 150 ms")
    ph(x, y * 1e-9, 2000.0, label="y minuscule")
    ph(x * 1e9, y, 2000.0, label="x énorme")

    # ---- np.unwrap (primitive de phase_delay_ms et de la boucle ouverte)
    pi = np.pi
    ulp = np.nextafter(pi, 4) - pi
    out["unwrap"] = []
    for name, v in {"vide": [], "un": [1.5], "deux": [0.0, 7.0], "pi exact": [0.0, pi, 0.0], "-pi exact": [0.0, -pi, 0.0],
                    "pi+ulp": [0.0, pi + ulp, 0.0], "pi-ulp": [0.0, pi - ulp, 0.0], "-pi-ulp": [0.0, -pi - ulp, 0.0],
                    "2pi": [0.0, 2 * pi, 4 * pi], "3pi": [0.0, 3 * pi, -3 * pi], "zéros négatifs": [-0.0, 0.0, -0.0],
                    "nan": [0.0, np.nan, 1.0, 9.0], "inf": [0.0, np.inf, 1.0], "pi+pi exact": [0.0, pi, 2 * pi, 3 * pi],
                    "plateau": [1.0] * 6, "aléatoire": list(rng.uniform(-30, 30, 200)),
                    "marche": list(np.angle(np.exp(1j * np.cumsum(rng.normal(0, 2.5, 300)))))}.items():
        out["unwrap"].append({"label": name, "in": ref(np.asarray(v, float)), "out": ref(np.unwrap(np.asarray(v, float)))})

    # ---- journaux : stick_p95_dps et compute_delays
    def mk(n, fs, name, unfilt=True, dterm=True, moving=True, gap=None, nan=False, **kw):
        lg = make_log(rng, n, fs, flags="all", quiet=not moving, **kw)
        x, y = delayed(rng, n, fs, d=fs / 1000.0 * 5.0, fc=80.0)
        x = (x * (0.8 if moving else 0.02)).astype(np.float64)
        sp = np.vstack([x, x * 0.7, x * 0.3]).astype(np.float32) if moving else np.vstack([x, x, x]).astype(np.float32)
        gu = np.vstack([x + rng.normal(0, 5, n), x + rng.normal(0, 5, n), x]).astype(np.float32)
        gy = np.vstack([lowpass(gu[0], 90, fs), lowpass(gu[1], 90, fs), y]).astype(np.float32)
        dt = np.vstack([np.gradient(gy[0]), np.gradient(gy[1]), np.zeros(n)]).astype(np.float32) * -3.0
        dt = lowpass(dt, 70, fs).astype(np.float32) if dterm else np.zeros((3, n), np.float32)
        t = lg.t.copy()
        if gap is not None:
            t[gap:] += 30.0 / fs
        if nan:
            gy[0, n // 2] = np.nan
        return with_(lg, t=t, setpoint=sp, gyro=gy, gyro_unfilt=gu if unfilt else None, d_term=dt)

    named = [("vol", mk(2000, 1000.0, "vol")), ("sans_gyro_non_filtre", mk(2000, 1000.0, "x", unfilt=False)),
             ("terme_d_nul", mk(2000, 1000.0, "x", dterm=False)), ("manche_faible", mk(2000, 1000.0, "x", moving=False)),
             ("trou", mk(2000, 1000.0, "x", gap=900)), ("court", mk(60, 1000.0, "x")), ("tres_court", mk(40, 500.0, "x")),
             ("nan_gyro", mk(2000, 1000.0, "x", nan=True)), ("vol_2kHz", mk(2000, 2000.0, "x")),
             ("vol_500Hz", mk(2000, 500.0, "x")), ("vol_4kHz", mk(1500, 4000.0, "x"))]
    base = named[0][1]
    n0 = base.n
    sp = base.setpoint.copy()

    def sp_with(rows):
        return with_(base, setpoint=np.asarray(rows, np.float32))

    nan_roll = sp.copy()
    nan_roll[0, 100] = np.nan
    nan_pitch = sp.copy()
    nan_pitch[1, 100] = np.nan
    named += [("nan_consigne_roll", with_(base, setpoint=nan_roll)), ("nan_consigne_pitch", with_(base, setpoint=nan_pitch)),
              ("pitch_plus_fort", sp_with([sp[1] * 0.2, sp[0], sp[2]])),
              ("d_roll_nul", with_(base, d_term=np.vstack([np.zeros(n0), base.d_term[1], base.d_term[2]]).astype(np.float32))),
              ("d_pitch_seul_nul", with_(base, d_term=np.vstack([base.d_term[0], np.zeros(n0), base.d_term[2]]).astype(np.float32)))]
    t35 = base.t.copy()
    t35[900:] += 2.5 / base.fs  # pas de 1 + 2,5 = 3,5 périodes : entre les seuils 3 et 4 (aucun raccord)
    t40 = base.t.copy()
    t40[900:] += 3.0 / base.fs  # pas de 1 + 3 = 4 périodes : pile le seuil (`>` strict : aucun raccord)
    named += [("trou_3_5_periodes", with_(base, t=t35)), ("trou_4_periodes", with_(base, t=t40))]
    alt = np.where(np.arange(n0) % 2 == 0, 5.0, -5.0).astype(np.float32)
    named += [("ecart_type_5_pile", sp_with([alt, alt, alt])),
              ("manche_30_5", sp_with(np.full((3, n0), 30.5))), ("manche_30", sp_with(np.full((3, n0), 30.0))),
              ("manche_29_9", sp_with(np.full((3, n0), 29.9)))]
    # cohérence entre 0,5 et 0,6 dans la bande : du bruit ajouté au gyro filtré jusqu'à ce que les seuils 0,5 et 0,6 donnent
    # deux retards différents
    gu0 = base.gyro_unfilt
    for amp in (10.0, 20.0, 30.0, 40.0, 60.0, 80.0, 120.0, 160.0, 220.0):
        gy = (lowpass(gu0[0], 90, base.fs) + rng.normal(0, amp, n0)).astype(np.float32)
        a, b = (delay.phase_delay_ms(gu0[0].astype(float), gy.astype(float), base.fs, (10.0, 100.0), c) for c in (0.5, 0.6))
        if not (np.isnan(a) and np.isnan(b)) and a != b:
            named.append(("coherence_0_5_a_0_6", with_(base, gyro=np.vstack([gy, gy, base.gyro[2]]).astype(np.float32),
                                                       gyro_unfilt=np.vstack([gu0[0], gu0[0], gu0[2]]))))
            break
    else:
        sys.exit("aucun bruit n'a donné de cohérence entre 0,5 et 0,6")
    out["names"] = [n for n, _ in named]
    logs = [lg for _, lg in named]
    for i, lg in enumerate(logs):
        out["logs"].append(enc_log(lg))
        n = lg.n
        for sel in (None, [0, n], [n // 4, 3 * n // 4], [n, n], [0, 0], [n + 5, n + 9], [10, min(n, 70)]):
            sl = None if sel is None else slice(sel[0], sel[1])
            r, e = call(delay.stick_p95_dps, lg, sl if sl is not None else slice(None))
            out["stick"].append({"log": i, "sel": sel, "error": e, "out": None if e else hx(r)})
            r, e = call(delay.compute_delays, lg, sl)
            rec = {"log": i, "sel": sel, "error": e, "out": None}
            if not e:
                rec["out"] = {"gyro": hx(r.gyro_ms), "dterm": hx(r.dterm_ms), "sp": [hx(v) for v in r.sp_gyro_ms],
                              "stick": hx(r.stick_p95_dps), "moving": bool(r.moving)}
            out["compute"].append(rec)
    out["inputs"] = INPUTS
    Path(sys.argv[1]).write_text(json.dumps(out, separators=(",", ":")))
    print({k: len(v) for k, v in out.items() if isinstance(v, list)}, len(INPUTS), "entrées")


if __name__ == "__main__":
    main()
