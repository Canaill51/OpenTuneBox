"""Le filet de la migration vers Rust (`scripts/golden.py`, D-031) : il voit une différence, jamais à tort."""

import importlib
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

SCRIPTS = Path(__file__).parent.parent / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))
golden = importlib.import_module("golden")

LOG = Path(__file__).parent / "data" / "jeno5_5s.bbl"


def test_decode_digest_stable_and_sensitive(tmp_path):
    """Deux décodages du même fichier ont les mêmes empreintes ; un octet changé dans les trames les change."""
    a = golden.decode_digest(LOG)
    assert a == golden.decode_digest(LOG) and a
    data = bytearray(LOG.read_bytes())
    data[len(data) // 2] ^= 0x01
    altered = tmp_path / "alt.bbl"
    altered.write_bytes(bytes(data))
    assert golden.decode_digest(altered) != a


@dataclass
class _Res:
    x: np.ndarray
    name: str


def test_flatten_covers_every_value():
    flat = golden.flatten({"a": _Res(np.arange(3.0), "roll"), "b": [1, None], "c": (np.float32(2.5),)}, "o")
    assert set(flat) == {"o/a.x", "o/a.name", "o/b[0]", "o/b[1]", "o/c[0]"}
    assert flat["o/a.name"] == "roll"


def test_same_tolerance():
    a = np.array([1.0, 2.0, 0.0, np.nan])
    assert golden.same(a, a.copy(), 0.0)
    assert not golden.same(a, a * (1 + 1e-4), 0.0)  # exigence « identique » : tout écart est vu
    assert golden.same(a, a * (1 + 1e-6), 1e-5) and not golden.same(a, a * (1 + 1e-3), 1e-5)
    assert not golden.same(a, a[:3], 1.0)  # forme différente
    assert not golden.same(np.array(["x"]), np.array(["y"]), 1.0)
