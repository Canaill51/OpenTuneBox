"""En-tête normalisé par version (log/settings.py, decoder/header.py), tables de debug et de modes."""

import numpy as np

from opentunebox.decoder.header import LogHeader, _int
from opentunebox.log import modes
from opentunebox.log.debug_tables import TABLES, debug_mode_name, family
from opentunebox.log.settings import parse_settings

FW44 = "Betaflight 4.4.3 (abcd1234) STM32F405"
FW45 = "Betaflight 4.5.1 (abcd1234) STM32F7X2"
FW2025 = "Betaflight 2025.12.1 (abcd1234) STM32H743"
FW2026 = "Betaflight 2026.6.0 (abcd1234) STM32H743"


def _roll(fw, **raw):
    return parse_settings({"Firmware revision": fw, "rollPID": "45,80,40", **raw}).pids[0]


def test_d_and_dmax_conventions_edge_cases():
    """4.4 / 4.5 : rollPID D = D max, d_min = D de base (ignoré s'il est nul ou ≥ D) ; 2025.12+ : rollPID D = D de
    base, d_max = D max (ignoré s'il est ≤ D), d_min ignoré (L-004)."""
    assert (_roll(FW45, d_min="30,0,0").D, _roll(FW45, d_min="30,0,0").Dmax) == (30, 40)
    assert (_roll(FW44, d_min="30,0,0").D, _roll(FW44, d_min="30,0,0").Dmax) == (30, 40)
    for dmin in ("0,0,0", "40,0,0", "45,0,0", ""):
        p = _roll(FW45, d_min=dmin)
        assert (p.D, p.Dmax) == (40, 40), dmin
    assert (_roll(FW2025, d_max="50,0,0").D, _roll(FW2025, d_max="50,0,0").Dmax) == (40, 50)
    assert (_roll(FW2025, d_max="35,0,0").D, _roll(FW2025, d_max="35,0,0").Dmax) == (40, 40)
    p = _roll(FW2026, d_max="50,0,0", d_min="30,0,0")
    assert (p.D, p.Dmax) == (40, 50)
    # version illisible : traitée comme la plus récente (convention d_max), d_min ignoré — comportement actuel
    p = _roll("", d_min="30,0,0")
    assert (p.D, p.Dmax) == (40, 40)
    assert _roll(FW45, ff_weight="120,125").F == 120 and _roll(FW45).F == 0


def test_rates_and_p_interval():
    s = parse_settings({"looptime": "125", "pid_process_denom": "2", "P interval": "4"})
    assert (s.gyro_rate_hz, s.pid_rate_hz, s.log_rate_hz) == (8000, 4000, 1000) and s.p_interval == (1, 4)
    s = parse_settings({"looptime": "312", "P interval": "1/2"})
    assert s.p_interval == (1, 2) and abs(s.log_rate_hz - 1e6 / 312 / 2) < 1e-6
    assert parse_settings({}).log_rate_hz == 8000  # looptime 125, PID 1/1, log 1/1 par défaut
    assert parse_settings({"looptime": "0", "pid_process_denom": "0"}).pid_rate_hz == 1e6


def test_version_board_and_protocol():
    s = parse_settings({"Firmware revision": FW45})
    assert s.version == (4, 5, 1) and s.firmware_name == "Betaflight" and s.board == "STM32F7X2" and s.family == "4.5"
    s = parse_settings({"Firmware revision": "n'importe quoi"})
    assert s.version == () and s.family == "2026.6" and s.board == ""
    assert parse_settings({"motor_pwm_protocol": "5"}).is_dshot and not parse_settings({"motor_pwm_protocol": "4"}).is_dshot
    assert parse_settings({}).motor_protocol == 6  # DShot600 par défaut


def test_family_bounds():
    assert family(()) == "2026.6"
    assert [family(v) for v in ((4, 3, 0), (4, 4, 3), (4, 5, 0), (4, 6, 0))] == ["4.4", "4.4", "4.5", "4.5"]
    assert [family(v) for v in ((2025, 12, 0), (2026, 5, 0), (2026, 6, 0), (2027, 1, 0))] == \
        ["2025.12", "2025.12", "2026.6", "2026.6"]
    assert family((2026,)) == "2025.12"  # mineur absent = 0 (jamais produit par le regex de version)


def test_debug_mode_names_fallbacks_and_renames():
    """Numéros de debug différents selon la version (L-003) ; replis : nom explicite, index absent, hors table."""
    assert debug_mode_name(None, (4, 5, 0)) == "NONE"
    assert debug_mode_name(500, (4, 5, 0)) == "DEBUG_500"
    assert debug_mode_name(3, (4, 5, 0), " gyro_scaled ") == "GYRO_SCALED"
    assert debug_mode_name(6, (4, 5, 0)) == "GYRO_SCALED" and debug_mode_name(6, (2025, 12, 0)) == "RC_INTERPOLATION"
    assert debug_mode_name(17, (4, 5, 0)) == "FFT_FREQ" and debug_mode_name(16, (2025, 12, 0)) == "FFT_FREQ"
    assert debug_mode_name(45, (4, 4, 0)) == debug_mode_name(45, (2025, 12, 0)) == "DSHOT_RPM_TELEMETRY"
    assert debug_mode_name(47, (4, 5, 0)) == "D_MIN" and debug_mode_name(47, (2025, 12, 0)) == "D_MAX"
    for fam, table in TABLES.items():
        assert len(set(table)) == len(table), f"doublon dans la table {fam}"
    s = parse_settings({"Firmware revision": FW2025, "debug_mode": "16"})
    assert s.debug_index == 16 and s.debug_mode == "FFT_FREQ"


def test_mode_names_and_changes():
    assert modes.mode_names(0b10011, (4, 5, 0)) == ["ARM", "ANGLE", "HEADFREE"]
    assert modes.mode_names(1 << 4, (2025, 12, 0)) == ["ALTHOLD"]  # bit 4 = HEADFREE en 4.x, ALTHOLD depuis 2025.12
    assert modes.mode_names(1 << 11, (2026, 6, 0)) == ["AUTOPILOT"]
    assert modes.mode_names(1 << 11, (2025, 12, 0)) == []  # ANTIGRAVITY : interrupteur non affiché
    assert modes.mode_names(1 << 11, (2025, 12, 0), shown_only=False) == ["ANTIGRAVITY"]
    t = np.arange(5) / 10
    assert modes.mode_changes(np.array([1, 1, 3, 3, 1]), t, (4, 5, 0)) == [(0.0, "|ARM"), (0.2, "|ARM|ANGLE"), (0.4, "|ARM")]
    assert modes.mode_changes(None, t, (4, 5, 0)) == [] and modes.mode_changes(np.zeros(0), t, (4, 5, 0)) == []
    assert modes.mode_changes(np.array([1, 1 | 1 << 11, 1]), t[:3], (4, 5, 0)) == [(0.0, "|ARM")]  # BEEPERON : rien
    assert modes.mode_changes(np.array([0, 0]), t[:2], (4, 5, 0)) == [(0.0, "|—")]


def test_log_header_numeric_fallbacks():
    assert _int("0x10") == 16 and _int("1.9") == 1 and _int("abc", 7) == 7 and _int(None, 3) == 3
    h = lambda raw: LogHeader(raw, {}, 0, 0)  # noqa: E731
    assert h({"motorOutput": "48,2047"}).motor_output == (48, 2047)
    assert h({"motorOutput": "1000"}).motor_output == (1000, 2047)
    assert h({"minthrottle": "1070", "maxthrottle": "2000"}).motor_output == (1070, 2000)
    assert h({}).motor_output == (0, 2047)
    assert h({"I interval": "0"}).i_interval == 1 and h({}).data_version == 2 and h({"Data version": "1"}).data_version == 1
    assert h({"P interval": "3"}).p_interval == (1, 3) and h({"P interval": "2/3"}).p_interval == (2, 3)
    assert h({"P interval": "0/0"}).p_interval == (1, 1)


def test_filters_and_rpm_fallbacks():
    s = parse_settings({"gyro_notch_hz": "0,300", "gyro_notch_cutoff": "0,200", "rpm_filter_harmonics": "3"})
    assert s.gyro_notches == [(300, 200)] and s.rpm_weights == [100, 100, 100] and s.rpm_q == 500
    assert parse_settings({"rpm_filter_harmonics": "0"}).rpm_weights == [100]
    assert parse_settings({"gyro_lpf1_type": "1", "gyro_lpf1_dyn_hz": "250,500"}).gyro_lpf1.dynamic
    assert parse_settings({"gyro_lpf1_type": "1", "gyro_lpf1_dyn_hz": "250"}).gyro_lpf1.dyn_max_hz == 0
    assert parse_settings({"acc_1G": "0"}).acc_1g == 1 and parse_settings({"motor_poles": "1"}).motor_poles == 2
    assert parse_settings({"blackbox_high_resolution": "1"}).high_resolution
    assert not parse_settings({"blackbox_high_resolution": "0"}).high_resolution
