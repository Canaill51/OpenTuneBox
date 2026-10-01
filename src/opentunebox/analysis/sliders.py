"""Curseurs du réglage simplifié Betaflight ⇄ valeurs PID.

Réimplémentation de `config/simplified_tuning.c` (spec-betaflight.md §5), vérifiée sur les
tags 4.4.3, 4.5.5, 2025.12.5 et 2026.6.1 :

- P = Pdef·M·PI·pp, I = Idef·M·PI·Ig·pp, F = Fdef·M·pp·FF (pp = pitch_pi sur le tangage seul) ;
- 4.4/4.5 (D_MIN_DEFAULT) : d_min = Dmin·M·Dg·pr, D(max) = Dmin·M·Dg·pr·(1 + (Ddef−Dmin)/Dmin·s) ;
- 2025.12+ (D_MAX_DEFAULT) : D = Ddef·M·Dg·pr, d_max = Dmax·M·Dg·pr·(s + (1−s)·Ddef/Dmax) ;
- calcul en flottant 32 bits dans l'ordre du C, avec la sémantique -ffast-math du firmware
  (division par 100 → multiplication par 0.01f, ce qui donne 53 et non 54 pour 45 × 120 %),
  troncature vers zéro (constrain(int)),
  bornes PID_GAIN_MAX = 250, F_GAIN_MAX = 1000.

Les clés des dictionnaires de curseurs sont celles de `Settings.sliders()` (en %, 100 = défaut).
Les noms CLI sont identiques aux clés d'en-tête (mêmes macros PARAM_NAME_* du firmware) ;
seul le curseur D max change de nom : `simplified_dmax_gain` (4.x) / `simplified_d_max_gain` (2025.12+).
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from ..log.settings import AxisPID, Settings

PID_GAIN_MAX = 250
F_GAIN_MAX = 1000
LPF_MAX_HZ = 1000
DYN_LPF_MAX_HZ = 1000
SLIDER_MAX = 200
FILTER_SLIDER_MIN = 10

# (P, I, D, F) par défaut, par famille de version (flight/pid.h)
_PID_DEFAULTS_4X = ((45, 80, 40, 120), (47, 84, 46, 125), (45, 80, 0, 120))
_PID_DEFAULTS_NEW = ((45, 80, 30, 120), (47, 84, 34, 125), (45, 80, 0, 120))
_D_MIN_DEFAULT_4X = (30, 34, 0)  # 4.4/4.5 : D de base (D_MIN_DEFAULT)
_D_MAX_DEFAULT_NEW = (40, 46, 0)  # 2025.12+ : D maximum (D_MAX_DEFAULT)

# Valeurs par défaut des curseurs (identiques dans toutes les versions)
SLIDER_DEFAULTS = {
    "mode": 2, "master": 100, "i_gain": 100, "d_gain": 100, "pi_gain": 100, "ff_gain": 100,
    "pitch_d": 100, "pitch_pi": 100, "dmax_gain": 100,
    "dterm_filter": 1, "dterm_mult": 100, "gyro_filter": 1, "gyro_mult": 100,
}
PID_SLIDER_KEYS = ("master", "pi_gain", "i_gain", "d_gain", "ff_gain", "pitch_d", "pitch_pi", "dmax_gain")

# Catégories de l'écran PIDtoolbox → curseur
PTB_CATEGORIES = {
    "Damping": "d_gain",
    "Tracking": "pi_gain",
    "Stick Resp.": "ff_gain",
    "Dynamic D.": "dmax_gain",
    "Drift-Wobble": "i_gain",
    "Pitch D.": "pitch_d",
    "Pitch T.": "pitch_pi",
    "Master": "master",
}

MODE_NAMES = ("OFF", "RP", "RPY")
OFF_ON = ("OFF", "ON")


def _is_new(family: str) -> bool:
    return family not in ("4.4", "4.5")


def default_pids(family: str) -> list[AxisPID]:
    """PID par défaut du firmware (forme normalisée AxisPID : D = D de base, Dmax = D maximum)."""
    out = []
    for ax in range(3):
        if _is_new(family):
            p, i, d, f = _PID_DEFAULTS_NEW[ax]
            dmax = max(d, _D_MAX_DEFAULT_NEW[ax])
            out.append(AxisPID(P=p, I=i, D=d, Dmax=dmax, F=f))
        else:
            p, i, d, f = _PID_DEFAULTS_4X[ax]
            dmin = _D_MIN_DEFAULT_4X[ax]
            out.append(AxisPID(P=p, I=i, D=dmin if 0 < dmin < d else d, Dmax=d, F=f))
    return out


# ---------------------------------------------------------------- curseurs → PID


def _f32(v) -> np.float32:
    return np.float32(v)


def _trunc(v: np.float32, hi: int) -> int:
    """constrain(int) du firmware : conversion float → int par troncature, puis bornes."""
    return int(min(max(int(v), 0), hi))


def pids_from_sliders(sliders: dict, family: str, base: list[AxisPID] | None = None) -> list[AxisPID]:
    """PID calculés par le firmware à partir des curseurs (`simplified_tuning apply`).

    Les axes non couverts par le mode (lacet en mode RP, tout en mode OFF) gardent les valeurs de
    `base` (par défaut les PID par défaut de la famille). Curseurs manquants → 100.
    """
    sl = {**SLIDER_DEFAULTS, **{k: v for k, v in sliders.items() if v is not None and v >= 0}}
    out = [AxisPID(**vars(p)) for p in (base or default_pids(family))]
    # -ffast-math : « x / 100.0f » est compilé en « x * 0.01f » (0.01f < 0,01 → 54 devient 53,99…)
    pct = _f32(0.01)
    m = _f32(sl["master"]) * pct
    pi = _f32(sl["pi_gain"]) * pct
    dg = _f32(sl["d_gain"]) * pct
    ff = _f32(sl["ff_gain"]) * pct
    ig = _f32(sl["i_gain"]) * pct
    s = _f32(sl["dmax_gain"]) * pct
    new = _is_new(family)
    defaults = _PID_DEFAULTS_NEW if new else _PID_DEFAULTS_4X
    mode = int(sl["mode"])
    # -ffast-math autorise aussi le réassociement : les produits invariants de la boucle sont
    # calculés une fois (M·PI, M·Dg, M·FF), puis × défaut, puis × facteur de tangage. Cet ordre
    # reproduit les arrondis du firmware sur tout le corpus (voir tests/test_sliders.py).
    m_pi = m * pi
    m_pi_ig = m_pi * ig
    m_dg = m * dg
    m_ff = m * ff
    for ax in range(0, min(mode, 2) + 1 if mode > 0 else 0):
        pr = _f32(sl["pitch_d"]) * pct if ax == 1 else _f32(1.0)
        pp = _f32(sl["pitch_pi"]) * pct if ax == 1 else _f32(1.0)
        pd, id_, dd, fd = (_f32(v) for v in defaults[ax])
        P = _trunc(pd * m_pi * pp, PID_GAIN_MAX)
        I = _trunc(id_ * m_pi_ig * pp, PID_GAIN_MAX)
        F = _trunc(fd * m_ff * pp, F_GAIN_MAX)
        if new:
            dmax_def = _D_MAX_DEFAULT_NEW[ax]
            D = _trunc(dd * m_dg * pr, PID_GAIN_MAX)
            if dmax_def > 0:
                gain = s + (_f32(1.0) - s) * dd / _f32(dmax_def)
                dmax = _trunc(_f32(dmax_def) * m_dg * pr * gain, PID_GAIN_MAX)
            else:
                dmax = out[ax].Dmax if out[ax].Dmax > out[ax].D else 0
            out[ax] = AxisPID(P=P, I=I, D=D, Dmax=dmax if dmax > D else D, F=F)
        else:
            dmin_def = _D_MIN_DEFAULT_4X[ax]
            dmn = _f32(dmin_def)
            if dmin_def > 0:
                ratio = _f32(1.0) + ((dd - dmn) / dmn) * s
            else:
                ratio = _f32(1.0)
            d_lo_f = dmn * m_dg * pr
            d_hi = _trunc(d_lo_f * ratio, PID_GAIN_MAX)
            d_lo = _trunc(d_lo_f, PID_GAIN_MAX)
            out[ax] = AxisPID(P=P, I=I, D=d_lo if 0 < d_lo < d_hi else d_hi, Dmax=d_hi, F=F)
    return out


def filters_from_sliders(gyro_mult: int, dterm_mult: int, family: str = "2026.6",
                         settings: Settings | None = None) -> dict[str, int]:
    """Fréquences des filtres imposées par les curseurs de filtres (arithmétique entière du firmware).

    Le firmware ne modifie que les filtres déjà actifs (valeur ≠ 0) : si `settings` est fourni,
    les filtres désactivés restent à 0. Identique dans toutes les familles supportées.
    """
    del family  # mêmes valeurs par défaut de 4.4 à 2026.6

    def scaled(default: int, mult: int, hi: int) -> int:
        return min(max(default * int(mult) // 100, 0), hi)

    out = {
        "gyro_lpf1_dyn_min_hz": scaled(250, gyro_mult, DYN_LPF_MAX_HZ),
        "gyro_lpf1_dyn_max_hz": scaled(500, gyro_mult, DYN_LPF_MAX_HZ),
        "gyro_lpf1_static_hz": scaled(250, gyro_mult, DYN_LPF_MAX_HZ),
        "gyro_lpf2_static_hz": scaled(500, gyro_mult, LPF_MAX_HZ),
        "dterm_lpf1_dyn_min_hz": scaled(75, dterm_mult, DYN_LPF_MAX_HZ),
        "dterm_lpf1_dyn_max_hz": scaled(150, dterm_mult, DYN_LPF_MAX_HZ),
        "dterm_lpf1_static_hz": scaled(75, dterm_mult, DYN_LPF_MAX_HZ),
        "dterm_lpf2_static_hz": scaled(150, dterm_mult, LPF_MAX_HZ),
    }
    if settings is not None:
        s = settings
        current = {
            "gyro_lpf1_dyn_min_hz": s.gyro_lpf1.dyn_min_hz, "gyro_lpf1_dyn_max_hz": s.gyro_lpf1.dyn_min_hz,
            "gyro_lpf1_static_hz": s.gyro_lpf1.static_hz, "gyro_lpf2_static_hz": s.gyro_lpf2.static_hz,
            "dterm_lpf1_dyn_min_hz": s.dterm_lpf1.dyn_min_hz, "dterm_lpf1_dyn_max_hz": s.dterm_lpf1.dyn_min_hz,
            "dterm_lpf1_static_hz": s.dterm_lpf1.static_hz, "dterm_lpf2_static_hz": s.dterm_lpf2.static_hz,
        }
        out = {k: (v if current[k] else 0) for k, v in out.items()}
    return out


# ---------------------------------------------------------------- PID → curseurs (estimation)


def sliders_from_pids(pids: list[AxisPID], family: str) -> dict[str, float]:
    """Estime les positions des curseurs (en %, flottants) qui reproduiraient ces PID.

    Utile quand le réglage simplifié est désactivé. Moindres carrés en logarithme sur P, I, D, F
    de roulis/tangage/lacet. Le maître multiplie tout : seule son produit avec chaque curseur est
    identifiable ; par convention le maître est pris égal à la moyenne géométrique des échelles
    PI et D (Tracking × Damping = 1).
    """
    new = _is_new(family)
    defaults = _PID_DEFAULTS_NEW if new else _PID_DEFAULTS_4X
    base_d = _PID_DEFAULTS_NEW if new else None
    # inconnues (log) : 0 mPI, 1 Ig, 2 mDg, 3 mFF, 4 pp, 5 pr
    rows, rhs = [], []

    def eq(cols: dict[int, float], value: float, ref: float):
        if value > 0 and ref > 0:
            r = np.zeros(6)
            for c, v in cols.items():
                r[c] = v
            rows.append(r)
            rhs.append(np.log(value / ref))

    for ax, p in enumerate(pids[:3]):
        pitch = 1.0 if ax == 1 else 0.0
        pd, id_, _, fd = defaults[ax]
        eq({0: 1, 4: pitch}, p.P, pd)
        eq({0: 1, 1: 1, 4: pitch}, p.I, id_)
        eq({3: 1, 4: pitch}, p.F, fd)
        d_ref = base_d[ax][2] if new else _D_MIN_DEFAULT_4X[ax]
        eq({2: 1, 5: pitch}, p.D, d_ref)
    est = np.zeros(6)
    if rows:
        a = np.vstack(rows)
        sol, *_ = np.linalg.lstsq(a, np.asarray(rhs), rcond=None)
        # colonnes non contraintes (aucune équation) → 0 (curseur 100 %)
        used = np.any(a != 0, axis=0)
        est = np.where(used, sol, 0.0)
    m_pi, ig, m_dg, m_ff, pp, pr = (float(v) for v in np.exp(est))
    master = float(np.sqrt(m_pi * m_dg))

    # Curseur D max : rapport Dmax/D = 1 + s·(Dref_max/Dref_min − 1) (même forme dans les deux familles)
    ss = []
    for ax in range(2):
        p = pids[ax] if ax < len(pids) else None
        if p is None or p.D <= 0:
            continue
        if new:
            span = _D_MAX_DEFAULT_NEW[ax] / _PID_DEFAULTS_NEW[ax][2] - 1.0
        else:
            span = (_PID_DEFAULTS_4X[ax][2] - _D_MIN_DEFAULT_4X[ax]) / _D_MIN_DEFAULT_4X[ax]
        ss.append((max(p.Dmax, p.D) / p.D - 1.0) / span)
    dmax = float(np.mean(ss)) if ss else 1.0
    return {
        "master": 100.0 * master,
        "pi_gain": 100.0 * m_pi / master,
        "i_gain": 100.0 * ig,
        "d_gain": 100.0 * m_dg / master,
        "ff_gain": 100.0 * m_ff / master,
        "pitch_d": 100.0 * pr,
        "pitch_pi": 100.0 * pp,
        "dmax_gain": 100.0 * max(0.0, dmax),
    }


# ---------------------------------------------------------------- vue PIDtoolbox


@dataclass
class SliderView:
    values: dict[str, float]  # catégorie PTB → fraction (1,0 = 100 %)
    from_header: bool  # True : curseurs de l'en-tête ; False : estimation depuis les PID
    consistent: bool  # les PID de l'en-tête correspondent aux curseurs (ou estimation exacte)


def slider_view(settings: Settings) -> SliderView:
    """Les 8 curseurs affichés par PIDtoolbox, en fractions (1,0 = 100)."""
    fam = settings.family
    sl = settings.sliders()
    if sl.get("mode", 0) > 0:
        src = {k: float(sl.get(k, SLIDER_DEFAULTS[k])) for k in PID_SLIDER_KEYS}
        calc = pids_from_sliders(sl, fam, base=settings.pids)
        consistent = all(_same(a, b) for a, b in zip(calc, settings.pids))
        from_header = True
    else:
        src = sliders_from_pids(settings.pids, fam)
        calc = pids_from_sliders({**{k: round(v) for k, v in src.items()}, "mode": 2}, fam)
        consistent = all(_same(a, b) for a, b in zip(calc, settings.pids))
        from_header = False
    values = {cat: src[key] / 100.0 for cat, key in PTB_CATEGORIES.items()}
    return SliderView(values=values, from_header=from_header, consistent=consistent)


def _same(a: AxisPID, b: AxisPID) -> bool:
    return (a.P, a.I, a.D, a.Dmax, a.F) == (b.P, b.I, b.D, b.Dmax, b.F)


# ---------------------------------------------------------------- commandes CLI


def cli_commands(sliders: dict, family: str) -> list[str]:
    """Lignes CLI Betaflight pour appliquer ces curseurs, suivies de `simplified_tuning apply` et `save`."""
    dmax_key = "simplified_d_max_gain" if _is_new(family) else "simplified_dmax_gain"
    spec = [  # clé interne, nom CLI, min, max, table de choix
        ("mode", "simplified_pids_mode", 0, 2, MODE_NAMES),
        ("master", "simplified_master_multiplier", 0, SLIDER_MAX, None),
        ("i_gain", "simplified_i_gain", 0, SLIDER_MAX, None),
        ("d_gain", "simplified_d_gain", 0, SLIDER_MAX, None),
        ("pi_gain", "simplified_pi_gain", 0, SLIDER_MAX, None),
        ("dmax_gain", dmax_key, 0, SLIDER_MAX, None),
        ("ff_gain", "simplified_feedforward_gain", 0, SLIDER_MAX, None),
        ("pitch_d", "simplified_pitch_d_gain", 0, SLIDER_MAX, None),
        ("pitch_pi", "simplified_pitch_pi_gain", 0, SLIDER_MAX, None),
        ("dterm_filter", "simplified_dterm_filter", 0, 1, OFF_ON),
        ("dterm_mult", "simplified_dterm_filter_multiplier", FILTER_SLIDER_MIN, SLIDER_MAX, None),
        ("gyro_filter", "simplified_gyro_filter", 0, 1, OFF_ON),
        ("gyro_mult", "simplified_gyro_filter_multiplier", FILTER_SLIDER_MIN, SLIDER_MAX, None),
    ]
    lines = []
    for key, name, lo, hi, table in spec:
        v = sliders.get(key)
        if v is None or v < 0:
            continue
        iv = int(min(max(round(float(v)), lo), hi))
        lines.append(f"set {name} = {table[iv] if table else iv}")
    lines += ["simplified_tuning apply", "save"]
    return lines
