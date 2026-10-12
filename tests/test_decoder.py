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


# ---------------------------------------------------------------- encodages groupés (aller-retour avec la fabrique)


@pytest.mark.parametrize("values, size", [
    ([1, -2, 0], 1),  # 2/2/2 bits
    ([-16, 15, -8], 2),  # 5/5/4 bits
    ([-128, 63, -64], 3),  # 8/7/7 bits
    ([200, -300, 70000], 1 + 2 + 2 + 3),  # largeurs par valeur : 16, 16, 24 bits
    ([2**31 - 1, -(2**31), 0], 1 + 4 + 4 + 1),
])
def test_tag2_3svariable_roundtrip(values, size):
    from bbl_factory import enc_tag2_3svariable

    data = enc_tag2_3svariable(values)
    assert len(data) == size
    buf, st = stream(data)
    out = np.zeros(8, dtype=np.int64)
    core._read_tag2_3svariable(buf, st, out)
    assert out[:3].tolist() == values and st[core.ST_POS] == size


@pytest.mark.parametrize("values", [[1, -2, 0], [7, -8, 3], [-32, 31, 0], [100, -1000, 70000], [2**31 - 1, -(2**31), 1]])
def test_tag2_3s32_roundtrip(values):
    from bbl_factory import enc_tag2_3s32

    buf, st = stream(enc_tag2_3s32(values))
    out = np.zeros(8, dtype=np.int64)
    core._read_tag2_3s32(buf, st, out)
    assert out[:3].tolist() == values


def test_tag8_4s16_v2_roundtrip_random():
    """Quartets décalés après un champ de 4 bits : 200 quadruplets au hasard, toutes largeurs mélangées."""
    from bbl_factory import enc_tag8_4s16_v2

    rng = np.random.default_rng(3)
    out = np.zeros(8, dtype=np.int64)
    for _ in range(200):
        bits = rng.integers(0, 4, 4)
        values = [0 if b == 0 else int(rng.integers(-(1 << (w - 1)), 1 << (w - 1))) for b, w in zip(bits, (1, 4, 8, 16))]
        data = enc_tag8_4s16_v2(values)
        buf, st = stream(data)
        core._read_tag8_4s16_v2(buf, st, out)
        assert out[:4].tolist() == values, values
        assert st[core.ST_POS] == len(data)


def test_tag8_4s16_v1_pairs_nibbles_and_widths():
    from bbl_factory import enc_tag8_4s16_v1

    out = np.zeros(8, dtype=np.int64)
    for values in ([3, -4, 100, -2000], [0, -100, 7, 2], [1000, 0, 0, -1], [0, 0, 0, 0]):
        data = enc_tag8_4s16_v1(values)
        buf, st = stream(data)
        core._read_tag8_4s16_v1(buf, st, out)
        assert out[:4].tolist() == values, values
        assert st[core.ST_POS] == len(data)


def test_tag8_8svb_single_value_is_plain_svb():
    from bbl_factory import enc_tag8_8svb

    buf, st = stream(enc_tag8_8svb([-77]))
    out = np.zeros(8, dtype=np.int64)
    core._read_tag8_8svb(buf, st, out, 1)
    assert out[0] == -77 and st[core.ST_POS] == 2


def _frame(pred, enc, data: bytes, n=None, data_version=2):
    """Décode une trame I (sans historique) de `n` champs avec `_parse_frame` ; renvoie les valeurs et la position."""
    n = n or len(pred)
    buf, st = stream(data)
    cur = np.zeros(n, dtype=np.int64)
    ctx = np.zeros(core.CTX_SIZE, dtype=np.int64)
    ctx[core.CTX_DATA_VERSION] = data_version
    values = np.zeros(8, dtype=np.int64)
    core._parse_frame(buf, st, np.array(pred, dtype=np.int64), np.array(enc, dtype=np.int64), n, cur, cur.copy(),
                      cur.copy(), False, 0, ctx, np.zeros(2, dtype=np.int64), values)
    return cur.tolist(), int(st[core.ST_POS])


def test_parse_frame_mixed_encodings():
    """Une trame qui mêle tous les encodages : chaque groupe consomme ses champs, les prédicteurs s'appliquent."""
    from bbl_factory import enc_neg_14bit, enc_tag2_3svariable, enc_tag8_4s16_v2, enc_tag8_8svb
    from opentunebox.decoder.header import (ENC_NEG_14BIT, ENC_SIGNED_VB, ENC_TAG2_3SVARIABLE, ENC_TAG8_4S16,
                                            ENC_TAG8_8SVB, ENC_UNSIGNED_VB, PRED_1500, PRED_ZERO)

    pred = [PRED_ZERO] * 12 + [PRED_1500] * 3
    enc = ([ENC_UNSIGNED_VB, ENC_SIGNED_VB] + [ENC_TAG2_3SVARIABLE] * 3 + [ENC_TAG8_4S16] * 4
           + [ENC_TAG8_8SVB] * 3 + [ENC_NEG_14BIT] + [ENC_SIGNED_VB] * 2)
    data = (enc_uvb(300) + enc_svb(-5) + enc_tag2_3svariable([10, -20, 7]) + enc_tag8_4s16_v2([0, 3, -100, 2000])
            + enc_tag8_8svb([0, -1, 6]) + enc_neg_14bit(-1234) + enc_svb(-9) + enc_svb(4))
    got, pos = _frame(pred, enc, data)
    assert got == [300, -5, 10, -20, 7, 0, 3, -100, 2000, 0, -1, 6, 1500 - 1234, 1500 - 9, 1500 + 4]
    assert pos == len(data)


def test_parse_frame_group_longer_than_definition_is_truncated():
    """Un groupe TAG8_4S16 qui dépasse le nombre de champs (en-tête bancal) ne déborde pas du tableau."""
    from bbl_factory import enc_tag8_4s16_v2
    from opentunebox.decoder.header import ENC_TAG8_4S16, PRED_ZERO

    got, pos = _frame([PRED_ZERO] * 2, [ENC_TAG8_4S16] * 2, enc_tag8_4s16_v2([5, -6, 7, 8]))
    assert got == [5, -6] and pos == 4  # tout le groupe est lu, deux valeurs gardées


def test_parse_frame_data_version_1_uses_v1_tag8():
    from bbl_factory import enc_tag8_4s16_v1
    from opentunebox.decoder.header import ENC_TAG8_4S16, PRED_ZERO

    data = enc_tag8_4s16_v1([3, -4, 100, -2000])
    assert _frame([PRED_ZERO] * 4, [ENC_TAG8_4S16] * 4, data, data_version=1) == ([3, -4, 100, -2000], len(data))


def test_parse_frame_unknown_encoding_reads_nothing():
    from opentunebox.decoder.header import ENC_SIGNED_VB, PRED_ZERO

    got, pos = _frame([PRED_ZERO] * 2, [42, ENC_SIGNED_VB], enc_svb(-3))
    assert got == [0, -3] and pos == 1


# ---------------------------------------------------------------- en-tête


def test_parse_header_lines_and_data_start():
    from opentunebox.decoder.header import ENC_SIGNED_VB, PRED_PREVIOUS, find_log_starts, parse_header

    head = (b"H Product:Blackbox flight data recorder by Nicholas Sherlock\r\n"  # fin de ligne Windows
            b"H Data version:2\n"
            b"H Field I name:loopIteration,time,gyroADC[0]\n"
            b"H Field I signed:0,0,1\n"
            b"H Field I predictor:0,0,0\n"
            b"H Field I encoding:1,1,0\n"
            b"H Field P predictor:6,2,1\n"
            b"H Field P encoding:9,0,0\n"
            b"H ligne sans deux-points\n"
            b"H Firmware revision:Betaflight 4.5.1 (abc) STM32F7X2\n"
            b"H rollPID:45,80,40\n"
            b"\x00\x00")  # octets parasites avant la première trame
    data = head + b"I" + b"\x01\x02"
    h = parse_header(data, 0, len(data))
    assert h.raw["Firmware revision"] == "Betaflight 4.5.1 (abc) STM32F7X2" and h.raw["rollPID"] == "45,80,40"
    assert h.raw["Product"] == "Blackbox flight data recorder by Nicholas Sherlock"  # sans le \r
    assert "ligne sans deux-points" not in h.raw and data[h.data_start:h.data_start + 1] == b"I"
    fi, fp = h.frames["I"], h.frames["P"]
    assert fi.complete() and fp.complete() and fi.count == 3 and fi.index("time") == 1 and fi.index("x") == -1
    assert fp.names == fi.names and fp.signed == [0, 0, 1]  # P reprend noms et signes de I
    assert fp.predictor[2] == PRED_PREVIOUS and fp.encoding[2] == ENC_SIGNED_VB
    assert h.data_version == 2 and h.i_interval == 1 and h.p_interval == (1, 1)
    assert find_log_starts(data + data) == [0, len(data)]


def test_log_header_numeric_properties():
    from opentunebox.decoder.header import FrameDef, LogHeader, _int, _ints, as_array

    assert (_int("0x10"), _int(" 12.0 "), _int("abc", 7), _int(None, 3)) == (16, 12, 7, 3)
    assert _ints("1,2.0,x,4") == [1, 2, 4] and _ints(None) == []
    mk = lambda **raw: LogHeader(raw, {}, 0, 0)  # noqa: E731
    assert mk(**{"P interval": "2"}).p_interval == (1, 2) and mk(**{"P interval": "3/8"}).p_interval == (3, 8)
    assert mk(**{"P interval": "0/0"}).p_interval == (1, 1) and mk(**{"I interval": "0"}).i_interval == 1
    assert mk(motorOutput="48,2047").motor_output == (48, 2047) and mk(motorOutput="100").motor_output == (100, 2047)
    assert mk(minthrottle="1070", maxthrottle="2000").motor_output == (1070, 2000) and mk().motor_output == (0, 2047)
    assert mk(vbatref="1680").vbatref == 1680 and mk().data_version == 2
    assert as_array([1, 2, 3], 2).tolist() == [1, 2] and as_array([1], 3).tolist() == [1, 0, 0]
    assert as_array([], 0).tolist() == [0]  # jamais vide : numba veut un tableau
    assert not FrameDef(names=["a"], predictor=[0]).complete() and not FrameDef().complete()
