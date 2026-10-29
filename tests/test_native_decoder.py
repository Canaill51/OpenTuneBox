"""Parité du décodeur Rust (`opentunebox._native`) et du décodeur numba (D-031) : mêmes tableaux, au bit près,
sur un vrai log, des logs fabriqués et des fichiers abîmés (octets changés, coupés, retirés, insérés)."""

import numpy as np
import pytest

from opentunebox.decoder import header
from opentunebox.decoder.reader import decode_file, list_sessions
import bbl_factory as bf

native = pytest.importorskip("opentunebox._native")

from pathlib import Path  # noqa: E402

REAL = Path(__file__).parent / "data" / "jeno5_5s.bbl"
ARRAYS = ("main", "main_kind", "slow", "gps", "gps_at", "events")


def _decode(path, backend, monkeypatch):
    monkeypatch.setenv("OTB_BACKEND", backend)
    return decode_file(path)


def _assert_same(path, monkeypatch):
    py, rs = _decode(path, "python", monkeypatch), _decode(path, "rust", monkeypatch)
    assert len(py) == len(rs)
    for a, b in zip(py, rs):
        for name in ARRAYS:
            x, y = getattr(a, name), getattr(b, name)
            assert x.dtype == y.dtype and x.shape == y.shape, name
            assert np.array_equal(x, y), name
        assert a.stats == b.stats
    return py


def test_constants_match_python():
    """Les numéros de prédicteurs, d'encodages et du contexte sont écrits des deux côtés."""
    import re
    from opentunebox.decoder import core
    src = (Path(__file__).parent.parent / "rust" / "otb-decoder" / "src" / "frame.rs").read_text(encoding="utf-8")
    rust = {m[0]: int(m[1]) for m in re.findall(r"pub const (\w+): (?:i64|usize) = (\d+);", src)}
    for name, value in rust.items():
        mod = header if hasattr(header, name) else core
        assert getattr(mod, name) == value, name
    assert {n for n in vars(header) if n.startswith(("ENC_", "PRED_")) and n != "PRED_ZERO"} <= set(rust)


def test_real_log_identical(monkeypatch):
    sessions = _assert_same(REAL, monkeypatch)
    assert sessions and sessions[0].main.shape[0] > 1000


def test_bad_backend_name(monkeypatch):
    monkeypatch.setenv("OTB_BACKEND", "inconnu")
    with pytest.raises(ValueError, match="OTB_BACKEND"):
        decode_file(REAL)


def test_single_field_header_is_ignored_by_both(tmp_path, monkeypatch):
    """Sans champ de temps (en-tête à un seul champ I), la session est inutilisable : ni plantage, ni lecture
    hors tableau (numba la tolérait par hasard, le Rust paniquait : relecture du 2026-10-29)."""
    header = (b"H Product:Blackbox flight data recorder by Nicholas Sherlock\nH Data version:2\nH I interval:32\n"
              b"H P interval:1/1\nH Field I name:loopIteration\nH Field I signed:0\nH Field I predictor:0\n"
              b"H Field I encoding:1\nH Field P predictor:0\nH Field P encoding:1\n")
    path = bf.write(tmp_path, header + b"I\x05I\x06")
    assert _decode(path, "python", monkeypatch) == [] == _decode(path, "rust", monkeypatch)


def test_bad_definitions_raise_value_error():
    """Une définition de trame plus courte que le nombre de champs est une erreur Python ordinaire."""
    z = np.zeros(1, dtype=np.int64)
    args = (np.zeros(10, dtype=np.uint8), 0, 10, z, z, z, z, 5, z, z, 0, z, z, 0, z, z, 0,
            False, False, False, False, 32, 1, 1, np.zeros(9, dtype=np.int64), 16)
    with pytest.raises(ValueError):
        native.decode_stream(*args)


def _random_case(rng):
    """Définitions de trames et flux d'octets au hasard (biaisé vers les marqueurs de trames), tous
    prédicteurs et encodages, trames S/G/H/E, « End of log » : les paramètres de `decode_stream`."""
    preds = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 256]
    encs = [0, 1, 3, 6, 7, 8, 9, 10, 2, 42]

    def fdef(n, main=False):
        p, e = rng.choice(preds, n).astype(np.int64), rng.choice(encs, n).astype(np.int64)
        if main:
            p[0], e[0] = rng.choice([0, 6]), 1
            p[1], e[1] = rng.choice([0, 1, 2, 10]), rng.choice([0, 1])
        return p, e

    n, ns, ng, nh = (int(rng.integers(a, b)) for a, b in ((2, 12), (0, 5), (0, 6), (1, 4)))
    ip, ie = fdef(n, True)
    pp, pe = fdef(n, True)
    sp, se = fdef(max(ns, 1))
    gp, ge = fdef(max(ng, 1))
    hp, he = fdef(nh)
    for a in (sp, gp, hp):  # PRED_MOTOR_0 hors trame principale lit hors tableau : numba, valeur indéfinie
        a[a == 5] = 0
    ctx = np.zeros(9, dtype=np.int64)
    ctx[1], ctx[4], ctx[5] = rng.integers(0, n), rng.integers(0, nh), rng.integers(0, nh)
    ctx[0], ctx[2], ctx[3], ctx[8] = rng.integers(0, 2000), rng.integers(0, 500), rng.integers(0, 2000), rng.integers(1, 3)
    length = int(rng.integers(0, 3000))
    b = rng.integers(0, 256, length).astype(np.uint8)
    m = rng.random(length) < 0.15
    b[m] = rng.choice([73, 80, 71, 72, 83, 69], m.sum())
    if rng.random() < 0.3 and length > 20:
        k = int(rng.integers(0, length - 13))
        b[k:k + 13] = np.frombuffer(b"E\xffEnd of log\x00", dtype=np.uint8)
    buf = np.concatenate([b, rng.integers(0, 256, int(rng.integers(0, 5))).astype(np.uint8)])
    return (buf, int(rng.integers(0, 3)) if length > 3 else 0, length, ip, ie, pp, pe, n, sp, se, ns, gp, ge, ng, hp, he,
            nh, bool(rng.random() < .8), ns > 0, ng > 0, bool(rng.random() < .7), int(rng.integers(1, 64)),
            int(rng.integers(1, 4)), int(rng.integers(1, 9)), ctx, int(rng.integers(0, 50)))


@pytest.mark.parametrize("seed", range(8))
def test_random_streams_identical(seed):
    """Comparaison directe des deux `decode_stream` sur des flux aléatoires : 8 tableaux, types et formes compris."""
    from opentunebox.decoder import core
    rng = np.random.default_rng(seed)
    for _ in range(40):
        args = _random_case(rng)
        expected = core.decode_stream(*args[:-2], args[-2].copy(), args[-1])
        got = native.decode_stream(*args)
        for x, y, name in zip(expected, got, "main kind slow slow_at gps gps_at events stats".split()):
            assert x.dtype == y.dtype and x.shape == y.shape and np.array_equal(x, y), name


@pytest.mark.parametrize("seed", range(40))
def test_damaged_real_log_identical(seed, tmp_path, monkeypatch):
    """Resynchronisation, trames rejetées, fin prématurée : le Rust suit le numba octet par octet."""
    data = bytearray(REAL.read_bytes())
    h = list_sessions(REAL)[0]
    rng = np.random.default_rng(seed)
    lo, hi = h.data_start, h.data_end
    kind = seed % 4
    for _ in range(int(rng.integers(1, 40))):
        pos = int(rng.integers(lo, min(hi, len(data))))
        if kind == 0:  # octets changés
            data[pos] = int(rng.integers(0, 256))
        elif kind == 1:  # un bit inversé
            data[pos] ^= 1 << int(rng.integers(0, 8))
        elif kind == 2:  # octets retirés
            del data[pos:pos + int(rng.integers(1, 30))]
        else:  # octets insérés
            data[pos:pos] = bytes(rng.integers(0, 256, int(rng.integers(1, 30)), dtype=np.uint8))
    if seed % 5 == 0:  # fichier coupé
        del data[int(rng.integers(lo + 1, len(data))):]
    path = tmp_path / "damaged.bbl"
    path.write_bytes(bytes(data))
    _assert_same(path, monkeypatch)


@pytest.mark.parametrize("p_num,p_denom", [(1, 1), (1, 2), (1, 4), (3, 4), (2, 8)])
def test_synthetic_sessions_identical(p_num, p_denom, tmp_path, monkeypatch):
    """Logs fabriqués : intervalles I et P variés, trames lentes, fin de log."""
    fields = bf.gyro_fields()
    rows = bf.rows_2khz(400, p_num=p_num, p_denom=p_denom, gyro=5)
    data = bf.session(fields, rows, p_num=p_num, p_denom=p_denom, slow=["flightModeFlags", "stateFlags"],
                      slow_rows={50: [3, 4], 200: [5, 6]})
    py = _assert_same(bf.write(tmp_path, data), monkeypatch)
    assert py and py[0].main.shape[0] > 50 and py[0].slow.shape[0] == py[0].main.shape[0]
    assert py[0].stats["log_end_seen"]
