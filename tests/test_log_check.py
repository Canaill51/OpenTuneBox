"""Vérification rapide des logs (étape « Vérifier mes logs » du guide intégré)."""

import time

import numpy as np
import pytest
from corpus_paths import LOGS

from opentunebox.advisor.log_check import BLOCK, CONFIGURATOR_SLIDERS, OK, WARN, check_log, check_logs
from opentunebox.analysis.sliders import PTB_CATEGORIES
from opentunebox.analysis.step import input_windows, step_response


def _load(rel):
    from opentunebox.analysis.period import auto_period
    from opentunebox.log.flightlog import load

    lg = load(LOGS / rel)[0]
    return lg, lg.slice_time(*auto_period(lg))


def _item(c, title):
    return next((it for it in c.items if it.title == title), None)


def _usage(c, label):
    return next(u for u in c.usages if u.label == label)


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


@pytest.mark.parametrize("rel", ["1. PTB/BlackSummer/Damping 0.8.BFL"])
@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")
def test_low_rate_series_flight(rel):
    lg, sel = _load(rel)
    c = check_log(lg, sel, "d08")
    rate = _item(c, "Fréquence d'enregistrement")
    assert rate.status == BLOCK and rate.fixes[0].cli == "set blackbox_sample_rate = 1/4"
    assert "Blackbox" in rate.fixes[0].where  # configurateur d'abord
    assert not _usage(c, "Identifier le bruit (filtres)").ok
    assert _usage(c, "Vol d'une série PID").ok  # 1 kHz suffit pour la réponse indicielle


@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")
def test_feedforward_flight_refused_for_series():
    lg, sel = _load("Cedric/JeNo 5/FINAL.bbl")
    t0 = time.perf_counter()
    c = check_log(lg, sel)
    assert time.perf_counter() - t0 < 0.5  # vérification instantanée, même sur un vol de 3 min
    assert _item(c, "Fréquence d'enregistrement").status == OK
    assert _item(c, "Feedforward").status == WARN
    assert not _usage(c, "Vol d'une série PID").ok
    assert _usage(c, "Juger le retard des filtres").ok and c.moving
    assert "set simplified_feedforward_gain = 0" in c.cli_commands()


@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")
def test_hover_flight_noise_only():
    lg, sel = _load("1. PTB/Kris/BTFL_BLACKBOX_LOG_TP_20260914_181415_F722_AIO.bbl")
    c = check_log(lg, sel)
    assert not c.moving
    assert "stationnaire" in _usage(c, "Juger le retard des filtres").reason or \
        not _usage(c, "Juger le retard des filtres").ok


@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")
def test_series_groups():
    a, _ = _load("1. PTB/BlackSummer/Damping 0.8.BFL")
    b, _ = _load("1. PTB/BlackSummer/Damping 1.2.BFL")
    groups = check_logs([("a", a), ("b", b)])
    assert len(groups) == 1 and groups[0].status == OK and "Atténuation (Damping)" in groups[0].detail
    other, _ = _load("Cedric/JeNo 5/FINAL.bbl")
    assert len(check_logs([("a", a), ("c", other)])) == 0  # drones différents : pas de groupe
