#!/usr/bin/env python3
"""Cas de test de la raie fixe et du notch dynamique de `rules.py` pour `rust/otb-advisor/src/rules.rs` (phase 5a-3c).

Usage : rules_notch_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python`)

Porté en 5a-3c : `frame_resonance`, `line_follows_motors`, `_frame_resonance`, `dyn_notch_view` et `_dyn_notch_q_n_rules` (puis `_filter_rules`,
qui les assemble : couvert par la couche corpus de `otb-parity check-advisor`, 630 sessions, et par `rules_state_cases.py`). Ici, des appels
directs : chaque cas appelle la fonction de Python et garde sa sortie (nombre, constat, dict, exception) ; `rust/otb-advisor/tests/rules_notch.rs`
rejoue l'appel en Rust. Les couvertures de branches de Python sont mesurées par `coverage run --branch` sur ce script.

Journaux fabriqués :
- `sweep` : vol de 6 s à 400 Hz dont les gaz balayent 10–90 % trois fois (`frame_resonance` range chaque fenêtre par gaz : il faut au moins
  8 tranches de plus de 3 fenêtres), moteurs à `mbase` (44 Hz) × (0,7 + 0,6 × gaz), raie fixe et/ou harmoniques moteur ajoutées ;
- `short_log` / `lined` de `rules_rpm_cases` (2 s à 1 kHz, moteurs à ~43 Hz) pour le notch dynamique et le notch statique.
Les tableaux inutiles aux fonctions (termes PID, consigne…) sont mis à zéro pour que le fichier ne les répète pas.

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception. Les nombres issus de FFT sont comparés avec une tolérance
mesurée côté Rust ; les textes au caractère près.
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, enc_log, ref
from rules_rpm_cases import enc_f, lined, raw_with, sl, tiny
from rules_state_cases import build, fd, hxl, with_

from opentunebox.advisor import rules as RU
from opentunebox.advisor.profiles import PROFILES
from opentunebox.advisor.rules import Finding
from opentunebox.analysis import spectral

DIRECT: list = []
SEEN: dict[str, int] = {}
NAN = float("nan")
INF = float("inf")
SFS = 400.0


# ---------------------------------------------------------------- journaux

def sweep(seconds=6.0, mbase=44.0, line=None, harm=None, seed=0, passes=3, thr_range=(10.0, 90.0), unfilt=True, nan_gyro=False):
    """Vol dont les gaz balayent `thr_range` `passes` fois. `line` = (fréquence, amplitude) : raie fixe (gyro et gyro non filtré, roll et
    pitch) ; `harm` = {harmonique: amplitude} : raies qui suivent les moteurs (non filtré, un dixième dans le filtré)."""
    lg = build(fs=SFS, seconds=seconds, motor_hz=mbase, seed=seed)
    n, t = lg.n, lg.t
    lo, hi = thr_range
    thr = (lo + hi) / 2 + (hi - lo) / 2 * np.sin(np.linspace(0, passes * 2 * np.pi, n))
    mh = mbase * (0.7 + 0.6 * thr / 100.0)
    phase = 2 * np.pi * np.cumsum(mh) / SFS
    r = np.random.default_rng(seed)
    g, u = r.normal(0, 0.05, (3, n)), r.normal(0, 0.2, (3, n))
    for ax in (0, 1):
        for h, a in (harm or {}).items():
            u[ax] += a * np.sin(h * phase)
            g[ax] += 0.1 * a * np.sin(h * phase)
        if line:
            u[ax] += line[1] * np.sin(2 * np.pi * line[0] * t)
            g[ax] += line[1] * np.sin(2 * np.pi * line[0] * t)
    if nan_gyro:
        g[:] = np.nan
        u[:] = np.nan
    mot = np.vstack([mh + r.normal(0, 1.0, n) for _ in range(4)])
    z = np.zeros_like
    return with_(lg, throttle=thr.astype(np.float32), motor_hz=mot.astype(np.float32), gyro=g.astype(np.float32),
                 gyro_unfilt=u.astype(np.float32) if unfilt else None, setpoint=z(lg.setpoint), rc_command=z(lg.rc_command),
                 p_term=z(lg.p_term), i_term=z(lg.i_term), d_term=z(lg.d_term), f_term=z(lg.f_term), motors=z(lg.motors))


def slim(lg):
    """Même journal sans les tableaux inutiles aux fonctions de ce fichier (remplacés par des zéros, répétés une seule fois)."""
    z = np.zeros_like
    return with_(lg, setpoint=z(lg.setpoint), rc_command=z(lg.rc_command), p_term=z(lg.p_term), i_term=z(lg.i_term), d_term=z(lg.d_term),
                 f_term=z(lg.f_term), motors=z(lg.motors))


def short_log(seconds=2.0):
    """`rules_rpm_cases.base_log` plus court (2 s) et sans les tableaux inutiles : bruit seul, moteurs vers 43 Hz à 1 kHz."""
    lg = build(fs=1000.0, seconds=seconds, motor_hz=40.0)
    mhz = 40.0 * (0.7 + 0.6 * lg.throttle.astype(np.float64) / 100.0)
    ph = 2 * np.pi * np.cumsum(mhz) / lg.fs
    u = lg.gyro_unfilt.astype(np.float64) - 6.0 * 0.05 * np.sin(2 * ph)
    return slim(with_(lg, gyro_unfilt=u.astype(np.float32)))


def maps_of(lg, sel):
    """Ce que `frame_resonance` donne à `line_follows_motors` : carte moyenne des deux axes, fréquences, tranches mesurées (roll puis pitch)."""
    x = lg.gyro_unfilt if lg.gyro_unfilt is not None else lg.gyro
    maps, ok = [], None
    for a in range(2):
        ft = spectral.freq_throttle(x[a, sel], lg.throttle[sel], lg.fs, mode="amp", breaks=lg.breaks(sel))
        ok = ft.counts > 3
        maps.append(ft.z)
    return np.mean(maps, axis=0), ft.freqs, ok


# ---------------------------------------------------------------- encodage

def enc_rec(lg, sel):
    return {"log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
            "sel": None if sel is None else list(sel)}


def enc_out(kind, out):
    if kind == "frame":
        return {"frame": None if out is None else [hx(out[0]), hx(out[1]), bool(out[2])]}
    if kind == "dn":
        if out is None:
            return {"dn": None}
        return {"dn": {"count": int(out["count"]), "min_hz": int(out["min_hz"]), "max_hz": int(out["max_hz"]), "peaks": hxl(out["peaks"]),
                       "widths": hxl(out["widths"]), "heights": hxl(out["heights"]), "left": hxl(out["left"]),
                       "q_need": enc_f(out["q_need"])}}
    if kind == "num":
        return {"num": enc_f(out)}
    if kind == "finding":
        if out is not None:
            SEEN[out.key] = SEEN.get(out.key, 0) + 1
        return {"finding": None if out is None else fd(out)}
    if kind == "findings":
        for f in out:
            SEEN[f.key] = SEEN.get(f.key, 0) + 1
        return {"findings": [fd(f) for f in out]}
    raise ValueError(kind)


def direct(label, call, kind, lg, args, fn, sel=None):
    """Un appel direct : `fn()` rend la valeur à encoder selon `kind` ; une exception de Python est la valeur attendue."""
    rec = {"label": label, "call": call, "args": args, **enc_rec(lg, sel)}
    try:
        out = fn()
    except Exception as e:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        rec |= {"error": True, "out": None}
        print(f"  [{call}] {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
        DIRECT.append(rec)
        return None
    rec |= {"error": False, "out": enc_out(kind, out)}
    if kind in ("finding", "findings"):
        shown = ", ".join(f.key for f in ([out] if isinstance(out, Finding) else (out or [])))
    elif kind == "dn" and out is not None:
        shown = f"peaks {np.round(out['peaks'], 1).tolist()} q_need {out['q_need']}"
    elif kind == "frame" and out is not None:
        shown = f"f0 {out[0]:.1f} excès {out[1]:.2f} moteur {out[2]}"
    else:
        shown = str(out)
    print(f"  [{call}] {label} : {shown}", file=sys.stderr)
    DIRECT.append(rec)
    return out


# ---------------------------------------------------------------- appels

def frame(label, lg, sel=None, lo_hz=20.0):
    return direct(label, "frame", "frame", lg, {"lo_hz": hx(lo_hz)}, lambda: RU.frame_resonance(lg, sl(lg, sel), lo_hz), sel)


def follows(label, lg, f0, z, f, ok, sel=None):
    """`z` : carte fréquence × tranche (à plat, `len(f)` lignes) ; `ok` : tranches mesurées."""
    a = {"f0": hx(f0), "z": ref(np.ravel(z)), "f": ref(f), "ok": [bool(v) for v in ok]}
    return direct(label, "follows", "num", lg, a, lambda: RU.line_follows_motors(lg, sl(lg, sel), f0, np.asarray(z).reshape(len(f), -1), f, ok),
                  sel)


def fres(label, lg, res, profile="five", sel=None):
    a = {"res": None if res is None else [hx(res[0]), hx(res[1]), bool(res[2])], "profile": profile}
    return direct(label, "fres", "finding", lg, a,
                  lambda: RU._frame_resonance(lg, res, PROFILES[profile], None if sel is None else slice(*sel)), sel)


def dnview(label, lg, fixed_hz=None, sel=None):
    return direct(label, "dnview", "dn", lg, {"fixed_hz": enc_f(fixed_hz)}, lambda: RU.dyn_notch_view(lg, sl(lg, sel), fixed_hz), sel)


def dn_of(peaks, widths, heights, left, q_need, count=3, min_hz=100, max_hz=600):
    return {"count": count, "min_hz": min_hz, "max_hz": max_hz, "peaks": list(peaks), "widths": list(widths), "heights": list(heights),
            "left": list(left), "q_need": q_need}


def qn(label, lg, dn, lines_pass=False, gyro_noisy=False, dterm_noisy=False, present=(), hover=False):
    """`present` : constats déjà posés dans `F` ; chaque élément est une clé, ou (clé, réglages cités)."""
    fs_ = []
    for p in present:
        key, sets = (p, ()) if isinstance(p, str) else p
        fs_.append(Finding(key, "Filtres", RU.CONSEIL, "t", "d", ["a"], settings=tuple(sets)))
    a = {"dn": {"count": dn["count"], "min_hz": dn["min_hz"], "max_hz": dn["max_hz"], "peaks": hxl(dn["peaks"]), "widths": hxl(dn["widths"]),
                "heights": hxl(dn["heights"]), "left": hxl(dn["left"]), "q_need": enc_f(dn["q_need"])},
         "lines_pass": lines_pass, "gyro_noisy": gyro_noisy, "dterm_noisy": dterm_noisy, "hover": hover,
         "present": [[f.key, list(f.settings)] for f in fs_]}
    return direct(label, "qn", "findings", lg, a,
                  lambda: RU._dyn_notch_q_n_rules(lg.settings, dn, lines_pass, gyro_noisy, dterm_noisy, fs_, hover))


# ---------------------------------------------------------------- cas

def main():
    t0 = tiny()
    pid1k = {"pid_process_denom": "8"}

    # ---- frame_resonance
    s0 = slim(sweep())
    frame("bruit seul : pas de raie marquée (2e harmonique de la vitesse)", s0)
    fixed = slim(sweep(line=(110.0, 0.6)))
    frame("raie fixe à 110 Hz : excès élevé, pas une raie moteur", fixed)
    frame("... période explicite", fixed, sel=(200, 2300))
    frame("... période plus longue que le journal", fixed, sel=(600, 90000))
    frame("... lo_hz = 100 (raie dans la bande)", fixed, lo_hz=100.0)
    frame("... lo_hz = 120 (raie sous la bande)", fixed, lo_hz=120.0)
    frame("... lo_hz = 0", fixed, lo_hz=0.0)
    frame("... lo_hz = 140 : 13 fréquences, le lissage échoue (Python lève)", fixed, lo_hz=140.0)
    frame("... lo_hz = 145 : 11 fréquences (Python lève)", fixed, lo_hz=145.0)
    frame("... lo_hz = 149 : 10 fréquences, le lissage échoue (Python lève)", fixed, lo_hz=149.0)
    frame("... lo_hz = 152 : 9 fréquences seulement", fixed, lo_hz=152.0)
    frame("... lo_hz = 170 : moins de 10 fréquences", fixed, lo_hz=170.0)
    frame("raie à 26,7 Hz avec lo_hz = 22,5 : collée au bord bas (moins de 5 Hz), écartée", slim(sweep(line=(26.7, 0.6))), lo_hz=22.5)
    frame("... lo_hz = 500 : bande vide", fixed, lo_hz=500.0)
    frame("... sans gyro non filtré : le gyro filtré sert", with_(fixed, gyro_unfilt=None))
    frame("... sans eRPM : pas de test moteur", with_(fixed, motor_hz=None))
    frame("... armement coupé : vitesse de vol stationnaire inconnue", with_(fixed, mode_flags=np.zeros(fixed.n, np.int32)))
    frame("gaz constants : trop peu de tranches de gaz", slim(sweep(passes=0)))
    frame("gaz dans une plage étroite (30–38 %) : 8 tranches à peine", slim(sweep(thr_range=(30.0, 38.0))))
    frame("période vide (aucune fenêtre)", fixed, sel=(100, 150))
    harm1 = slim(sweep(mbase=38.0, harm={1: 1.0}))
    frame("fondamentale moteur à 43 Hz (vitesse 38 Hz) : le test des deux trajets la reconnaît", harm1)
    fix48 = slim(sweep(line=(48.0, 0.6)))
    frame("raie fixe à 47 Hz près de la fondamentale (44 Hz) : test des deux trajets, raie fixe", fix48)
    frame("raie fixe à 40 Hz, dans la marge de la fondamentale", slim(sweep(line=(40.0, 0.6))))
    frame("raie fixe à 88 Hz : 2e harmonique de la vitesse de vol stationnaire", slim(sweep(line=(88.0, 0.6))))
    frame("raie fixe à 132 Hz : 3e harmonique", slim(sweep(line=(132.0, 0.6))))
    frame("raie fixe à 100 Hz : ni fondamentale ni harmonique", slim(sweep(line=(100.0, 0.6))))
    frame("raie fixe à 93 Hz : 2e harmonique à 6 % près", slim(sweep(line=(93.0, 0.6))))
    frame("raie fixe à 96 Hz : juste hors de la marge de la 2e harmonique", slim(sweep(line=(96.0, 0.6))))
    nan_log = slim(sweep(nan_gyro=True))
    frame("bruit seul (graine 3) : raie près de la fondamentale, score entre -2 et 5 dB : raie moteur", slim(sweep(seed=3)))
    frame("gyro entièrement NaN : excès NaN", nan_log)
    frame("gyro NaN, sans eRPM", with_(nan_log, motor_hz=None))

    # ---- line_follows_motors
    for name, lg, f0 in (("raie moteur (fondamentale)", harm1, 43.3), ("raie fixe à 47 Hz", fix48, 46.7), ("bruit seul", s0, 43.0),
                         ("raie fixe à 110 Hz, fenêtre hors des moteurs", fixed, 110.0)):
        z, f, ok = maps_of(lg, slice(0, lg.n))
        follows(f"{name} (f0 = {f0:g})", lg, f0, z, f, ok)
    z, f, ok = maps_of(harm1, slice(0, harm1.n))
    follows("... sans eRPM : NaN", with_(harm1, motor_hz=None), 43.3, z, f, ok)
    follows("... f0 = 20 : fenêtre sous les moteurs", harm1, 20.0, z, f, ok)
    follows("... f0 = 30", harm1, 30.0, z, f, ok)
    follows("... f0 = 60", harm1, 60.0, z, f, ok)
    follows("... f0 = 1e5 : fenêtre vide (Python lève)", harm1, 1e5, z, f, ok)
    follows("... f0 = 1e5 sans tranche utile", harm1, 1e5, z, f, np.zeros_like(ok))
    ok2 = np.zeros_like(ok)
    ok2[[45, 55]] = True
    follows("... deux tranches seulement (45 et 55 %) : NaN", harm1, 60.0, z, f, ok2)
    ok3 = np.zeros_like(ok)
    ok3[[45, 55, 65]] = True
    follows("... trois tranches (45, 55 et 65 %)", harm1, 60.0, z, f, ok3)
    fast_log = with_(harm1, motor_hz=(harm1.motor_hz * 8).astype(np.float32))
    ff = np.linspace(0.0, 1000.0, 300)
    zf = np.abs(np.random.default_rng(5).normal(1.0, 0.3, (300, z.shape[1])))
    follows("... moteurs rapides (×8), f0 = 283,6 : écart minimal de 4 % de f0 (11,3 Hz) supérieur à 3 cases (10 Hz)", fast_log, 283.6, zf, ff, ok)
    follows("... toutes les tranches y compris vides", harm1, 60.0, z, f, np.ones_like(ok))
    follows("... période explicite", harm1, 60.0, z, f, ok, sel=(600, 2000))
    follows("... période vide", harm1, 60.0, z, f, ok, sel=(100, 100))
    sq = with_(harm1, throttle=np.where(np.arange(harm1.n) % 2 == 0, 0.0, 100.0).astype(np.float32))
    follows("... gaz en créneaux 0 / 100 : tranches mesurées sans échantillon (c3 = 0)", sq, 60.0, z, f, ok)
    thr_odd = harm1.throttle.copy()
    thr_odd[::7] = np.nan
    thr_odd[3::11] = 250.0
    thr_odd[5::13] = -20.0
    follows("... gaz NaN, > 100 % et < 0 : tranches 0 et dernière", with_(harm1, throttle=thr_odd), 60.0, z, f, ok)
    zn = z.copy()
    zn[:, np.flatnonzero(ok)[5]] = np.nan
    follows("... une tranche de la carte NaN : moyenne NaN", harm1, 60.0, zn, f, ok)
    follows("... carte avec des zéros (plancher 1e-9)", harm1, 60.0, np.where(z > np.median(z), z, 0.0), f, ok)
    for nf in (1, 10, 14, 15, 16):
        ff = np.linspace(0, 3.3 * (nf - 1), nf) if nf > 1 else np.array([0.0])
        zz = np.abs(np.random.default_rng(nf).normal(1.0, 0.3, (nf, z.shape[1])))
        follows(f"... {nf} fréquences seulement", harm1, 30.0, zz, ff, ok)

    # ---- _frame_resonance
    q = short_log()
    found = lined(q, lines=[(0, 105.0, 0.5, 0.4), (1, 105.0, 0.5, 0.4)])
    fres("pas de résultat : rien", q, None)
    fres("excès sous le seuil du profil (6,8 dB)", q, (300.0, 6.7, False))
    fres("excès égal au seuil : constat", q, (300.0, 6.8, False))
    fres("raie moteur : rien", q, (300.0, 30.0, True))
    fres("excès NaN : pas de comparaison vraie, constat", q, (300.0, NAN, False))
    fres("excès -inf : rien", q, (300.0, -INF, False))
    fres("excès infini", q, (300.0, INF, False))
    fres("raie à 300 Hz, notch dynamique en service, dans la plage", q, (300.0, 12.4, False))
    fres("... profil 7 pouces (seuil 8,2) : excès 8,0 sous le seuil", q, (300.0, 8.0, False), profile="seven")
    fres("... profil 7 pouces : excès 8,3", q, (300.0, 8.3, False), profile="seven")
    fres("raie à 300 Hz, période explicite", q, (300.0, 12.4, False), sel=(300, 2500))
    fres("raie hors plage du notch (750 Hz)", q, (750.0, 12.4, False))
    fres("raie sous la plage du notch (60 Hz) : mise en garde", q, (60.0, 12.4, False))
    fres("raie à 99,6 Hz", q, (99.6, 12.4, False))
    fres("raie à 100 Hz pile", q, (100.0, 12.4, False))
    fres("notch hors service : mise en service", raw_with(q, dyn_notch_count="0"), (300.0, 12.4, False))
    fres("boucle 1 kHz, raie à 300 Hz : notch statique, coupure de repli (sel absent)", raw_with(q, **pid1k), (300.0, 12.4, False))
    fres("boucle 1 kHz, raie mesurée à 105 Hz : coupure calculée (sel absent)", raw_with(found, **pid1k), (105.0, 12.4, False))
    fres("... période explicite", raw_with(found, **pid1k), (105.0, 12.4, False), sel=(300, 1500))
    half = with_(found, gyro_unfilt=np.concatenate([found.gyro_unfilt[:, :500], q.gyro_unfilt[:, 500:]], axis=1),
                 gyro=np.concatenate([found.gyro[:, :500], q.gyro[:, 500:]], axis=1))  # la raie n'existe que dans les 500 premières ms
    fres("... raie présente un quart du temps seulement, période entière", raw_with(half, **pid1k), (105.0, 12.4, False))
    fres("... même journal, période de la raie : coupure plus basse", raw_with(half, **pid1k), (105.0, 12.4, False), sel=(0, 500))
    fres("boucle 1 kHz, raie à 60 Hz : jamais de statique", raw_with(found, **pid1k), (60.0, 12.4, False))
    fres("7 notchs en service, raie à 105 Hz : statique", raw_with(found, dyn_notch_count="7"), (105.0, 12.4, False))
    fres("raie au-delà de la portée (990 Hz) : statique", raw_with(found, dyn_notch_count="3"), (990.0, 12.4, False))
    fres("sans gyro non filtré : statique sans mesure de la raie", with_(raw_with(found, **pid1k), gyro_unfilt=None), (105.0, 12.4, False))
    fres("sans eRPM : statique sans mesure de la raie", with_(raw_with(found, **pid1k), motor_hz=None), (105.0, 12.4, False))
    fres("raie NaN avec notch en service : comparaisons fausses", q, (NAN, 12.4, False))
    fres("raie infinie, notch hors service (int() lève)", raw_with(q, dyn_notch_count="0"), (INF, 12.4, False))

    # ---- dyn_notch_view
    dq = q
    dnview("bruit seul", dq)
    two = lined(q, lines=[(0, 105.0, 0.5, 0.4), (1, 230.0, 0.5, 0.4)])
    dnview("deux raies, une par axe : le plus de pics l'emporte", two)
    pitch = lined(q, lines=[(1, 105.0, 0.5, 0.4), (1, 230.0, 0.5, 0.4)])
    dnview("deux raies sur le pitch seulement : le pitch l'emporte", pitch)
    roll = lined(q, lines=[(0, 105.0, 0.5, 0.4), (0, 230.0, 0.5, 0.4)])
    dnview("deux raies sur le roll seulement : le roll l'emporte", roll)
    both = lined(q, lines=[(0, 105.0, 0.5, 0.4), (1, 105.0, 0.5, 0.4)])
    dnview("la même raie sur les deux axes : égalité, le roll", both)
    dnview("raie fixe signalée, loin des pics : ajoutée", both, fixed_hz=300.0)
    dnview("raie fixe signalée, sur un pic (à 37 Hz près) : pas ajoutée", both, fixed_hz=120.0)
    pk = RU.dyn_notch_view(both, slice(0, both.n))["peaks"][0]
    x = pk + 37.0
    while abs(x - pk) != 37.0:  # exactement 37 Hz d'écart (l'arrondi de la somme peut en donner un peu plus ou un peu moins)
        x = np.nextafter(x, pk + 37.0 + (1 if abs(x - pk) < 37.0 else -1))
    dnview("raie fixe signalée à exactement 37 Hz d'un pic : pas ajoutée (strictement plus de 37)", both, fixed_hz=float(x))
    dnview("raie fixe signalée à 37,1 Hz d'un pic : ajoutée", both, fixed_hz=pk + 37.1)
    dnview("bruit seul et raie fixe signalée : seule", dq, fixed_hz=300.0)
    dnview("raie fixe en tête (triée avant les pics)", both, fixed_hz=70.0)
    dnview("période explicite", two, sel=(300, 2500))
    dnview("période vide", two, sel=(100, 150))
    dnview("notch hors service (dyn_notch_count 0)", raw_with(two, dyn_notch_count="0"))
    dnview("notch hors service (dyn_notch_max_hz 0)", raw_with(two, dyn_notch_max_hz="0"))
    dnview("boucle 1 kHz : notch inutilisable", raw_with(two, **pid1k))
    dnview("boucle 2 kHz pile", raw_with(two, looptime="500"))
    dnview("sans gyro non filtré", with_(two, gyro_unfilt=None))
    dnview("sans eRPM", with_(two, motor_hz=None))
    dnview("rpm_filter_harmonics 0 : sans filtre RPM", raw_with(two, rpm_filter_harmonics="0"))
    dnview("rpm_filter_harmonics 1", raw_with(two, rpm_filter_harmonics="1"))
    dnview("plage basse (min 40 Hz : bande dès 20 Hz)", raw_with(two, dyn_notch_min_hz="40"))
    dnview("plage étroite autour d'une raie", raw_with(two, dyn_notch_min_hz="200", dyn_notch_max_hz="260"))
    for hz in (420.0, 430.0):
        top = lined(q, lines=[(0, hz, 0.5, 0.4), (1, hz, 0.5, 0.4)])
        dnview(f"raie à {hz:g} Hz près de la borne haute de la bande (max 400 + 37 Hz)", raw_with(top, dyn_notch_max_hz="400"))
    edge = lined(q, lines=[(0, 386.75, 0.6, 0.5), (1, 386.75, 0.6, 0.5)])
    dnview("raie dans l'avant-dernière case de la bande (max 360 + 37 Hz) : une case de plus qu'avec 359", raw_with(edge, dyn_notch_max_hz="360"))
    dnview("plage au-delà de Nyquist (max 900)", raw_with(two, dyn_notch_max_hz="900"))
    dnview("plage vide (min > max)", raw_with(two, dyn_notch_min_hz="500", dyn_notch_max_hz="150"))
    dnview("5 notchs", raw_with(two, dyn_notch_count="5"))
    harm = lined(q, lines=[(0, 105.0, 0.5, 0.0), (1, 105.0, 0.5, 0.0)], harm_unf=(1.0, 1.0, 1.0))
    dnview("harmoniques moteur fortes et raie effacée par les filtres", harm)
    dnview("... raie fixe signalée", harm, fixed_hz=200.0)
    strong = lined(q, lines=[(0, 150.0, 0.5, 0.4)], harm_unf=(1.0, 1.0, 1.0))
    dnview("raie masquée par les harmoniques (largeur NaN)", raw_with(strong, rpm_filter_harmonics="1"))
    veryh = lined(q, lines=[(0, 150.0, 0.5, 0.4)], harm_unf=(2.0, 2.0, 2.0))
    dnview("... harmoniques très fortes", raw_with(veryh, rpm_filter_harmonics="3"))
    dnview("... sans raie à suivre mais fixe signalée sur une harmonique", raw_with(veryh, rpm_filter_harmonics="3"), fixed_hz=86.0)
    fast = slim(build(fs=1000.0, seconds=2.0, motor_hz=250.0))
    dnview("moteurs rapides : bande entièrement masquée", fast)

    # ---- _dyn_notch_q_n_rules
    # Q à baisser : raie large et haute qui passe encore les filtres
    low1 = dn_of([105.0, 230.0], [30.0, 12.0], [14.0, 9.0], [8.0, 5.0], 262.0)
    qn("Q trop haut : calcul 262 → 250, une raie visible", t0, low1)
    qn("... Q de départ 300 (défaut) et raie visible seule en vol stationnaire", t0, low1, hover=True)
    low2 = dn_of([105.0], [60.0], [20.0], [10.0], 140.0)
    qn("Q sous le plancher (140 < 250) : « plus bas Q utilisé », cause mécanique", t0, low2)
    qn("... avec une raie fixe signalée (lines_pass) : la baisse reste", t0, low2, lines_pass=True, gyro_noisy=True, dterm_noisy=True)
    qn("... largeur NaN : forme absente du texte", t0, dn_of([105.0, 230.0], [NAN, 12.0], [14.0, 9.0], [8.0, 5.0], 262.0))
    qn("... hauteur NaN : forme absente du texte", t0, dn_of([105.0, 230.0], [30.0, 12.0], [NAN, 9.0], [8.0, 5.0], 262.0))
    qn("... aucune forme finie, raie visible", t0, dn_of([105.0], [NAN], [NAN], [8.0], 262.0))
    qn("... hauteur filtrée exactement au seuil (4 dB) : pas visible, on monte", t0, dn_of([105.0], [30.0], [14.0], [4.0], 262.0))
    qn("... hauteur filtrée NaN : pas visible", t0, dn_of([105.0], [30.0], [14.0], [NAN], 262.0))
    qn("... hauteurs filtrées négatives : formats signés", t0, dn_of([105.0, 230.0, 300.0], [30.0, 12.0, 9.0], [14.0, 9.0, 7.0], [-0.4, 4.6, 5.4], 262.0))
    qn("... hauteur à -0,3 dB et +0,4 dB (« -0 » / « +0 »)", t0, dn_of([105.0, 230.0], [30.0, 12.0], [-0.3, 0.4], [4.5, 4.5], 262.0))
    qn("... Q calculé 250 pile : arrondi au plancher, pas sous le Q actuel (300)", t0, dn_of([105.0], [30.0], [14.0], [8.0], 250.0))
    qn("... Q calculé 299 : arrondi 250 < 300", t0, dn_of([105.0], [30.0], [14.0], [8.0], 299.0))
    qn("... Q calculé 300 : pas sous le Q actuel", t0, dn_of([105.0], [30.0], [14.0], [8.0], 300.0))
    qn("... Q calculé 349 (arrondi 300)", t0, dn_of([105.0], [30.0], [14.0], [8.0], 349.0))
    qn("... Q actuel 450, calcul 349 : arrondi 300", raw_with(t0, dyn_notch_q="450"), dn_of([105.0], [30.0], [14.0], [8.0], 349.0))
    qn("... Q actuel 250, calcul 100 : plancher 250 pas sous 250", raw_with(t0, dyn_notch_q="250"), dn_of([105.0], [30.0], [14.0], [8.0], 100.0))
    # Q à monter
    ok1 = dn_of([105.0, 230.0], [30.0, 12.0], [14.0, 9.0], [1.0, 0.0], 1467.0)
    qn("Q peut monter : calcul 1467 → 500 (borne), rien ne passe", t0, ok1)
    qn("... vol stationnaire", t0, ok1, hover=True)
    qn("... calcul infini (aucune raie n'en a besoin)", t0, dn_of([105.0], [30.0], [3.0], [1.0], INF))
    qn("... calcul infini, aucune forme (formes absentes)", t0, dn_of([105.0], [NAN], [NAN], [1.0], INF))
    qn("... calcul NaN : traité comme infini", t0, dn_of([105.0], [30.0], [14.0], [1.0], NAN))
    qn("... calcul 380 → 350", t0, dn_of([105.0], [30.0], [14.0], [1.0], 380.0))
    qn("... calcul 320 → 300 : pas plus haut que le Q actuel", t0, dn_of([105.0], [30.0], [14.0], [1.0], 320.0))
    qn("... calcul 250 : arrondi 250 sous le Q actuel, pas de hausse", t0, dn_of([105.0], [30.0], [14.0], [1.0], 250.0))
    qn("... aucune raie mesurable (q_need None) : petit pas", t0, dn_of([], [], [], [], None))
    qn("... aucune raie mesurable, Q 480 : plafonné à 500", raw_with(t0, dyn_notch_q="480"), dn_of([], [], [], [], None))
    qn("... raies sans forme mesurable, q_need None", t0, dn_of([105.0], [NAN], [NAN], [1.0], None))
    qn("... Q déjà à 500 : rien", raw_with(t0, dyn_notch_q="500"), ok1)
    qn("... Q à 550 : rien", raw_with(t0, dyn_notch_q="550"), ok1)
    qn("... Q à 450 : 500", raw_with(t0, dyn_notch_q="450"), ok1)
    qn("... une raie fixe passe : pas de hausse", t0, ok1, lines_pass=True)
    qn("... gyro bruité : pas de hausse", t0, ok1, gyro_noisy=True)
    qn("... D bruité : pas de hausse", t0, ok1, dterm_noisy=True)
    qn("... raie visible mais calcul sans baisse (q_need 400 > 300) : on monte", t0, dn_of([105.0], [30.0], [14.0], [8.0], 400.0))
    qn("... raie visible, calcul infini : on monte", t0, dn_of([105.0], [30.0], [14.0], [8.0], INF))
    qn("... baisse sans raie visible, calcul 262 : ni baisse ni hausse (262 → 250 sous Q)", t0, dn_of([105.0], [30.0], [14.0], [1.0], 262.0))
    # nombre de notchs
    many = dn_of([105.0, 150.0, 230.0, 300.0], [30.0, 12.0, 9.0, 9.0], [14.0, 9.0, 8.0, 8.0], [1.0, 1.0, 1.0, 1.0], 400.0, count=3)
    qn("plus de pics que de notchs, raie qui passe : dyn_notch_more", t0, many, lines_pass=True)
    qn("... gyro bruité seul", t0, many, gyro_noisy=True)
    qn("... D bruité seul : rien", t0, many, dterm_noisy=True)
    qn("... ni raie ni bruit : rien (Q monte seulement)", t0, many)
    qn("... réglage déjà conseillé (dyn_notch_count) : rien", t0, many, lines_pass=True, present=[("dyn_notch_count_advice", ["dyn_notch_count"])])
    qn("... autre réglage déjà conseillé : dyn_notch_more", t0, many, lines_pass=True, present=[("rpm_q", ["rpm_filter_q"])])
    qn("... constat sans réglage : dyn_notch_more", t0, many, lines_pass=True, present=["gyro_noisy"])
    nine = dn_of([60.0, 105.0, 150.0, 230.0, 300.0, 400.0, 500.0, 560.0, 590.0], [10.0] * 9, [9.0] * 9, [1.0] * 9, 400.0, count=3)
    qn("... 9 pics : plafonné à 7", t0, nine, lines_pass=True)
    qn("... autant de pics que de notchs : rien", t0, dn_of([105.0, 150.0, 230.0], [30.0, 12.0, 9.0], [14.0, 9.0, 8.0], [1.0] * 3, 400.0), lines_pass=True)
    qn("... plus de notchs que de pics : rien", raw_with(t0, dyn_notch_count="5"), many.copy() | {"count": 5, "peaks": [105.0], "widths": [30.0],
                                                                                              "heights": [14.0], "left": [1.0]}, lines_pass=True)
    qn("... baisse de Q et plus de notchs ensemble", t0, dn_of([105.0, 150.0, 230.0, 300.0], [30.0, 12.0, 9.0, 9.0], [14.0, 9.0, 8.0, 8.0],
                                                             [8.0, 1.0, 1.0, 1.0], 262.0, count=3), lines_pass=True, hover=True)

    out = {"cases": DIRECT, "inputs": INPUTS}
    text = json.dumps(out, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print(len(DIRECT), "appels directs,", sum(1 for c in DIRECT if c["error"]), "erreurs de Python,", f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)
    print("constats vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    want = ("frame_line_fixed", "dyn_notch_q_lower", "dyn_notch_q_raise", "dyn_notch_more")
    missing = [k for k in want if k not in SEEN]
    print("constats jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
