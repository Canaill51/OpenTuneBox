"""Tests du décodeur : encodages élémentaires (aller-retour) et logs réels."""


import numpy as np
import pytest

from opentunebox.decoder import core
from opentunebox.log.debug_tables import TABLES, debug_mode_name
from opentunebox.log.settings import parse_settings

from bbl_factory import enc_svb, enc_uvb  # noqa: E402
from corpus_paths import LOGS, corpus_log  # noqa: E402


def stream(data: bytes):
    buf = np.frombuffer(data, dtype=np.uint8)
    st = np.zeros(3, dtype=np.int64)
    st[core.ST_END] = len(buf)
    return buf, st


@pytest.mark.parametrize("v", [0, 1, 127, 128, 300, 16383, 2**31 - 1, 2**32 - 1])
def test_unsigned_vb(v):
    buf, st = stream(enc_uvb(v))
    assert core._read_uvb(buf, st) == v


@pytest.mark.parametrize("v", [0, 1, -1, 63, -64, 1000, -1000, 2**31 - 1, -(2**31)])
def test_signed_vb(v):
    buf, st = stream(enc_svb(v))
    assert core._read_svb(buf, st) == v


def test_tag8_8svb_group():
    vals = [5, 0, -3, 0, 0, 100, 0]
    header = sum(1 << i for i, x in enumerate(vals) if x)
    data = bytes([header]) + b"".join(enc_svb(x) for x in vals if x)
    buf, st = stream(data)
    out = np.zeros(8, dtype=np.int64)
    core._read_tag8_8svb(buf, st, out, len(vals))
    assert out[: len(vals)].tolist() == vals


def test_tag2_3s32_small_and_wide():
    # sélecteur 0 : trois valeurs sur 2 bits
    buf, st = stream(bytes([0b00_01_11_00]))
    out = np.zeros(8, dtype=np.int64)
    core._read_tag2_3s32(buf, st, out)
    assert out[:3].tolist() == [1, -1, 0]
    # sélecteur 3 : largeurs 8, 16, 32 bits (2 bits par valeur, poids faibles d'abord)
    lead = 0xC0 | (0 << 0) | (1 << 2) | (3 << 4)
    data = bytes([lead, 0xFE]) + (-300 & 0xFFFF).to_bytes(2, "little") + (123456789).to_bytes(4, "little")
    buf, st = stream(data)
    core._read_tag2_3s32(buf, st, out)
    assert out[:3].tolist() == [-2, -300, 123456789]


def test_tag8_4s16_v2_nibbles():
    # sélecteurs : 4 bits, 8 bits, 16 bits, zéro → les champs suivants sont décalés d'un quartet
    selector = 1 | (2 << 2) | (3 << 4)
    data = bytes([selector, 0x7A, 0xBC, 0x12, 0x34])
    buf, st = stream(data)
    out = np.zeros(8, dtype=np.int64)
    core._read_tag8_4s16_v2(buf, st, out)
    # quartet haut 7 → 7 ; puis 8 bits 0xAB → −85 ; puis 16 bits 0xC123 → −16093 ; puis 0
    assert out[:4].tolist() == [7, -85, -16093, 0]


# ---------------------------------------------------------------- tables et réglages


def test_debug_tables_known_indices():
    assert debug_mode_name(17, (4, 5, 2)) == "FFT_FREQ"
    assert debug_mode_name(16, (2025, 12, 2)) == "FFT_FREQ"
    assert debug_mode_name(6, (4, 4, 2)) == "GYRO_SCALED"
    assert debug_mode_name(46, (2026, 6, 1)) == "RPM_FILTER"
    assert debug_mode_name(96, (2026, 6, 1)) == "CHIRP"
    assert debug_mode_name(97, (2025, 12, 5)) == "CHIRP"
    assert debug_mode_name(79, (4, 4, 3)) == "DSHOT_TELEMETRY_COUNTS"
    assert TABLES["4.5"][75] == "GPS_CONNECTION" and TABLES["4.5"][89] == "EZLANDING"


def test_settings_d_max_conventions():
    old = parse_settings({"Firmware revision": "Betaflight 4.5.1 (x) STM32", "rollPID": "45,80,40",
                          "d_min": "30,34,0"})
    assert (old.pids[0].D, old.pids[0].Dmax) == (30, 40)
    new = parse_settings({"Firmware revision": "Betaflight 2025.12.2 (x) STM32", "rollPID": "45,80,30",
                          "d_max": "40,46,0"})
    assert (new.pids[0].D, new.pids[0].Dmax) == (30, 40)


# ---------------------------------------------------------------- logs réels (corpus)


@pytest.mark.corpus
@pytest.mark.skipif(not LOGS.exists(), reason="corpus de logs absent")
def test_real_log_loads():
    from opentunebox.log.flightlog import load

    logs = load(corpus_log("1. PTB/Apri/btfl_008.bbl"))
    assert len(logs) == 1
    lg = logs[0]
    assert 1900 < lg.fs < 2100
    assert lg.settings.debug_mode == "FFT_FREQ"
    assert lg.gyro_unfilt is not None and lg.motor_hz is not None
    assert np.all(np.diff(lg.t) > 0)
    assert 0 <= np.median(lg.throttle) <= 100


def test_parse_header_gps_predictors_shorter_than_names():
    """En-tête tronqué : la trame G a des noms mais moins de prédicteurs ; pas d'IndexError, le second
    HOME_COORD n'est renommé que s'il existe."""
    from opentunebox.decoder.header import PRED_HOME_COORD, PRED_HOME_COORD_1, parse_header

    data = (b"H Field G name:GPS_home[0],GPS_home[1],GPS_numSat\n"
            b"H Field G predictor:%d\n" % PRED_HOME_COORD + b"I")
    hdr = parse_header(data, 0, len(data))
    assert hdr.frames["G"].predictor == [PRED_HOME_COORD]
    data = (b"H Field G name:GPS_home[0],GPS_home[1],GPS_numSat\n"
            b"H Field G predictor:%d,%d,0\n" % (PRED_HOME_COORD, PRED_HOME_COORD) + b"I")
    assert parse_header(data, 0, len(data)).frames["G"].predictor == [PRED_HOME_COORD, PRED_HOME_COORD_1, 0]
