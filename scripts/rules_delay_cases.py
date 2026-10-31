#!/usr/bin/env python3
"""Cas de test des règles de retard de `rules.py` pour `rust/otb-advisor/src/rules.rs` (phase 5a-3a).

Usage : rules_delay_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python`)

Porté en 5a-3a : `_delay_rules` (retards des filtres gyro et D), `_dterm_lift`, `_dterm_lift_noise`, `predict_filter_change`,
`_noise_after`, `_gyro_lpf1_off`, `main_action` et `filter_strategy`. Deux familles de cas, rejouées par
`rust/otb-advisor/tests/rules_delay.rs` :
- `analyze` : l'étalon Python `analyze(log, sel, profil)` sur un vol fabriqué (1 kHz, 3 s : `build` de `rules_state_cases.py`) ; on garde de
  son rapport **seulement** les constats de `KEYS` (ceux de `_delay_rules`). Les retards d'un vol fabriqué sont minuscules (0,03 ms) :
  les repères du profil sont remplacés (`over`) pour tomber de part et d'autre de la mesure ; les seuils de bruit sont posés à mi-chemin
  entre deux prévisions du simulateur (jamais à l'égalité : les niveaux viennent d'une FFT, Rust et Python diffèrent de ~1e-12 dB) ;
- `direct` : appel direct de la fonction (`gyro_lpf1_off`, `dterm_lift`, `dterm_lift_noise`, `predict`, `strategy`) pour les branches que
  `analyze` n'atteint pas (D sans pente sur le roll avec retard fini, constat sans action, filtres plafonnés, `filter_strategy` sur des constats
  fabriqués ; `filter_rules` n'est pas encore en Rust, `gyro_lpf1_off` n'est donc testé qu'en direct).

Un flottant s'écrit en hexadécimal ; les courbes sont des float32 (D-029). `"error": true` = Python a levé une exception.
"""
import copy
import dataclasses
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, enc_log
from rules_state_cases import build, fd, hxl, noisy, rows, with_

from opentunebox.advisor import rules as RU
from opentunebox.advisor.profiles import PROFILES
from opentunebox.advisor.rules import Finding
from opentunebox.analysis import bf_filters
from opentunebox.log.settings import parse_settings

# Constats produits par `_delay_rules`
KEYS = ("delay_not_representative", "gyro_filter_cautious", "gyro_delay", "dterm_delay", "dterm_filter_too_strong", "dterm_lift")

CASES: list = []
DIRECT: list = []
SEEN: dict[str, int] = {}


def B(**kw):
    """Vol de 3 s à 1 kHz, boucle PID à 1 kHz (non replié : aucun constat d'enregistrement n'empêche les règles de retard)."""
    return build(fs=1000.0, seconds=3.0, looptime="1000", **kw)


def raw_with(lg, **kw):
    """Même vol avec d'autres réglages d'en-tête (`None` retire la clé)."""
    raw = {**lg.settings.raw, **kw}
    return with_(lg, settings=parse_settings({k: v for k, v in raw.items() if v is not None}))


def enc_prof(base, over):
    return {"base": base, "over": {k: hx(v) for k, v in over.items()}}


def enc_rec(lg, sel, base, over):
    return {"log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
            "sel": None if sel is None else list(sel), "profile": enc_prof(base, over)}


def case(label, lg, base="five", sel=None, **over):
    """Un cas `analyze` : l'étalon Python avec le profil `base` dont les seuils `over` sont remplacés."""
    profile = dataclasses.replace(PROFILES[base], **over)
    rec = {"label": label, **enc_rec(lg, sel, base, over)}
    try:
        r = RU.analyze(lg, None if sel is None else slice(*sel), profile)
    except Exception as e:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        rec |= {"error": True, "out": None}
        print(f"  {label} : Python lève {type(e).__name__}", file=sys.stderr)
        CASES.append(rec)
        return None
    kept = [f for f in r.findings if f.key in KEYS]
    for f in kept:
        SEEN[f.key] = SEEN.get(f.key, 0) + 1
    d = r.measures["delays"]
    print(f"  {label} : " + ", ".join(f"{f.key}/{f.severity}" for f in kept) +
          f"  [mouv {d.moving}, gyro {d.gyro_ms:.3f}, D {d.dterm_ms:.3f}, replié {r.measures['aliased']}]", file=sys.stderr)
    rec |= {"error": False, "out": {"findings": [fd(f) for f in kept]}}
    CASES.append(rec)
    return r


def direct(label, call, lg, args, fn, base="five", sel=None, **over):
    """Un appel direct : `fn()` rend un `Finding`, `None` ou un dict ; une exception de Python est la valeur attendue."""
    rec = {"label": label, "call": call, "args": args, **enc_rec(lg, sel, base, over)}
    try:
        out = fn()
    except Exception as e:  # noqa: BLE001
        rec |= {"error": True, "out": None}
        print(f"  [direct] {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
        DIRECT.append(rec)
        return
    if isinstance(out, Finding):
        enc = {"finding": fd(out)}
        SEEN[out.key] = SEEN.get(out.key, 0) + 1
    elif out is None:
        enc = {"finding": None}
    else:
        enc = {"pred": {k: (hxl(v) if isinstance(v, list) else hx(v)) for k, v in out.items()}}
    print(f"  [direct] {label} : " + (out.key if isinstance(out, Finding) else str(None if out is None else "prévision")), file=sys.stderr)
    rec |= {"error": False, "out": enc}
    DIRECT.append(rec)


def measures(lg, base="five", **over):
    return RU.analyze(lg, None, dataclasses.replace(PROFILES[base], **over)).measures


def predicted_d(lg, factor, sel=None):
    """Terme D (moyenne roll / pitch maximale) prévu avec des coupures D multipliées par `factor` (référence pour poser un seuil)."""
    s = lg.settings
    mod = copy.deepcopy(s)
    mod.dterm_lpf1 = bf_filters.scale_lowpass(s.dterm_lpf1, factor)
    mod.dterm_lpf2 = bf_filters.scale_lowpass(s.dterm_lpf2, factor)
    m = measures(lg)
    hi = min(500.0, 0.95 * lg.fs / 2)
    pred = RU.predict_filter_change(lg, sel or slice(0, lg.n), mod, hi)
    return RU._noise_after(pred, m["dterm_noise_db"], m["dterm_peak_ptb_db"])[1]


def between(a, b):
    return (a + b) / 2


def enc_args(**kw):
    return {k: ([hx(x) for x in v] if isinstance(v, list) else (hx(v) if isinstance(v, float) else v)) for k, v in kw.items()}


def main():
    five = "five"
    base = B()
    mb = measures(base)
    g_ms, d_ms = mb["delays"].gyro_ms, mb["delays"].dterm_ms
    print(f"base : gyro {g_ms:.4f} ms, D {d_ms:.4f} ms, bruit D {mb['dterm_noise_db']}, sommet {mb['dterm_peak_ptb_db']:.2f}", file=sys.stderr)
    low_g, low_d = g_ms / 2, d_ms / 2  # repères sous les mesures : retard au-dessus du repère

    # ---- retards sous les repères (bornes par défaut du profil)
    case("retards sous les repères, D propre : dterm_lift", base)
    case("profil seven (7 pouces)", base, base="seven")
    case("profil large (8 pouces)", base, base="large")
    case("profil micro (3 pouces)", base, base="micro")
    case("profil whoop : pas d'objectif de retard", base, base="whoop")

    # ---- vol sans mouvement : retards peu représentatifs (la consigne est réduite, le reste du vol est le même)
    still_sp = rows(base, "setpoint", lambda a: a.__imul__(np.float32(0.01)))
    case("sans mouvement, retards gyro et D finis", still_sp)
    case("sans mouvement, profil whoop (pas d'objectif)", still_sp, base="whoop")
    case("sans mouvement, profil large", still_sp, base="large")
    case("sans mouvement, gyro seul fini (D nul)", rows(still_sp, "d_term", lambda a: a.__setitem__(slice(None), 0.0)))
    case("sans mouvement, D seul fini (pas de gyro non filtré)", with_(still_sp, gyro_unfilt=None))
    case("sans mouvement, aucun retard fini", B(moving=False))
    case("avec mouvement, D nul : pas de retard D", rows(base, "d_term", lambda a: a.__setitem__(slice(None), 0.0)))
    case("avec mouvement, sans gyro non filtré : pas de retard gyro", with_(base, gyro_unfilt=None))

    # ---- retard gyro
    case("gyro au-dessus du repère, bruit faible : filtrage prudent (RPM q 500)", base, gyro_delay_max_ms=low_g)
    case("... profil whoop : sans objectif", base, base="whoop", gyro_delay_max_ms=low_g)
    case("... rpm_filter_q 1000 : pas d'action RPM", raw_with(base, rpm_filter_q="1000"), gyro_delay_max_ms=low_g)
    case("... rpm_filter_q 950 : plafonné à 1000", raw_with(base, rpm_filter_q="950"), gyro_delay_max_ms=low_g)
    case("... rpm_filter_q 1200 : au-delà du plafond", raw_with(base, rpm_filter_q="1200"), gyro_delay_max_ms=low_g)
    case("... sans filtre RPM (0 harmonique)", raw_with(base, rpm_filter_harmonics="0"), gyro_delay_max_ms=low_g)
    case("gyro au-dessus du repère, gyro bruité : retard informatif", base, gyro_delay_max_ms=low_g, gyro_noise_high_db=-70.0)
    case("gyro au-dessus du repère, D bruité (corpus) : retard informatif", base, gyro_delay_max_ms=low_g, dterm_noise_high_db=-80.0)
    case("gyro sous le repère : OK", base, gyro_delay_max_ms=g_ms * 2)

    # ---- retard du D : relèvement conseillé, refusé, sans simulation
    k1, k2 = predicted_d(base, 1.1), predicted_d(base, 1.1 ** 2)
    print(f"  D prévu : x1,1 {k1:.3f} dB, x1,21 {k2:.3f} dB", file=sys.stderr)
    case("D au-dessus du repère, D propre, relèvement fini : remplacé par dterm_lift", base, dterm_delay_max_ms=low_d)
    case("... D dépasserait les repères avec +0,1 : dterm_delay INFO", base, dterm_delay_max_ms=low_d,
         dterm_noise_high_db=k1 - 0.5)
    # le D prévu ne bouge pas (coupures déjà à 1000 Hz) : dterm_filter_too_strong de _dterm_lift reste, _dterm_lift_noise passe son tour
    capped = raw_with(base, dterm_lpf1_static_hz="1000", dterm_lpf1_dyn_hz="0,0", dterm_lpf2_static_hz="1000")
    case("... coupures du D à 1000 Hz : too_strong garde la place (lift_noise : rien ne change)", capped, dterm_delay_max_ms=low_d)
    loud = noisy(base, d=8.0)  # sommet du D vers -14 dB : au-dessus de l'idéal de -20 dB, sous le repère de -10 dB
    case("... D plus bruyant que -20 dB : too_strong, pas de lift_noise", loud, dterm_delay_max_ms=-1.0, dterm_noise_high_db=0.0)
    for mult in ("110", "120", "50"):
        case(f"... D plus bruyant que -20 dB, curseur {mult} : too_strong", raw_with(loud, simplified_dterm_filter_multiplier=mult),
             dterm_delay_max_ms=-1.0, dterm_noise_high_db=0.0)
    case("... D plus bruyant, curseur éteint : too_strong", raw_with(loud, simplified_dterm_filter="0"), dterm_delay_max_ms=-1.0,
         dterm_noise_high_db=0.0)
    case("... D bruité (corpus) : ni too_strong ni lift", base, dterm_delay_max_ms=low_d, dterm_noise_high_db=-100.0)
    case("log replié (250 Hz) : pas de simulation", build(), dterm_delay_max_ms=low_d)
    case("log replié, retard D sous le repère", build())
    case("D sous le repère : OK + lift_noise", base, dterm_delay_max_ms=d_ms * 2)
    for mult in ("110", "120", "130", "50"):
        case(f"curseur « filtre D » {mult} : remplacé par lift_noise", raw_with(base, simplified_dterm_filter_multiplier=mult),
             dterm_delay_max_ms=low_d, dterm_noise_high_db=0.0)
    case("curseur « filtre D » éteint : coupures × 1,1", raw_with(base, simplified_dterm_filter="0"), dterm_delay_max_ms=low_d)
    no_sliders = raw_with(base, **{k: None for k in base.settings.raw if k.startswith("simplified_")})
    case("pas de curseurs dans l'en-tête", no_sliders, dterm_delay_max_ms=low_d)
    case("... D plus bruyant, sans curseurs : too_strong", noisy(no_sliders, d=8.0), dterm_delay_max_ms=-1.0, dterm_noise_high_db=0.0)

    # ---- _dterm_lift_noise : plus grand pas qui tient les repères
    case("lift_noise : pas 2 (curseur 100 → 1,2), repère large", base, dterm_noise_high_db=0.0)
    case("lift_noise : pas 1 (le pas 2 dépasse le repère)", base, dterm_noise_high_db=between(k1, k2))
    case("lift_noise : aucun pas ne tient", base, dterm_noise_high_db=k1 - 0.5)
    case("lift_noise : seuil de D juste sous la mesure actuelle (D bruité, rien)", base, dterm_noise_high_db=-70.0)
    for mult in ("110", "120", "130", "140", "90", "50", "10"):
        case(f"lift_noise : curseur {mult}", raw_with(base, simplified_dterm_filter_multiplier=mult), dterm_noise_high_db=0.0)
    case("lift_noise : curseur {multiplicateur absent} (-1)", raw_with(base, simplified_dterm_filter_multiplier=None),
         dterm_noise_high_db=0.0)
    case("lift_noise sans curseur : LPF1 dynamique + LPF2 statique", raw_with(base, simplified_dterm_filter="0"), dterm_noise_high_db=0.0)
    case("lift_noise sans curseur : LPF1 statique + LPF2 statique",
         raw_with(base, simplified_dterm_filter="0", dterm_lpf1_dyn_hz="0,0"), dterm_noise_high_db=0.0)
    case("lift_noise sans curseur : LPF1 coupé, LPF2 statique",
         raw_with(base, simplified_dterm_filter="0", dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="0"), dterm_noise_high_db=0.0)
    case("lift_noise sans curseur : LPF1 statique, LPF2 coupé",
         raw_with(base, simplified_dterm_filter="0", dterm_lpf1_dyn_hz="0,0", dterm_lpf2_static_hz="0"), dterm_noise_high_db=0.0)
    case("lift_noise sans curseur : LPF1 dynamique, LPF2 coupé",
         raw_with(base, simplified_dterm_filter="0", dterm_lpf2_static_hz="0"), dterm_noise_high_db=0.0)
    case("lift_noise : les deux passe-bas coupés (rien ne change)",
         raw_with(base, simplified_dterm_filter="0", dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="0", dterm_lpf2_static_hz="0"),
         dterm_noise_high_db=0.0)
    case("lift_noise : coupures à 1000 Hz partout (plafonnées)", capped, dterm_noise_high_db=0.0)
    case("lift_noise : une seule coupure plafonnée", raw_with(capped, dterm_lpf2_static_hz="150"), dterm_noise_high_db=0.0)
    case("lift_noise : coupures de 950 Hz (pas 1 et 2 plafonnés)",
         raw_with(base, simplified_dterm_filter="0", dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="950", dterm_lpf2_static_hz="950"),
         dterm_noise_high_db=0.0)
    case("lift_noise en vol stationnaire (sans mouvement) : pas de retard mesuré, note", still_sp, dterm_noise_high_db=0.0)
    case("lift_noise sans mouvement du tout (retards NaN)", B(moving=False), dterm_noise_high_db=0.0)
    case("lift_noise : D nul sur le roll seulement", rows(base, "d_term", lambda a: a.__setitem__((0, slice(None)), 0.0)),
         dterm_noise_high_db=0.0)
    case("lift_noise : D nul sur le pitch seulement", rows(base, "d_term", lambda a: a.__setitem__((1, slice(None)), 0.0)),
         dterm_noise_high_db=0.0)
    case("lift_noise : D à -19,6 dB (au-dessus de l'idéal -20) : rien", noisy(base, d=4.0), dterm_noise_high_db=0.0)
    case("lift_noise : gyro bruité : rien", base, gyro_noise_high_db=-70.0, dterm_noise_high_db=0.0)
    case("lift_noise : log replié : rien", build(), dterm_noise_high_db=0.0)
    case("lift_noise : D bruité (PTB) : rien", noisy(base, d=10.0), dterm_noise_high_db=0.0)
    case("lift_noise : sans gyro non filtré (pas de prévision du gyro)", with_(base, gyro_unfilt=None), dterm_noise_high_db=0.0)
    case("période explicite", base, sel=(300, 2500), dterm_noise_high_db=0.0)
    case("période explicite, retard gyro au-dessus du repère", base, sel=(300, 2500), gyro_delay_max_ms=low_g)
    case("période dépassant le journal", base, sel=(300, base.n + 500), dterm_noise_high_db=0.0)

    # ---- appels directs
    hi = min(500.0, 0.95 * base.fs / 2)
    sel = slice(0, base.n)
    prof = dataclasses.replace(PROFILES[five])
    g_db, d_db, d_ptb = mb["gyro_noise_db"], mb["dterm_noise_db"], mb["dterm_peak_ptb_db"]
    dargs = enc_args(hi=hi, g_db=g_db, d_db=d_db, d_ptb=d_ptb)

    # prévisions : LPF1 du gyro coupé, passe-bas du D relevés (y compris plafonnés : rien ne change), sans gyro non filtré
    def mod_gyro_off(lg):
        m = copy.deepcopy(lg.settings)
        m.gyro_lpf1 = bf_filters.lowpass_off(lg.settings.gyro_lpf1)
        return m

    def mod_dterm(lg, f):
        m = copy.deepcopy(lg.settings)
        m.dterm_lpf1 = bf_filters.scale_lowpass(lg.settings.dterm_lpf1, f)
        m.dterm_lpf2 = bf_filters.scale_lowpass(lg.settings.dterm_lpf2, f)
        return m

    def pred_of(lg, m):
        return lambda: RU.predict_filter_change(lg, sel, m, hi)

    direct("prévision : LPF1 du gyro coupé", "predict", base, {"mod": "gyro_lpf1_off", "hi": hx(hi)}, pred_of(base, mod_gyro_off(base)))
    direct("prévision : coupures du D x1,1", "predict", base, {"mod": "dterm", "factor": hx(1.1), "hi": hx(hi)},
           pred_of(base, mod_dterm(base, 1.1)))
    direct("prévision : coupures du D x1,3", "predict", base, {"mod": "dterm", "factor": hx(1.3), "hi": hx(hi)},
           pred_of(base, mod_dterm(base, 1.3)))
    direct("prévision : réglages inchangés (écart nul)", "predict", base, {"mod": "dterm", "factor": hx(1.0), "hi": hx(hi)},
           pred_of(base, mod_dterm(base, 1.0)))
    nu = with_(base, gyro_unfilt=None)
    direct("prévision sans gyro non filtré : D seul", "predict", nu, {"mod": "dterm", "factor": hx(1.1), "hi": hx(hi)},
           pred_of(nu, mod_dterm(nu, 1.1)))
    nd = rows(base, "d_term", lambda a: a.__setitem__(slice(None), 0.0))
    direct("prévision sans terme D : gyro seul", "predict", nd, {"mod": "gyro_lpf1_off", "hi": hx(hi)}, pred_of(nd, mod_gyro_off(nd)))
    d1 = rows(base, "d_term", lambda a: a.__setitem__((0, slice(None)), 0.0))
    direct("prévision, D nul sur le roll seulement", "predict", d1, {"mod": "dterm", "factor": hx(1.1), "hi": hx(hi)},
           pred_of(d1, mod_dterm(d1, 1.1)))
    direct("prévision : bande vide (hi sous 80 Hz) : Python lève", "predict", base, {"mod": "dterm", "factor": hx(1.1), "hi": hx(60.0)},
           lambda: RU.predict_filter_change(base, sel, mod_dterm(base, 1.1), 60.0))
    # bande de 1 à 4 points (np.convolve échange alors ses arguments : 5 valeurs en sortie)
    for h, nm in ((80.0 + 3.3, "1 point"), (80.0 + 4.0, "2 points"), (80.0 + 7.0, "3 points"), (80.0 + 10.5, "4 points"), (80.0 + 14.0, "5 points")):
        direct(f"prévision : bande de {nm}", "predict", base, {"mod": "dterm", "factor": hx(1.1), "hi": hx(h)},
               (lambda h=h: RU.predict_filter_change(base, sel, mod_dterm(base, 1.1), h)))

    # _gyro_lpf1_off
    fa = {**dargs, "hover": False}
    direct("gyro_lpf1_off : LPF1 dynamique, bruit prévu sous les repères", "gyro_lpf1_off", base, fa,
           lambda: RU._gyro_lpf1_off(base, sel, hi, prof, g_db, d_db, d_ptb, False))
    direct("gyro_lpf1_off : vol stationnaire", "gyro_lpf1_off", base, {**dargs, "hover": True},
           lambda: RU._gyro_lpf1_off(base, sel, hi, prof, g_db, d_db, d_ptb, True))
    st = raw_with(base, gyro_lpf1_dyn_hz="0,0", gyro_lpf1_static_hz="300", gyro_lpf1_type="2")
    direct("gyro_lpf1_off : LPF1 statique PT2", "gyro_lpf1_off", st, fa,
           lambda: RU._gyro_lpf1_off(st, sel, hi, prof, g_db, d_db, d_ptb, False))
    direct("gyro_lpf1_off : gyro prévu au-dessus du seuil", "gyro_lpf1_off", base, fa,
           lambda: RU._gyro_lpf1_off(base, sel, hi, dataclasses.replace(prof, gyro_noise_high_db=-60.0), g_db, d_db, d_ptb, False),
           gyro_noise_high_db=-60.0)
    direct("gyro_lpf1_off : D prévu au-dessus du seuil", "gyro_lpf1_off", base, fa,
           lambda: RU._gyro_lpf1_off(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=-60.0), g_db, d_db, d_ptb, False),
           dterm_noise_high_db=-60.0)
    direct("gyro_lpf1_off : sommet du D prévu au-dessus de -10 dB", "gyro_lpf1_off", base, {**fa, "d_ptb": hx(-9.0)},
           lambda: RU._gyro_lpf1_off(base, sel, hi, prof, g_db, d_db, -9.0, False))
    direct("gyro_lpf1_off : pas de gyro non filtré (g inconnu)", "gyro_lpf1_off", nu, fa,
           lambda: RU._gyro_lpf1_off(nu, sel, hi, prof, g_db, d_db, d_ptb, False))
    direct("gyro_lpf1_off : sans terme D (d inconnu)", "gyro_lpf1_off", nd, {**fa, "d_db": [], "d_ptb": hx(float("nan"))},
           lambda: RU._gyro_lpf1_off(nd, sel, hi, prof, g_db, [], float("nan"), False))
    direct("gyro_lpf1_off : sommet du D non fini", "gyro_lpf1_off", base, {**fa, "d_ptb": hx(float("nan"))},
           lambda: RU._gyro_lpf1_off(base, sel, hi, prof, g_db, d_db, float("nan"), False))
    off = raw_with(base, gyro_lpf1_dyn_hz="0,0", gyro_lpf1_static_hz="0")
    direct("gyro_lpf1_off : LPF1 déjà coupé (rien ne change)", "gyro_lpf1_off", off, fa,
           lambda: RU._gyro_lpf1_off(off, sel, hi, prof, g_db, d_db, d_ptb, False))

    # _dterm_lift
    la = {**dargs, "delay_ms": hx(0.35), "d_goal": " Objectif X."}
    direct("dterm_lift : relèvement qui tient les repères", "dterm_lift", base, la,
           lambda: RU._dterm_lift(base, sel, hi, prof, 0.35, d_db, d_ptb, " Objectif X."))
    direct("dterm_lift : le D dépasserait les repères", "dterm_lift", base, la,
           lambda: RU._dterm_lift(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=k1 - 0.5), 0.35, d_db, d_ptb, " Objectif X."),
           dterm_noise_high_db=k1 - 0.5)
    direct("dterm_lift : sommet du D prévu au-dessus de -10 dB", "dterm_lift", base, {**la, "d_ptb": hx(-9.9), "d_goal": ""},
           lambda: RU._dterm_lift(base, sel, hi, prof, 0.35, d_db, -9.9, ""))
    d_roll0 = rows(base, "d_term", lambda a: a.__setitem__((0, slice(None)), 0.0))
    direct("dterm_lift : D nul sur le roll : pas de simulation", "dterm_lift", d_roll0, la,
           lambda: RU._dterm_lift(d_roll0, sel, hi, prof, 0.35, d_db, d_ptb, " Objectif X."))
    direct("dterm_lift : curseur 120", "dterm_lift", raw_with(base, simplified_dterm_filter_multiplier="120"), la,
           lambda: RU._dterm_lift(raw_with(base, simplified_dterm_filter_multiplier="120"), sel, hi, prof, 0.35, d_db, d_ptb, " Objectif X."))
    direct("dterm_lift : D inconnu (liste vide) : Python lève", "dterm_lift", base, {**la, "d_db": [], "d_goal": ""},
           lambda: RU._dterm_lift(base, sel, hi, prof, 0.35, [], d_ptb, ""))
    direct("dterm_lift : sommet du D non fini : Python lève", "dterm_lift", base, {**la, "d_ptb": hx(float("nan")), "d_goal": ""},
           lambda: RU._dterm_lift(base, sel, hi, prof, 0.35, d_db, float("nan"), ""))

    # _dterm_lift_noise appelé directement (D nul sur le roll : jamais atteint par analyze)
    ln = {**dargs, "hover": False, "delay_ms": hx(0.35)}
    direct("dterm_lift_noise : pas 2", "dterm_lift_noise", base, ln,
           lambda: RU._dterm_lift_noise(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=0.0), d_db, d_ptb, False, 0.35),
           dterm_noise_high_db=0.0)
    direct("dterm_lift_noise : retard NaN, vol stationnaire", "dterm_lift_noise", base, {**ln, "hover": True, "delay_ms": hx(float("nan"))},
           lambda: RU._dterm_lift_noise(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=0.0), d_db, d_ptb, True, float("nan")),
           dterm_noise_high_db=0.0)
    direct("dterm_lift_noise : aucun pas ne tient", "dterm_lift_noise", base, ln,
           lambda: RU._dterm_lift_noise(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=k1 - 0.5), d_db, d_ptb, False, 0.35),
           dterm_noise_high_db=k1 - 0.5)
    direct("dterm_lift_noise : sommet du D non fini (dp inconnu)", "dterm_lift_noise", base, {**ln, "d_ptb": hx(float("nan"))},
           lambda: RU._dterm_lift_noise(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=0.0), d_db, float("nan"), False, 0.35),
           dterm_noise_high_db=0.0)
    direct("dterm_lift_noise : D inconnu (liste vide)", "dterm_lift_noise", base, {**ln, "d_db": []},
           lambda: RU._dterm_lift_noise(base, sel, hi, dataclasses.replace(prof, dterm_noise_high_db=0.0), [], d_ptb, False, 0.35),
           dterm_noise_high_db=0.0)

    # main_action et filter_strategy sur des constats fabriqués
    def fnd(key, sev, title, actions):
        return Finding(key, "Filtres", sev, title, "d", actions)

    def fe(f):
        return {"key": f.key, "severity": f.severity, "title": f.title, "actions": list(f.actions)}

    def strat(label, fs_, moving=True):
        direct(label, "strategy", base, {"findings": [fe(f) for f in fs_], "moving": moving},
               lambda: RU.filter_strategy(fs_, moving))

    cur = fnd("gyro_filter_cautious", RU.CONSEIL, "Gyro prudent", ["Texte sans repère.", "Curseur « filtre gyro » : +0,2."])
    eq = fnd("dterm_lift", RU.CONSEIL, "D propre", ["Après un vol.", "dterm_lpf1_static_hz = 90"])
    arrow = fnd("rpm_q_raise", RU.CONSEIL, "Q RPM", ["Rien.", "Filtre RPM : rpm_filter_q 500 → 600"])
    plain = fnd("gyro_noisy", RU.ATTENTION, "Gyro bruité", ["Première action sans repère.", "Seconde sans repère."])
    plain2 = fnd("frame_line_fixed", RU.CONSEIL, "Raie du cadre", ["Seulement du texte."])
    rec_ = fnd("log_rate_low", RU.ATTENTION, "Log lent", ["blackbox_sample_rate = 1/2"])
    strat("stratégie : aucun constat", [])
    strat("stratégie : une seule étape", [cur])
    strat("stratégie : deux constats de la même étape", [cur, fnd("gyro_lpf1_off", RU.CONSEIL, "LPF1 coupé", ["gyro_lpf1_static_hz = 0"])])
    strat("stratégie : deux étapes", [eq, cur])
    strat("stratégie : deux étapes, vol sans mouvement", [eq, cur], moving=False)
    strat("stratégie : toutes les étapes, ordre des étapes et non des constats", [eq, cur, arrow, plain, plain2, rec_,
          fnd("hover_line_residual", RU.CONSEIL, "Raie en vol", ["dyn_notch_count = 4"]),
          fnd("dyn_notch_q_lower", RU.CONSEIL, "Q notch", ["dyn_notch_q = 250"]),
          fnd("dterm_filter_too_strong", RU.CONSEIL, "D trop filtré", ["Curseur « filtre D » : +0,1."])])
    strat("stratégie : action principale = première si aucun repère", [plain, plain2])
    strat("stratégie : trois constats dans une étape", [cur, fnd("gyro_lpf1_off", RU.CONSEIL, "LPF1", ["gyro_lpf1_static_hz = 0"]),
          fnd("gyro_lpf_biquad", RU.CONSEIL, "Biquad", ["gyro_lpf2_type = BIQUAD"]), eq])
    strat("stratégie : constats d'info, ok et sans action écartés", [
        fnd("gyro_noisy", RU.INFO, "Info", ["a = b"]), fnd("dterm_noisy", RU.OK, "Ok", ["a = b"]),
        fnd("rpm_q_raise", RU.CONSEIL, "Sans action", []), eq])
    strat("stratégie : gravité alerte écartée, attention gardée", [fnd("gyro_noisy", RU.ALERTE, "Alerte", ["a = b"]), eq, plain])
    strat("stratégie : clé inconnue ignorée", [fnd("autre", RU.CONSEIL, "Autre", ["a = b"]), cur])
    strat("stratégie : action avec « → » avant « = »", [fnd("rpm_q_raise", RU.CONSEIL, "Q", ["x → y", "a = b"]), cur])

    out = {"keys": list(KEYS), "cases": CASES, "direct": DIRECT, "inputs": INPUTS}
    text = json.dumps(out, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print(len(CASES), "cas analyze,", len(DIRECT), "appels directs,", sum(1 for c in CASES + DIRECT if c["error"]),
          "erreurs de Python,", f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)
    print("constats vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    missing = [k for k in (*KEYS, "gyro_lpf1_off", "filter_strategy") if k not in SEEN]
    print("constats jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
