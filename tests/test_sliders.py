"""Tests des curseurs du réglage simplifié (analysis/sliders.py)."""


import pytest

from opentunebox.analysis import sliders as sl
from opentunebox.log.settings import parse_settings

from corpus_paths import LOGS  # noqa: E402


def _t(pids):
    return [p.text() for p in pids]


@pytest.mark.parametrize("fam", ["4.4", "4.5", "2025.12", "2026.6"])
def test_defaults_give_default_pids(fam):
    pids = sl.pids_from_sliders({"mode": 2}, fam)
    # forme normalisée P,I,D,Dmax,F identique dans toutes les versions
    assert _t(pids) == ["45,80,30,40,120", "47,84,34,46,125", "45,80,0,0,120"]
    assert _t(pids) == _t(sl.default_pids(fam))


def test_fast_math_truncation():
    # 45 × 1,2 = 54 en exact, mais 53 dans le firmware (division par 100 → × 0.01f)
    pids = sl.pids_from_sliders({"mode": 2, "master": 120, "i_gain": 100}, "4.5")
    assert (pids[0].P, pids[0].I, pids[0].F) == (53, 95, 143)


def test_dmax_gain_zero_and_mode_rp():
    pids = sl.pids_from_sliders({"mode": 1, "master": 100, "d_gain": 100, "dmax_gain": 0}, "2025.12")
    assert (pids[0].D, pids[0].Dmax) == (30, 30)
    old = sl.pids_from_sliders({"mode": 1, "dmax_gain": 0}, "4.5")
    assert (old[0].D, old[0].Dmax) == (30, 30)
    # mode RP : le lacet reste celui de la base
    assert pids[2].text() == "45,80,0,0,120"


@pytest.mark.parametrize("fam", ["4.5", "2026.6"])
def test_inverse_roundtrip(fam):
    target = {"mode": 2, "master": 100, "pi_gain": 120, "i_gain": 80, "d_gain": 120, "ff_gain": 90,
              "pitch_d": 115, "pitch_pi": 105, "dmax_gain": 60}
    pids = sl.pids_from_sliders(target, fam)
    est = sl.sliders_from_pids(pids, fam)
    # le maître n'est pas identifiable seul : on compare les produits maître × curseur
    m = est["master"] / 100
    assert m * est["pi_gain"] == pytest.approx(120, rel=0.03)
    assert m * est["d_gain"] == pytest.approx(120, rel=0.03)
    assert m * est["ff_gain"] == pytest.approx(90, rel=0.03)
    assert est["i_gain"] == pytest.approx(80, rel=0.03)
    assert est["pitch_d"] == pytest.approx(115, rel=0.04)
    assert est["pitch_pi"] == pytest.approx(105, rel=0.04)
    assert est["dmax_gain"] == pytest.approx(60, abs=8)
    assert sl.sliders_from_pids(sl.default_pids(fam), fam) == pytest.approx(
        {k: 100.0 for k in sl.PID_SLIDER_KEYS})


def test_filters_from_sliders():
    f = sl.filters_from_sliders(120, 80, "4.5")
    assert f["gyro_lpf1_dyn_min_hz"] == 300 and f["gyro_lpf2_static_hz"] == 600
    assert f["dterm_lpf1_dyn_max_hz"] == 120 and f["dterm_lpf2_static_hz"] == 120
    assert sl.filters_from_sliders(200, 200)["gyro_lpf2_static_hz"] == 1000
    s = parse_settings({"gyro_lpf1_static_hz": "0", "gyro_lpf1_dyn_hz": "250,500",
                        "gyro_lpf2_static_hz": "0", "dterm_lpf1_dyn_hz": "0,0",
                        "dterm_lpf1_static_hz": "75", "dterm_lpf2_static_hz": "150"})
    f = sl.filters_from_sliders(100, 100, "2026.6", s)
    assert f["gyro_lpf2_static_hz"] == 0 and f["gyro_lpf1_dyn_max_hz"] == 500
    assert f["dterm_lpf1_dyn_min_hz"] == 0 and f["dterm_lpf1_static_hz"] == 75


def test_cli_commands_names():
    new = sl.cli_commands({"mode": 2, "dmax_gain": 80, "gyro_filter": 1, "gyro_mult": 5}, "2025.12")
    assert "set simplified_pids_mode = RPY" in new
    assert "set simplified_d_max_gain = 80" in new
    assert "set simplified_gyro_filter = ON" in new
    assert "set simplified_gyro_filter_multiplier = 10" in new  # minimum 10
    assert new[-2:] == ["simplified_tuning apply", "save"]
    old = sl.cli_commands({"dmax_gain": 80, "pitch_d": 110}, "4.5")
    assert "set simplified_dmax_gain = 80" in old
    assert "set simplified_pitch_d_gain = 110" in old


def test_slider_view():
    raw = {"Firmware revision": "Betaflight 4.5.1 (x) STM32", "rollPID": "53,95,47",
           "pitchPID": "56,100,55", "yawPID": "53,95,0", "d_min": "35,40,0", "ff_weight": "143,149,143",
           "simplified_pids_mode": "2", "simplified_master_multiplier": "120",
           "simplified_d_gain": "100", "simplified_dmax_gain": "100"}
    v = sl.slider_view(parse_settings(raw))
    assert v.from_header and v.consistent
    assert v.values["Master"] == pytest.approx(1.2) and v.values["Damping"] == 1.0
    raw["simplified_pids_mode"] = "0"
    v = sl.slider_view(parse_settings(raw))
    assert not v.from_header
    assert set(v.values) == set(sl.PTB_CATEGORIES)


# ---------------------------------------------------------------- logs réels (corpus)


@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus de logs absent")
def test_corpus_sliders_match_header_pids():
    from opentunebox.decoder.reader import list_sessions

    total = ok = 0
    for f in LOGS.rglob("*"):
        if not (f.is_file() and f.suffix.lower() in (".bbl", ".bfl")):
            continue
        try:
            headers = list_sessions(f)
        except Exception:
            continue
        for h in headers:
            s = parse_settings(h.raw)
            sliders = s.sliders()
            if sliders.get("mode", 0) <= 0:
                continue
            total += 1
            calc = sl.pids_from_sliders(sliders, s.family, base=s.pids)
            ok += _t(calc) == _t(s.pids)
    assert total > 0
    # mesuré : 740 / 741 sessions (un seul F de tangage à 244 au lieu de 245)
    assert ok / total > 0.98
