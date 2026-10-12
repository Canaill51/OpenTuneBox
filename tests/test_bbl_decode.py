"""Décodage de bout en bout de fichiers Blackbox fabriqués (tests/bbl_factory.py), sans corpus :
P interval, trames corrompues et resynchronisation, prédicteurs, événements, sessions multiples."""

import numpy as np
import pytest

from opentunebox.decoder import core
from opentunebox.decoder.header import PRED_AVERAGE_2, PRED_PREVIOUS, PRED_STRAIGHT_LINE
from opentunebox.decoder.reader import _forward_fill, decode_file

import bbl_factory as bf  # noqa: E402

FIELDS = bf.gyro_fields()


def _decode(tmp_path, data: bytes, only=None):
    return decode_file(bf.write(tmp_path, data), only=only)


def test_roundtrip_i_and_p_frames(tmp_path):
    """Toutes les itérations d'une boucle logguée à 1/1 reviennent, dans l'ordre, avec leurs valeurs."""
    rows = bf.rows_2khz(100, gyro=1000)
    (s,) = _decode(tmp_path, bf.session(FIELDS, rows))
    assert s.main_names == [f.name for f in FIELDS]
    assert s.main[:, 0].tolist() == list(range(100))
    assert s.main[:, 1].tolist() == [1_000_000 + 125 * k for k in range(100)]
    assert s.main[:, 2].tolist() == [1000 + k for k in range(100)]
    assert s.main[:, 3].tolist() == [-1000 - k for k in range(100)]
    assert s.main_kind.tolist() == [0 if k % 32 == 0 else 1 for k in range(100)]
    assert s.stats["i_frames"] == 4 and s.stats["p_frames"] == 96
    assert s.stats["corrupt_frames"] == 0 and s.stats["desync_frames"] == 0
    assert s.stats["skipped_iterations"] == 0 and s.stats["log_end_seen"]


@pytest.mark.parametrize("p_num, p_denom, p_interval", [(1, 2, "2"), (1, 2, "1/2"), (1, 4, "1/4"), (3, 4, "3/4")])
def test_p_interval_iterations_follow_firmware(tmp_path, p_num, p_denom, p_interval):
    """« P interval:N » (= 1/N) ou « a/b » : l'itération de chaque trame P est reconstruite d'après les trames
    que le firmware n'écrit pas (prédicteur INC), et ces trames absentes sont comptées comme voulues."""
    rows = bf.rows_2khz(64, p_num=p_num, p_denom=p_denom)
    data = bf.session(FIELDS, rows, p_num=p_num, p_denom=p_denom)
    data = data.replace(f"H P interval:{p_num}/{p_denom}".encode(), f"H P interval:{p_interval}".encode(), 1)
    (s,) = _decode(tmp_path, data)
    assert s.header.p_interval == (p_num, p_denom)
    expected = [k for k in range(64) if k % 32 == 0 or bf.should_have_frame(k, 32, p_num, p_denom)]
    assert s.main[:, 0].tolist() == expected
    assert s.main[:, 1].tolist() == [1_000_000 + 125 * k for k in expected]
    # itérations absentes volontairement, comptées jusqu'à la dernière trame reçue
    assert s.stats["skipped_iterations"] == expected[-1] + 1 - len(expected)


def test_should_have_frame_matches_firmware():
    for p_num, p_denom in ((1, 1), (1, 2), (1, 4), (1, 8), (3, 4), (2, 3)):
        for k in range(96):
            assert bool(core._should_have_frame(k, 32, p_num, p_denom)) == bf.should_have_frame(k, 32, p_num, p_denom)


def test_corrupt_frame_resyncs_and_keeps_the_rest(tmp_path):
    """Une trame P abîmée (octet parasite) est comptée corrompue ; le décodeur reprend à la trame I suivante."""
    w = bf.Writer(FIELDS)
    rows = bf.rows_2khz(64)
    out = bytearray(bf.header_bytes(FIELDS))
    for row in rows:
        if row[0] == 20:
            out += b"\x00\x00\x00"  # déchets au milieu du flux
        out += w.iframe(row) if row[0] % 32 == 0 else w.pframe(row)
    out += bf.event_log_end()
    (s,) = _decode(tmp_path, bytes(out))
    its = s.main[:, 0].tolist()
    assert s.stats["corrupt_frames"] >= 1
    assert its == sorted(set(its))  # strictement croissantes
    # une trame n'est validée que si un marqueur la suit : celle d'avant les déchets (19) est perdue aussi ;
    # les trames P qui suivent sont refusées jusqu'à la trame I du bloc suivant (32)
    assert its == list(range(19)) + list(range(32, 64))


def test_frame_longer_than_256_bytes_is_rejected(tmp_path):
    fields = bf.gyro_fields([f"x[{i}]" for i in range(60)])  # 65 champs
    w = bf.Writer(fields)
    big = [0, 1_000_000] + [0] * 3 + [2**30] * 60  # chaque champ svb sur 5 octets : trame de ~300 octets
    small = [0, 1_000_000] + [0] * 63
    data = bf.header_bytes(fields) + w.iframe(big) + w.iframe(small) + bf.event_log_end()
    (s,) = _decode(tmp_path, data)
    assert s.stats["corrupt_frames"] >= 1 and s.main.shape[0] == 1 and s.main[0, 5] == 0


def test_i_frame_jump_rejected_with_following_p_frames(tmp_path):
    """Une trame I qui saute trop loin (> 5000 itérations) est refusée, et ses trames P avec elle."""
    w = bf.Writer(FIELDS)
    out = bytearray(bf.header_bytes(FIELDS))
    out += w.iframe([0, 1_000_000, 0, 0, 0]) + w.pframe([1, 1_000_125, 0, 0, 0])
    out += w.iframe([32000, 1_000_000 + 32000 * 125, 0, 0, 0]) + w.pframe([32001, 1_000_000 + 32001 * 125, 0, 0, 0])
    out += bf.event_log_end()
    (s,) = _decode(tmp_path, bytes(out))
    assert s.main[:, 0].tolist() == [0, 1]
    assert s.stats["desync_frames"] == 2


def test_logging_resume_event_allows_the_jump(tmp_path):
    """Après un événement LOGGING_RESUME (itération, temps), la trame I lointaine est acceptée."""
    w = bf.Writer(FIELDS)
    it, tm = 32000, 1_000_000 + 32000 * 125
    out = bytearray(bf.header_bytes(FIELDS))
    out += w.iframe([0, 1_000_000, 0, 0, 0]) + bf.event(core.EVENT_LOGGING_RESUME, it, tm)
    out += w.iframe([it, tm, 5, 6, 7]) + w.pframe([it + 1, tm + 125, 5, 6, 7]) + bf.event_log_end()
    (s,) = _decode(tmp_path, bytes(out))
    assert s.main[:, 0].tolist() == [0, it, it + 1] and s.stats["desync_frames"] == 0
    assert s.events[:, 0].tolist() == [core.EVENT_LOGGING_RESUME, core.EVENT_LOG_END]
    assert s.events[0].tolist() == [core.EVENT_LOGGING_RESUME, 1, it, tm]  # ligne principale suivante, données


def test_flight_mode_event_and_bad_log_end(tmp_path):
    w = bf.Writer(FIELDS)
    out = bytearray(bf.header_bytes(FIELDS))
    out += w.iframe([0, 1_000_000, 0, 0, 0]) + bf.event(core.EVENT_FLIGHT_MODE, 0b11, 0b1)
    out += w.pframe([1, 1_000_125, 0, 0, 0]) + b"E\xffEnd of lag\x00" + w.pframe([2, 1_000_250, 0, 0, 0])
    out += bf.event_log_end()
    (s,) = _decode(tmp_path, bytes(out))
    assert s.events[0].tolist() == [core.EVENT_FLIGHT_MODE, 1, 0b11, 0b1]
    assert s.stats["log_end_seen"] and s.stats["desync_frames"] >= 1  # faux « End of log » : désynchro
    assert 1 in s.main[:, 0].tolist()


def test_truncated_last_frame_is_dropped(tmp_path):
    """Fichier coupé (trou de carte SD) : la dernière trame, incomplète, n'est pas gardée."""
    w = bf.Writer(FIELDS)
    rows = bf.rows_2khz(10)
    body = b"".join(w.iframe(r) if r[0] % 32 == 0 else w.pframe(r) for r in rows)
    (s,) = _decode(tmp_path, bf.header_bytes(FIELDS) + body[:-2], )
    assert s.main[:, 0].tolist() == list(range(9)) or s.main[:, 0].tolist() == list(range(10))[:-1]
    assert not s.stats["log_end_seen"]


def test_slow_frames_are_forward_filled(tmp_path):
    rows = bf.rows_2khz(8)
    data = bf.session(FIELDS, rows, slow=["flightModeFlags", "stateFlags"], slow_rows={0: [1, 0], 5: [3, 4]})
    (s,) = _decode(tmp_path, data)
    assert s.slow_names == ["flightModeFlags", "stateFlags"]
    assert s.column("flightModeFlags").tolist() == [1] * 5 + [3] * 3
    assert s.column("stateFlags").tolist() == [0] * 5 + [4] * 3


def test_forward_fill_zero_before_first_slow_frame():
    out = _forward_fill(np.array([[1], [2]]), np.array([2, 4]), 6, 1)
    assert out[:, 0].tolist() == [0, 0, 1, 1, 2, 2]


def test_multiple_sessions_and_only(tmp_path):
    a = bf.session(FIELDS, bf.rows_2khz(5), raw={"Craft name": "A"})
    b = bf.session(FIELDS, bf.rows_2khz(7), raw={"Craft name": "B"})
    sessions = _decode(tmp_path, a + b)
    assert [s.index for s in sessions] == [1, 2]
    assert [s.main.shape[0] for s in sessions] == [5, 7]
    assert [s.header.raw["Craft name"] for s in sessions] == ["A", "B"]
    (only,) = _decode(tmp_path, a + b, only=2)
    assert only.index == 2 and only.main.shape[0] == 7


def test_session_without_complete_i_definition_is_skipped(tmp_path):
    data = bf.session(FIELDS, bf.rows_2khz(5))
    data = data.replace(b"H Field I encoding:", b"H Field I encodage:", 1)  # ligne absente → définition incomplète
    assert _decode(tmp_path, data) == []


@pytest.mark.parametrize("pred, prev, prev2, v, expected", [
    (PRED_PREVIOUS, 10, 0, 5, 15),
    (PRED_STRAIGHT_LINE, 10, 4, 1, 17),  # v + 2·prev − prev2
    (PRED_AVERAGE_2, -3, 0, 0, -1),  # (−3 + 0) / 2 tronqué vers zéro : −1, pas −2
    (PRED_AVERAGE_2, 3, 0, 0, 1),
    (PRED_AVERAGE_2, -5, -4, 2, -2),
])
def test_predictors_with_history(pred, prev, prev2, v, expected):
    cur = np.zeros(1, dtype=np.int64)
    ctx = np.zeros(core.CTX_SIZE, dtype=np.int64)
    home = np.zeros(1, dtype=np.int64)
    got = core._predict(0, pred, np.int64(v), cur, np.array([prev], dtype=np.int64), np.array([prev2], dtype=np.int64),
                        True, ctx, home)
    assert got == expected
    # sans historique (première trame) : la valeur brute
    assert core._predict(0, pred, np.int64(v), cur, cur, cur, False, ctx, home) == v


def test_predictors_from_context():
    from opentunebox.decoder.header import (PRED_1500, PRED_LAST_MAIN_FRAME_TIME, PRED_MINMOTOR, PRED_MINTHROTTLE,
                                            PRED_MOTOR_0, PRED_VBATREF)

    cur = np.array([0, 0, 1234], dtype=np.int64)
    ctx = np.zeros(core.CTX_SIZE, dtype=np.int64)
    ctx[core.CTX_MINTHROTTLE], ctx[core.CTX_MOTOR_MIN], ctx[core.CTX_VBATREF] = 1070, 48, 1680
    ctx[core.CTX_MOTOR0_IDX] = 2
    home = np.zeros(1, dtype=np.int64)
    pr = lambda p, v: core._predict(0, p, np.int64(v), cur, cur, cur, True, ctx, home)  # noqa: E731
    assert pr(PRED_MINTHROTTLE, 5) == 1075 and pr(PRED_MINMOTOR, 5) == 53
    assert pr(PRED_MOTOR_0, 6) == 1240 and pr(PRED_1500, -7) == 1493 and pr(PRED_VBATREF, 2) == 1682
    assert pr(PRED_LAST_MAIN_FRAME_TIME, 9) == 9  # pas de trame précédente
    ctx[core.CTX_HAS_LAST_TIME], ctx[core.CTX_LAST_TIME] = 1, 500
    assert pr(PRED_LAST_MAIN_FRAME_TIME, 9) == 509
    # valeurs négatives codées sur 32 bits non signés : ramenées en entier signé avant d'ajouter le contexte
    assert pr(PRED_MINTHROTTLE, 2**32 - 70) == 1000


def test_load_flightlog_from_fabricated_file(tmp_path):
    """De bout en bout : le fichier fabriqué donne un FlightLog en unités physiques à la bonne fréquence."""
    from opentunebox.log.flightlog import load

    raw = {"Firmware revision": "Betaflight 4.5.1 (abcd1234) STM32F7X2", "looptime": "125", "pid_process_denom": "1",
           "gyro_scale": "0x3e8"}
    rows = bf.rows_2khz(400, p_num=1, p_denom=4, gyro=100)
    (lg,) = load(bf.write(tmp_path, bf.session(FIELDS, rows, p_num=1, p_denom=4, raw=raw)))
    assert lg.n == len(rows) and abs(lg.fs - 2000) < 1
    assert lg.gyro[0, 0] == 100.0 and lg.gyro[2, 0] == 7.0
    assert lg.stats["skipped_iterations"] == rows[-1][0] + 1 - len(rows) and lg.stats["log_end_seen"]
    assert lg.settings.firmware.startswith("Betaflight 4.5.1") and lg.settings.log_rate_hz == 2000
