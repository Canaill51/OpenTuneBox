"""Règles de la réponse indicielle (seuils de pic, marges) et vérification des logs du guide (log_check), sur des
réponses fabriquées et des logs synthétiques, sans corpus."""

from types import SimpleNamespace

import pytest

from opentunebox.advisor import rules
from opentunebox.advisor.profiles import PROFILES
from opentunebox.advisor.rules import ATTENTION, CONSEIL, OK, AxisStep
from opentunebox.analysis.loop import MEASURED, LoopMetrics
from opentunebox.analysis.step import StepMetrics

from synthetic import make_flightlog


def _step_findings(peak, lat, ff=0, profile="five", n=50, ff_cut=False, **loop):
    r = SimpleNamespace(n=n, metrics=StepMetrics(peak=peak, latency_ms=lat), loop=LoopMetrics(**loop))
    F = []
    rules._step_rules(F, AxisStep("Roll", r), PROFILES[profile], ff, ff_cut)
    return F


def _titles(*a, **kw):
    return [(f.title, f.severity) for f in _step_findings(*a, **kw)]


def test_step_peak_thresholds_without_feedforward():
    """Paliers 1,20 (conseil) et 1,25 (attention) sans feedforward ; réponse molle sous 1,01 et lente."""
    assert _titles(1.20, 15) == [("Réponse indicielle roll", OK)]
    assert _titles(1.21, 15) == [("Léger dépassement en roll", CONSEIL)]
    assert _titles(1.25, 15) == [("Léger dépassement en roll", CONSEIL)]
    assert _titles(1.26, 15) == [("Fort dépassement en roll", ATTENTION)]
    f = _step_findings(1.26, 15)[0]
    assert "> 1,25 sans feedforward" in f.detail and "Damping" in f.actions[0]
    assert _titles(1.00, 22.1) == [("Réponse molle en roll", CONSEIL)]  # latence > 22 ms (5")
    assert _titles(1.01, 22.1) == [("Réponse indicielle roll", OK)]  # pic au p10 du corpus : pas molle
    assert _titles(1.00, 22.0) == [("Réponse indicielle roll", OK)]
    assert _titles(1.00, 22.1, profile="large") == [("Réponse indicielle roll", OK)]  # 8"+ : jusqu'à 30 ms
    assert _titles(1.5, 15, n=19) == []  # pas assez de fenêtres


def test_slider_steps():
    """Pas des curseurs choisis par Cédric : Damping +0,2 pour dégrossir (fort dépassement), 0,1 en réglage fin ;
    plus de Tracking ; feedforward renvoyé à la série du guide (pas de 0,5 depuis 0)."""
    assert _step_findings(1.26, 15)[0].actions == ["Curseur Damping (D) : +0,2."]
    assert _step_findings(1.21, 15)[0].actions == ["Curseur Damping (D) : +0,1."]
    soft = _step_findings(1.00, 22.1)[0].actions
    assert soft[0] == "Curseur Damping (D) : −0,1." and "0, 0,5, 1" in soft[1]
    ff = _step_findings(1.61, 9.9, ff=100)[0].actions
    assert "0, 0,5, 1" in ff[0] and "Acro" in ff[0]
    for peak, lat, ff_gain in ((1.26, 15, 0), (1.21, 15, 0), (1.00, 22.1, 0), (1.61, 9.9, 100)):
        assert not any("Tracking" in a for f in _step_findings(peak, lat, ff=ff_gain) for a in f.actions)

def test_step_windows_warning():
    """Sous 200 fenêtres [S35], les constats du pic disent « mesure moins sûre » ; pas les marges (boucle ouverte)."""
    f = _step_findings(1.26, 15, n=199)[0]
    assert f.detail.endswith("Mesure moins sûre : 199 fenêtres (200 conseillées, soit environ 20 s de wobbles roll "
                             "et pitch mêlés).")
    assert "moins sûre" in _step_findings(1.0, 15, n=20)[0].detail  # ligne « ok » aussi
    assert "moins sûre" not in _step_findings(1.26, 15, n=200)[0].detail
    edge = dict(sensitivity_peak_db=7.1, sensitivity_peak_hz=20.0)
    margin = [x for x in _step_findings(1.0, 15, n=50, **edge) if x.title.startswith("Marge")][0]
    assert "moins sûre" not in margin.detail


def test_step_peak_thresholds_with_feedforward():
    """Avec feedforward : paliers 1,60 / 1,70 ; montée plus rapide que la latence basse du profil → le
    feedforward est en cause, pas le D."""
    assert _titles(1.60, 15, ff=100) == [("Réponse indicielle roll", OK)]
    assert _titles(1.61, 15, ff=100) == [("Léger dépassement en roll", CONSEIL)]
    assert _titles(1.71, 15, ff=100) == [("Fort dépassement en roll", ATTENTION)]
    assert _titles(1.61, 9.9, ff=100) == [("Dépassement dû au feedforward en roll", CONSEIL)]  # latence < 10 ms (5")
    assert _titles(1.71, 9.9, ff=100) == [("Dépassement dû au feedforward en roll", ATTENTION)]
    assert _titles(1.61, 10.0, ff=100) == [("Léger dépassement en roll", CONSEIL)]
    f = _step_findings(1.61, 9.9, ff=100)[0]
    assert "F = 100" in f.detail and "série Feedforward" in f.actions[0]


def test_step_peak_angle_mode_uses_thresholds_without_feedforward():
    """Mode Angle : le firmware coupe le feedforward (L-005) ; F > 0 dans l'en-tête ne change pas les paliers et
    le dépassement n'est jamais attribué au feedforward."""
    from opentunebox.analysis.step_series import FlightConditions
    angle, acro = FlightConditions((100, 120), 0.95, 1.0), FlightConditions((100, 120), 0.5, 1.0)
    assert (angle.ff_gain(0), angle.ff_gain(1), acro.ff_gain(1)) == (0, 0, 120)
    assert _titles(1.30, 9.9, ff=angle.ff_gain(0), ff_cut=True) == [("Fort dépassement en roll", ATTENTION)]
    assert "sans feedforward (coupé par le mode Angle)" in _step_findings(1.21, 15, ff_cut=True)[0].detail


def test_stability_margin_rules():
    edge = dict(sensitivity_peak_db=7.1, sensitivity_peak_hz=20.0)
    assert ("Marge de stabilité faible en roll", CONSEIL) in _titles(1.0, 15, **edge)
    assert ("Marge de stabilité faible en roll", CONSEIL) not in _titles(1.0, 15, sensitivity_peak_db=7.0)
    f = [x for x in _step_findings(1.0, 15, sensitivity_peak_at_edge=True, **edge) if x.title.startswith("Marge")][0]
    assert "au moins 7.1 dB" in f.detail and "Valeur approchée" in f.detail and f.actions[0] == "Master −0,1 à −0,2."
    assert "Mesure arrêtée à 20 Hz" in f.actions[-1] and "wobbles" in f.actions[-1]  # W-004
    assert not any("chirp" in a.lower() for a in f.actions)  # D-019
    f = [x for x in _step_findings(1.0, 15, method=MEASURED, d_ratio=1.5, **edge) if x.title.startswith("Marge")][0]
    assert "Valeur approchée" not in f.detail and "Le D domine déjà" in f.actions[0]
    assert not any(a.startswith("Mesure arrêtée") for a in f.actions)  # pic dans la bande
    f = [x for x in _step_findings(1.0, 15, method=MEASURED, **edge) if x.title.startswith("Marge")][0]
    assert f.actions[0] == "Damping (D) +0,1, ou Master −0,1 à −0,2."
    f = [x for x in _step_findings(1.0, 15, profile="whoop", **edge) if x.title.startswith("Marge")][0]
    assert "Fréquent sur un whoop" in f.detail
    assert ("Marge de phase réduite en roll", CONSEIL) in _titles(1.0, 15, phase_margin_deg=34.9, crossover_hz=25.0)
    assert not any(t.startswith("Marge") for t, _s in _titles(1.0, 15, phase_margin_deg=35.0, crossover_hz=25.0))
    # pic de sensibilité trop haut ET marge de phase faible : un seul constat (le pic)
    both = _titles(1.0, 15, phase_margin_deg=20.0, crossover_hz=25.0, **edge)
    assert sum(t.startswith("Marge") for t, _s in both) == 1


# ---------------------------------------------------------------- vérification des logs du guide


def test_log_check_rates_versions_and_fields():
    from opentunebox.advisor.log_check import BLOCK, OK, PID, WARN, check_log

    c = check_log(make_flightlog(seconds=12.0))
    assert c.item("version").status == OK and c.item("duration").status == WARN  # 12 s < 15 s
    assert c.item("log_rate").status == OK and c.item("pid_log_rate").status == OK
    assert c.item("gyro_unfilt").status == OK and c.item("rpm").status == OK
    assert c.item("debug_mode").status == WARN and "FFT_FREQ" in c.item("debug_mode").detail  # notch dynamique sans FFT_FREQ
    assert c.role == PID and c.moving
    # 990 Hz pour une boucle PID à 8 kHz : bloquant pour les filtres (2 kHz), assez pour les PID (1 kHz)
    c = check_log(make_flightlog(seconds=12.0, fs=990.0))
    assert c.item("log_rate").status == BLOCK and "2000 Hz" in c.item("log_rate").detail
    assert c.item("pid_log_rate").status == OK
    assert check_log(make_flightlog(seconds=12.0, fs=600.0)).item("pid_log_rate").status == WARN
    assert check_log(make_flightlog(seconds=12.0, fs=450.0)).item("pid_log_rate").status == BLOCK
    # 3e harmonique moteur repliée (moteurs à 480 Hz, log à 2 kHz) : à revoir
    c = check_log(make_flightlog(seconds=12.0, motor_hz=480.0))
    assert c.item("log_rate").status == WARN and "3e harmonique" in c.item("log_rate").detail
    # version trop ancienne
    old = check_log(make_flightlog(seconds=6.0, **{"Firmware revision": "Betaflight 4.3.2 (abcd1234) STM32F405"}))
    assert old.item("version").status == BLOCK and old.item("debug_mode") is None  # famille 4.4 : pas de FFT_FREQ exigé
    assert check_log(make_flightlog(seconds=6.0, **{"debug_mode_name": "FFT_FREQ"})).item("debug_mode") is None
    # gyro non filtré absent : remède selon la version
    c44 = check_log(make_flightlog(seconds=6.0, unfilt=False, **{"Firmware revision": "Betaflight 4.4.3 (abcd1234) STM32F405"}))
    assert c44.item("gyro_unfilt").status == BLOCK and "GYRO_SCALED" in c44.item("gyro_unfilt").detail
    c45 = check_log(make_flightlog(seconds=6.0, unfilt=False))
    assert c45.item("gyro_unfilt").status == BLOCK and "désactivé" in c45.item("gyro_unfilt").detail
    assert not c45.usage("noise").ok and not c45.usage("delay").ok
    assert check_log(make_flightlog(seconds=6.0, rpm=False)).item("rpm").status == WARN


def test_log_check_roles_and_usages():
    from opentunebox.advisor.log_check import FILTERS, PID, ROLE_ITEMS, check_log

    hover = check_log(make_flightlog(seconds=25.0, moving=False))
    assert hover.role == FILTERS and not hover.moving and hover.parts["hover"] is not None
    assert hover.usage("noise").ok and not hover.usage("delay").ok and not hover.usage("series").ok
    assert not {it.key for it in hover.items_for(FILTERS)} & set(ROLE_ITEMS[PID])
    moving = check_log(make_flightlog(seconds=25.0))
    assert moving.role == PID and moving.usage("delay").ok
    assert not {it.key for it in moving.items_for(PID)} & set(ROLE_ITEMS[FILTERS])
    assert {it.key for it in moving.items} >= {"pid_log_rate", "windows", "axes_mixed"}
    assert moving.item("flight_mode").status != "ok" or moving.usage("series").ok is not None
    assert all(it.key in {k for keys in ROLE_ITEMS.values() for k in keys} or it.key in
               {"version", "duration", "parts", "rates", "angle_limit", "flight_mode", "simplified", "crash", "gaps"}
               for it in moving.items), [it.key for it in moving.items]


@pytest.mark.parametrize("flags, title", [(0b011, "mode Angle"), (0b001, "Acro")])
def test_log_check_flight_mode(flags, title):
    from opentunebox.advisor.log_check import OK, check_log

    c = check_log(make_flightlog(seconds=25.0, mode_flags=flags, ff_weight="0,0,0", simplified_i_gain="25"))
    assert c.item("flight_mode").status == OK and title in c.item("flight_mode").detail
    assert c.item("i_slider") is None and c.usage("series").ok
    c = check_log(make_flightlog(seconds=25.0, mode_flags=flags, ff_weight="0,0,0"))  # curseur I à 1,0 : hors plage
    assert c.item("i_slider") is not None and not c.usage("series").ok and "Curseur I" in c.usage("series").reason


def test_recording_gaps_finding():
    """Un trou dans la période analysée (pause d'écriture) : constat informatif dans l'assistant et dans la
    vérification des logs ; les analyses écartent les fenêtres qui le chevauchent (D-020)."""
    from opentunebox.advisor.log_check import INFO, check_log
    from opentunebox.advisor.rules import analyze

    log = make_flightlog(seconds=25.0)
    assert log.recording_gaps() == (0, 0.0)
    assert "Trous dans l'enregistrement" not in {f.title for f in analyze(log).findings}
    assert check_log(log).item("gaps") is None
    log.t[log.n // 2:] += 0.2  # 0,2 s perdues au milieu du vol
    n, lost = log.recording_gaps()
    assert n == 1 and lost == pytest.approx(0.2, abs=1e-6)
    f = next(f for f in analyze(log).findings if f.title == "Trous dans l'enregistrement")
    assert f.severity == rules.INFO and "1 trou dans la période analysée (0,2 s perdues" in f.detail
    assert check_log(log).item("gaps").status == INFO
