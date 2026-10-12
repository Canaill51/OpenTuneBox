"""Fiche drone et parcours du guide intégré (sans Qt)."""

import dataclasses

import pytest
from corpus_paths import LOGS

from opentunebox.advisor.drone_card import DroneCard, card_for_log, pitch_series
from opentunebox.advisor.log_check import FILTERS, PID, CheckItem, LogCheck, Usage
from opentunebox.advisor.tuning_path import AUTO_BAD, AUTO_OK, auto_status, phases


def _check(items=(), usages=(), sliders=None, label="log", role=FILTERS):
    return LogCheck(label, True, {"hover": None, "moving": (0.0, 20.0)}, list(items), list(usages),
                    sliders=sliders or {}, role=role)


def test_card_values_depend_on_card():
    small = DroneCard("a", bf="4.4", size="five", pid_khz=8.0)
    big = DroneCard("b", bf="2025.12", size="seven", pid_khz=3.2, esc="BLHeli_S")
    assert small.log_rate() == (2000.0, "1/4") and big.log_rate() == (1600.0, "1/2")
    assert small.debug_mode() == "GYRO_SCALED" and big.debug_mode() == "FFT_FREQ"
    assert small.angle_limit_setting() == "level_limit" and big.angle_limit_setting() == "angle_limit"
    assert small.damping_series()[0] == 0.6 and big.damping_series()[0] == 1.0
    assert small.i_range() == (0.8, 1.0) and big.i_range() == (0.6, 0.8)
    assert not big.bidir
    assert DroneCard.from_dict(big.to_dict()) == big
    assert DroneCard.from_dict({"name": "x", "size": "inconnue", "autre": 1}).size == "five"


def test_card_for_log_and_pitch_series():
    cards = [DroneCard("JeNo 5"), DroneCard("Air65")]
    assert card_for_log(cards, " jeno 5 ") is cards[0] and card_for_log(cards, "") is None
    assert pitch_series(1.0, +1) == [1.2, 1.4, 1.6] and pitch_series(0.6, -1) == [0.4, 0.2]


def test_every_step_has_text_and_unique_key():
    for card in (DroneCard("a"), DroneCard("b", bf="4.4", size="large", esc="BLHeli_S")):
        keys = [s.key for ph in phases(card) for s in ph.steps]
        assert len(keys) == len(set(keys))
        for ph in phases(card):
            assert ph.intro
            for s in ph.steps:
                assert s.text(card) and s.help_text(card)


def test_auto_status_from_checks():
    steps = {s.key: s for ph in phases(DroneCard("a")) for s in ph.steps}
    good = _check([CheckItem("rpm", "Télémétrie RPM", "ok", "Présente.")])
    bad = _check([CheckItem("rpm", "Télémétrie RPM", "à revoir", "Absente.")])
    assert auto_status(steps["bidir"], [])[0] is None
    assert auto_status(steps["bidir"], [bad, good])[0] == AUTO_OK  # le log le plus récent compte
    assert auto_status(steps["bidir"], [good, bad])[0] == AUTO_BAD
    assert auto_status(steps["sliders"], [good])[0] == AUTO_OK  # pas de point « réglage simplifié » : actif
    series_ok = Usage("series", "Vol d'une série PID", True, "conditions respectées.")
    flight = _check(usages=[series_ok], sliders={"Damping": 0.8}, role=PID)
    assert auto_status(steps["Damping_0.8"], [flight])[0] == AUTO_OK
    assert auto_status(steps["Damping_1.0"], [flight])[0] is None
    ff_ok = Usage("ff_series", "Vol d'une série Feedforward", True, "conditions respectées.")
    ff = _check(usages=[series_ok, ff_ok], sliders={"Stick Resp.": 0.5}, role=PID)
    assert auto_status(steps["Stick Resp._0.5"], [ff])[0] == AUTO_OK and "ff_read" in steps
    ff.usages[1] = Usage("ff_series", "Vol d'une série Feedforward", False, "en mode Angle.")
    assert auto_status(steps["Stick Resp._0.5"], [ff])[0] == AUTO_BAD  # la série PID lisible ne suffit pas


def test_steps_read_logs_of_their_role():
    """Logs des filtres et logs des séries PID jugés à part : un vol de série PID enregistré lentement ne fait
    pas échouer la case d'enregistrement des filtres."""
    steps = {s.key: s for ph in phases(DroneCard("a")) for s in ph.steps}
    slow = CheckItem("log_rate", "Fréquence d'enregistrement (filtres)", "bloquant", "1000 Hz.")
    pid_ok = CheckItem("pid_log_rate", "Fréquence d'enregistrement (PID)", "ok", "1000 Hz.")
    filt = _check([CheckItem("log_rate", "Fréquence d'enregistrement (filtres)", "ok", "2000 Hz.")], label="filtres")
    pid = _check([slow, pid_ok], label="série", role=PID)
    assert auto_status(steps["recording"], [filt, pid])[0] == AUTO_OK  # le log PID, plus récent, ne compte pas
    assert auto_status(steps["pid_recording"], [filt, pid])[0] == AUTO_OK
    assert auto_status(steps["pid_recording"], [filt])[0] is None
    assert auto_status(steps["retrieve"], [pid])[0] is None and auto_status(steps["pid_logs"], [pid])[0] == AUTO_OK
    assert steps["retrieve"].open_logs and steps["pid_logs"].open_logs and steps["ff_logs"].open_logs
    assert pid.items_for() == [pid_ok] and filt.items_for(PID) == []


@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")
def test_auto_status_on_real_log():
    from opentunebox.advisor.log_check import check_log, filter_targets
    from opentunebox.analysis.period import auto_period
    from opentunebox.log.flightlog import load

    lg = load(LOGS / "Cedric/JeNo 5/FINAL.bbl")[0]
    c = check_log(lg, lg.slice_time(*auto_period(lg)), "FINAL")
    steps = {s.key: s for ph in phases(DroneCard("JeNo 5")) for s in ph.steps}
    assert auto_status(steps["recording"], [c])[0] == AUTO_OK
    assert c.role == FILTERS and auto_status(steps["log_rates"], [c])[0] is None  # vol avec stationnaire
    pid = dataclasses.replace(c, role=PID)  # lu comme vol de série : rates du pilote, pas le profil « LOG »
    assert auto_status(steps["log_rates"], [pid])[0] == AUTO_BAD
    state, txt = auto_status(steps["filter_targets"], [c], filter_targets(lg, c.parts, 5))
    assert state == AUTO_BAD and "Terme D" in txt
