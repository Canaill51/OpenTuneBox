#!/usr/bin/env python3
"""Cas de test des éléments sans état de `advisor/rules.py` pour `rust/otb-advisor/src/rules.rs` (5a-2a).

Usage : rules_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python`)

Sections :
- `constants`    : les constantes portées (flottants en hexadécimal) ;
- `gaps`         : `gaps_text` (1 trou ou plusieurs, durée nulle, NaN, égalités d'arrondi) ;
- `fr`           : `_fr` ;
- `rates`        : `log_rate_needed` (avec et sans fréquence moteur) et `pid_log_rate_needed` ;
- `cli`          : `sample_rate_cli` (égalités exactes : le premier gagne) ;
- `loop_texts`   : `_loop_caveat`, `_whoop_margin_note`, `_edge_actions`, `_margin_actions` ;
- `imbalance`    : `_imbalance_finding` (4 moteurs ou non, aucun seuil, égalité gap == dgap, erreurs de Python) ;
- `sorted`       : `Report.sorted` (tri stable, gravité inconnue = 9) ;
- `step`         : `_step_rules` (nombre de fenêtres, pics, latences, gains F, marges de la boucle) ;
- `step_findings`: `_step_findings` appelé seul.

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception.
"""
import json
import math
import random
import struct
import sys

import numpy as np

from opentunebox.advisor import rules
from opentunebox.advisor.profiles import PROFILES
from opentunebox.analysis.loop import APPROX, MEASURED, LoopMetrics
from opentunebox.analysis.step import StepMetrics, StepResult

NAN, INF = float("nan"), float("inf")


def hx(x) -> str:
    return struct.pack("<d", float(x)).hex()


def up(x):
    return math.nextafter(x, INF)


def down(x):
    return math.nextafter(x, -INF)


def call(fn, *args):
    try:
        return fn(*args), False
    except Exception:  # noqa: BLE001
        return None, True


def fd(f) -> dict:
    return {"key": f.key, "category": f.category, "severity": f.severity, "title": f.title, "detail": f.detail,
            "actions": list(f.actions), "axis": f.axis, "settings": list(f.settings)}


def lp_json(lp) -> dict:
    return {"measured": lp.measured, "notes": list(lp.notes), "sens": hx(lp.sensitivity_peak_db),
            "hz": hx(lp.sensitivity_peak_hz), "edge": lp.sensitivity_peak_at_edge, "pm": hx(lp.phase_margin_deg),
            "xo": hx(lp.crossover_hz), "dr": hx(lp.d_ratio)}


def make_lp(measured=False, notes=(), sens=NAN, hz=NAN, edge=False, pm=NAN, xo=NAN, dr=NAN) -> LoopMetrics:
    return LoopMetrics(method=MEASURED if measured else APPROX, notes=list(notes), sensitivity_peak_db=sens,
                       sensitivity_peak_hz=hz, sensitivity_peak_at_edge=edge, phase_margin_deg=pm,
                       crossover_hz=xo, d_ratio=dr)


def make_step(axis, n, pk, lat, lp):
    z = np.zeros(0)
    r = StepResult(t_ms=z, responses=z, mean=z, std=z, n=n, metrics=StepMetrics(peak=pk, latency_ms=lat), loop=lp,
                   max_inputs=z)
    return rules.AxisStep(axis, r)


def constants() -> dict:
    r = rules
    return {
        "step_peak_high": [hx(x) for x in r.STEP_PEAK_HIGH], "step_peak_high_ff": [hx(x) for x in r.STEP_PEAK_HIGH_FF],
        "step_peak_flat": hx(r.STEP_PEAK_FLAT), "step_peak_soft": hx(r.STEP_PEAK_SOFT),
        "step_min_windows": r.STEP_MIN_WINDOWS, "step_sure_windows": r.STEP_SURE_WINDOWS,
        "ff_series_advice": r.FF_SERIES_ADVICE, "sensitivity_high_db": hx(r.SENSITIVITY_HIGH_DB),
        "phase_margin_low_deg": hx(r.PHASE_MARGIN_LOW_DEG), "d_dominant_ratio": hx(r.D_DOMINANT_RATIO),
        "severities": [r.INFO, r.OK, r.CONSEIL, r.ATTENTION, r.ALERTE], "severity_order": r.SEVERITY_ORDER,
        "tracked_settings": list(r.TRACKED_SETTINGS), "loop_margin_keys": list(r.LOOP_MARGIN_KEYS),
        "log_rate_min_hz": hx(r.LOG_RATE_MIN_HZ), "log_rate_fractions": list(r.LOG_RATE_FRACTIONS),
        "pid_log_rate_min_hz": hx(r.PID_LOG_RATE_MIN_HZ), "pid_log_rate_floor_hz": hx(r.PID_LOG_RATE_FLOOR_HZ),
    }


def gaps_cases() -> list:
    out = []
    for n in (0, 1, 2, 12):
        for lost in (0.0, 0.05, 0.15, 0.25, 0.35, 0.45, 0.75, 1.25, 12.34, 99.95, NAN, INF, -1.0):
            for span in (0.0, 0.35, 5.0, 10.0, 33.3, 120.0, 1e-9, -5.0, -0.4, NAN, INF):
                s, _ = call(rules.gaps_text, n, lost, span)
                out.append({"n": n, "lost": hx(lost), "span": hx(span), "out": s})
    return out


def fr_cases() -> list:
    xs = [0.0, -0.0, 0.5, 1.0, 1.005, 1.2, 1.25, 1.6, 1.7, 2.675, 0.125, 0.135, 0.375, 1.03, 1.01, 100.0, 1e16, 1e-9,
          -1.2, -0.005, NAN, INF, -INF, 0.004999999999999999, 0.005, 0.015, 0.025]
    return [{"x": hx(x), "out": rules._fr(x)} for x in xs]


def rates_cases() -> list:
    pids = [8000, 6400, 4000, 3200, 2000, 1600, 1500, 1000, 800, 500, 250, 100, 0, -2000, NAN, INF, 3999.9, 1684.2]
    motors = [None, 0.0, 100.0, 266.6, 266.7, 300.0, 345.0, 400.0, 600.0, 900.0, 1000.0, 1300.0, -50.0, NAN, INF]
    out = []
    for p in pids:
        pidneed, _ = call(rules.pid_log_rate_needed, p)
        for m in motors:
            need, err = call(rules.log_rate_needed, p, m)
            out.append({"pid": hx(p), "motor": None if m is None else hx(m), "need": None if err else hx(need),
                        "pidneed": None if err else hx(pidneed)})
    # frontières de 0,95 × besoin (fraction de la boucle PID juste au-dessus, au niveau ou juste au-dessous du seuil)
    for need, motor in ((1600.0, None), (1000.0, None), (2400.0, 400.0), (1800.0, 300.0), (3000.0, 500.0)):
        for base in (0.95 * need, 0.95 * 1600.0, 0.95 * 1000.0, 0.95 * 6.0 * (motor or 0.0)):
            for d in (1, 2, 4, 8, 16):
                for p in (base * d, up(base * d), down(base * d)):
                    out.append({"pid": hx(p), "motor": None if motor is None else hx(motor),
                                "need": hx(rules.log_rate_needed(p, motor)),
                                "pidneed": hx(rules.pid_log_rate_needed(p))})
    return out


def cli_cases() -> list:
    out = []
    for p in (8000, 4000, 3200, 2000, 1000, 500, 1500, 0, NAN, INF, -8000):
        rates = [0.0, 1.0, 250.0, 500.0, 1000.0, 1600.0, 2000.0, 3200.0, 4000.0, 8000.0, 1e9, NAN, INF, -INF]
        for d in (1, 2, 4, 8, 16):
            rates.append(p / d)
        for d in (1, 2, 4, 8):
            rates.append((p / d + p / (2 * d)) / 2)  # à égalité exacte entre deux fractions : la première gagne
            rates.append(up((p / d + p / (2 * d)) / 2))
            rates.append(down((p / d + p / (2 * d)) / 2))
        for r in rates:
            s, err = call(rules.sample_rate_cli, p, r)
            out.append({"pid": hx(p), "rate": hx(r), "out": s})
    return out


def loop_variants() -> list:
    out = []
    for measured in (False, True):
        for notes in ((), ("pas assez de bande",), ("a", "b; c")):
            for sens in (NAN, 6.9, 7.0, up(7.0), 8.46, 12.0, INF):
                for edge in (False, True):
                    out.append(make_lp(measured, notes, sens, 31.7, edge, 50.0, 22.0, 0.4))
    for measured in (False, True):
        for hz in (NAN, 0.4, 0.5, 1.5, 2.5, 45.0, 123.456):
            out.append(make_lp(measured, (), 9.0, hz, True))
        for dr in (NAN, -1.0, 0.0, 0.5, 1.0, down(1.0), up(1.0), 1.04, 1.05, 1.15, 2.37, INF):
            out.append(make_lp(measured, (), 9.0, 30.0, False, 20.0, 25.0, dr))
            out.append(make_lp(measured, ("x",), 9.0, 30.0, True, 20.0, 25.0, dr))
    return out


def loop_text_cases() -> list:
    out = []
    for lp in loop_variants():
        out.append({"lp": lp_json(lp), "caveat": rules._loop_caveat(lp), "edge": rules._edge_actions(lp),
                    "margin": rules._margin_actions(lp)})
    for p in PROFILES.values():
        out.append({"profile": p.key, "whoop": rules._whoop_margin_note(p)})
    return out


def imbalance_cases() -> list:
    out = []

    def add(m, imb):
        f, err = call(rules._imbalance_finding, list(m), imb)
        out.append({"m": [hx(v) for v in m], "imb": hx(imb), "error": err, "out": None if err else fd(f)})

    grid = (0.0, 10.0, 20.0, 30.0)
    for a in grid:
        for b in grid:
            for c in grid:
                for d in grid:
                    add([a, b, c, d], 20.0)
    for base in (100.0, 20.0, 33.3):
        for k, delta in enumerate((0.0, 5.0, 11.99, 12.0, 12.01, 13.0, 30.0, -30.0, 60.0)):
            for i in range(4):
                m = [base] * 4
                m[i] += delta
                add(m, 12.5 + k)
            m = [base, base + delta, base + delta, base]  # côtés / diagonales variés
            add(m, 33.33)
            m = [base + delta, base, base, base + delta]
            add(m, 33.5)
    # égalité exacte gap == dgap (le côté gagne) ; ex æquo entre côtés (le premier du dictionnaire gagne)
    add([10.0, 14.0, 10.0, 10.0], 13.5)
    add([10.0, 10.0, 14.0, 10.0], 13.5)
    add([10.0, 12.0, 10.0, 10.0], 13.5)
    add([80.0, 80.0, 80.0, 80.0], 0.0)
    add([60.0, 60.0, 60.0, 60.0], NAN)
    add([NAN, 10.0, 20.0, 30.0], 0.5)
    add([10.0, NAN, 20.0, 30.0], 0.5)
    add([10.0, 20.0, NAN, NAN], 0.5)
    add([INF, 10.0, 20.0, 30.0], INF)
    rng = random.Random(7)
    for _ in range(120):
        m = [round(rng.uniform(0, 100), rng.choice((0, 1, 6))) for _ in range(4)]
        add(m, rng.choice((0.5, 12.5, 13.5, 27.0, 40.123)))
    # autre nombre de moteurs : pas de lecture par côté ; moteur le plus chargé en premier sur les ex æquo
    for m in ([], [0.0], [30.0], [20.0, 30.0], [30.0, 30.0, 10.0], [10.0, 20.0, 30.0, 40.0, 50.0],
              [50.0, 50.0, 10.0, 10.0, 10.0, 50.0], [1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [NAN, 3.0], [3.0, NAN, NAN],
              [0.0, 0.0, 0.0], [10.5, 10.5]):
        for imb in (0.4, 12.5, 13.5, 100.0):
            add(m, imb)
    return out


def sorted_cases() -> list:
    sev = [rules.INFO, rules.OK, rules.CONSEIL, rules.ATTENTION, rules.ALERTE, "zzz", ""]
    rng = random.Random(3)
    out = []
    for n in (0, 1, 2, 5, 12, 12, 30):
        items = [(f"k{i}", rng.choice(sev)) for i in range(n)]
        fs = [rules.Finding(k, "Cat", s, "t", "d") for k, s in items]
        rep = rules.Report(profile=None, profile_reason="", period=(0.0, 1.0), findings=fs, steps=[], measures={})
        out.append({"items": items, "out": [f.key for f in rep.sorted()]})
    return out


PKS = [NAN, 0.5, 0.9, 1.0, 1.005, down(1.01), 1.01, up(1.01), 1.03, 1.1, 1.19, down(1.2), 1.2, up(1.2), 1.21, 1.25,
       up(1.25), 1.3, 1.5, 1.59, down(1.6), 1.6, up(1.6), 1.65, 1.7, up(1.7), 1.75, 2.5, INF, -INF]


def lat_values(p) -> list:
    l0, l1 = p.latency_ms
    return [NAN, 0.0, 2.5, down(l0), l0, up(l0), (l0 + l1) / 2, down(l1), l1, up(l1), l1 + 1.0, 100.0, INF]


def step_inputs() -> list:
    """Liste de (profil, axe, n, pic, latence, boucle, gain F, F coupé)."""
    cases = []
    axes = ("Roll", "Pitch", "Yaw")
    benign = make_lp()
    ffs = ((0, False), (0, True), (3, False), (40, True), (-1, False))
    # 1. pic × latence × profil × gain F
    for p in PROFILES.values():
        full = p.key == "five"  # les 4 autres profils : un pic sur trois, une latence sur trois
        for pk in PKS if full else PKS[::3]:
            for lat in lat_values(p) if full else lat_values(p)[::3]:
                for ff, cut in ffs[:3]:
                    cases.append((p.key, "Roll", 250, pk, lat, benign, ff, cut))
    # 2. cas rares : F coupé en mode Angle, gain négatif ou fort, axes variés
    for pk in (1.0, 1.25, 1.3, 1.8, NAN):
        for lat in (NAN, 8.0, 30.0):
            for ff, cut in ffs:
                for ax in axes + ("ÉCART",):
                    cases.append(("five", ax, 250, pk, lat, benign, ff, cut))
    # 3. boucle ouverte
    for p in ("whoop", "five"):
        for pk in (1.0, 1.8):
            for lp in loop_variants()[::2] if pk > 1.5 else loop_variants():
                cases.append((p, "Pitch", 250, pk, 15.0, lp, 0, False))
    for p in ("micro", "large"):
        for lp in loop_variants()[::3]:
            cases.append((p, "Yaw", 250, 1.0, 20.0, lp, 3, False))
    for measured in (False, True):
        for pm in (NAN, -10.0, 0.0, 20.5, down(35.0), 35.0, up(35.0), 49.9, 80.0, INF):
            for sens in (NAN, 3.0, 7.0, 7.5):
                for xo in (NAN, 25.5):
                    cases.append(("five", "Roll", 250, 1.0, 15.0, make_lp(measured, (), sens, 40.0, False, pm, xo, 0.4),
                                  0, False))
    # 4. nombre de fenêtres (sous 20 : rien ; 20 à 199 : « mesure moins sûre » sauf marges de la boucle)
    for n in (0, 1, 19, 20, 21, 100, 199, 200, 201, 100000):
        for pk, lat, lp, ff in ((1.0, 15.0, benign, 0), (1.3, 15.0, benign, 0), (1.0, 5.0, benign, 3),
                                (1.65, 5.0, benign, 3), (1.0, 40.0, benign, 0), (0.95, 40.0, benign, 0),
                                (1.0, 15.0, make_lp(True, (), 9.0, 30.0, True, 20.0, 25.0, 1.5), 0),
                                (1.0, 15.0, make_lp(False, ("n",), NAN, NAN, False, 20.0, 25.0, NAN), 0),
                                (1.3, 15.0, make_lp(True, (), 9.0, 30.0, False, 20.0, 25.0, 0.5), 0)):
            for p in ("whoop", "large"):
                cases.append((p, "Roll", n, pk, lat, lp, ff, False))
    # 5. tirage aléatoire de combinaisons
    rng = random.Random(11)
    keys = list(PROFILES)
    for _ in range(250):
        p = PROFILES[rng.choice(keys)]
        lp = make_lp(rng.random() < 0.5, rng.choice(((), ("a",), ("a", "b"))),
                     rng.choice((NAN, 3.0, 7.0, 7.4, 9.9, 15.0)), rng.choice((NAN, 20.0, 31.4, 150.0)),
                     rng.random() < 0.5, rng.choice((NAN, 20.0, 35.0, 60.0)), rng.choice((NAN, 18.0, 66.0)),
                     rng.choice((NAN, 0.3, 1.0, 1.6)))
        cases.append((p.key, rng.choice(axes), rng.choice((5, 25, 150, 400)), rng.choice(PKS), rng.choice(lat_values(p)),
                      lp, rng.choice((0, 0, 2, 7)), rng.random() < 0.3))
    return cases


def step_json(c) -> dict:
    key, axis, n, pk, lat, lp, ff, cut = c
    return {"profile": key, "axis": axis, "n": n, "pk": hx(pk), "lat": hx(lat), "lp": lp_json(lp), "ff": ff, "cut": cut}


def step_cases() -> tuple[list, list]:
    out, out_f = [], []
    inputs = step_inputs()
    for i, c in enumerate(inputs):
        key, axis, n, pk, lat, lp, ff, cut = c
        p = PROFILES[key]
        F: list = []
        rules._step_rules(F, make_step(axis, n, pk, lat, lp), p, ff, cut)
        d = step_json(c)
        d["out"] = [fd(f) for f in F]
        out.append(d)
        if n in (250, 0, 20, 199) and i % 10 == 0:  # _step_findings seul : sans le seuil de fenêtres ni la réserve
            F = []
            rules._step_findings(F, make_step(axis, n, pk, lat, lp), p, ff, cut)
            e = step_json(c)
            e["out"] = [fd(f) for f in F]
            out_f.append(e)
    return out, out_f


def main(path: str) -> None:
    step, step_f = step_cases()
    cases = {"constants": constants(), "gaps": gaps_cases(), "fr": fr_cases(), "rates": rates_cases(),
             "cli": cli_cases(), "loop_texts": loop_text_cases(), "imbalance": imbalance_cases(),
             "sorted": sorted_cases(), "step": step, "step_findings": step_f}
    with open(path, "w", encoding="utf-8") as f:
        json.dump(cases, f, ensure_ascii=False, separators=(",", ":"))
    print(" ".join(f"{k}={len(v)}" for k, v in cases.items() if k != "constants"), "->", path)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
