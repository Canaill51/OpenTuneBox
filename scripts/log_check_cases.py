#!/usr/bin/env python3
"""Cas de test de `advisor/log_check.py` sur des journaux fabriqués, pour `rust/otb-advisor/src/log_check.rs` (phase 5b-2).

Usage : log_check_cases.py SORTIE.json[.gz]    (à exécuter avec `.venv/bin/python -I`)

Complète la couche corpus (`otb-parity check-guide`) : des vols courts fabriqués (modèle de `rules_state_cases.py:build`) font atteindre les
branches que le corpus ne couvre pas. Chaque cas appelle l'étalon `check_log(log, sel, label, exact)` et garde **tout** : points (clé, titre,
état, texte, corrections), usages, parties du vol, fenêtres, curseurs, rôle, retards, et les vues `items_for` / `usages_for` /
`cli_commands` pour les deux rôles. Un flottant s'écrit en hexadécimal.

Familles de cas :
- enregistrement : version (ancienne, autre firmware, absente, 2025.12), durée, fréquence du log (trop basse, limite, repliée), gyro non
  filtré absent (4.4 et plus récent), mode de débogage, télémétrie RPM ;
- protocole : rates (ACTUAL conformes ou non, autre type, listes courtes), limite d'angle (absente, 30, autre, Acro), mode de vol (Angle,
  Acro feedforward à 0, Acro avec feedforward, modes absents), axes mêlés, curseur I, Dynamic Damping, estimation rapide et nombre exact
  de fenêtres ;
- général : réglage simplifié coupé, trous dans l'enregistrement, périodes (début, milieu, vide, hors du journal) ;
- parties du vol : stationnaire seul, mouvements seuls, les deux, vol trop court ;
- groupes de logs (`check_logs`) : chaque curseur en série, série Feedforward avec retards, vols identiques, hors série, noms et cartes ;
- repères de la phase Filtres (`filter_targets`, `FilterTargets.checks` / `reached`).
"""
import gzip
import json
import os
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))  # -I retire le dossier du script de sys.path
from bf_filters_cases import hx  # noqa: E402
from flight_stats_cases import INPUTS  # noqa: E402
from flight_stats_cases import enc_log as _enc_log  # noqa: E402
from rules_state_cases import build, rows, with_  # noqa: E402

from opentunebox.advisor import log_check as LC  # noqa: E402
from opentunebox.advisor.log_check import FilterTargets, check_log, check_logs, filter_targets  # noqa: E402
from opentunebox.analysis.period import flight_parts  # noqa: E402

# Colonnes que ni `check_log` ni `filter_targets` ne lisent (vérifié : leurs sorties sont identiques avec ces colonnes à zéro) : écrites en
# zéros, elles n'alourdissent pas le fichier (une même suite de zéros n'y figure qu'une fois)
UNREAD = tuple(x for x in os.environ.get("OTB_ZERO", "p_term,i_term,f_term,rc_command,debug,motors").split(",") if x)


def lite(lg):
    """Le journal avec les colonnes `UNREAD` à zéro : appliqué AVANT de calculer la réponse de Python, pour que la réponse et les données
    écrites dans le fichier correspondent."""
    z = {k: np.zeros_like(getattr(lg, k)) for k in UNREAD if getattr(lg, k, None) is not None}
    return with_(lg, **z)


enc_log = _enc_log


CASES: list = []
SEEN: dict[str, int] = {}
STATUS: dict[str, int] = {}


def enc_check(ck) -> dict:
    def item(it):
        return {"key": it.key, "title": it.title, "status": it.status, "detail": it.detail,
                "fixes": [[f.where, f.cli] for f in it.fixes]}

    def usage(u):
        return {"key": u.key, "label": u.label, "ok": bool(u.ok), "reason": u.reason, "shown": bool(u.shown)}

    part = lambda p: None if p is None else [hx(p[0]), hx(p[1])]  # noqa: E731
    return {"label": ck.label, "moving": bool(ck.moving), "hover": part(ck.parts["hover"]), "moving_part": part(ck.parts["moving"]),
            "items": [item(i) for i in ck.items], "usages": [usage(u) for u in ck.usages], "windows": list(ck.windows),
            "exact": bool(ck.exact), "sliders": [[k, hx(v)] for k, v in ck.sliders.items()], "role": ck.role,
            "sp_delay": [hx(v) for v in ck.sp_delay_ms],
            "items_for": {r: [i.key for i in ck.items_for(r)] for r in (None, LC.FILTERS, LC.PID)},
            "usages_for": {r: [u.key for u in ck.usages_for(r)] for r in (None, LC.FILTERS, LC.PID)},
            "cli": ck.cli_commands()}


def case(label, lg, sel=None, exact=False):
    lg = lite(lg)
    rec = {"label": label, "log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
           "sel": None if sel is None else list(sel), "exact": exact}
    try:
        ck = check_log(lg, None if sel is None else slice(*sel), label, exact)
    except Exception as e:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        rec |= {"error": True, "out": None}
        print(f"  {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
        CASES.append(rec)
        return None
    for it in ck.items:
        SEEN[it.key] = SEEN.get(it.key, 0) + 1
        STATUS[f"{it.key}:{it.status}"] = STATUS.get(f"{it.key}:{it.status}", 0) + 1
    for u in ck.usages:
        STATUS[f"usage {u.key}:{u.ok}"] = STATUS.get(f"usage {u.key}:{u.ok}", 0) + 1
    rec |= {"error": False, "out": enc_check(ck)}
    CASES.append(rec)
    return ck


def hover_then_moving(lg, hover_s):
    """Le même vol dont la consigne est nulle pendant les `hover_s` premières secondes."""
    k = int(hover_s * lg.fs)
    return rows(lg, "setpoint", lambda a: a.__setitem__((slice(None), slice(0, k)), 0.0))


def hover_only(lg):
    return rows(lg, "setpoint", lambda a: a.fill(0.0))


def with_gap(lg, at, lost):
    t = lg.t.copy()
    t[at:] += lost / lg.fs
    return with_(lg, t=t)


def main():
    out_path = sys.argv[1]
    rates_ok = dict(rates_type="3", rc_rates="20,20,20", rates="20,20,20", rc_expo="0,0,0")
    L = lambda **kw: build(fs=kw.pop("fs", 500.0), seconds=kw.pop("seconds", 4.0), looptime=kw.pop("looptime", "250"), **kw)  # noqa: E731

    # --- enregistrement
    case("base", L())
    case("base exact", L(), exact=True)
    for name, fw in (("4.3", "Betaflight 4.3.1 (abcd) STM32F7X2"), ("4.4", "Betaflight 4.4.3 (abcd) STM32F7X2"),
                     ("4.5", "Betaflight 4.5.1 (abcd) STM32F7X2"), ("2025.12", "Betaflight 2025.12.0 (abcd) STM32H743"),
                     ("emu", "EmuFlight 0.4.3 (abcd) STM32F7X2"), ("kiss", "Kiss 1.0"), ("sans", None), ("tordu", "Betaflight x (abcd)")):
        case(f"firmware {name}", L(**{"Firmware revision": fw}))
    case("4.4 sans gyro non filtré", L(unfilt=False, **{"Firmware revision": "Betaflight 4.4.3 (abcd) STM32F7X2"}))
    case("4.5 sans gyro non filtré", L(unfilt=False))
    case("sans rpm", L(rpm=False))
    for dm in ("0", "6", "16", "17", "3", "99"):
        case(f"debug_mode {dm}", L(debug_mode=dm))
    case("debug 4.4", L(debug_mode="6", **{"Firmware revision": "Betaflight 4.4.3 (abcd) STM32F7X2"}))
    case("debug sans notch", L(debug_mode="6", dyn_notch_count="0"))
    for seconds in (1.0, 4.0, 14.9, 15.0, 20.0):
        case(f"durée {seconds}", L(seconds=seconds, fs=100.0))
    for fs, loop, denom in ((250.0, "125", "1"), (400.0, "125", "1"), (760.0, "125", "1"), (800.0, "125", "1"), (1000.0, "125", "1"),
                            (2000.0, "125", "1"), (2000.0, "250", "1"), (500.0, "250", "2"),
                            (4000.0, "250", "2"), (333.0, "125", "1"), (1000.0, "500", "1")):
        case(f"fs {fs} looptime {loop} denom {denom}", build(fs=fs, seconds=0.5 if fs > 1500 else 1.5, looptime=loop, pid_process_denom=denom))
    case("harmonique repliée", build(fs=1000.0, seconds=1.5, looptime="125", motor_hz=420.0))
    case("harmonique repliée fs 2000", build(fs=2000.0, seconds=1.5, looptime="125", motor_hz=700.0))
    case("moteurs lents", build(fs=1000.0, seconds=1.5, looptime="125", motor_hz=60.0))


    # --- valeurs exactement sur un seuil (relecture des mutations : chaque comparaison `>=` / `>` doit être tranchée par un cas)
    thr_rate = 0.95 * 2000.0  # base_hz d'un 8 kHz = 2 kHz
    for fs in (thr_rate, float(np.nextafter(thr_rate, 0)), float(np.nextafter(thr_rate, 1e9))):
        case(f"fs au seuil du log des filtres {fs!r}", build(fs=fs, seconds=0.5, looptime="125"))
    for fs, name in ((0.95 * 500.0, "plancher PID"), (0.95 * 1000.0, "minimum PID")):
        for f in (fs, float(np.nextafter(fs, 0)), float(np.nextafter(fs, 1e9))):
            case(f"fs au seuil {name} {f!r}", build(fs=f, seconds=0.5, looptime="125"))
    case("durée 15 s pile", build(fs=100.0, seconds=15.01, looptime="250"))
    case("durée juste sous 15 s", build(fs=100.0, seconds=15.0, looptime="250"))

    def with_hz(lg, hz):
        return with_(lg, motor_hz=np.full(lg.motor_hz.shape, hz, np.float32))

    case("harmonique 3 pile à Nyquist", with_hz(build(fs=1902.0, seconds=0.5, looptime="125"), 317.0))
    case("harmonique 3 juste au-dessus de Nyquist", with_hz(build(fs=1902.0, seconds=0.5, looptime="125"), 317.5))
    target = 2000.0  # harm_hz de 8 kHz / 4
    fs105 = target / 1.05
    for cand in (fs105, float(np.nextafter(fs105, 0)), float(np.nextafter(fs105, 1e9))):
        for k in range(-3, 4):
            c2 = float(np.nextafter(cand, 0 if k < 0 else 1e9)) if k else cand
            if c2 * 1.05 == target:
                case(f"harm_hz pile à 1,05 fs {c2!r}", with_hz(build(fs=c2, seconds=0.5, looptime="125"), 330.0))
                break
    case("sans gyro non filtré mais log rapide", hover_only(build(fs=2000.0, seconds=1.5, looptime="125", unfilt=False)))
    case("sans gyro non filtré en 4.4, log rapide", hover_only(build(fs=2000.0, seconds=1.5, looptime="125", unfilt=False,
                                                                      **{"Firmware revision": "Betaflight 4.4.3 (abcd) STM32F7X2"})))
    short = lambda **kw: build(fs=250.0, seconds=3.0, looptime="250", **kw)  # noqa: E731
    for lvl in (29.9, 30.0, 30.5, 31.0, 40.0):
        case(f"consigne constante à {lvl}", rows(short(), "setpoint", lambda a, v=lvl: (a.fill(0.0), a.__setitem__((0, slice(None)), v))))
    case("angle_limit 0", L(seconds=3.0, flags=0b10, angle_limit="0"))
    case("F roll seul à 0", L(seconds=3.0, flags=0b1, ff_weight="0,125,0"))
    case("F pitch seul à 0", L(seconds=3.0, flags=0b1, ff_weight="120,0,0"))
    case("rates : centre 100", hover_then_moving(L(seconds=3.0, rates_type="3", rc_rates="10,10,10", rates="20,20,20", rc_expo="0,0,0"), 0.1))
    case("rates : maximum 100", hover_then_moving(L(seconds=3.0, rates_type="3", rc_rates="20,20,20", rates="10,10,10", rc_expo="0,0,0"), 0.1))
    case("trou de 3,5 trames", with_gap(L(seconds=3.0), 1000, 3.5))
    case("trou de 2,5 trames", with_gap(L(seconds=3.0), 1000, 2.5))
    case("trou de 4,5 trames", with_gap(L(seconds=3.0), 1000, 4.5))
    cr = build(fs=1000.0, seconds=2.0, looptime="125")
    for run in (59, 60, 61, 62):
        case(f"crash de {run} ms", rows(cr, "gyro", lambda a, r=run: a.__setitem__((1, slice(900, 900 + r)), 1901.0)))
    sq = rows(short(), "setpoint", lambda a: (a.fill(0.0), a.__setitem__((0, slice(None)), np.where(np.arange(a.shape[1]) % 2 == 0, 5.0, -5.0)),
                                              a.__setitem__((1, slice(None)), 100.0)))
    ck = case("écart-type de la consigne roll pile à 5", sq)
    assert ck is not None and np.isfinite(ck.sp_delay_ms[0]), ck and ck.sp_delay_ms
    mx = rows(short(), "setpoint", lambda a: (a.fill(0.0), a.__setitem__((0, slice(0, 450)), 100.0), a.__setitem__((1, slice(150, 750)), 100.0)))
    ck = case("axes mêlés : 0,4 pile", mx)
    print("  axes mêlés :", ck.item("axes_mixed").detail, file=sys.stderr)
    # nombre de fenêtres exactement sur les seuils (20 et 200), par recherche : consigne constante, journal coupé à n échantillons
    from opentunebox.analysis.step import input_windows
    big = L(seconds=40.0, fs=250.0)
    big = rows(big, "setpoint", lambda a: a.fill(100.0))

    def cut(lg, n):
        f = lambda x: None if x is None else x[..., :n]  # noqa: E731
        return with_(lg, t=lg.t[:n], gyro=f(lg.gyro), gyro_unfilt=f(lg.gyro_unfilt), setpoint=f(lg.setpoint), rc_command=f(lg.rc_command),
                     p_term=f(lg.p_term), i_term=f(lg.i_term), d_term=f(lg.d_term), f_term=f(lg.f_term), throttle=f(lg.throttle),
                     motors=f(lg.motors), motor_hz=f(lg.motor_hz), debug=f(lg.debug), mode_flags=f(lg.mode_flags))

    # la partie avec mouvements se compte en blocs de 1 s : N = 19 et 20 par la longueur du journal, N = 199 et 200 par une fréquence
    # d'enregistrement non entière (la longueur d'un bloc, en trames, ne change pas, le pas des fenêtres si) ; recherche confirmée par `check_log`
    for w in (19, 20):
        for n in range(780, 840, 1):
            ck = check_log(cut(big, n), None, "", False)
            if ck.windows[:1] == [w]:
                case(f"{w} fenêtres", cut(big, n))
                break
        else:
            raise SystemExit(f"pas de journal à {w} fenêtres")
    for w in (199, 200):
        done = False
        for fs_try in np.arange(200.0, 260.0, 0.05):
            L_try = int(round(fs_try))
            for nb in (14, 15):
                if input_windows(np.full(nb * L_try, 100.0), float(fs_try)) != w:
                    continue
                lg = rows(build(fs=float(fs_try), seconds=nb + 0.5, looptime="250"), "setpoint", lambda a: a.fill(100.0))
                ck = check_log(lg, None, "", False)
                if ck.windows[:2] == [w, w]:
                    case(f"{w} fenêtres", lg)
                    done = True
                    break
            if done:
                break
        if not done:
            raise SystemExit(f"pas de journal à {w} fenêtres")
    case("série lisible mais I trop haut", L(seconds=8.0, flags=0b10, angle_limit="30", **rates_ok))
    gv = hover_then_moving(L(seconds=20.0, fs=500.0), 6.0)
    case("trou dans la partie avec mouvements (exact)", with_gap(gv, 7000, 40), exact=True)
    case("trou dans la partie avec mouvements (rapide)", with_gap(gv, 7000, 40))

    # --- usages qui réussissent
    case("série lisible", L(seconds=8.0, flags=0b10, angle_limit="30", simplified_i_gain="25", **rates_ok), exact=True)
    case("série lisible (rapide)", L(seconds=8.0, flags=0b10, angle_limit="30", simplified_i_gain="25", **rates_ok))
    case("bruit lisible", hover_only(build(fs=2000.0, seconds=2.0, looptime="125")))
    case("bruit et retard lisibles", hover_then_moving(build(fs=2000.0, seconds=3.0, looptime="125"), 1.0))

    # --- roll et pitch aux nombres de fenêtres très différents : c'est le plus petit qui décide
    no_pitch = rows(L(seconds=6.0), "setpoint", lambda a: a.__setitem__((1, slice(None)), 0.0))
    ck = case("fenêtres : pitch sans consigne", no_pitch)
    assert ck.windows[0] > 20 > ck.windows[1], ck.windows
    case("fenêtres : pitch sans consigne (exact)", no_pitch, exact=True)

    # --- parties du vol
    hm = L(seconds=20.0, fs=500.0)
    case("stationnaire puis mouvements", hover_then_moving(hm, 10.0), exact=True)
    case("stationnaire puis mouvements (rapide)", hover_then_moving(hm, 10.0))
    case("mouvements seuls", hm)
    case("stationnaire seul", hover_only(hm))
    case("vol court en mouvements", L(seconds=3.0, fs=250.0))
    case("vol court stationnaire", hover_only(L(seconds=3.0, fs=250.0)))
    case("stationnaire puis mouvements courts", hover_then_moving(L(seconds=5.0), 3.0))
    case("mouvements puis stationnaire", rows(hm, "setpoint", lambda a: a.__setitem__((slice(None), slice(5000, None)), 0.0)))

    # --- protocole : rates
    rates = lambda t, rc, mx, ex: dict(rates_type=t, rc_rates=rc, rates=mx, rc_expo=ex)  # noqa: E731
    for name, kw in (("conformes", rates("3", "20,20,20", "20,20,20", "0,0,0")),
                     ("centre différent", rates("3", "20,25,20", "20,20,20", "0,0,0")),
                     ("maximum différent", rates("3", "20,20,20", "20,20,30", "0,0,0")),
                     ("expo", rates("3", "20,20,20", "20,20,20", "0,5,0")),
                     ("BETAFLIGHT", rates("0", "20,20,20", "20,20,20", "0,0,0")),
                     ("liste courte", rates("3", "20,20", "20,20,20", "0,0,0")),
                     ("liste longue", rates("3", "20,20,20,20", "20,20,20,20", "0,0,0,0")),
                     ("sans rates_type", dict(rates_type=None))):
        case(f"rates {name}", hover_then_moving(L(seconds=3.0, **kw), 0.1))
    # --- limite d'angle et mode de vol
    for name, flags, extra in (("Angle, limite 30", 0b10, dict(angle_limit="30")), ("Angle, limite 55", 0b10, dict(angle_limit="55")),
                               ("Angle, limite absente", 0b10, {}), ("Acro, F à 0", 0b1, dict(ff_weight="0,0,0")),
                               ("Acro, F actif", 0b1, {}), ("modes absents, F actif", None, {}),
                               ("modes absents, F à 0", None, dict(ff_weight="0,0,0")), ("Horizon", 0b100, dict(angle_limit="30")),
                               ("Angle + Horizon", 0b110, dict(angle_limit="30")), ("Acro, limite 30", 0b1, dict(angle_limit="30"))):
        case(f"mode {name}", L(seconds=3.0, flags=flags, **extra))
    # --- axes mêlés
    mixed = L(seconds=3.0)
    case("axes mêlés pitch coupé", rows(mixed, "setpoint", lambda a: a.__setitem__((1, slice(None)), 0.0)))
    case("axes mêlés pitch décalé", rows(mixed, "setpoint", lambda a: a.__setitem__((1, slice(None)), np.roll(a[0], 700))))
    # --- curseurs
    for name, kw in (("I à 25", dict(simplified_i_gain="25")), ("I à 100", dict(simplified_i_gain="100")), ("I à 10", dict(simplified_i_gain="10")),
                     ("I à 35", dict(simplified_i_gain="35")), ("I à 15", dict(simplified_i_gain="15")),
                     ("dmax 0", dict(simplified_dmax_gain="0")), ("dmax 5", dict(simplified_dmax_gain="5")),
                     ("dmax 6", dict(simplified_dmax_gain="6")), ("d_max_gain", dict(simplified_dmax_gain=None, simplified_d_max_gain="80")),
                     ("simplifié coupé", dict(simplified_pids_mode="0")), ("simplifié coupé, I", dict(simplified_pids_mode="0", simplified_i_gain="25"))):
        case(f"curseur {name}", L(seconds=3.0, **kw))
    # --- trous, périodes
    gap = L(seconds=5.0)
    case("trou court", with_gap(gap, 1000, 6))
    case("trou long", with_gap(gap, 1000, 80))
    case("deux trous", with_gap(with_gap(gap, 1000, 30), 2500, 30))
    n = gap.n
    case("période stop 0, vol court (Python lève)", L(seconds=3.0), sel=[0, 0])
    for name, sel in (("début", [0, n // 2]), ("milieu", [n // 4, 3 * n // 4]), ("vide", [10, 10]), ("hors du journal", [n + 5, n + 9]),
                      ("tout", [0, n]), ("stop 0", [0, 0]), ("un point", [5, 6]), ("fin", [n - 3, n]), ("dépasse", [100, n + 500])):
        case(f"période {name}", gap, sel=sel)
        case(f"période {name} exacte", gap, sel=sel, exact=True)

    # --- groupes de logs
    tiny = lambda **kw: build(fs=250.0, seconds=1.0, **kw)  # noqa: E731
    slider_keys = {"Damping": "simplified_d_gain", "Tracking": "simplified_pi_gain", "Stick Resp.": "simplified_feedforward_gain",
                   "Dynamic D.": "simplified_dmax_gain", "Drift-Wobble": "simplified_i_gain", "Pitch D.": "simplified_pitch_d_gain",
                   "Pitch T.": "simplified_pitch_pi_gain", "Master": "simplified_master_multiplier"}
    groups = []

    def group(label, logs, delays=None):
        logs = [(lab, lite(lg)) for lab, lg in logs]
        d = delays or {}
        out = check_logs(logs, d)
        groups.append({"label": label, "logs": [[lab, {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)}]
                                                for lab, lg in logs],
                       "delays": {k: [hx(v) for v in vs] for k, vs in d.items()},
                       "out": [{"craft": g.craft, "labels": list(g.labels), "status": g.status, "detail": g.detail} for g in out]})
        for g in out:
            SEEN[f"group:{g.status}:{g.detail.split(' ')[0]}"] = SEEN.get(f"group:{g.status}:{g.detail.split(' ')[0]}", 0) + 1
        print(f"  groupe {label} : {[(g.status, g.detail[:60]) for g in out]}", file=sys.stderr)

    for cat, key in slider_keys.items():
        vals = ("0", "50", "100") if cat == "Stick Resp." else ("60", "80", "100", "120")
        group(f"série {cat}", [(f"v{i}", tiny(**{key: v})) for i, v in enumerate(vals)])
    group("série Feedforward avec retards", [(f"f{i}", tiny(simplified_feedforward_gain=v)) for i, v in enumerate(("0", "50", "100"))],
          {"f0": [0.004, float("nan")], "f1": [0.005, 0.0051], "f2": [float("nan"), float("nan")]})
    group("série Feedforward sans retards", [(f"f{i}", tiny(simplified_feedforward_gain=v)) for i, v in enumerate(("0", "50", "100"))])
    group("série Feedforward, retards courts", [(f"f{i}", tiny(simplified_feedforward_gain=v)) for i, v in enumerate(("0", "50", "100"))],
          {"f0": [], "f1": [1.234], "f2": [1.0, 2.0, 3.0]})
    group("vols identiques", [("a", tiny()), ("b", tiny()), ("c", tiny())])
    group("hors série", [("a", tiny()), ("b", tiny(simplified_d_gain="60", simplified_i_gain="50"))])
    group("série + identiques + hors série", [("a", tiny(simplified_d_gain="60")), ("b", tiny(simplified_d_gain="80")),
                                             ("c", tiny(simplified_d_gain="80")), ("d", tiny(simplified_pi_gain="40", simplified_i_gain="50")),
                                             ("e", tiny(simplified_d_gain="100"))])
    group("filtres différents", [("a", tiny()), ("b", tiny(gyro_lpf1_dyn_hz="300,600"))])
    group("deux drones", [("a1", tiny(**{"Craft name": "A"})), ("a2", tiny(**{"Craft name": "A", "simplified_d_gain": "80"})),
                          ("b1", tiny(**{"Craft name": "B"})), ("b2", tiny(**{"Craft name": "B"})), ("c", tiny(**{"Craft name": "C"}))])
    group("sans nom", [("a", tiny(**{"Craft name": ""})), ("b", tiny(**{"Craft name": ""}))])
    group("sans nom ni carte", [("a", tiny(**{"Craft name": "", "Firmware revision": "Betaflight 4.5.1"})),
                                ("b", tiny(**{"Craft name": "", "Firmware revision": "Betaflight 4.5.1"}))])
    group("même nom, cartes différentes", [("a", tiny()), ("b", tiny(**{"Firmware revision": "Betaflight 4.5.1 (abcd1234) STM32H743"}))])
    group("un seul log", [("a", tiny())])
    group("aucun log", [])
    group("simplifié coupé", [("a", tiny(simplified_pids_mode="0")), ("b", tiny(simplified_pids_mode="0", rollPID="50,80,40"))])

    # --- repères de la phase Filtres
    targets = []

    def tgt(label, lg, parts, inch):
        lg = lite(lg)
        rec = {"label": label, "log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
               "hover": None if parts.get("hover") is None else [hx(parts["hover"][0]), hx(parts["hover"][1])],
               "moving": None if parts.get("moving") is None else [hx(parts["moving"][0]), hx(parts["moving"][1])],
               "inch": None if inch is None else hx(inch)}
        try:
            ft = filter_targets(lg, parts, inch)
        except Exception as e:  # noqa: BLE001
            rec |= {"error": True, "out": None}
            print(f"  {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
            targets.append(rec)
            return
        rec["error"] = False
        rec["out"] = None if ft is None else {
            "vals": [hx(ft.gyro_ptb_db), hx(ft.dterm_ptb_db), hx(ft.gyro_delay_ms), hx(ft.dterm_delay_ms)],
            "goal": None if ft.delay_goal_ms is None else [hx(ft.delay_goal_ms[0]), hx(ft.delay_goal_ms[1])],
            "checks": [[a, b, c] for a, b, c in ft.checks()], "reached": ft.reached()}
        targets.append(rec)

    vol = hover_then_moving(hm, 8.0)
    p = flight_parts(vol, float(vol.t[0]), float(vol.t[-1]))
    print("  parties du vol de repères :", p, file=sys.stderr)
    for inch in (None, 5.0, 3.0, 7.0, 2.5, 1.0, 0.0, 12.0):
        tgt(f"vol complet {inch}", vol, p, inch)
    tone = rows(vol, "gyro", lambda a: a.__setitem__((slice(0, 2), slice(0, 4000)),
                                                       a[0:2, 0:4000] + 40.0 * np.sin(2 * np.pi * 232.0 * np.arange(4000) / 500.0)))
    tgt("raie à 232 Hz dans la bande", tone, p, 5.0)
    tgt("sans stationnaire", vol, {"hover": None, "moving": p["moving"]}, 5.0)
    tgt("sans mouvements", vol, {"hover": p["hover"], "moving": None}, 5.0)
    tgt("rien", vol, {"hover": None, "moving": None}, 5.0)
    tgt("sans clé", vol, {}, 5.0)
    tgt("sans gyro non filtré", with_(vol, gyro_unfilt=None), p, 5.0)
    tgt("sans terme D", rows(vol, "d_term", lambda a: a.fill(0.0)), p, 5.0)
    tgt("terme D sur le seul pitch", rows(vol, "d_term", lambda a: a.__setitem__((0, slice(None)), 0.0)), p, 5.0)
    tgt("terme D sur le seul roll", rows(vol, "d_term", lambda a: a.__setitem__((1, slice(None)), 0.0)), p, 5.0)
    tgt("trou dans la partie stationnaire", with_gap(vol, 1500, 40), p, 5.0)
    tgt("bande vide (fs bas)", build(fs=120.0, seconds=10.0, looptime="500"), {"hover": (1.0, 8.0), "moving": None}, 5.0)
    tgt("partie hors du journal", vol, {"hover": (100.0, 120.0), "moving": None}, 5.0)

    checks = []
    nan, inf = float("nan"), float("inf")
    vals = (-45.0, -30.0, -29.99, -30.01, -10.0, -9.99, -10.01, 0.0, nan, inf, -inf)
    delays = (0.5, 1.0, 1.5, 2.0, 2.55, 2.5, 3.0, nan, inf, 0.0, -1.0)
    for g in vals:
        for d in vals:
            checks.append(FilterTargets(g, d, 0.7, 1.4, (1.0, 2.0)))
    for gd in delays:
        for dd in delays:
            for goal in ((1.0, 2.0), (0.8, 1.5), (3.0, 0.0), None):
                checks.append(FilterTargets(-35.0, -15.0, gd, dd, goal))
    out_checks = [{"vals": [hx(f.gyro_ptb_db), hx(f.dterm_ptb_db), hx(f.gyro_delay_ms), hx(f.dterm_delay_ms)],
                   "goal": None if f.delay_goal_ms is None else [hx(f.delay_goal_ms[0]), hx(f.delay_goal_ms[1])],
                   "checks": [[a, b, c] for a, b, c in f.checks()], "reached": f.reached()} for f in checks]

    rates_units = []
    for rt in (3, 0, None, 4):
        for rc in ("20,20,20", "20,20", "", "20,20,20,5", "a,b,c", "-5,0,5", "255,255,255"):
            for ex in ("0,0,0", "50,50", "0,0,0,0"):
                kw = {"rates_type": None if rt is None else str(rt), "rc_rates": rc, "rates": rc, "rc_expo": ex}
                s = build(fs=250.0, seconds=0.2, **kw).settings
                r = LC.log_rates(s)
                rates_units.append({"raw": {k: v for k, v in kw.items() if v is not None}, "out": r and [list(x) for x in r]})
    mixed_units = []
    ml = lite(L(seconds=4.0))
    for sel in ((0, ml.n), (0, 0), (10, 11), (ml.n // 2, ml.n)):
        for thr in (0.0, 20.0, 100.0, 1e9):
            mixed_units.append({"sel": list(sel), "thr": hx(thr), "out": hx(LC.axes_mixed_share(ml, slice(*sel), thr))})
    fix = LC.log_rates_fix()

    result = {"cases": CASES, "groups": groups, "targets": targets, "target_checks": out_checks, "rates": rates_units,
              "mixed": {"log": {**enc_log(ml), "raw": dict(ml.settings.raw), "columns": sorted(ml.pid_columns)}, "cases": mixed_units},
              "rates_fix": [fix.where, fix.cli], "inputs": INPUTS,
              "constants": {"configurator": LC.CONFIGURATOR_SLIDERS, "role_names": LC.ROLE_NAMES}}
    text = json.dumps(result, separators=(",", ":"))
    if out_path.endswith(".gz"):  # mtime=0 : même contenu, mêmes octets d'une régénération à l'autre
        with gzip.GzipFile(out_path, "wb", compresslevel=9, mtime=0) as fh:
            fh.write(text.encode())
    else:
        Path(out_path).write_text(text)
    print(len(CASES), "cas,", sum(1 for c in CASES if c["error"]), "erreurs de Python,", len(groups), "groupes,", len(targets), "repères,",
          f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)
    print("points vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    print("états vus :", dict(sorted(STATUS.items())), file=sys.stderr)
    required = ("version:ok", "version:bloquant", "duration:ok", "duration:à revoir", "log_rate:ok", "log_rate:bloquant", "log_rate:à revoir",
                "pid_log_rate:ok", "pid_log_rate:à revoir", "pid_log_rate:bloquant", "gyro_unfilt:ok", "gyro_unfilt:bloquant", "debug_mode:à revoir",
                "rpm:ok", "rpm:à revoir", "parts:info", "windows:ok", "windows:à revoir", "windows:bloquant", "rates:ok", "rates:à revoir",
                "angle_limit:ok", "angle_limit:à revoir", "angle_limit:info", "flight_mode:ok", "flight_mode:info", "flight_mode:à revoir",
                "axes_mixed:ok", "axes_mixed:à revoir", "i_slider:à revoir", "dyn_damping:à revoir", "simplified:à revoir", "gaps:info",
                "usage series:True", "usage series:False", "usage noise:True", "usage noise:False", "usage delay:True", "usage delay:False",
                "usage ff_series:True", "usage ff_series:False")
    missing = [k for k in required if k not in STATUS]
    print("jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
