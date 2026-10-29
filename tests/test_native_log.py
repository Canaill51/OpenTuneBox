"""Parité des noyaux Rust `_native.drop_implausible` et `_native.rows_f32` (`rust/otb-log`) avec le code numpy
d'origine de `flightlog._drop_implausible` / `from_session`, gardé tel quel dans `tests/ref_flightlog.py`
(D-031, phase 3a). TDD : écrits avant les noyaux.

Les deux noyaux doivent donner des résultats IDENTIQUES AU BIT PRÈS (aucune tolérance) : ce sont des entiers
ramenés à 32 bits, des masques, et des conversions int64 -> float32 suivies d'une division float32. Un écart
d'un seul bit changerait les spectres. Les tests de la section C (intégration) confrontent `from_session`, branché
sur les noyaux, à `from_session_ref` (le code numpy d'origine) : ils échouent si le branchement diffère."""

import copy
import dataclasses
from pathlib import Path

import numpy as np
import pytest

_native = pytest.importorskip("opentunebox._native")

from opentunebox.decoder.reader import decode_file, list_sessions  # noqa: E402
from opentunebox.log import flightlog  # noqa: E402
from corpus_paths import corpus_log  # noqa: E402
from ref_flightlog import drop_implausible_ref, from_session_ref  # noqa: E402
from synthetic import make_session  # noqa: E402

REAL = Path(__file__).resolve().parent / "data" / "jeno5_5s.bbl"
I64_MAX, I64_MIN = np.iinfo(np.int64).max, np.iinfo(np.int64).min

# Un nom par genre de colonne (0 ignorée, 1 gyro/consigne, 2 terme PID, 3 ramener seulement)
NAME_BY_KIND = {1: ["gyroADC[0]", "gyroUnfilt[1]", "setpoint[2]"], 2: ["axisP[0]", "axisI[1]", "axisD[2]", "axisF[0]"],
                3: ["motor[0]", "rcCommand[0]", "setpoint[3]", "debug[0]", "eRPM[0]"]}


def kinds_for(names):
    """Genre de chaque colonne, avec les mêmes règles de noms que l'étalon `drop_implausible_ref`."""
    out = []
    for n in names:
        if n in ("loopIteration", "time"):
            out.append(0)
        elif n.startswith(("gyroADC[", "gyroUnfilt[")) or (n.startswith("setpoint[") and n != "setpoint[3]"):
            out.append(1)
        elif n.startswith(("axisP[", "axisI[", "axisD[", "axisF[")):
            out.append(2)
        else:
            out.append(3)
    return np.array(out, dtype=np.uint8)


def session_of(main, names):
    """Session minimale portant `main` : seules les colonnes principales comptent pour l'étalon."""
    sess = make_session({"gyroADC[0]": [0, 0]})
    n = main.shape[0]
    sess.main, sess.main_names = main.copy(), list(names)
    sess.main_kind = np.zeros(n, dtype=np.int64)
    sess.slow = np.zeros((n, 0), dtype=np.int64)
    return sess


def names_for(ncols):
    """`loopIteration`, `time`, puis des colonnes de tous les genres, en tournant."""
    pool = [x for k in (1, 2, 3) for x in NAME_BY_KIND[k]]
    return ["loopIteration", "time"] + [pool[j % len(pool)] for j in range(ncols - 2)]


def check_drop(main, names, hr):
    """Le noyau et l'étalon écartent les mêmes trames et laissent les mêmes valeurs (int64, au bit près)."""
    sess = session_of(main, names)
    sess_ref = copy.deepcopy(sess)
    n_bad = drop_implausible_ref(sess_ref, hr)
    got = main.copy()
    bad = _native.drop_implausible(got, kinds_for(names), hr)
    assert bad.dtype == np.bool_ and bad.shape == (main.shape[0],)
    assert got.dtype == np.int64 and got.shape == main.shape
    assert int(bad.sum()) == n_bad
    assert np.array_equal(got[~bad], sess_ref.main)
    assert sess_ref.main.dtype == np.int64
    if n_bad == 0:
        assert np.array_equal(got, sess_ref.main)
    return bad


def base_matrix(shape, seed, hr):
    """Valeurs plausibles : gyro et consigne < 4000·hr, termes PID < 65535 (colonnes `time` et `loop` croissantes)."""
    rng = np.random.default_rng(seed)
    n, c = shape
    names = names_for(c) if c >= 2 else ["loopIteration"]
    main = np.zeros(shape, dtype=np.int64)
    kinds = kinds_for(names)
    for j in range(c):
        lim = {0: 10, 1: int(3900 * hr), 2: 60000, 3: 2**20}[int(kinds[j])]
        main[:, j] = rng.integers(-lim, lim + 1, n)
    if c >= 2:
        main[:, 0] = np.arange(n)
        main[:, 1] = 1_000_000 + 500 * np.arange(n)
    return main, names, rng


# ----------------------------------------------------------------------------------------------------------
# A. drop_implausible

SHAPES = [(1, 5), (1000, 12), (50_000, 86)]
WIDE_VALUES = [2**31, 2**31 + 5, -2**31 - 1, 2**40, I64_MAX - 3, I64_MIN + 3, -2**31, 2**31 - 1, -2**40, I64_MAX, I64_MIN]


@pytest.mark.parametrize("hr", [1.0, 10.0])
@pytest.mark.parametrize("shape", SHAPES)
@pytest.mark.parametrize("seed", [0, 1, 2])
def test_drop_implausible_in_range_values_match_reference(shape, seed, hr):
    main, names, _ = base_matrix(shape, seed, hr)
    bad = check_drop(main, names, hr)
    assert not bad.any()


@pytest.mark.parametrize("hr", [1.0, 10.0])
@pytest.mark.parametrize("shape", SHAPES)
@pytest.mark.parametrize("seed", range(6))
def test_drop_implausible_out_of_32bit_range_in_one_column_matches_reference(shape, seed, hr):
    """Valeurs hors de ±2**31 dans UNE colonne (genre tournant avec la graine) : colonne ramenée en entier, y
    compris le débordement silencieux de `c + 2**31` près des bornes int64, identique à numpy."""
    main, names, rng = base_matrix(shape, seed, hr)
    n, c = shape
    target = 2 + seed % (c - 2)
    rows = rng.integers(0, n, max(1, n // 20))
    main[rows, target] = rng.choice(np.array(WIDE_VALUES, dtype=np.int64), rows.size)
    check_drop(main, names, hr)


@pytest.mark.parametrize("value", WIDE_VALUES)
@pytest.mark.parametrize("kind", [1, 2, 3])
def test_drop_implausible_each_wide_value_matches_reference_for_each_kind(value, kind):
    names = ["loopIteration", "time", NAME_BY_KIND[kind][0]]
    main = np.array([[0, 100, 5], [1, 200, value], [2, 300, -7]], dtype=np.int64)
    check_drop(main, names, 1.0)


@pytest.mark.parametrize("hr", [1.0, 10.0])
@pytest.mark.parametrize("delta", [-1, 0, 1])
@pytest.mark.parametrize("sign", [1, -1])
def test_drop_implausible_rejection_bounds_are_exact(hr, delta, sign):
    """Rejet strictement au-dessus de 4000·hr (gyro, consigne) et de 65535 (termes PID) ; la borne elle-même reste."""
    names = ["loopIteration", "time", "gyroADC[0]", "setpoint[1]", "axisP[0]", "axisD[1]", "motor[0]"]
    lim = int(4000 * hr)
    for col, limit in ((2, lim), (3, lim), (4, 65535), (5, 65535)):
        main = np.zeros((3, len(names)), dtype=np.int64)
        main[:, 1] = [1, 2, 3]
        main[1, col] = sign * (limit + delta)
        bad = check_drop(main, names, hr)
        assert bool(bad[1]) == (delta > 0)
    # la colonne « ramener seulement » n'est jamais rejetée, même énorme
    main = np.zeros((3, len(names)), dtype=np.int64)
    main[1, 6] = 2**30
    assert not check_drop(main, names, hr).any()


@pytest.mark.parametrize("hr", [1.0, 10.0])
def test_drop_implausible_exact_minus_2_31_is_rejected_for_gyro_and_pid(hr):
    """-2**31 n'est pas ramené (borne incluse) et sa valeur absolue 2**31 dépasse les deux seuils."""
    names = ["loopIteration", "time", "gyroADC[0]", "axisP[0]"]
    main = np.array([[0, 1, -2**31, 0], [1, 2, 0, -2**31], [2, 3, 0, 0]], dtype=np.int64)
    bad = check_drop(main, names, hr)
    assert bad.tolist() == [True, True, False]


def test_drop_implausible_ignored_columns_with_wild_values_stay_unchanged():
    """`loopIteration` et `time` (genre 0) ne sont ni ramenées ni contrôlées."""
    names = ["loopIteration", "time", "gyroADC[0]"]
    main = np.array([[I64_MAX, I64_MIN, 1], [2**40, -2**40, 2], [-2**33, 2**33, 3]], dtype=np.int64)
    got = main.copy()
    bad = _native.drop_implausible(got, kinds_for(names), 1.0)
    assert not bad.any() and np.array_equal(got, main)
    check_drop(main, names, 1.0)


@pytest.mark.parametrize("hr", [1.0, 10.0])
def test_drop_implausible_all_rows_bad(hr):
    main, names, _ = base_matrix((200, 8), 3, hr)
    main[:, 2] = 70000 * 10  # la 1re colonne de gyro dépasse 4000·hr partout
    names[2] = "gyroADC[0]"
    bad = check_drop(main, names, hr)
    assert bad.all()


def test_drop_implausible_no_row_bad_leaves_matrix_with_reference_values():
    main, names, _ = base_matrix((300, 10), 4, 1.0)
    bad = check_drop(main, names, 1.0)
    assert not bad.any()


def test_drop_implausible_zero_rows_returns_empty_mask():
    main = np.zeros((0, 6), dtype=np.int64)
    bad = _native.drop_implausible(main, kinds_for(names_for(6)), 1.0)
    assert bad.dtype == np.bool_ and bad.shape == (0,)


def test_drop_implausible_does_not_filter_rows():
    """Le noyau ne retire aucune ligne : la forme de `main` reste (n, colonnes), Python fait `main[~bad]`."""
    main = np.array([[0, 1, 9999], [1, 2, 1]], dtype=np.int64)
    got = main.copy()
    bad = _native.drop_implausible(got, kinds_for(["loopIteration", "time", "gyroADC[0]"]), 1.0)
    assert got.shape == (2, 3) and bad.tolist() == [True, False]


# --- erreurs de drop_implausible : exceptions Python claires, jamais de panique

def _ok_args():
    return np.zeros((4, 3), dtype=np.int64), np.array([0, 0, 1], dtype=np.uint8)


def test_drop_implausible_rejects_kinds_of_wrong_length():
    main, _ = _ok_args()
    with pytest.raises(ValueError):
        _native.drop_implausible(main, np.zeros(2, dtype=np.uint8), 1.0)
    with pytest.raises(ValueError):
        _native.drop_implausible(main, np.zeros(4, dtype=np.uint8), 1.0)


def test_drop_implausible_rejects_unknown_kind_value():
    main, _ = _ok_args()
    with pytest.raises(ValueError):
        _native.drop_implausible(main, np.array([0, 0, 4], dtype=np.uint8), 1.0)


def test_drop_implausible_rejects_non_contiguous_main():
    wide = np.zeros((4, 6), dtype=np.int64)
    kinds = np.array([0, 0, 1], dtype=np.uint8)
    with pytest.raises(ValueError):
        _native.drop_implausible(wide[:, ::2], kinds, 1.0)
    with pytest.raises(ValueError):
        _native.drop_implausible(np.zeros((3, 4), dtype=np.int64).T, kinds, 1.0)


@pytest.mark.parametrize("dtype", [np.float64, np.int32])
def test_drop_implausible_rejects_wrong_main_dtype(dtype):
    _, kinds = _ok_args()
    with pytest.raises(TypeError):
        _native.drop_implausible(np.zeros((4, 3), dtype=dtype), kinds, 1.0)


def test_drop_implausible_rejects_int64_kinds():
    main, _ = _ok_args()
    with pytest.raises(TypeError):
        _native.drop_implausible(main, np.array([0, 0, 1], dtype=np.int64), 1.0)


def test_drop_implausible_rejects_read_only_main():
    """`main` est modifié sur place : un tableau en lecture seule doit lever une erreur claire (le type exact,
    ValueError ou TypeError, est à constater avec le noyau écrit), sans rien modifier."""
    main, kinds = _ok_args()
    main[:, 2] = 2**40
    before = main.copy()
    main.flags.writeable = False
    with pytest.raises((ValueError, TypeError)):
        _native.drop_implausible(main, kinds, 1.0)
    assert np.array_equal(main, before)


# ----------------------------------------------------------------------------------------------------------
# B. rows_f32

def ref_rows(src, cols, scale):
    """Reproduit `axes()` de `from_session_ref` : colonne absente (-1) -> zéros, puis division float32."""
    out = np.zeros((len(cols), src.shape[0]), dtype=np.float32)
    for i, c in enumerate(cols):
        if c >= 0:
            out[i] = src[:, c]
            if scale != 1.0:
                out[i] /= scale
    return out


def check_rows(src, cols, scale):
    cols = np.asarray(cols, dtype=np.int64)
    got = _native.rows_f32(src, cols, scale)
    ref = ref_rows(src, cols, scale)
    assert got.dtype == np.float32 and got.shape == ref.shape
    assert np.array_equal(got, ref)
    return got


F32_EDGE = [2**24 - 1, 2**24, 2**24 + 1, 2**24 + 2, 2**24 + 3, 2**40, 2**40 + 1, -(2**24 + 1), -(2**40) - 12345,
            0, 1, -1, 2**31 - 1, -2**31, I64_MAX, I64_MIN]


@pytest.mark.parametrize("scale", [1.0, 10.0, 2048.0, 4096.0, 1.5])
@pytest.mark.parametrize("seed", range(4))
def test_rows_f32_mixed_present_and_absent_columns_match_reference(scale, seed):
    rng = np.random.default_rng(seed)
    src = rng.integers(-2**31, 2**31, (5000, 9), dtype=np.int64)
    cols = rng.permutation([0, 3, 8, -1, 5, -1, 1, 1, 7])[: rng.integers(1, 10)]
    check_rows(src, cols, scale)


@pytest.mark.parametrize("scale", [1.0, 10.0, 2048.0, 4096.0, 1.5])
def test_rows_f32_rounding_to_float32_near_2_pow_24_and_2_pow_40_matches_reference(scale):
    """Au-delà de 2**24, un entier n'est plus représentable en float32 : arrondi au plus proche, pair en cas d'égalité."""
    src = np.array(F32_EDGE, dtype=np.int64).reshape(-1, 1)
    check_rows(src, [0], scale)
    check_rows(np.ascontiguousarray(np.hstack([src, -src])), [1, -1, 0], scale)


@pytest.mark.parametrize("scale", [1.0, 10.0, 1.5])
def test_rows_f32_random_wide_integers_match_reference(scale):
    rng = np.random.default_rng(7)
    src = rng.integers(I64_MIN, I64_MAX, (20_000, 3), dtype=np.int64, endpoint=True)
    check_rows(src, [2, 0, 1], scale)


@pytest.mark.parametrize("scale", [1.0, 10.0])
def test_rows_f32_all_columns_absent_gives_zeros(scale):
    src = np.arange(12, dtype=np.int64).reshape(4, 3)
    got = check_rows(src, [-1, -1], scale)
    assert not got.any() and got.shape == (2, 4)


@pytest.mark.parametrize("scale", [1.0, 10.0])
def test_rows_f32_negative_values_match_reference(scale):
    src = np.array([[-5, -2**24 - 1], [-2048, -4097], [-7, 3]], dtype=np.int64)
    check_rows(src, [0, 1], scale)


@pytest.mark.parametrize("k", [1, 3])
def test_rows_f32_zero_rows_returns_empty_array_of_right_shape(k):
    got = _native.rows_f32(np.zeros((0, 4), dtype=np.int64), np.zeros(k, dtype=np.int64), 10.0)
    assert got.dtype == np.float32 and got.shape == (k, 0)


def test_rows_f32_one_row():
    check_rows(np.array([[3, -4, 2**40]], dtype=np.int64), [2, 1, -1, 0], 2048.0)


def test_rows_f32_single_output_row():
    check_rows(np.arange(30, dtype=np.int64).reshape(10, 3) * 12345, [1], 10.0)


def test_rows_f32_zero_requested_columns_returns_empty_first_axis():
    got = _native.rows_f32(np.ones((5, 2), dtype=np.int64), np.zeros(0, dtype=np.int64), 1.0)
    assert got.dtype == np.float32 and got.shape == (0, 5)


# --- erreurs de rows_f32

def test_rows_f32_rejects_non_contiguous_src():
    base = np.zeros((6, 6), dtype=np.int64)
    with pytest.raises(ValueError):
        _native.rows_f32(base.T, np.array([0], dtype=np.int64), 1.0)
    with pytest.raises(ValueError):
        _native.rows_f32(base[:, ::2], np.array([0], dtype=np.int64), 1.0)


@pytest.mark.parametrize("bad", [3, 100, -2, -2**40])
def test_rows_f32_rejects_column_index_out_of_range(bad):
    """Colonnes valides : 0..2 ou -1 (absente) ; tout le reste est une ValueError, pas une lecture hors tableau."""
    with pytest.raises(ValueError):
        _native.rows_f32(np.zeros((4, 3), dtype=np.int64), np.array([0, bad], dtype=np.int64), 1.0)


@pytest.mark.parametrize("dtype", [np.float64, np.int32])
def test_rows_f32_rejects_wrong_src_dtype(dtype):
    with pytest.raises(TypeError):
        _native.rows_f32(np.zeros((4, 3), dtype=dtype), np.array([0], dtype=np.int64), 1.0)


def test_rows_f32_rejects_int32_cols():
    with pytest.raises(TypeError):
        _native.rows_f32(np.zeros((4, 3), dtype=np.int64), np.array([0], dtype=np.int32), 1.0)


# ----------------------------------------------------------------------------------------------------------
# C. Intégration : `from_session` branché sur les noyaux = `from_session_ref`
# (`from_session` nouveau contre `from_session_ref` : le code numpy d'origine, copié tel quel)

def same_flightlog(a, b):
    """Tous les champs identiques : tableaux au bit près (NaN égaux), même dtype et forme ; sinon égalité simple."""
    assert (a is None) == (b is None)
    if a is None:
        return
    for f in dataclasses.fields(a):
        x, y = getattr(a, f.name), getattr(b, f.name)
        if isinstance(x, np.ndarray):
            assert isinstance(y, np.ndarray), f.name
            assert x.dtype == y.dtype and x.shape == y.shape, f.name
            assert np.array_equal(x, y, equal_nan=x.dtype.kind == "f"), f.name
        elif x is None or y is None:
            assert x is None and y is None, f.name
        else:
            assert x == y or (isinstance(x, float) and np.isnan(x) and np.isnan(y)), f.name
    assert a.stats["implausible_frames"] == b.stats["implausible_frames"]


def compare(sess, path=Path("x.bbl")):
    new = flightlog.from_session(path, copy.deepcopy(sess))
    ref = from_session_ref(path, copy.deepcopy(sess))
    same_flightlog(new, ref)
    return new


def synthetic_columns(n, seed):
    rng = np.random.default_rng(seed)
    cols = {}
    for i in range(3):
        cols[f"gyroADC[{i}]"] = rng.integers(-500, 500, n).tolist()
        cols[f"gyroUnfilt[{i}]"] = rng.integers(-500, 500, n).tolist()
        cols[f"setpoint[{i}]"] = rng.integers(-800, 800, n).tolist()
        cols[f"axisP[{i}]"] = rng.integers(-3000, 3000, n).tolist()
        cols[f"axisI[{i}]"] = rng.integers(-3000, 3000, n).tolist()
        cols[f"axisD[{i}]"] = rng.integers(-3000, 3000, n).tolist()
        cols[f"axisF[{i}]"] = rng.integers(-3000, 3000, n).tolist()
        cols[f"accSmooth[{i}]"] = rng.integers(-4096, 4096, n).tolist()
    cols["setpoint[3]"] = rng.integers(0, 1000, n).tolist()
    for i in range(4):
        cols[f"rcCommand[{i}]"] = rng.integers(1000, 2000, n).tolist()
        cols[f"motor[{i}]"] = rng.integers(48, 2047, n).tolist()
        cols[f"eRPM[{i}]"] = rng.integers(0, 3000, n).tolist()
    for i in range(8):
        cols[f"debug[{i}]"] = rng.integers(-100, 100, n).tolist()
    cols["vbatLatest"] = rng.integers(1400, 1700, n).tolist()
    cols["amperageLatest"] = rng.integers(0, 3000, n).tolist()
    cols["flightModeFlags"] = rng.integers(0, 2**20, n).tolist()
    return cols


HEADERS = [{}, {"blackbox_high_resolution": "1"}, {"acc_1G": "4096", "debug_mode": "6"}]


@pytest.mark.parametrize("header", HEADERS, ids=["hr1", "hr10", "acc4096"])
def test_from_session_synthetic_session_identical_to_reference(header):
    lg = compare(make_session(synthetic_columns(400, 0), header))
    assert lg is not None and lg.stats["implausible_frames"] == 0


@pytest.mark.parametrize("header", HEADERS[:2], ids=["hr1", "hr10"])
@pytest.mark.parametrize("seed", range(5))
def test_from_session_with_impossible_and_wide_values_identical_to_reference(header, seed):
    """Gyro à 5000 °/s, terme P à 70000, valeur >= 2**31 dans une colonne ordinaire (moteur, rcCommand)."""
    n = 300
    sess = make_session(synthetic_columns(n, seed), header)
    rng = np.random.default_rng(100 + seed)
    names = sess.main_names
    for name, value in (("gyroADC[0]", 5000), ("axisP[1]", 70000), ("motor[2]", 2**31 + 7), ("rcCommand[1]", -2**40),
                        ("setpoint[0]", 40001), ("axisD[2]", -65536), ("eRPM[1]", I64_MAX - 3)):
        rows = rng.integers(0, n, 3)
        sess.main[rows, names.index(name)] = value
    lg = compare(sess)
    assert lg is not None and lg.stats["implausible_frames"] > 0


def test_from_session_mostly_corrupt_session_is_none_on_both_sides():
    n = 20
    cols = synthetic_columns(n, 1)
    cols["gyroADC[0]"] = [5000] * 15 + [10] * 5
    assert compare(make_session(cols)) is None


def test_from_session_session_with_one_row_left_is_none_on_both_sides():
    assert compare(make_session({"gyroADC[0]": [5000, 5000, 5000, 100]})) is None


def test_from_session_real_log_identical_to_reference():
    for sess in decode_file(REAL):
        assert compare(sess, REAL) is not None


@pytest.mark.parametrize("seed", range(40))
def test_from_session_damaged_real_log_identical_to_reference(seed, tmp_path):
    """Mêmes fichiers abîmés que `test_native_decoder.test_damaged_real_log_identical` (octets changés, un bit
    inversé, retirés, insérés, fichier coupé), pour exercer des trames impossibles réelles."""
    data = bytearray(REAL.read_bytes())
    h = list_sessions(REAL)[0]
    rng = np.random.default_rng(seed)
    lo, hi = h.data_start, h.data_end
    kind = seed % 4
    for _ in range(int(rng.integers(1, 40))):
        pos = int(rng.integers(lo, min(hi, len(data))))
        if kind == 0:
            data[pos] = int(rng.integers(0, 256))
        elif kind == 1:
            data[pos] ^= 1 << int(rng.integers(0, 8))
        elif kind == 2:
            del data[pos:pos + int(rng.integers(1, 30))]
        else:
            data[pos:pos] = bytes(rng.integers(0, 256, int(rng.integers(1, 30)), dtype=np.uint8))
    if seed % 5 == 0:
        del data[int(rng.integers(lo + 1, len(data))):]
    path = tmp_path / "damaged.bbl"
    path.write_bytes(bytes(data))
    for sess in decode_file(path):
        compare(sess, path)


@pytest.mark.corpus
def test_from_session_corpus_air65_all_sessions_identical_to_reference():
    path = corpus_log("Cedric/BF Chirp/Air65-2.bbl")
    sessions = decode_file(path)
    assert sessions
    for sess in sessions:
        compare(sess, path)


# ----------------------------------------------------------------------------------------------------------
# D. Déterminisme et aliasing

def wild_matrix():
    main, names, rng = base_matrix((2000, 20), 11, 1.0)
    main[rng.integers(0, 2000, 40), 5] = rng.choice(np.array(WIDE_VALUES, dtype=np.int64), 40)  # colonne 5 hors plage
    main[rng.integers(0, 2000, 10), 8] = 70000  # une trame impossible (terme PID)
    return main, names


def test_drop_implausible_is_deterministic():
    main, names = wild_matrix()
    kinds = kinds_for(names)
    a, b = main.copy(), main.copy()
    bad_a = _native.drop_implausible(a, kinds, 1.0)
    bad_b = _native.drop_implausible(b, kinds, 1.0)
    assert np.array_equal(bad_a, bad_b) and a.tobytes() == b.tobytes()


def test_drop_implausible_leaves_ignored_and_in_range_columns_byte_identical():
    """Colonnes ignorées (0) et colonnes sans valeur hors plage : octets identiques à l'entrée ; seule la colonne
    hors plage change."""
    main, names = wild_matrix()
    kinds = kinds_for(names)
    wild = 5
    assert (main[:, wild] >= 2**31).any() or (main[:, wild] < -2**31).any()
    got = main.copy()
    _native.drop_implausible(got, kinds, 1.0)
    for j in range(main.shape[1]):
        if j == wild:
            assert not np.array_equal(got[:, j], main[:, j])
        else:
            assert got[:, j].tobytes() == main[:, j].tobytes(), names[j]


def test_rows_f32_is_deterministic_and_does_not_touch_its_input():
    rng = np.random.default_rng(5)
    src = rng.integers(-2**40, 2**40, (3000, 6), dtype=np.int64)
    keep = src.copy()
    cols = np.array([4, -1, 2, 0], dtype=np.int64)
    a = _native.rows_f32(src, cols, 10.0)
    b = _native.rows_f32(src, cols, 10.0)
    assert a.tobytes() == b.tobytes() and src.tobytes() == keep.tobytes()
    assert a is not b and not np.shares_memory(a, src)


# ---------------------------------------------------------------- rows_f32_scaled (une échelle par colonne)

@pytest.mark.parametrize("seed", range(4))
def test_rows_f32_scaled_equals_separate_calls_bit_for_bit(seed):
    """Une seule passe pour toutes les colonnes (`from_session`) : chaque ligne vaut l'appel à échelle unique."""
    rng = np.random.default_rng(seed)
    src = rng.integers(-(2**40), 2**40, size=(500, 9), dtype=np.int64)
    cols = np.array([3, 0, -1, 8, 3, 5], dtype=np.int64)
    scales = np.array([10.0, 1.0, 4.0, 2048.0, 1.5, 1.0])
    got = _native.rows_f32_scaled(src, cols, scales)
    assert got.dtype == np.float32 and got.shape == (6, 500)
    for i, (c, s) in enumerate(zip(cols, scales)):
        assert np.array_equal(got[i], _native.rows_f32(src, np.array([c], np.int64), float(s))[0]), (i, c, s)


def test_rows_f32_scaled_errors():
    src = np.zeros((4, 3), dtype=np.int64)
    with pytest.raises(ValueError):  # une échelle par colonne
        _native.rows_f32_scaled(src, np.array([0, 1], np.int64), np.array([1.0]))
    with pytest.raises(ValueError):
        _native.rows_f32_scaled(src, np.array([3], np.int64), np.array([1.0]))
    with pytest.raises(ValueError):
        _native.rows_f32_scaled(np.asfortranarray(src), np.array([0], np.int64), np.array([1.0]))
    with pytest.raises(TypeError):
        _native.rows_f32_scaled(src, np.array([0], np.int64), np.array([1.0], np.float32))
    assert _native.rows_f32_scaled(src[:0], np.array([0, 1], np.int64), np.array([2.0, 1.0])).shape == (2, 0)


# ----------------------------------------------------------------------------------------------------------
# E. Cas limites de `from_session` (relecture) : voies absentes, ordre, dtype ; mémoire retenue par les vues

def without(cols, *names):
    return {k: v for k, v in cols.items() if k not in names}


EDGE_CASES = {
    "gyroADC[1] absent": lambda c: without(c, "gyroADC[1]"),
    "debug[1] absent": lambda c: without(c, "debug[1]"),
    "debug[0..2] seulement": lambda c: without(c, *(f"debug[{i}]" for i in range(3, 8))),
    "axisD[2] absent": lambda c: without(c, "axisD[2]"),
    "setpoint[3] absent": lambda c: without(c, "setpoint[3]"),
    "sans eRPM": lambda c: {k: v for k, v in c.items() if not k.startswith("eRPM")},
    "sans gyroUnfilt": lambda c: {k: v for k, v in c.items() if not k.startswith("gyroUnfilt")},
    "ordre inversé": lambda c: dict(reversed(list(c.items()))),
}


@pytest.mark.parametrize("case", sorted(EDGE_CASES))
@pytest.mark.parametrize("header", [{}, {"debug_mode": "GYRO_SCALED"}], ids=["défaut", "gyro_scaled"])
def test_from_session_missing_or_reordered_fields_identical_to_reference(case, header):
    """Les deux chemins de `axes()` : vue du lot (voies consécutives) et copie (voie absente, ordre inversé)."""
    compare(make_session(EDGE_CASES[case](synthetic_columns(250, 3)), header))


@pytest.mark.parametrize("how", ["fortran", "int32"])
def test_from_session_main_with_another_layout_or_dtype_identical_to_reference(how):
    sess = make_session(synthetic_columns(250, 4), {})
    sess.main = np.asfortranarray(sess.main) if how == "fortran" else sess.main.astype(np.int32)
    compare(sess)


def test_from_session_read_only_main_still_works():
    """L'étalon plantait sur un tableau en lecture seule (écriture sur place) ; le port en travaille sur une copie."""
    sess = make_session(synthetic_columns(250, 5), {})
    expected = flightlog.from_session(Path("x.bbl"), copy.deepcopy(sess))
    sess.main.flags.writeable = False
    same_flightlog(flightlog.from_session(Path("x.bbl"), sess), expected)


def test_from_session_main_names_shorter_than_columns_is_a_clear_error():
    sess = make_session(synthetic_columns(100, 6), {})
    sess.main_names = sess.main_names[:-1]
    with pytest.raises(ValueError, match="kinds"):
        flightlog.from_session(Path("x.bbl"), sess)


def test_flightlog_arrays_do_not_keep_unused_rows_of_the_shared_batch_alive():
    """Les voies lues en une passe sont des VUES d'un même tableau : il ne doit contenir que des lignes que le
    FlightLog garde (gyro 3, gyro non filtré 3, debug 8, rcCommand 4, termes PID 12, acc 3), pas la consigne, les
    moteurs, l'eRPM, la tension ni le courant, déjà recopiés ailleurs : ≈ 34 Mo de trop pour 600 000 trames."""
    lg = flightlog.from_session(Path("x.bbl"), make_session(synthetic_columns(300, 7), {}))
    kept = 3 + 3 + 8 + 4 + 12 + 3
    base = lg.gyro.base
    assert base is not None and base.shape == (kept, lg.n), base.shape
    for name in ("gyro_unfilt", "rc_command", "p_term", "i_term", "d_term", "f_term", "debug", "acc"):
        assert getattr(lg, name).base is base, name
