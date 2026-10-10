"""Diagnostic du réglage par la réponse indicielle (advisor/step_tuning.py), vols fabriqués."""

from opentunebox.advisor.rules import CONSEIL, OK
from opentunebox.advisor.step_tuning import step_tuning
from opentunebox.analysis.sliders import PTB_CATEGORIES
from opentunebox.analysis.step import StepMetrics
from opentunebox.analysis.step_series import FlightConditions, FlightStep, find_series

FILTERS = ("filtres",)
ALIASES = {"Pitch_T": "Pitch T.", "Pitch_D": "Pitch D."}  # noms de curseurs utilisables en argument


def flight(label, peak=1.03, lat=(15.0, 15.5), ripple=(0.0, 0.0), waves=1.1, ff=(0, 0), angle=1.0, i=0.3,
           filters=FILTERS, **sliders):
    sl = {k: 1.0 for k in PTB_CATEGORIES} | {"Drift-Wobble": i}
    sl.update({ALIASES.get(k, k): v for k, v in sliders.items()})
    pk = peak if isinstance(peak, tuple) else (peak, peak)
    m = [StepMetrics(peak=pk[a], latency_ms=lat[a], ripple=ripple[a], ripple_std=0.01, waves_ratio=waves)
         for a in range(2)]
    return FlightStep(label, sl, filters, FlightConditions(ff, angle, i), m, [300, 300], [16.0, 16.5])


def block(rep, start):
    return next(b for b in rep.blocks if b.title.startswith(start))


def test_find_series_one_slider_only():
    fl = [flight("a", Damping=0.8), flight("b", Damping=1.0), flight("c", Damping=1.2, Master=1.2),
          flight("d", Damping=1.0, filters=("autres",))]
    series = find_series(fl)
    assert [(s.key, s.flights, s.values) for s in series] == [("Damping", [0, 1], [0.8, 1.0])]


def test_damping_first_without_overshoot():
    fl = [flight("d06", Damping=0.6, peak=1.22), flight("d08", Damping=0.8, peak=1.12),
          flight("d10", Damping=1.0, peak=1.03), flight("d12", Damping=1.2, peak=1.02)]
    b = block(step_tuning(fl), "Équilibre P/D")
    assert b.best == 2 and b.finding.severity == OK and "1,0" in b.finding.title


def test_damping_judged_on_roll_pitch_by_pitch_d():
    """Air65 : roll sans dépassement à 1,2 (1,052 affiché 1,05), pitch encore 1,08 → Damping 1,2 et Pitch D."""
    fl = [flight("d10", Damping=1.0, peak=(1.08, 1.11)), flight("d12", Damping=1.2, peak=(1.052, 1.08))]
    b = block(step_tuning(fl), "Équilibre P/D")
    assert b.best == 1 and "1,2" in b.finding.title
    assert b.finding.severity == CONSEIL and any(a.startswith("Pitch D.") for a in b.finding.actions)
    # série Pitch D. : jugée sur le pic du pitch
    fl = [flight("p10", Pitch_D=1.0, peak=(1.03, 1.10)), flight("p12", Pitch_D=1.2, peak=(1.03, 1.04))]
    b = block(step_tuning(fl), "Équilibre roll / pitch (série Pitch D.)")
    assert b.best == 1 and b.finding.severity == OK


def test_damping_peak_plateau_and_too_low():
    # le pic ne baisse plus de 1,2 à 1,4 : 1,2 suffit
    fl = [flight("a", Damping=1.0, peak=1.11), flight("b", Damping=1.2, peak=1.06), flight("c", Damping=1.4, peak=1.055)]
    assert block(step_tuning(fl), "Équilibre P/D").best == 1
    # encore du dépassement en fin de série : voler plus haut
    fl = [flight("a", Damping=0.8, peak=1.2), flight("b", Damping=1.0, peak=1.12)]
    b = block(step_tuning(fl), "Équilibre P/D")
    assert b.best is None and "1,2" in b.finding.actions[0]


def test_master_stops_before_clear_ripple():
    """Série de l'Air65 : cassure nette à 1,2 → garder 1,0 (choix de Cédric)."""
    fl = [flight("mm1", Master=1.0, lat=(13.3, 15.4), ripple=(0.014, 0.039)),
          flight("mm2", Master=1.2, lat=(12.5, 14.1), ripple=(0.080, 0.058))]
    b = block(step_tuning(fl), "Niveau des gains")
    assert b.best == 0 and "1,0" in b.finding.title
    assert any("1,1" in a for a in b.finding.actions)  # affinage entre les deux crans


def test_master_plateau_with_ripple_stops_early_signs_do_not():
    lat = [(18.0, 18.5), (16.5, 17.0), (16.2, 16.8)]  # dernier cran : à peine 2 % de gain
    fl = [flight("a", Master=1.0, lat=lat[0], ripple=(0.0, 0.0)),
          flight("b", Master=1.2, lat=lat[1], ripple=(0.02, 0.03)),
          flight("c", Master=1.4, lat=lat[2], ripple=(0.055, 0.05))]
    assert block(step_tuning(fl), "Niveau des gains").best == 1
    fl[2] = flight("c", Master=1.4, lat=lat[2], ripple=(0.035, 0.03))  # plateau mais petite cassure
    b = block(step_tuning(fl), "Niveau des gains")
    assert b.best == 2 and b.finding.severity == CONSEIL and "optimum" in b.finding.title


def test_master_waves_stop():
    fl = [flight("a", Master=1.0, lat=(18.0, 18.5)), flight("b", Master=1.2, lat=(16.5, 17.0), waves=4.0)]
    assert block(step_tuning(fl), "Niveau des gains").best == 0


def test_roll_pitch_gap():
    # pitch 2,1 ms plus lent : monter Pitch T.
    b = block(step_tuning([flight("a", lat=(13.3, 15.4))]), "Équilibre roll / pitch")
    assert b.finding.severity == CONSEIL and "+0,2" in b.finding.actions[0]
    assert block(step_tuning([flight("a", lat=(14.0, 14.6))]), "Équilibre roll / pitch").finding.severity == OK
    # série Pitch T. : la valeur qui rapproche le plus les latences sans cassure sur le pitch
    fl = [flight("p10", Pitch_T=1.0, lat=(14.0, 17.0)), flight("p12", Pitch_T=1.2, lat=(14.0, 15.5)),
          flight("p14", Pitch_T=1.4, lat=(14.0, 14.4), ripple=(0.0, 0.09))]
    b = block(step_tuning(fl), "Équilibre roll / pitch")
    assert b.best == 1


def test_conditions():
    fl = [flight("acro", ff=(120, 125), angle=0.0, i=1.0, Master=1.0), flight("angle", ff=(120, 125), Master=1.2)]
    rep = step_tuning(fl)
    titles = [c.title for c in rep.conditions]
    assert "Feedforward actif" in titles and "Curseur I hors de 0,2–0,3" in titles
    assert "acro" in rep.conditions[0].detail and "angle" not in rep.conditions[0].detail.replace("Angle", "")
    fl = [flight("a", Damping=1.0, peak=1.1), flight("b", Damping=1.1, peak=1.04)]
    assert "trop fin" in block(step_tuning(fl), "Équilibre P/D").finding.detail
