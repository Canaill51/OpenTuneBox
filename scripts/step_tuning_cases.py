#!/usr/bin/env python3
"""Cas de test de `advisor/step_tuning.py` sur des listes de `FlightStep` fabriquées, pour `rust/otb-advisor/src/step_tuning.rs` (5a-5).

Usage : step_tuning_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python -I`)

Complète la couche corpus (`otb-parity check-advisor`, clés `tuning|…` : un fichier par dossier de drone) : des vols fabriqués (curseurs,
signature de filtres, conditions, métriques par axe, fenêtres) font atteindre **chaque** constat et chaque branche, y compris les
valeurs juste sur les seuils (pic 1,05, arrondi `round(x, 2)` au pair : 1,045, 1,055, 1,005…), les NaN (pic, latence, cassure, vagues),
les ex æquo (`max`/`min` gardent le premier) et les erreurs (indice hors tableau : Python lève, Rust aussi).

Chaque cas garde l'entrée (flottants en hexadécimal) et le `TuningReport` entier de l'étalon : conditions, blocs (titre, série, lignes,
constat, vol conseillé). Familles :
- `step_tuning` : grilles de deux vols par série Damping (pics aux seuils), tirages libres de séries Damping / Pitch D. / Master / Pitch T. /
  Feedforward mêlées, conditions du protocole, vols seuls ;
- appels directs de `damping_block`, `master_block`, `roll_pitch_block` (série ou vol de référence) et `ff_block` avec des séries écrites à
  la main (valeurs ex æquo, -0,0, vols hors liste, curseur inconnu) ;
- formats : `_fmt`, `_n`, `_gap_action`.
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))  # -I retire le dossier du script de sys.path
from bf_filters_cases import hx  # noqa: E402
from rules_state_cases import fd  # noqa: E402

from opentunebox.advisor import step_tuning as ST  # noqa: E402
from opentunebox.analysis.sliders import PTB_CATEGORIES  # noqa: E402
from opentunebox.analysis.step import StepMetrics  # noqa: E402
from opentunebox.analysis.step_series import FlightConditions, FlightStep, Series  # noqa: E402

CATS = list(PTB_CATEGORIES)
NAN = float("nan")
CASES: list = []
SEEN: dict[str, int] = {}
TEXTS: list[str] = []
COMBOS: set = set()


def mk(label, peak=1.03, lat=(15.0, 15.5), ripple=(0.0, 0.0), rstd=(0.01, 0.01), waves=(1.1, 1.1), n=(300, 300), ff=(0, 0),
       angle=1.0, i=0.3, filt=0, nmet=2, drift=0.3, **sliders):
    """Un vol fabriqué ; `peak`, `lat`… : un nombre (les deux axes) ou un couple (roll, pitch)."""
    two = lambda v: v if isinstance(v, tuple) else (v, v)  # noqa: E731
    peak, waves = two(peak), two(waves)
    aliases = {"Pitch_T": "Pitch T.", "Pitch_D": "Pitch D.", "Stick_Resp": "Stick Resp."}
    sl = {k: 1.0 for k in CATS} | {"Drift-Wobble": drift}
    sl.update({aliases.get(k, k): v for k, v in sliders.items()})
    m = [StepMetrics(peak=peak[a], latency_ms=lat[a], ripple=ripple[a], ripple_std=rstd[a], waves_ratio=waves[a]) for a in range(nmet)]
    return FlightStep(label, sl, ("filtres", filt), FlightConditions(ff, angle, i), m, list(n), [16.0, 16.5])


def enc_flights(fl):
    return [{"label": f.label, "sliders": [hx(f.sliders[c]) for c in CATS], "filters": int(f.filters[1]),
             "ff": list(f.conditions.ff), "angle_share": hx(f.conditions.angle_share), "i_slider": hx(f.conditions.i_slider),
             "metrics": [{k: hx(getattr(m, k)) for k in ("peak", "latency_ms", "ripple", "ripple_std", "waves_ratio")}
                         for m in f.metrics],
             "n": list(f.n)} for f in fl]


def enc_series(s):
    return None if s is None else {"key": s.key, "flights": [int(i) for i in s.flights], "values": [hx(v) for v in s.values]}


def enc_block(b):
    return None if b is None else {"title": b.title, "series": enc_series(b.series), "lines": list(b.lines),
                                   "finding": None if b.finding is None else fd(b.finding), "best": b.best}


def enc_report(rep):
    return {"conditions": [fd(f) for f in rep.conditions], "blocks": [enc_block(b) for b in rep.blocks]}


def note(out):
    """Compte les constats et garde les textes (pour vérifier que chaque branche est atteinte)."""
    fs = [*out["conditions"], *[b["finding"] for b in out["blocks"] if b["finding"]]] if "blocks" in out else []
    for f in fs:
        SEEN[f["key"]] = SEEN.get(f["key"], 0) + 1
        TEXTS.append(json.dumps(f, ensure_ascii=False))
        if f["key"] == "master_choice":
            COMBOS.add(("master_choice", any(a.startswith("Affinage") for a in f["actions"]),
                        any("incertitude" in a for a in f["actions"])))
        if f["key"] == "series_choice":
            COMBOS.add(("series_choice", f["severity"], f["title"].split(" ")[0]))
    for b in out.get("blocks", []):
        TEXTS.extend(b["lines"])


def case(label, fl):
    """`step_tuning(fl)` de l'étalon."""
    rec = {"label": label, "kind": "tuning", "flights": enc_flights(fl)}
    try:
        out = enc_report(ST.step_tuning(fl))
    except Exception as e:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        rec |= {"error": True, "out": None}
        print(f"  {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
    else:
        rec |= {"error": False, "out": out}
        note(out)
    CASES.append(rec)


def direct(label, fn, fl, key=None, idx=(), vals=(), ref=None):
    """Appel direct d'un bloc : `fn` vaut damping, master, roll_pitch, roll_pitch_ref ou ff."""
    s = None if fn == "roll_pitch_ref" else Series(key, list(idx), list(vals))
    rec = {"label": label, "kind": fn, "flights": enc_flights(fl), "series": enc_series(s), "ref": ref}
    try:
        if fn == "damping":
            b = ST.damping_block(fl, s)
        elif fn == "master":
            b = ST.master_block(fl, s)
        elif fn == "ff":
            b = ST.ff_block(fl, s)
        else:
            b = ST.roll_pitch_block(fl, s, ref)
        out = enc_block(b)
    except Exception as e:  # noqa: BLE001
        rec |= {"error": True, "out": None}
        print(f"  {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
    else:
        rec |= {"error": False, "out": out}
        if b is not None and b.finding is not None:
            note({"conditions": [], "blocks": [out]})
    CASES.append(rec)


def run(rng):
    # ---------------------------------------------------------------- conditions du protocole
    base = mk("base")
    case("aucun vol", [])
    case("un vol propre", [base])
    case("ff actif hors Angle", [mk("a", ff=(100, 0), angle=0.0), mk("b", ff=(0, 120), angle=0.0), mk("c", ff=(100, 100), angle=1.0)])
    case("ff actif, modes non enregistrés (angle nan)", [mk("a", ff=(100, 100), angle=NAN)])
    case("Angle à 0,9 : pas de ff actif", [mk("a", ff=(100, 100), angle=0.9), mk("b", ff=(100, 100), angle=0.8999999999999999)])
    for v in (0.1, 0.14999999, 0.15, 0.2, 0.35, 0.35000001, 0.4, 1.0, NAN, 0.30000000000000004):
        case(f"curseur I {v}", [mk("a", i=v), mk("b")])
    for n in ((10, 10), (19, 19), (20, 19), (19, 20), (), (0,), (300,), (5,)):
        case(f"fenêtres {n}", [mk("a", n=n), mk("b")])
    for n in ((150, 300), (300, 150), (25, 25), (199, 300), (200, 200), (300, 199), (300, 10), (10, 300), (300,), (200, 5000)):
        case(f"fenêtres sûres {n}", [mk("a", n=n), mk("b")])
    case("série Feedforward en Acro : pas de ff_active", [mk("a", Stick_Resp=0.5, ff=(100, 100), angle=0.0),
                                                           mk("b", Stick_Resp=1.0, ff=(100, 100), angle=0.0),
                                                           mk("c", Stick_Resp=1.5, ff=(100, 100), angle=0.0)])
    case("série Feedforward en Angle", [mk("a", Stick_Resp=0.5, ff=(100, 100)), mk("b", Stick_Resp=1.0, ff=(100, 100), angle=0.0),
                                        mk("c", Stick_Resp=1.5, ff=(100, 100), angle=0.95)])
    case("série Feedforward avec un vol ff actif hors série", [mk("a", Stick_Resp=0.5, ff=(1, 1), angle=0.0), mk("b", Stick_Resp=1.0),
                                                                mk("z", ff=(5, 5), angle=0.0, filt=9)])
    case("série Feedforward, valeurs ex æquo", [mk("a", Stick_Resp=1.0, n=(300, 300)), mk("b", Stick_Resp=1.0, n=(400, 400)),
                                                mk("c", Stick_Resp=1.5, n=(300, 300))])

    # ---------------------------------------------------------------- Damping : grilles de pics aux seuils
    peaks = [0.99, 1.0, 1.03, 1.045, 1.05, 1.0500001, 1.055, 1.0550001, 1.06, 1.075, 1.08, 1.09, 1.095, 1.1, 1.1049999, 1.105, 1.11, 1.12,
             1.2, NAN]
    for a in peaks:
        for b in peaks:
            case(f"Damping 2 vols : pics {a} / {b}", [mk("d1", Damping=1.0, peak=a), mk("d2", Damping=1.2, peak=b)])
    for a in (1.07, 1.08, 1.09, 1.1, 1.2):  # plateau : écart de 0,02 aux arrondis près (peak − peak suivant < 0,02 − 1e-9)
        for d in (0.01, 0.0199999, 0.02, 0.0200001, 0.03):
            case(f"Damping plateau {a} puis −{d}", [mk("d1", Damping=1.0, peak=a), mk("d2", Damping=1.2, peak=a - d), mk("d3", Damping=1.4)])
    case("Damping pas de 0,1", [mk("d1", Damping=1.0, peak=1.2), mk("d2", Damping=1.1, peak=1.03)])
    case("Damping pas de 0,05 et 0,15", [mk("d1", Damping=1.0, peak=1.2), mk("d2", Damping=1.05, peak=1.2), mk("d3", Damping=1.2, peak=1.02)])
    case("Damping : pitch dépasse encore", [mk("d1", Damping=1.0, peak=(1.12, 1.2)), mk("d2", Damping=1.2, peak=(1.03, 1.2))])
    case("Damping : pitch dépasse de peu (écart 0,02)", [mk("d1", Damping=1.0, peak=(1.12, 1.2)), mk("d2", Damping=1.2, peak=(1.03, 1.06))])
    case("Damping : pitch 1,05 pile", [mk("d1", Damping=1.0, peak=(1.12, 1.2)), mk("d2", Damping=1.2, peak=(1.03, 1.05))])
    case("Damping : pitch nan", [mk("d1", Damping=1.0, peak=(1.12, 1.2)), mk("d2", Damping=1.2, peak=(1.03, NAN))])
    case("Damping : axe pitch non lu", [mk("d1", Damping=1.0, n=(300, 5)), mk("d2", Damping=1.2, n=(300, 5))])
    case("Damping : axe roll non lu", [mk("d1", Damping=1.0, n=(5, 300)), mk("d2", Damping=1.2, n=(5, 300))])
    case("Damping : un seul vol lu", [mk("d1", Damping=1.0, n=(5, 5)), mk("d2", Damping=1.2)])
    case("Damping : chaque valeur a deux vols", [mk("d1", Damping=1.0, n=(250, 250), peak=1.2), mk("d2", Damping=1.0, n=(300, 300), peak=1.1),
                                                 mk("d3", Damping=1.2, n=(300, 300), peak=1.03), mk("d4", Damping=1.2, n=(300, 300), peak=1.5)])
    case("Damping : valeurs 3, ordre quelconque", [mk("d1", Damping=1.4, peak=1.02), mk("d2", Damping=0.8, peak=1.3),
                                                   mk("d3", Damping=1.1, peak=1.2)])
    case("Damping : valeurs flottantes sales", [mk(f"d{j}", Damping=0.6 + 0.2 * j, peak=1.3 - 0.1 * j) for j in range(6)])
    # série Pitch D. : jugée sur le pitch
    for a, b in ((1.1, 1.03), (1.03, 1.1), (1.2, 1.12), (1.06, 1.05), (NAN, 1.03), (1.03, NAN)):
        case(f"Pitch D. : pics pitch {a} / {b}", [mk("p1", Pitch_D=1.0, peak=(1.0, a)), mk("p2", Pitch_D=1.2, peak=(1.0, b))])
    case("Pitch D. : pas de 0,1", [mk("p1", Pitch_D=1.0, peak=(1.0, 1.2)), mk("p2", Pitch_D=1.1, peak=(1.0, 1.01))])

    # ---------------------------------------------------------------- tirages libres de séries
    for t in range(600):
        case(f"tirage Damping {t}", series_flights(rng, "Damping"))
    for t in range(700):
        case(f"tirage Master {t}", series_flights(rng, "Master"))
    for t in range(250):
        case(f"tirage Pitch T. {t}", series_flights(rng, "Pitch_T"))
    for t in range(80):
        case(f"tirage Pitch D. {t}", series_flights(rng, "Pitch_D"))
    for t in range(40):
        case(f"tirage Feedforward {t}", series_flights(rng, "Stick_Resp"))
    for t in range(400):  # plusieurs groupes de filtres : une série chacun, plus des vols seuls
        fl = []
        for g, key in enumerate(rng.choice(["Damping", "Master", "Pitch_T", "Pitch_D", "Stick_Resp", "single"], size=int(rng.integers(1, 5)))):
            fl += series_flights(rng, str(key), filt=g)
        rng.shuffle(fl)
        case(f"mélange {t}", fl)
    for t in range(150):  # vol de référence sans série Pitch T. : équilibre roll / pitch, ex æquo de fenêtres
        fl = [mk(f"v{j}", lat=(float(rng.choice([14.0, 15.0, 16.0, 17.5, 18.5, 12.0, NAN, 0.0])), float(rng.choice([14.0, 15.0, 16.0, 17.5, 18.5, NAN]))),
                 n=tuple(int(x) for x in rng.choice([10, 150, 250, 300, 300, 500], size=2)), filt=j, ripple=(0.0, float(rng.choice([0.0, 0.05, 0.07]))))
              for j in range(int(rng.integers(1, 4)))]
        case(f"référence libre {t}", fl)
    for gp in (0.5, -0.5, 1.0, -1.0, 1.0000000000000002, -1.0000000000000002, 1.5, -1.5, 2.0, -2.0, 2.0000000000000004, 2.5, -2.5, 0.0, NAN, -0.0):
        case(f"écart de latence {gp}", [mk("a", lat=(15.0, 15.0 + gp))])
    case("deux vols sans série, ex æquo de fenêtres", [mk("a", lat=(15.0, 17.0), filt=1), mk("b", lat=(15.0, 15.1), filt=2)])
    case("deux vols sans série, le second a plus de fenêtres", [mk("a", lat=(15.0, 17.0), filt=1), mk("b", lat=(15.0, 15.1), filt=2, n=(400, 300))])
    case("un vol dont un axe manque", [mk("a", n=(300, 5))])

    # ---------------------------------------------------------------- erreurs (IndexError de Python)
    case("vol sans métriques dans une série Damping", [mk("a", Damping=1.0, nmet=0), mk("b", Damping=1.2, nmet=0)])
    case("vol à une seule métrique, série Pitch D.", [mk("a", Pitch_D=1.0, nmet=1), mk("b", Pitch_D=1.2, nmet=1)])
    case("vol à une seule métrique, série Master", [mk("a", Master=1.0, nmet=1), mk("b", Master=1.2, nmet=1)])
    case("vol à une seule métrique, série Pitch T.", [mk("a", Pitch_T=1.0, nmet=1), mk("b", Pitch_T=1.2, nmet=1)])
    case("vol à une seule métrique, série Feedforward", [mk("a", Stick_Resp=1.0, nmet=1), mk("b", Stick_Resp=1.5, nmet=1)])
    case("vol sans métriques, sans série", [mk("a", nmet=0)])
    case("vol sans métriques et sans n", [mk("a", nmet=0, n=())])

    # ---------------------------------------------------------------- appels directs
    fl3 = [mk("a", Damping=1.0, peak=1.2), mk("b", Damping=1.2, peak=1.03), mk("c", Damping=1.4, peak=1.02)]
    direct("Damping direct", "damping", fl3, "Damping", [0, 1, 2], [1.0, 1.2, 1.4])
    direct("Damping : clé inconnue (KeyError)", "damping", fl3, "Tracking", [0, 1, 2], [1.0, 1.2, 1.4])
    direct("Damping : vol hors liste", "damping", fl3, "Damping", [0, 7], [1.0, 1.2])
    direct("Damping : valeur ex æquo avec un vol hors liste", "damping", fl3, "Damping", [0, 7], [1.0, 1.0])
    direct("Damping : série vide", "damping", fl3, "Damping", [], [])
    direct("Damping : un seul vol", "damping", fl3, "Damping", [0], [1.0])
    direct("Damping : listes de longueurs différentes", "damping", fl3, "Damping", [0, 1, 2], [1.0, 1.2])
    direct("Damping : -0,0 et 0,0 ex æquo", "damping", fl3, "Damping", [0, 1, 2], [-0.0, 0.0, 0.2])
    direct("Damping : 0,0 puis -0,0", "damping", fl3, "Damping", [0, 1, 2], [0.0, -0.0, 0.2])
    direct("Damping : valeurs égales à un ulp près", "damping", fl3, "Damping", [0, 1, 2], [1.0, 1.0000000000000002, 1.2])
    direct("Damping : valeurs négatives et grandes", "damping", fl3, "Damping", [0, 1, 2], [-0.3, 99.999, 1000.0])
    direct("Damping : valeurs désordonnées", "damping", fl3, "Damping", [0, 1, 2], [1.4, 1.0, 1.2])
    direct("Damping : toutes égales", "damping", fl3, "Damping", [0, 1, 2], [1.0, 1.0, 1.0])
    direct("Pitch D. direct", "damping", fl3, "Pitch D.", [0, 1, 2], [1.0, 1.2, 1.4])
    direct("Master direct", "master", [mk("a", lat=(18.0, 18.5)), mk("b", lat=(16.0, 16.5), ripple=(0.01, 0.02)),
                                       mk("c", lat=(15.0, 15.5), ripple=(0.07, 0.01))], "Master", [0, 1, 2], [1.0, 1.2, 1.4])
    direct("Master : série vide", "master", fl3, "Master", [], [])
    direct("Master : un seul vol", "master", fl3, "Master", [0], [1.0])
    direct("Master : un seul vol avec signe", "master", [mk("a", ripple=(0.04, 0.01))], "Master", [0], [1.0])
    direct("Master : vol hors liste", "master", fl3, "Master", [0, 9], [1.0, 1.2])
    direct("Master : clé inconnue (la clé n'est pas lue)", "master", fl3, "Tracking", [0, 1], [1.0, 1.2])
    direct("Master : valeurs ex æquo", "master", [mk("a", n=(300, 300)), mk("b", n=(300, 300)), mk("c", n=(500, 500), lat=(14.0, 14.0))],
           "Master", [0, 1, 2], [1.0, 1.0, 1.2])
    direct("Master : vol hors liste sans ex æquo", "master", fl3, "Master", [0, 9], [1.0, 1.2])
    direct("Roll/pitch série direct", "roll_pitch", [mk("a", lat=(15.0, 16.5)), mk("b", lat=(15.0, 15.4))], "Pitch T.", [0, 1], [1.0, 1.2])
    direct("Roll/pitch série : clé inconnue", "roll_pitch", fl3, "Tracking", [0, 1], [1.0, 1.2])
    direct("Roll/pitch série : vide", "roll_pitch", fl3, "Pitch T.", [], [])
    direct("Roll/pitch série : vol hors liste", "roll_pitch", fl3, "Pitch T.", [0, 9], [1.0, 1.2])
    direct("Roll/pitch série : ex æquo d'écart", "roll_pitch", [mk("a", lat=(15.0, 16.0)), mk("b", lat=(16.0, 15.0)), mk("c", lat=(15.0, 15.0))],
           "Pitch T.", [0, 1, 2], [1.0, 1.2, 1.4])
    direct("Roll/pitch série : écart -0,0 contre 0,0", "roll_pitch", [mk("a", lat=(15.0, 15.0)), mk("b", lat=(15.0, 15.0))],
           "Pitch T.", [0, 1], [1.0, 1.2])
    direct("Roll/pitch série : cassure pitch juste sous et sur le seuil", "roll_pitch",
           [mk("a", lat=(15.0, 15.1), ripple=(0, 0.065)), mk("b", lat=(15.0, 15.5), ripple=(0, 0.0649999)), mk("c", lat=(15.0, 15.3), ripple=(0, NAN))],
           "Pitch T.", [0, 1, 2], [1.0, 1.2, 1.4])
    direct("Roll/pitch série : pas fin", "roll_pitch", [mk("a", lat=(15.0, 18.0)), mk("b", lat=(15.0, 17.5))], "Pitch T.", [0, 1], [1.0, 1.1])
    direct("Roll/pitch série : tous incomplets", "roll_pitch", [mk("a", n=(300, 5)), mk("b", n=(5, 300))], "Pitch T.", [0, 1], [1.0, 1.2])
    direct("Roll/pitch série : une métrique", "roll_pitch", [mk("a", nmet=1), mk("b", nmet=1)], "Pitch T.", [0, 1], [1.0, 1.2])
    direct("Roll/pitch série : vol sans second axe lu mais deux métriques", "roll_pitch", [mk("a", n=(300, 5)), mk("b")], "Pitch T.", [0, 1], [1.0, 1.2])
    fl_ref = [mk("a", lat=(15.0, 16.5)), mk("b", n=(5, 5))]
    direct("Roll/pitch référence None", "roll_pitch_ref", fl_ref, ref=None)
    direct("Roll/pitch référence 0", "roll_pitch_ref", fl_ref, ref=0)
    direct("Roll/pitch référence sans gap", "roll_pitch_ref", fl_ref, ref=1)
    direct("Roll/pitch référence hors liste", "roll_pitch_ref", fl_ref, ref=5)
    direct("Roll/pitch référence, une métrique", "roll_pitch_ref", [mk("a", nmet=1)], ref=0)
    direct("FF direct", "ff", [mk("a", Stick_Resp=0.0), mk("b", Stick_Resp=0.5), mk("c", Stick_Resp=1.0)], "Stick Resp.", [0, 1, 2], [0.0, 0.5, 1.0])
    direct("FF : valeurs ex æquo", "ff", [mk("a", n=(300, 300)), mk("b", n=(301, 300))], "Stick Resp.", [0, 1], [0.5, 0.5])
    direct("FF : vol hors liste", "ff", [mk("a")], "Stick Resp.", [0, 3], [0.5, 1.0])
    direct("FF : série vide", "ff", [mk("a")], "Stick Resp.", [], [])
    direct("FF : clé quelconque", "ff", [mk("a"), mk("b")], "Damping", [0, 1], [0.5, 1.0])


def series_flights(rng, key, filt=0):
    """Une série (ou des vols seuls) de 2 à 5 vols d'un groupe de filtres, avec des mesures tirées au hasard dans des valeurs aux seuils."""
    pick = lambda xs: float(rng.choice(xs))  # noqa: E731
    n_fl = int(rng.integers(2, 6))
    if key == "single":
        return [mk(f"s{filt}", filt=filt, peak=pick([1.03, 1.2, NAN]), n=tuple(int(x) for x in rng.choice([5, 100, 300], size=2)),
                   i=pick([0.1, 0.3, 0.4]), ff=(int(rng.choice([0, 100])), 0), angle=pick([0.0, 1.0, NAN]))]
    step = pick([0.2, 0.2, 0.2, 0.1, 0.4, 0.05])
    start = pick([0.6, 0.8, 1.0, 1.2])
    ns = [(300, 300), (300, 300), (150, 300), (300, 150), (500, 500), (250, 250), (10, 300), (300, 10), (10, 10), (199, 200), (200, 200)]
    fl = []
    for j in range(n_fl):
        if rng.random() < 0.15 and j:  # deux vols pour la même valeur : celui qui a le plus de fenêtres (le premier à égalité)
            v = start + step * (j - 1)
        else:
            v = start + step * j
        peak = (pick([1.0, 1.03, 1.045, 1.05, 1.055, 1.07, 1.09, 1.1, 1.105, 1.15, 1.25, NAN]), pick([1.0, 1.03, 1.05, 1.08, 1.12, 1.25, NAN]))
        lat0 = pick([13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 20.0, 0.0, NAN, -1.0])
        shrink = 1.0 - pick([0.0, 0.01, 0.03, 0.039999, 0.04, 0.05, 0.08, 0.12, -0.05]) * j
        lat = (lat0 * shrink, lat0 * shrink + pick([-3.0, -1.0, -0.4, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, NAN]))
        ripple = (pick([0.0, 0.01, 0.0299999, 0.03, 0.05, 0.0649999, 0.065, 0.07, 0.0999999, 0.1, 0.12, NAN]),
                  pick([0.0, 0.01, 0.04, 0.05, 0.065, 0.0649999, 0.08, NAN]))
        rstd = (pick([0.01, 0.03, 0.0300001, 0.05, NAN]), pick([0.01, 0.031, NAN]))
        waves = (pick([1.0, 1.5, 2.4999999, 2.5, 3.0, NAN]), pick([1.0, 2.0, 2.6, NAN]))
        n = ns[int(rng.integers(0, len(ns)))] if rng.random() < 0.5 else (300, 300)
        # le curseur change de façon bruitée (flottants sales) ; les autres restent à 1,0
        fl.append(mk(f"{key}{filt}-{j}", filt=filt, peak=peak, lat=lat, ripple=ripple, rstd=rstd, waves=waves, n=n,
                     ff=(100, 100) if key == "Stick_Resp" else (0, 0), angle=pick([0.0, 1.0]) if key == "Stick_Resp" else 1.0,
                     **{key: v}))
    return fl


def fmt_cases():
    vals = [0.0, -0.0, 0.2, 1.2, 1.15, 0.065, 0.0005, 0.0015, 0.0025, 1.0005, 10.0, 100.0, 0.1 + 0.2, -0.2, 1e-7, 12345.6789, 0.9995,
            0.0004999, 2.675, 1.0005000001, 1000000.0, NAN, float("inf"), -float("inf")]
    fm = [[hx(v), ST._fmt(v)] for v in vals]
    nn = [[hx(v), d, sg, ST._n(v, d, sg)] for v in [*vals, 0.05, 0.15, 0.25, 0.35, 1.5, 2.5, -2.5, -0.001, 0.004999, 1.0049999]
          for d in (0, 1, 2, 3) for sg in (False, True)]
    ga = [[hx(g), ST._gap_action(g)] for g in (1.5, -1.5, 0.0, -0.0, NAN, 1e-9)]
    return fm, nn, ga


def main():
    rng = np.random.default_rng(20261031)
    run(rng)
    fm, nn, ga = fmt_cases()
    out = {"cases": CASES, "fmt": fm, "n": nn, "gap_action": ga}
    text = json.dumps(out, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print(len(CASES), "cas,", sum(1 for c in CASES if c["error"]), "erreurs de Python,", f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)
    print("constats vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    required = ("ff_active", "ff_series_angle", "i_slider_out", "few_windows", "few_windows_unsure", "series_incomplete",
                "series_more_overshoot", "series_already_flat", "series_choice", "master_near_optimum", "master_no_sign", "master_choice",
                "pitch_series_incomplete", "pitch_balance_ok", "pitch_balance_near", "pitch_balance_slower", "ff_series_info")
    needles = ("Affinage possible", "grande incertitude", "vagues non lues", "gain – %", "Le pitch dépasse encore", "le cran suivant ne baisse plus",
               "premier pic roll sans dépassement", "premier pic pitch sans dépassement", "Pas de 0,1 entre deux vols", "Pas de 0,05 entre deux vols",
               "Le roll répond plus lentement", "Le pitch répond plus lentement", "−0,2", "+0,2", "(facultatif)", "(N = ", "latence presque plus",
               "trop haut", "premiers signes", "propre", "cassure +0,", "cassure –", "± ", "pic roll 1,", "pic pitch 1,", "latence roll", "[]")
    alltext = "\n".join(TEXTS)
    missing = [k for k in required if k not in SEEN] + [s for s in needles if s != "[]" and s not in alltext]
    combos = {("master_choice", a, u) for a in (True, False) for u in (True, False)}
    missing += [f"combinaison {c}" for c in sorted(combos - COMBOS, key=str)]
    sev = {c for c in COMBOS if c[0] == "series_choice"}
    for s in ("ok", "conseil", "attention"):
        if not any(c[1] == s for c in sev):
            missing.append(f"series_choice gravité {s}")
    print("jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
