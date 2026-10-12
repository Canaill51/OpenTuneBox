"""Vérification des logs pour le guide intégré : usages, protocole des vols avec mouvements, parties du vol."""

import time

import numpy as np
import pytest
from corpus_paths import LOGS

from opentunebox.advisor.log_check import (AXES_MIXED_MIN, BLOCK, CONFIGURATOR_SLIDERS, INFO, OK, WARN, check_log,
                                           check_logs)
from opentunebox.analysis.sliders import PTB_CATEGORIES
from opentunebox.analysis.step import input_windows, step_response

corpus = pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")


def _load(rel):
    from opentunebox.analysis.period import auto_period
    from opentunebox.log.flightlog import load

    lg = load(LOGS / rel)[0]
    return lg, lg.slice_time(*auto_period(lg))


def test_every_slider_has_a_configurator_label():
    assert set(CONFIGURATOR_SLIDERS) == set(PTB_CATEGORIES)


def test_input_windows_counts_step_candidates():
    fs = 1000.0
    t = np.arange(0, 30, 1 / fs)
    sp = np.where(t < 10, 5.0, 200 * np.sign(np.sin(2 * np.pi * 0.5 * t)))  # 10 s calmes puis des marches
    n = input_windows(sp, fs)
    assert 0 < n < 30 * 16  # pas de 62,5 ms : 16 fenêtres par seconde au plus
    assert n >= step_response(sp, sp, fs).n  # le contrôle qualité n'ajoute jamais de fenêtre
    assert input_windows(np.full_like(sp, 5.0), fs) == 0


def test_flight_parts_splits_hover_then_moves():
    from types import SimpleNamespace

    from opentunebox.analysis.period import flight_parts

    fs = 500.0
    t = np.arange(0, 60, 1 / fs)
    rng = np.random.default_rng(0)
    sp = np.where(t < 30, rng.normal(0, 5, t.size), 300 * np.sin(2 * np.pi * 1.3 * t))
    sp[int(12 * fs):int(12.3 * fs)] = 200  # un coup de manche isolé ne coupe pas le stationnaire
    log = SimpleNamespace(t=t, fs=fs, n=t.size, setpoint=np.vstack([sp, sp * 0.5, sp * 0]),
                          slice_time=lambda a, b: slice(int(a * fs), int(b * fs) + 1))
    parts = flight_parts(log, 0.0, 59.9)
    assert parts["hover"][0] < 1 and 28 < parts["hover"][1] < 32
    assert 28 < parts["moving"][0] < 32 and parts["moving"][1] > 58


@corpus
@pytest.mark.corpus
def test_low_rate_angle_wobble_flight():
    lg, sel = _load("1. PTB/Sohail/1.4mm.bbl")  # mode Angle, rates 200/200/0, limite d'angle 60°
    c = check_log(lg, sel, "sohail")
    rate = c.item("log_rate")
    assert rate.status == BLOCK and rate.fixes[0].cli == "set blackbox_sample_rate = 1/2"
    assert "Blackbox" in rate.fixes[0].where  # configurateur d'abord
    assert c.item("rates").status == OK and c.item("flight_mode").status == OK
    assert c.item("angle_limit").status == WARN and "set angle_limit = 30" in c.cli_commands()
    assert c.item("axes_mixed").status == OK
    assert not c.usage("noise").ok and c.parts["hover"] is None
    series = c.usage("series")
    assert series.ok and "limite d'angle" in series.reason  # lisible, mais à améliorer


@corpus
@pytest.mark.corpus
def test_feedforward_flight_refused_for_series():
    lg, sel = _load("Cedric/JeNo 5/FINAL.bbl")
    t0 = time.perf_counter()
    c = check_log(lg, sel)
    assert time.perf_counter() - t0 < 0.5  # vérification rapide, même sur un vol de 3 min
    assert c.item("log_rate").status == OK and c.item("flight_mode").status == INFO
    assert c.item("rates").status == WARN and "set rates_type = ACTUAL" in c.cli_commands()
    assert not c.usage("series").ok and "feedforward à 0" in c.usage("series").reason
    assert c.usage("delay").ok and c.moving


@corpus
@pytest.mark.corpus
def test_acro_without_feedforward_accepted():
    lg, sel = _load("Cedric/Air65/MM.bbl")  # Acro, feedforward à 0, 1 kHz : vol de série PID
    c = check_log(lg, sel)
    assert c.role == "pid" and c.item("flight_mode").status == OK and "Acro" in c.item("flight_mode").detail
    assert c.item("angle_limit") is None  # sans effet en Acro
    assert c.item("pid_log_rate").status == OK and c.usage("series").ok
    assert "log_rate" not in [it.key for it in c.items_for()]  # fréquence des filtres hors sujet


@corpus
@pytest.mark.corpus
def test_feedforward_series_flight_in_acro():
    from opentunebox.analysis.period import auto_period
    from opentunebox.log.flightlog import load

    lg = load(LOGS / "Cedric/Air65/FF.bbl")[1]  # 2e vol du fichier : feedforward 1,0, en Acro
    c = check_log(lg, lg.slice_time(*auto_period(lg)))
    mode = c.item("flight_mode")
    assert mode.status == INFO and mode.detail.startswith("Acro, feedforward actif")
    assert c.item("angle_limit") is None  # sans effet hors mode Angle
    assert c.usage("ff_series").ok and not c.usage("series").ok
    assert c.usage("series").reason.startswith("feedforward actif en Acro")


@corpus
@pytest.mark.corpus
def test_one_axis_at_a_time_is_flagged():
    lg, sel = _load("1. PTB/Kris/BTFL_BLACKBOX_LOG_TP_20260914_181415_F722_AIO.bbl")
    c = check_log(lg, sel)
    mixed = c.item("axes_mixed")
    assert mixed.status == WARN and f"{AXES_MIXED_MIN:.0%}" in mixed.detail
    assert c.parts["hover"] is not None and c.parts["hover"][1] - c.parts["hover"][0] > 100


@corpus
@pytest.mark.corpus
def test_exact_windows_and_series_groups():
    a, sel = _load("1. PTB/BlackSummer/Damping 0.8.BFL")
    c = check_log(a, sel, exact=True)
    assert c.exact and min(c.windows) >= 200 and c.item("windows").status == OK
    assert min(c.windows) <= min(check_log(a, sel).windows)  # l'estimation rapide surestime
    b, _ = _load("1. PTB/BlackSummer/Damping 1.2.BFL")
    groups = check_logs([("a", a), ("b", b)])
    assert len(groups) == 1 and groups[0].status == OK and "Atténuation (Damping)" in groups[0].detail
    other, _ = _load("Cedric/JeNo 5/FINAL.bbl")
    assert len(check_logs([("a", a), ("c", other)])) == 0  # drones différents : pas de groupe


@corpus
@pytest.mark.corpus
def test_several_series_of_one_drone():
    """Logs PID d'un drone qui forment plusieurs séries : chacune reconnue, pas de « plusieurs curseurs »."""
    from opentunebox.analysis.period import auto_period
    from opentunebox.log.flightlog import load

    logs = [(lg.name, lg) for f in ("D", "MM", "FF") for lg in load(LOGS / f"Cedric/Air65/{f}.bbl")]
    details = [g.detail for g in check_logs(logs)]
    assert len(details) == 3 and all(g.status == OK for g in check_logs(logs))
    assert any(d.startswith("Série « Atténuation (Damping) » : 1,00 → 1,20 (D, D.02)") for d in details)
    assert any("Réponse des sticks" in d and "(FF, FF.02)" in d for d in details)
    checks = {n: check_log(lg, lg.slice_time(*auto_period(lg)), n) for n, lg in logs}
    ff = next(g.detail for g in check_logs(logs, {n: c.sp_delay_ms for n, c in checks.items()})
              if "Réponse des sticks" in g.detail)
    d0, d1 = checks["FF"].sp_delay_ms, checks["FF.02"].sp_delay_ms
    assert d1[0] < d0[0] - 5 and "retard consigne → gyro roll / pitch : 0,00 → " in ff and "Assistant" in ff
    ff0, ff1 = (check_log(lg, lg.slice_time(*auto_period(lg))) for _n, lg in logs[-2:])
    assert "ff_series" not in [u.key for u in ff0.usages_for()]  # feedforward à 0 : pas redit
    assert "ff_series" in [u.key for u in ff1.usages_for()]
