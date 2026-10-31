#!/usr/bin/env python3
"""Cas de test des règles de passe-bas, de filtre RPM et de raies du vol stationnaire de `rules.py` pour
`rust/otb-advisor/src/rules.rs` (phase 5a-3b).

Usage : rules_rpm_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python`)

Porté en 5a-3b : `_lowpass_setup_rules`, `rpm_harmonic_levels`, `_rpm_weight_rules`, `_rpm_q_raise`, `_hover_noise` (et `meas["hover_lines"]`),
`_residual_line`, `_line_shape`, `static_notch_cutoff`, `static_notch_case`, `dyn_notch_active`, `_dyn_notch_line_actions`,
`_static_notch_action`, `_static_notch_text`, `_low_line_caveat`, `dyn_notch_q_need`, `_q_round`.

**Appels directs** : ces règles ne sont appelées que par `_filter_rules` (porté en 5a-3c, comparé sur le corpus par `otb-parity
check-advisor`) ; les appels directs atteignent les branches que le corpus n'atteint pas. Chaque cas appelle la fonction de Python et garde
sa sortie (`Finding`, liste, nombre, exception) ; `rust/otb-advisor/tests/rules_rpm.rs` rejoue l'appel en Rust.

Journaux fabriqués (`rules_state_cases.build`, 1 kHz, 3 s, moteurs à ~43 Hz) auxquels `lined` ajoute des raies sinusoïdales : dans le gyro
non filtré (`unf`) et filtré (`filt`), par axe, et à chaque harmonique moteur (`harm_unf`, `harm_filt`). Une raie dans le gyro non filtré
seul est « effacée » par les filtres, une raie dans le gyro filtré aussi est « encore visible ». Les cas ne servant qu'aux réglages
utilisent un journal minuscule (même en-tête, 0,5 s).

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception. Les nombres issus de FFT (niveaux, raies) sont comparés
avec une tolérance mesurée côté Rust ; les textes au caractère près.
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, enc_log
from rules_state_cases import build, fd, hxl, with_

from opentunebox.advisor import rules as RU
from opentunebox.advisor.rules import Finding
from opentunebox.analysis.motors import SpectrumLine
from opentunebox.log.settings import parse_settings

DIRECT: list = []
SEEN: dict[str, int] = {}
FS = 1000.0
MOTOR_HZ = 40.0
NAN = float("nan")


# ---------------------------------------------------------------- journaux

def raw_with(lg, **kw):
    """Même vol avec d'autres réglages d'en-tête (`None` retire la clé)."""
    raw = {**lg.settings.raw, **kw}
    return with_(lg, settings=parse_settings({k: v for k, v in raw.items() if v is not None}))


def base_log(**header):
    """Vol de 3 s à 1 kHz, moteurs vers 43 Hz : la 2e harmonique que `build` ajoute au gyro non filtré est retirée (chaque harmonique est
    posée par `lined`), les niveaux de départ sont ceux du bruit seul."""
    lg = build(fs=FS, seconds=3.0, motor_hz=MOTOR_HZ, **header)
    mhz = MOTOR_HZ * (0.7 + 0.6 * lg.throttle.astype(np.float64) / 100.0)
    ph = 2 * np.pi * np.cumsum(mhz) / lg.fs
    u = lg.gyro_unfilt.astype(np.float64) - 6.0 * 0.05 * np.sin(2 * ph)
    return with_(lg, gyro_unfilt=u.astype(np.float32))


def lined(lg, lines=(), harm_unf=(0.0, 0.0, 0.0), harm_filt=(0.0, 0.0, 0.0)):
    """Ajoute des raies : `lines` = (axe, fréquence, amplitude dans le non filtré, amplitude dans le filtré) ; `harm_*` = amplitude de la
    1re, 2e, 3e harmonique moteur (roll et pitch), dans le non filtré / le filtré."""
    t = lg.t
    mhz = MOTOR_HZ * (0.7 + 0.6 * lg.throttle.astype(np.float64) / 100.0)
    ph = 2 * np.pi * np.cumsum(mhz) / lg.fs
    g = lg.gyro.astype(np.float64).copy()
    u = lg.gyro_unfilt.astype(np.float64).copy()
    for ax, f, au, af in lines:
        u[ax] += au * np.sin(2 * np.pi * f * t)
        g[ax] += af * np.sin(2 * np.pi * f * t)
    for h, (au, af) in enumerate(zip(harm_unf, harm_filt), start=1):
        for ax in (0, 1):
            u[ax] += au * np.sin(h * ph)
            g[ax] += af * np.sin(h * ph)
    return with_(lg, gyro=g.astype(np.float32), gyro_unfilt=u.astype(np.float32))


def tiny(**header):
    """Journal minuscule : seuls les réglages de l'en-tête comptent."""
    return build(fs=FS, seconds=0.5, motor_hz=MOTOR_HZ, **header)


# ---------------------------------------------------------------- encodage

def enc_rec(lg, sel):
    return {"log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
            "sel": None if sel is None else list(sel)}


def enc_f(x):
    return None if x is None else hx(x)


def enc_out(kind, out):
    if kind == "findings":
        for f in out:
            SEEN[f.key] = SEEN.get(f.key, 0) + 1
        return {"findings": [fd(f) for f in out]}
    if kind == "finding":
        if out is not None:
            SEEN[out.key] = SEEN.get(out.key, 0) + 1
        return {"finding": None if out is None else fd(out)}
    if kind == "levels":
        return {"raw": hxl(out[0]), "left": hxl(out[1])}
    if kind == "hover":
        findings, lines = out
        enc = enc_out("findings", findings)
        enc["lines"] = [hxl([ln.freq_hz, ln.raw_db, ln.left_db]) for ln in lines]
        return enc
    if kind == "cutoff":
        return {"cutoff": [int(out[0]), bool(out[1])]}
    if kind == "num":
        return {"num": enc_f(out)}
    if kind == "int":
        return {"int": int(out)}
    if kind == "str":
        return {"str": out}
    if kind == "opt":
        return {"opt": out}
    if kind == "actions":
        return {"strs": list(out[0]), "settings": list(out[1])}
    if kind == "strs":
        return {"strs": list(out)}
    if kind == "shape":
        return {"shape": hxl(out)}
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
    keys = ""
    if kind in ("findings", "finding"):
        keys = ", ".join(f.key for f in ([out] if isinstance(out, Finding) else (out or [])))
    elif kind == "hover":
        keys = ", ".join(f.key for f in out[0]) + f"  [{len(out[1])} raies]"
    elif kind == "levels":
        keys = f"raw {np.round(out[0], 1).tolist()} left {np.round(out[1], 1).tolist()}"
    else:
        keys = str(out)
    print(f"  [{call}] {label} : {keys}", file=sys.stderr)
    DIRECT.append(rec)
    return out


def sl(lg, sel):
    return slice(0, lg.n) if sel is None else slice(*sel)


# ---------------------------------------------------------------- appels

def lowpass(label, lg):
    direct(label, "lowpass", "findings", lg, {}, lambda: RU._lowpass_setup_rules(lg.settings))


def levels(label, lg, sel=None):
    direct(label, "levels", "levels", lg, {}, lambda: RU.rpm_harmonic_levels(lg, sl(lg, sel)), sel)


def weights(label, lg, lighten=True, h1=False, sel=None):
    direct(label, "weights", "findings", lg, {"lighten": lighten, "h1": h1},
           lambda: RU._rpm_weight_rules(lg, sl(lg, sel), lg.settings, lighten, h1), sel)


def qraise(label, lg, present=(), hover=False, sel=None):
    """`present` : constats déjà posés dans `F` (clés)."""
    fs_ = [Finding(k, "Filtres", RU.CONSEIL, "t", "d", ["a"]) for k in present]
    direct(label, "qraise", "finding", lg, {"present": list(present), "hover": hover},
           lambda: RU._rpm_q_raise(lg, sl(lg, sel), lg.settings, fs_, hover), sel)


def hover(label, lg, g_ptb=-40.0, d_ptb=-30.0, fixed_hz=None, aliased=False, sel=None):
    def run():
        meas: dict = {}
        out = RU._hover_noise(lg, sl(lg, sel), None, g_ptb, d_ptb, fixed_hz, meas, aliased)
        return out, meas["hover_lines"]
    return direct(label, "hover", "hover", lg, {"g_ptb": hx(g_ptb), "d_ptb": hx(d_ptb), "fixed_hz": enc_f(fixed_hz), "aliased": aliased},
                  run, sel)


def cutoff(label, f0, w, h, lg):
    direct(label, "cutoff", "cutoff", lg, {"f0": hx(f0), "w": hx(w), "h": hx(h)}, lambda: RU.static_notch_cutoff(f0, w, h))


def qneed(label, peaks, widths, heights, lg):
    direct(label, "qneed", "num", lg, {"peaks": hxl(peaks), "widths": hxl(widths), "heights": hxl(heights)},
           lambda: RU.dyn_notch_q_need(peaks, widths, heights))


def qround(label, q, low, lg):
    direct(label, "qround", "int", lg, {"q": hx(q), "low": low}, lambda: RU._q_round(q, low))


def ncase(label, lg, f0):
    direct(label, "ncase", "opt", lg, {"f0": hx(f0)}, lambda: RU.static_notch_case(lg.settings, f0))


def nactive(label, lg):
    out = RU.dyn_notch_active(lg.settings)
    DIRECT.append({"label": label, "call": "nactive", "args": {}, **enc_rec(lg, None), "error": False, "out": {"bool": bool(out)}})
    print(f"  [nactive] {label} : {out}", file=sys.stderr)


def nactions(label, lg, f0, fixed):
    direct(label, "nactions", "actions", lg, {"f0": hx(f0), "fixed": fixed}, lambda: RU._dyn_notch_line_actions(lg.settings, f0, fixed))


def naction(label, lg, f0, cut, lead="Cas exceptionnel,"):
    direct(label, "naction", "strs", lg, {"f0": hx(f0), "cutoff": [cut[0], cut[1]], "lead": lead},
           lambda: RU._static_notch_action(lg.settings, f0, cut, lead))


def ntext(label, f0, cut, lg):
    direct(label, "ntext", "str", lg, {"f0": hx(f0), "cutoff": [cut[0], cut[1]]}, lambda: RU._static_notch_text(f0, cut))


def caveat(label, f0, lg):
    direct(label, "caveat", "str", lg, {"f0": hx(f0)}, lambda: RU._low_line_caveat(f0))


def residual(label, lg, ln, cut=None):
    direct(label, "residual", "finding", lg,
           {"line": hxl([ln.freq_hz, ln.raw_db, ln.left_db]), "cutoff": None if cut is None else [cut[0], cut[1]]},
           lambda: RU._residual_line(lg.settings, ln, cut))


def shape(label, lg, f0, sel=None):
    direct(label, "shape", "shape", lg, {"f0": hx(f0)}, lambda: RU._line_shape(lg, sl(lg, sel), f0), sel)


# ---------------------------------------------------------------- cas

def main():
    q = base_log()  # bruit seul, 4 kHz de boucle... (en-tête de BASE_HEADER : boucle PID à 8 kHz)
    t0 = tiny()
    pid2k = {"looptime": "500"}  # boucle PID à 2 kHz : notch dynamique possible
    pid1k = {"pid_process_denom": "8"}  # 1 kHz : pas de notch dynamique

    # ---- _lowpass_setup_rules (réglages seuls)
    lowpass("en-tête de base : gyro_lpf2 actif, D d'ordre 2 : rien", t0)
    lowpass("pid 8 kHz == gyro 8 kHz : pas de LPF2 requis", raw_with(t0, gyro_lpf2_static_hz="0"))
    lowpass("pid 2 kHz < gyro 8 kHz, LPF2 coupé : ATTENTION (lent)", raw_with(t0, gyro_lpf2_static_hz="0", pid_process_denom="4"))
    lowpass("pid 4 kHz < gyro 8 kHz, LPF2 coupé : CONSEIL (rapide)", raw_with(t0, gyro_lpf2_static_hz="0", pid_process_denom="2"))
    lowpass("pid 2,7 kHz (denom 3) : lent", raw_with(t0, gyro_lpf2_static_hz="0", pid_process_denom="3"))
    lowpass("pid < gyro mais LPF2 actif : rien", raw_with(t0, pid_process_denom="2"))
    lowpass("D : aucun passe-bas", raw_with(t0, dterm_lpf1_static_hz="0", dterm_lpf1_dyn_hz="0,0", dterm_lpf2_static_hz="0"))
    lowpass("D : un seul PT1 (LPF2 coupé), LPF1 dynamique", raw_with(t0, dterm_lpf2_static_hz="0"))
    lowpass("D : un seul PT1 statique (LPF2 coupé)", raw_with(t0, dterm_lpf2_static_hz="0", dterm_lpf1_dyn_hz="0,0"))
    lowpass("D : un seul PT2 (LPF1 PT2 dynamique, LPF2 coupé) : ordre 2, rien", raw_with(t0, dterm_lpf1_type="2", dterm_lpf2_static_hz="0"))
    lowpass("D : LPF1 BIQUAD seul : ordre 2", raw_with(t0, dterm_lpf1_type="1", dterm_lpf2_static_hz="0"))
    lowpass("D : LPF1 PT3 seul : ordre 3", raw_with(t0, dterm_lpf1_type="3", dterm_lpf2_static_hz="0"))
    lowpass("D : LPF1 statique 90 Hz, LPF2 PT1 : statique conseillé dynamique", raw_with(t0, dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="90"))
    lowpass("D : LPF1 statique et LPF2 coupé : weak + statique", raw_with(t0, dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="90",
                                                                          dterm_lpf2_static_hz="0"))
    lowpass("D : LPF1 dynamique avec static_hz non nul : pas de constat statique", raw_with(t0, dterm_lpf1_static_hz="90"))
    lowpass("gyro LPF1 BIQUAD actif", raw_with(t0, gyro_lpf1_type="1", gyro_lpf1_dyn_hz="250,500"))
    lowpass("gyro LPF2 BIQUAD actif", raw_with(t0, gyro_lpf2_type="1"))
    lowpass("gyro LPF1 et LPF2 BIQUAD", raw_with(t0, gyro_lpf1_type="1", gyro_lpf1_static_hz="300", gyro_lpf1_dyn_hz="0,0", gyro_lpf2_type="1"))
    lowpass("gyro LPF1 BIQUAD mais coupé : rien", raw_with(t0, gyro_lpf1_type="1", gyro_lpf1_static_hz="0", gyro_lpf1_dyn_hz="0,0"))
    lowpass("tout : LPF2 coupé lent, D faible, statique, deux biquads",
            raw_with(t0, gyro_lpf2_static_hz="0", pid_process_denom="4", dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="90",
                     dterm_lpf2_static_hz="0", gyro_lpf1_type="1", gyro_lpf1_static_hz="300", gyro_lpf1_dyn_hz="0,0"))

    # ---- rpm_harmonic_levels
    levels("bruit seul", q)
    levels("raies dans les trois harmoniques", lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.05, 0.05, 0.05)))
    levels("période explicite", lined(q, harm_unf=(0.3, 0.3, 0.3)), sel=(300, 2500))
    levels("période hors du journal (plus longue)", q, sel=(300, 5000))
    levels("sans gyro non filtré : Python lève", with_(q, gyro_unfilt=None))
    levels("sans eRPM : Python lève", with_(q, motor_hz=None))
    fast = build(fs=FS, seconds=3.0, motor_hz=250.0)
    levels("moteurs rapides : harmoniques au-delà de Nyquist (NaN)", fast)
    levels("période vide (aucune fenêtre)", q, sel=(100, 150))

    # ---- _rpm_weight_rules
    weights("bruit seul : poids allégés (2e et 3e sans raie)", q)
    weights("bruit seul, lighten=False : rien", q, lighten=False)
    weights("bruit seul, version 4.4 : pas de poids RPM", raw_with(q, **{"Firmware revision": "Betaflight 4.4.2 (abcd1234) STM32F7X2"}))
    weights("bruit seul, version absente : allègement permis", raw_with(q, **{"Firmware revision": None}))
    weights("bruit seul, 4.5.0 : allègement permis", raw_with(q, **{"Firmware revision": "Betaflight 4.5.0 (abcd1234) STM32F7X2"}))
    weights("bruit seul, 2025.12 : allègement permis", raw_with(q, **{"Firmware revision": "Betaflight 2025.12.0 (abcd1234) STM32N6"}))
    mid = lined(q, harm_unf=(0.0, 0.04, 0.03))
    weights("2e et 3e un peu au-dessus du bruit avant filtre : « ne dépasse que de X dB »", mid)
    weights("... poids 100,90,90 : gain juste suffisant (0,5)", raw_with(mid, rpm_filter_weights="100,90,90"))
    weights("... poids 100,89,89 : gain juste insuffisant", raw_with(mid, rpm_filter_weights="100,89,89"))
    weights("... poids 100,70,70 : gain trop faible", raw_with(mid, rpm_filter_weights="100,70,70"))
    weights("... poids 100,100,90 : seule la 3e est allégée ou non", raw_with(mid, rpm_filter_weights="100,100,90"))
    weights("... poids 100 seul (complétés à trois)", raw_with(mid, rpm_filter_weights="100"))
    weights("... poids 100,100,100,100,100 (longueur 5)", raw_with(mid, rpm_filter_weights="100,100,100,100,100"))
    weights("... une seule harmonique filtrée (n=1) : rpm_harmonic_missing, pas d'allègement", raw_with(mid, rpm_filter_harmonics="1"))
    weights("... deux harmoniques filtrées (n=2)", raw_with(mid, rpm_filter_harmonics="2"))
    weights("... aucune harmonique filtrée (n=0)", raw_with(mid, rpm_filter_harmonics="0"))
    strong = lined(q, harm_unf=(0.3, 0.3, 0.3))
    weights("raies fortes avant filtre, effacées après : rien à alléger", strong)
    low = lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.0, 0.05, 0.05))
    weights("2e et 3e visibles après filtre, poids 100 : rpm_line_visible", low)
    weights("... et 1re déjà signalée (h1_flagged), la 1re n'est pas dans le lot", lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.4, 0.05, 0.05)), h1=True)
    weights("... 1re visible, h1_flagged faux : dans le lot", lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.4, 0.05, 0.05)))
    weights("... 1re seule visible et signalée : aucun constat de raie", lined(q, harm_unf=(0.3, 0.0, 0.0), harm_filt=(0.4, 0.0, 0.0)), h1=True)
    weights("poids bas 100,50,100 + 2e visible : rpm_weight_low", raw_with(low, rpm_filter_weights="100,50,100"))
    weights("poids bas 50,50,50 : low pour les trois visibles", raw_with(lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.4, 0.05, 0.05)),
                                                                          rpm_filter_weights="50,50,50"))
    weights("poids 60,100,100 + 1re et 2e visibles : low + visible", raw_with(lined(q, harm_unf=(0.3, 0.3, 0.0), harm_filt=(0.5, 0.5, 0.0)),
                                                                              rpm_filter_weights="60,100,100"))
    weights("rpm_q 250 : q_new plafonné à 250", raw_with(low, rpm_filter_q="250"))
    weights("rpm_q 300 : q_new 250", raw_with(low, rpm_filter_q="300"))
    weights("rpm_q 800 : q_new 700", raw_with(low, rpm_filter_q="800"))
    weights("n=1, 2e et 3e visibles : harmoniques manquantes 2 et 3", raw_with(low, rpm_filter_harmonics="1"))
    weights("n=2, 3e visible : harmonique manquante 3", raw_with(lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.0, 0.0, 0.05)),
                                                                  rpm_filter_harmonics="2"))
    weights("n=0, tout visible : manquantes 1, 2 et 3", raw_with(lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.4, 0.05, 0.05)),
                                                                  rpm_filter_harmonics="0"))
    weights("n=3 et poids bas + visible + allègement pas permis (4.4)", raw_with(low, rpm_filter_weights="100,50,100",
                                                                                **{"Firmware revision": "Betaflight 4.4.2 (abcd1234) STM32F7X2"}))
    weights("moteurs rapides : niveaux NaN (rien)", fast)
    weights("période vide : niveaux NaN", q, sel=(100, 150))
    weights("sans gyro non filtré : Python lève", with_(q, gyro_unfilt=None))
    weights("période explicite", mid, sel=(300, 2500))

    # ---- _rpm_q_raise
    qraise("bruit seul : tout effacé, Q peut monter", q)
    qraise("... vol stationnaire", q, hover=True)
    qraise("... rpm_weight_lighten déjà posé : un réglage à la fois", q, present=["rpm_weight_lighten"])
    qraise("... lighten et vol stationnaire", q, present=["rpm_weight_lighten"], hover=True)
    qraise("... un autre constat présent", q, present=["gyro_noisy", "rpm_weight_low"])
    qraise("rpm_q 950 : plafonné à 1000", raw_with(q, rpm_filter_q="950"))
    qraise("rpm_q 1000 : reste 1000", raw_with(q, rpm_filter_q="1000"))
    qraise("rpm_q 250", raw_with(q, rpm_filter_q="250"))
    qraise("une harmonique visible après filtre : rien", lined(q, harm_unf=(0.3, 0.3, 0.3), harm_filt=(0.0, 0.05, 0.0)))
    qraise("n=1 : une seule harmonique traitée, effacée", raw_with(q, rpm_filter_harmonics="1"))
    qraise("n=1, 1re visible : rien", raw_with(lined(q, harm_unf=(0.3, 0.0, 0.0), harm_filt=(0.4, 0.0, 0.0)), rpm_filter_harmonics="1"))
    qraise("n=1, 2e visible mais non traitée : Q peut monter", raw_with(lined(q, harm_unf=(0.0, 0.3, 0.0), harm_filt=(0.0, 0.05, 0.0)),
                                                                       rpm_filter_harmonics="1"))
    qraise("n=0 : rien de traité", raw_with(q, rpm_filter_harmonics="0"))
    qraise("moteurs rapides : 1re visible, 2e et 3e NaN", fast)
    qraise("période vide : niveaux NaN, rien de traité", q, sel=(100, 150))
    qraise("période explicite", q, sel=(300, 2500))
    qraise("sans gyro non filtré : Python lève", with_(q, gyro_unfilt=None))
    qraise("sans eRPM : Python lève", with_(q, motor_hz=None))

    # ---- dyn_notch_q_need, _q_round, static_notch_cutoff (fonctions pures)
    qneed("aucune raie", [], [], [], t0)
    qneed("largeur nulle ou NaN : ignorées", [200.0, 300.0, 400.0], [0.0, NAN, -3.0], [10.0, 10.0, 10.0], t0)
    qneed("hauteur NaN ou infinie", [200.0, 300.0], [10.0, 10.0], [NAN, float("inf")], t0)
    qneed("largeur infinie", [200.0], [float("inf")], [10.0], t0)
    qneed("hauteur sous le seuil : infini", [200.0], [10.0], [3.0], t0)
    qneed("hauteur égale au seuil : infini", [200.0], [10.0], [4.0], t0)
    qneed("une raie mesurée", [200.0], [10.0], [10.0], t0)
    qneed("deux raies, la plus exigeante gagne (la 2e)", [200.0, 300.0], [10.0, 5.0], [10.0, 20.0], t0)
    qneed("deux raies, la plus exigeante gagne (la 1re)", [200.0, 300.0], [4.0, 20.0], [20.0, 8.0], t0)
    qneed("une raie infinie et une mesurée", [200.0, 300.0], [10.0, 10.0], [2.0, 9.0], t0)
    qneed("hauteur à peine au-dessus du seuil", [200.0], [10.0], [4.0001], t0)
    qneed("hauteur très grande", [200.0], [10.0], [60.0], t0)
    qneed("raies du même Q (premier gardé)", [200.0, 200.0], [10.0, 10.0], [10.0, 10.0], t0)
    qneed("raie à fréquence nulle", [0.0], [10.0], [10.0], t0)
    qneed("longueurs différentes (zip)", [200.0, 300.0, 400.0], [10.0], [10.0, 12.0], t0)
    for qv in (0.0, 100.0, 249.9, 250.0, 299.99, 300.0, 449.0, 450.0, 1e4, -10.0, 1234.5, 49.999999999999, 50.0):
        qround(f"_q_round({qv}, 250)", qv, 250, t0)
        qround(f"_q_round({qv}, 100)", qv, 100, t0)
    qround("_q_round(inf, 250) : NaN → plancher", float("inf"), 250, t0)
    qround("_q_round(nan, 250) : plancher", NAN, 250, t0)
    qround("_q_round(-inf, 100)", float("-inf"), 100, t0)
    qround("_q_round(1e17, 250)", 1e17, 250, t0)
    cutoff("raie mesurée moyenne", 300.0, 10.0, 12.0, t0)
    cutoff("raie large et haute : Q au plancher 100", 300.0, 200.0, 40.0, t0)
    cutoff("raie fine et haute : Q au plafond 450", 300.0, 1.0, 12.0, t0)
    cutoff("raie fine et très basse : Q énorme, plafonné", 300.0, 0.01, 4.5, t0)
    cutoff("hauteur sous le seuil : repli 0,7", 300.0, 10.0, 3.0, t0)
    cutoff("largeur NaN : repli", 300.0, NAN, 12.0, t0)
    cutoff("largeur nulle : repli", 300.0, 0.0, 12.0, t0)
    cutoff("hauteur NaN : repli", 300.0, 10.0, NAN, t0)
    cutoff("repli, f0 = 143", 143.0, NAN, NAN, t0)
    cutoff("f0 NaN avec raie mesurée : Python lève", NAN, 10.0, 12.0, t0)
    cutoff("f0 NaN sans mesure : Python lève", NAN, NAN, NAN, t0)
    cutoff("f0 = 100, Q moyen", 100.0, 7.0, 9.0, t0)
    cutoff("f0 = 800, Q moyen", 800.0, 25.0, 15.0, t0)

    # ---- static_notch_case / dyn_notch_active
    ncase("sous 100 Hz : jamais", t0, 99.9)
    ncase("sous 100 Hz même avec boucle lente", raw_with(t0, **pid1k), 60.0)
    ncase("100 Hz pile, boucle rapide, notch libre : rien", t0, 100.0)
    ncase("boucle PID sous 2 kHz", raw_with(t0, **pid1k), 300.0)
    ncase("boucle PID à 2 kHz pile : rien", raw_with(t0, **pid2k), 300.0)
    ncase("raie à 980 Hz : 1000 pas dépassé", t0, 980.0)
    ncase("raie à 981 Hz : au-delà de la portée", t0, 981.0)
    ncase("raie à 1500 Hz", t0, 1500.0)
    ncase("boucle lente et raie lointaine : la boucle d'abord", raw_with(t0, **pid1k), 990.0)
    ncase("7 notchs en service", raw_with(t0, dyn_notch_count="7"), 300.0)
    ncase("8 notchs (au-delà de la borne) en service", raw_with(t0, dyn_notch_count="8"), 300.0)
    ncase("6 notchs en service : rien", raw_with(t0, dyn_notch_count="6"), 300.0)
    ncase("7 notchs mais plage max 0 : pas en service", raw_with(t0, dyn_notch_count="7", dyn_notch_max_hz="0"), 300.0)
    ncase("f0 NaN : aucune condition vraie", t0, NAN)
    nactive("notch dynamique de l'en-tête de base", t0)
    nactive("count 0", raw_with(t0, dyn_notch_count="0"))
    nactive("max_hz 0", raw_with(t0, dyn_notch_max_hz="0"))
    nactive("boucle 1 kHz", raw_with(t0, **pid1k))
    nactive("boucle 2 kHz pile", raw_with(t0, **pid2k))
    nactive("boucle à 1996 Hz (looptime 501)", raw_with(t0, looptime="501", pid_process_denom="1"))

    # ---- _dyn_notch_line_actions
    off = raw_with(t0, dyn_notch_count="0")
    nactions("notch hors service, boucle 1 kHz : rien", raw_with(off, **pid1k), 300.0, False)
    nactions("notch hors service : mise en service", off, 300.0, False)
    nactions("... raie basse : plancher de la plage", off, 15.0, False)
    nactions("... raie à 30 Hz : plafond au moins 200", off, 30.0, True)
    nactions("... raie à 985 Hz : plafond 1000", off, 985.0, False)
    nactions("... raie à 100 Hz", off, 100.0, False)
    nactions("... raie à 100,5 Hz", off, 100.5, False)
    nactions("... raie à 59,99 Hz", off, 59.99, False)
    nactions("... raie négative", off, -35.0, False)
    nactions("... raie NaN : Python lève", off, NAN, False)
    nactions("... raie infinie : Python lève", off, float("inf"), False)
    nactions("... max_hz 0 : hors service", raw_with(t0, dyn_notch_max_hz="0"), 400.0, False)
    nactions("en service, raie dans la plage, raie fixe", t0, 300.0, True)
    nactions("en service, raie dans la plage, passe quand même", t0, 300.0, False)
    nactions("... count 7 : reste 7", raw_with(t0, dyn_notch_count="7"), 300.0, False)
    nactions("... count 6 : monte à 7", raw_with(t0, dyn_notch_count="6"), 300.0, False)
    nactions("raie au bord bas (f0 - 20 == min) : dans la plage", t0, 120.0, False)
    nactions("raie juste sous le bord bas", t0, 119.99, False)
    nactions("raie au bord haut (f0 + 20 == max) : dans la plage", t0, 580.0, False)
    nactions("raie juste au-dessus du bord haut", t0, 580.01, False)
    nactions("raie sous la plage", t0, 60.0, False)
    nactions("raie sous la plage, fixe", t0, 60.0, True)
    nactions("raie au-dessus de la plage", t0, 800.0, False)
    nactions("raie hors plage des deux côtés (plage étroite)", raw_with(t0, dyn_notch_min_hz="300", dyn_notch_max_hz="320"), 310.0, False)
    nactions("raie sous 20 + 20 Hz : plancher 20", t0, 25.0, False)
    nactions("raie hors plage près de 1000 : plafond 1000", raw_with(t0, dyn_notch_max_hz="900"), 995.0, False)
    nactions("plage déjà plus large que la raie en bas", raw_with(t0, dyn_notch_min_hz="50"), 40.0, False)
    nactions("plage large : min_hz haut, la raie est dessous", raw_with(t0, dyn_notch_min_hz="500", dyn_notch_max_hz="900"), 200.0, False)
    nactions("en service, f0 NaN : aucune comparaison vraie, dans la plage", t0, NAN, False)
    nactions("en service, f0 NaN fixe", t0, NAN, True)
    nactions("en service, f0 infinie : hors plage, int() lève", t0, float("inf"), False)
    nactions("boucle 2 kHz pile : en service", raw_with(t0, **pid2k), 300.0, False)

    # ---- _static_notch_text, _static_notch_action, _low_line_caveat
    ntext("coupure calculée", 300.0, (212, True), t0)
    ntext("coupure de repli", 143.7, (100, False), t0)
    ntext("f0 .5 : arrondi pair", 100.5, (70, False), t0)
    ntext("f0 .5 : arrondi pair (101,5)", 101.5, (70, False), t0)
    caveat("99,99 Hz", 99.99, t0)
    caveat("100 Hz", 100.0, t0)
    caveat("NaN : mise en garde", NAN, t0)
    caveat("20 Hz", 20.0, t0)
    caveat("1000 Hz", 1000.0, t0)
    naction("sous 100 Hz : rien", t0, 60.0, (42, False))
    naction("boucle lente : action", raw_with(t0, **pid1k), 300.0, (212, True))
    naction("boucle lente, repli", raw_with(t0, **pid1k), 300.0, (210, False), lead="Si elle persiste, cas exceptionnel :")
    naction("portée dépassée", t0, 990.0, (700, False))
    naction("7 notchs", raw_with(t0, dyn_notch_count="7"), 300.0, (212, True))
    naction("cas normal : rien", t0, 300.0, (212, True))

    # ---- _residual_line
    ln = SpectrumLine(300.0, 18.4, 9.6)
    residual("raie à 300 Hz, notch en service, dans la plage", t0, ln)
    residual("... boucle lente : notch statique, repli (cutoff None)", raw_with(t0, **pid1k), ln)
    residual("... boucle lente : notch statique, coupure mesurée", raw_with(t0, **pid1k), ln, (212, True))
    residual("... 7 notchs : statique", raw_with(t0, dyn_notch_count="7"), ln, (212, True))
    residual("... notch hors service : mise en service", off, ln)
    residual("... raie hors de la plage : plage élargie", t0, SpectrumLine(750.0, 12.2, 6.4))
    residual("raie à 60 Hz : mise en garde, pas de statique", raw_with(t0, **pid1k), SpectrumLine(60.0, 13.0, 11.0))
    residual("raie à 99,6 Hz arrondie à 100", t0, SpectrumLine(99.6, 7.5, 5.5))
    residual("raie à 990 Hz : au-delà de la portée", t0, SpectrumLine(990.0, 7.5, 5.5), (700, True))
    residual("raie à 990 Hz : au-delà de la portée, sans coupure", t0, SpectrumLine(990.0, 7.5, 5.5))
    residual("raie NaN : Python lève", t0, SpectrumLine(NAN, 7.5, 5.5))
    residual("niveau après filtres NaN", t0, SpectrumLine(300.0, 7.5, NAN))
    residual("raie à 100 Hz pile", raw_with(t0, **pid1k), SpectrumLine(100.0, 7.5, 5.5))

    # ---- _line_shape
    sh = lined(q, lines=[(0, 105.0, 0.3, 0.2), (1, 105.0, 0.5, 0.3)])
    shape("raie à 105 Hz sur roll et pitch", sh, 105.0)
    shape("raie à 105 Hz, période explicite", sh, 105.0, sel=(300, 2500))
    shape("raie à 105 Hz sur roll seulement", lined(q, lines=[(0, 105.0, 0.5, 0.3)]), 105.0)
    shape("raie absente (bruit seul)", q, 233.0)
    shape("sans gyro non filtré : NaN", with_(q, gyro_unfilt=None), 105.0)
    shape("sans eRPM : NaN", with_(q, motor_hz=None), 105.0)
    shape("rpm_harmonics 0", raw_with(sh, rpm_filter_harmonics="0"), 105.0)
    shape("sur une harmonique moteur (masquée)", lined(q, harm_unf=(0.5, 0.5, 0.5)), 86.0)
    shape("raie hors fréquences", sh, 5000.0)
    shape("période vide", sh, 105.0, sel=(100, 150))

    # ---- _hover_noise
    L = [(0, 60.0, 0.3, 0.2), (1, 105.0, 0.5, 0.4)]
    hv = lined(q, lines=L)
    hover("deux raies (roll 60 Hz, pitch 105 Hz), notch en service", hv)
    hover("... tri : la plus haute d'abord", lined(q, lines=[(0, 105.0, 0.5, 0.4), (1, 60.0, 0.3, 0.2)]))
    hover("... aliased=True : bilan seul", hv, aliased=True)
    hover("... d_ptb NaN", hv, d_ptb=NAN)
    hover("... g_ptb NaN", hv, g_ptb=NAN)
    hover("... raie fixe signalée à 105 Hz : écartée", hv, fixed_hz=105.0)
    hover("... raie fixe signalée à 110 Hz (10 % près de 105 Hz hors) : écartée", hv, fixed_hz=110.0)
    hover("... raie fixe à 120 Hz : pas assez près", hv, fixed_hz=120.0)
    hover("... raie fixe à 60 Hz seulement : l'autre reste", hv, fixed_hz=60.0)
    hover("... boucle 1 kHz : statique pour la raie > 100 Hz", raw_with(hv, **pid1k))
    hover("... boucle 2 kHz", raw_with(hv, **pid2k))
    hover("... notch hors service", raw_with(hv, dyn_notch_count="0"))
    hover("... 7 notchs en service : statique", raw_with(hv, dyn_notch_count="7"))
    hover("... rpm_harmonics 0", raw_with(hv, rpm_filter_harmonics="0"))
    hover("... période explicite", hv, sel=(300, 2500))
    hover("... sans gyro non filtré : Python lève", with_(hv, gyro_unfilt=None))
    hover("... sans eRPM : Python lève", with_(hv, motor_hz=None))
    hover("bruit seul : aucune raie", q)
    hover("bruit seul, armement coupé (moteurs non reconnus) : pas de bilan moteur", with_(q, mode_flags=np.zeros(q.n, np.int32)))
    hover("moteurs rapides : niveaux d'harmoniques NaN", fast)
    hover("période vide : aucune fenêtre, niveaux d'harmoniques NaN (aucune ligne « hauteur »)", q, sel=(100, 150))
    hover("raie dans le non filtré seul : effacée par les filtres", lined(q, lines=[(0, 105.0, 0.5, 0.0)]))
    hover("raie qui passe les filtres", lined(q, lines=[(0, 105.0, 0.5, 0.5)]))
    hover("raie un peu plus forte après filtres que mesurée avant", lined(q, lines=[(0, 105.0, 0.3, 0.6)]))
    # fusion de raies proches entre le roll et le pitch : la plus haute remplace la plus basse, sinon la 1re reste
    hover("fusion : pitch plus haut (113 Hz) remplace roll (105 Hz)", lined(q, lines=[(0, 105.0, 0.3, 0.2), (1, 113.0, 0.6, 0.2)]))
    hover("fusion : pitch plus bas (113 Hz) : roll reste", lined(q, lines=[(0, 105.0, 0.6, 0.2), (1, 113.0, 0.3, 0.2)]))
    hover("fusion : même fréquence sur les deux axes", lined(q, lines=[(0, 105.0, 0.4, 0.2), (1, 105.0, 0.4, 0.3)]))
    hover("fusion : fréquences éloignées de plus de 10 %", lined(q, lines=[(0, 105.0, 0.4, 0.3), (1, 130.0, 0.4, 0.3)]))
    hover("trois raies, plus de cinq non atteint", lined(q, lines=[(0, 60.0, 0.3, 0.3), (0, 105.0, 0.4, 0.3), (1, 150.0, 0.5, 0.3)]))
    many = lined(q, lines=[(0, 60.0, 0.3, 0.2), (1, 105.0, 0.35, 0.3), (0, 150.0, 0.4, 0.3), (1, 192.0, 0.45, 0.3), (0, 232.0, 0.5, 0.3),
                           (1, 356.0, 0.55, 0.3)])
    hover("six raies : le bilan n'en cite que cinq", many)
    hover("six raies, boucle 1 kHz", raw_with(many, **pid1k))
    hover("six raies, 7 notchs en service", raw_with(many, dyn_notch_count="7"))
    hover("six raies, aliased", many, aliased=True)
    hover("raie sous 100 Hz seule, boucle 1 kHz : pas de statique", raw_with(lined(q, lines=[(0, 60.0, 0.5, 0.4)]), **pid1k))
    # notch statique sans mesure fiable de la raie : harmoniques fortes dans le gyro non filtré, masquées en partie seulement par
    # `line_shapes` (rpm_filter_harmonics les retire), d'où une hauteur sous 4 dB (Q infini) ou une largeur NaN : repli à 0,7 × la fréquence
    strongh = lined(q, lines=[(0, 150.0, 0.5, 0.4)], harm_unf=(1.0, 1.0, 1.0))
    hover("statique, repli : hauteur de la raie sous 4 dB (Q infini)", raw_with(strongh, rpm_filter_harmonics="1", **pid1k))
    veryh = lined(q, lines=[(0, 150.0, 0.5, 0.4)], harm_unf=(2.0, 2.0, 2.0))
    hover("statique, repli : largeur NaN", raw_with(veryh, rpm_filter_harmonics="0", **pid1k))
    hover("statique, mesure refaite sur le spectre après filtre RPM (3 harmoniques)", raw_with(strongh, rpm_filter_harmonics="3", **pid1k))
    hover("statique, 7 notchs, repli", raw_with(veryh, rpm_filter_harmonics="0", dyn_notch_count="7"))
    shape("harmoniques fortes, rpm_harmonics 1 : hauteur sous 4 dB", raw_with(strongh, rpm_filter_harmonics="1"), 150.0)
    shape("harmoniques très fortes, rpm_harmonics 0 : largeur NaN", raw_with(veryh, rpm_filter_harmonics="0"), 150.0)
    shape("harmoniques fortes, rpm_harmonics 3 : repli sur le spectre après filtre RPM", raw_with(strongh, rpm_filter_harmonics="3"), 150.0)
    hover("harmoniques fortes avant et après", lined(q, lines=L, harm_unf=(0.4, 0.4, 0.4), harm_filt=(0.2, 0.2, 0.2)))
    hover("harmoniques fortes, effacées", lined(q, lines=L, harm_unf=(0.4, 0.4, 0.4)))
    hover("moteurs à ~125 Hz (taille différente)", lined(build(fs=FS, seconds=3.0, motor_hz=100.0), lines=[(0, 60.0, 0.3, 0.3)]))

    out = {"cases": DIRECT, "inputs": INPUTS}
    text = json.dumps(out, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print(len(DIRECT), "appels directs,", sum(1 for c in DIRECT if c["error"]), "erreurs de Python,", f"{len(text) / 1e6:.2f} Mo",
          file=sys.stderr)
    print("constats vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    want = ("gyro_lpf2_off", "dterm_filter_weak", "dterm_lpf1_static", "gyro_lpf_biquad", "rpm_weight_low", "rpm_harmonic_missing",
            "rpm_line_visible", "rpm_weight_lighten", "rpm_q_raise", "hover_noise_summary", "hover_line_residual")
    missing = [k for k in want if k not in SEEN]
    print("constats jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
