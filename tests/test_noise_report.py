"""Diagnostic du bruit (advisor/noise.py) sans corpus : rangement des constats, blocs par filtre et leurs
verdicts selon ce que contient le log (eRPM, gyro non filtré, filtres coupés, boucle PID lente)."""

import dataclasses

import numpy as np
import pytest

from opentunebox.advisor import noise
from opentunebox.advisor.noise import NoiseReport, lowpass_text, noise_report, section_of
from opentunebox.advisor.rules import ATTENTION, CONSEIL, INFO, OK, Finding
from opentunebox.log.settings import LowpassSetting, parse_settings
from synthetic import BASE_HEADER, make_flightlog


def _f(key, category="Bruit", severity=INFO, actions=(), title="titre"):
    return Finding(key, category, severity, title, "détail", list(actions))


def test_section_of_every_kind():
    assert section_of(_f(noise.STRATEGY_KEY, "Filtres")) == "strategy"
    assert section_of(_f("log_rate_low", "Enregistrement")) == "conditions"
    assert section_of(_f("delay_not_representative", "Filtres")) == "conditions"
    assert section_of(_f("dterm_noisy")) == "dterm" and section_of(_f("dterm_delay")) == "dterm"
    assert section_of(_f("hover_line_residual")) == "gyro" and section_of(_f("rpm_q_raise", "Filtres")) == "gyro"
    assert section_of(_f("unknown_noise", "Bruit")) == "gyro"  # bruit non classé : partie gyro
    assert section_of(_f("step_overshoot_light", "Réponse")) is None  # hors sujet


def test_lowpass_text_and_level_note():
    assert lowpass_text(LowpassSetting("PT1")) == "coupé"
    assert lowpass_text(LowpassSetting("PT1", static_hz=250)) == "PT1 250 Hz"
    assert lowpass_text(LowpassSetting("PT2", dyn_min_hz=75, dyn_max_hz=150)) == "PT2 dynamique 75–150 Hz"
    assert "−30 dB" in noise.level_note("gyro").replace("-", "−") and "−10 dB" in noise.level_note("dterm").replace("-", "−")


def test_actionable_needs_actions_and_severity():
    assert noise._actionable([_f("x", severity=CONSEIL, actions=["faire"])])
    assert not noise._actionable([_f("x", severity=CONSEIL)])  # sans action
    assert not noise._actionable([_f("x", severity=INFO, actions=["faire"])])
    assert not noise._actionable([_f("x", severity=OK, actions=["faire"])])


def test_strategy_fallback_texts():
    f = noise._strategy_fallback([], moving=False)
    assert f.severity == OK and "mouvements francs" in f.actions[0]
    f = noise._strategy_fallback([_f("dterm_lpf1_static", severity=CONSEIL, actions=["dterm_lpf1_dyn_hz = 0"],
                                     title="LPF1 du D en mode statique")], moving=True)
    assert f.severity == CONSEIL and "LPF1 du D en mode statique" in f.detail
    assert f.actions[0] == "dterm_lpf1_dyn_hz = 0" and "même type de vol" in f.actions[1]


# ---------------------------------------------------------------- rapport complet sur le vol synthétique


@pytest.fixture(scope="module")
def synth():
    return make_flightlog(seconds=12.0)


def _settings(**over):
    raw = {**BASE_HEADER, **{k: v for k, v in over.items() if v is not None}}
    for k, v in over.items():
        if v is None:
            raw.pop(k, None)
    return parse_settings(raw)


def _blocks(nr: NoiseReport) -> dict:
    return {b.title: b for b in nr.gyro_blocks}


def test_noise_report_structure(synth):
    nr = noise_report(synth)
    assert nr.has_unfiltered and nr.moving and nr.harmonics is not None
    assert nr.band == (80.0, min(500.0, 0.95 * synth.fs / 2))
    assert [a.axis for a in nr.gyro] == ["Roll", "Pitch", "Yaw"] and [a.axis for a in nr.dterm] == ["Roll", "Pitch"]
    for a in nr.gyro:
        assert np.isfinite(a.before_db) and np.isfinite(a.after_db) and 80 <= a.peak_hz <= nr.band[1]
        assert a.reduction_db > 0  # le gyro filtré est moins bruyant que le non filtré
    assert [b.title for b in nr.gyro_blocks] == ["Filtre RPM", "Notch dynamique", "Passe-bas du gyro"]
    assert nr.dterm_block.title == "Passe-bas du D" and nr.strategy
    # chaque constat du gyro est dans un bloc ou dans « autres », jamais les deux
    in_blocks = [id(f) for b in nr.gyro_blocks for f in b.findings]
    assert len(in_blocks) == len(set(in_blocks))
    assert set(in_blocks) | {id(f) for f in nr.gyro_other} == {id(f) for f in nr.gyro_findings}
    for b in nr.gyro_blocks + [nr.dterm_block]:
        assert (b.verdict is None) == noise._actionable(b.findings)  # verdict seulement sans constat à agir
    rpm = _blocks(nr)["Filtre RPM"]
    assert "rpm_filter_harmonics = 3" in rpm.settings and len(rpm.observations) == 3
    assert all("harmonique" in o and ("effacée" in o or "encore visible" in o) for o in rpm.observations)
    assert "dyn_notch_count = 3" in _blocks(nr)["Notch dynamique"].settings
    assert "gyro_lpf1 : PT1 dynamique 250–500 Hz" in _blocks(nr)["Passe-bas du gyro"].settings
    assert "dterm_lpf2 : PT1 150 Hz" in nr.dterm_block.settings


def test_rpm_block_without_erpm(synth):
    no_rpm = dataclasses.replace(synth, motor_hz=None)
    v = _blocks(noise_report(no_rpm))["Filtre RPM"].verdict
    assert v.key == "rpm_block_no_rpm" and v.severity == CONSEIL
    assert v.actions[0].startswith("dshot_bidir = ON") and "rpm_filter_harmonics = 3" in v.actions[0]
    # DShot bidirectionnel déjà activé : c'est l'enregistrement qui manque
    bidir = dataclasses.replace(no_rpm, settings=_settings(dshot_bidir="1"))
    v = _blocks(noise_report(bidir))["Filtre RPM"].verdict
    assert v.severity == INFO and v.actions == ["blackbox_disable_rpm = OFF (dshot_bidir est déjà activé)."]


def test_rpm_block_filter_off_or_unmeasurable(synth):
    off = dataclasses.replace(synth, settings=_settings(rpm_filter_harmonics="0"))
    b = _blocks(noise_report(off))["Filtre RPM"]
    assert b.settings == "coupé (rpm_filter_harmonics = 0)"
    assert b.verdict.key == "rpm_block_off" and b.verdict.severity == CONSEIL
    no_unfilt = dataclasses.replace(synth, gyro_unfilt=None)
    nr = noise_report(no_unfilt)
    assert not nr.has_unfiltered and nr.harmonics is None
    assert len(nr.gyro) == 3 and all(np.isnan(a.before_db) and np.isfinite(a.after_db) for a in nr.gyro)
    b = _blocks(nr)["Filtre RPM"]
    assert b.verdict.key == "rpm_block_unmeasurable" and b.observations == []


def test_notch_block_off_and_slow_loop(synth):
    off = dataclasses.replace(synth, settings=_settings(dyn_notch_count="0"))
    b = _blocks(noise_report(off))["Notch dynamique"]
    assert b.settings == "coupé (dyn_notch_count = 0)"
    assert b.verdict.key == "notch_block_ok" and b.verdict.severity == OK  # filtre RPM actif
    both_off = dataclasses.replace(synth, settings=_settings(dyn_notch_count="0", rpm_filter_harmonics="0"))
    b = _blocks(noise_report(both_off))["Notch dynamique"]
    assert b.verdict.key == "notch_block_both_off" and b.verdict.severity == INFO
    slow = dataclasses.replace(synth, settings=_settings(pid_process_denom="8"))  # boucle PID 1 kHz
    b = _blocks(noise_report(slow))["Notch dynamique"]
    assert "inactif : boucle PID sous 2 kHz" in b.settings


def test_lowpass_block_verdict_depends_on_movement():
    hover = make_flightlog(seconds=12.0, moving=False)
    nr = noise_report(hover)
    assert not nr.moving
    b = _blocks(nr)["Passe-bas du gyro"]
    if b.verdict is not None:
        assert "vol avec mouvements" in b.verdict.detail
    assert nr.strategy and (nr.strategy[0].key != noise.STRATEGY_KEY or len(nr.strategy) >= 1)
    if nr.strategy[0].key == "filter_no_change":
        assert "mouvements francs" in nr.strategy[0].actions[0]


def test_findings_sorted_by_severity_within_sections(synth):
    nr = noise_report(synth)
    order = {ATTENTION: 1, CONSEIL: 2, INFO: 3, OK: 4}
    for part in (nr.conditions, nr.gyro_findings, nr.dterm_findings):
        ranks = [order.get(f.severity, 0) for f in part]
        assert ranks == sorted(ranks)
