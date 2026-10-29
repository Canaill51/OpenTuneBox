"""Lecture des en-têtes d'un fichier Blackbox et découpage en sessions.

Un fichier .bbl/.bfl peut contenir plusieurs sessions (une par armement). Chacune commence par
la ligne « H Product:Blackbox flight data recorder by Nicholas Sherlock », suivie de lignes
« H clé:valeur » puis des trames binaires.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

LOG_START_MARKER = b"H Product:Blackbox flight data recorder by Nicholas Sherlock"
FRAME_MARKERS = b"IPGHSE"

# Prédicteurs (voir docs/spec : format Blackbox)
PRED_ZERO = 0
PRED_PREVIOUS = 1
PRED_STRAIGHT_LINE = 2
PRED_AVERAGE_2 = 3
PRED_MINTHROTTLE = 4
PRED_MOTOR_0 = 5
PRED_INC = 6
PRED_HOME_COORD = 7
PRED_1500 = 8
PRED_VBATREF = 9
PRED_LAST_MAIN_FRAME_TIME = 10
PRED_MINMOTOR = 11
PRED_HOME_COORD_1 = 256  # second membre d'une paire lat/lon (réécrit à la lecture)

# Encodages
ENC_SIGNED_VB = 0
ENC_UNSIGNED_VB = 1
ENC_NEG_14BIT = 3
ENC_TAG8_8SVB = 6
ENC_TAG2_3S32 = 7
ENC_TAG8_4S16 = 8
ENC_NULL = 9
ENC_TAG2_3SVARIABLE = 10


# Indices dans le tableau de contexte `ctx`
CTX_MINTHROTTLE = 0
CTX_MOTOR0_IDX = 1
CTX_VBATREF = 2
CTX_MOTOR_MIN = 3
CTX_HOME0_IDX = 4
CTX_HOME1_IDX = 5
CTX_HAS_LAST_TIME = 6
CTX_LAST_TIME = 7
CTX_DATA_VERSION = 8
CTX_SIZE = 9


@dataclass
class FrameDef:
    """Définition d'un type de trame : noms, signes, prédicteurs et encodages des champs."""

    names: list[str] = field(default_factory=list)
    signed: list[int] = field(default_factory=list)
    predictor: list[int] = field(default_factory=list)
    encoding: list[int] = field(default_factory=list)

    @property
    def count(self) -> int:
        return len(self.names)

    def index(self, name: str) -> int:
        try:
            return self.names.index(name)
        except ValueError:
            return -1

    def complete(self) -> bool:
        n = self.count
        return n > 0 and len(self.predictor) == n and len(self.encoding) == n


@dataclass
class LogHeader:
    """En-tête d'une session : toutes les lignes H brutes + définitions de trames."""

    raw: dict[str, str]
    frames: dict[str, FrameDef]
    data_start: int  # position (dans le fichier) de la première trame binaire
    data_end: int  # fin de la session (début de la suivante ou fin du fichier)

    # Paramètres utiles au décodage
    @property
    def data_version(self) -> int:
        return _int(self.raw.get("Data version"), 2)

    @property
    def i_interval(self) -> int:
        return max(1, _int(self.raw.get("I interval"), 1))

    @property
    def p_interval(self) -> tuple[int, int]:
        """(numérateur, dénominateur) de « P interval » (forme « N » ou « num/den »)."""
        v = self.raw.get("P interval", "1")
        if "/" in v:
            a, b = v.split("/", 1)
            return max(1, _int(a, 1)), max(1, _int(b, 1))
        return 1, max(1, _int(v, 1))

    @property
    def minthrottle(self) -> int:
        return _int(self.raw.get("minthrottle"), 0)

    @property
    def motor_output(self) -> tuple[int, int]:
        vals = _ints(self.raw.get("motorOutput"))
        if len(vals) >= 2:
            return vals[0], vals[1]
        if len(vals) == 1:
            return vals[0], 2047
        return self.minthrottle, _int(self.raw.get("maxthrottle"), 2047)

    @property
    def vbatref(self) -> int:
        return _int(self.raw.get("vbatref"), 0)


def _int(v: str | None, default: int = 0) -> int:
    if v is None:
        return default
    try:
        return int(v.strip(), 0) if v.strip().lower().startswith("0x") else int(float(v.strip()))
    except ValueError:
        return default


def _ints(v: str | None) -> list[int]:
    if not v:
        return []
    out = []
    for part in v.split(","):
        try:
            out.append(int(float(part)))
        except ValueError:
            pass
    return out


def find_log_starts(data: bytes) -> list[int]:
    """Positions de début de chaque session dans le fichier."""
    starts = []
    pos = data.find(LOG_START_MARKER)
    while pos != -1:
        starts.append(pos)
        pos = data.find(LOG_START_MARKER, pos + len(LOG_START_MARKER))
    return starts


def parse_header(data: bytes, start: int, end: int) -> LogHeader:
    """Lit les lignes « H … » à partir de `start` jusqu'à la première trame binaire."""
    raw: dict[str, str] = {}
    frames: dict[str, FrameDef] = {}
    pos = start
    while pos < end:
        c = data[pos]
        if c == ord("H") and pos + 1 < end and data[pos + 1] == ord(" "):
            nl = data.find(b"\n", pos, end)
            if nl == -1:
                nl = end
            line = data[pos + 2 : nl].decode("latin-1").rstrip("\r")
            pos = nl + 1
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            raw[key] = value
            _apply_field_header(frames, key, value)
        elif c in FRAME_MARKERS:
            break
        else:
            pos += 1  # octets parasites avant la première trame

    # Les trames P reprennent les noms/signes des trames I
    fi = frames.get("I")
    fp = frames.get("P")
    if fi is not None and fp is not None:
        fp.names = list(fi.names)
        fp.signed = list(fi.signed)

    # Paires de coordonnées GPS : le second prédicteur HOME_COORD vise la longitude
    fg = frames.get("G")
    if fg is not None:
        for i in range(1, min(fg.count, len(fg.predictor))):  # en-tête tronqué : moins de prédicteurs que de noms
            if fg.predictor[i - 1] == PRED_HOME_COORD and fg.predictor[i] == PRED_HOME_COORD:
                fg.predictor[i] = PRED_HOME_COORD_1

    return LogHeader(raw=raw, frames=frames, data_start=pos, data_end=end)


def _apply_field_header(frames: dict[str, FrameDef], key: str, value: str) -> None:
    # Clés de la forme « Field I name », « Field P predictor », …
    if not key.startswith("Field ") or len(key) < 8:
        return
    ftype = key[6]
    prop = key[8:]
    fd = frames.setdefault(ftype, FrameDef())
    if prop == "name":
        fd.names = [n.strip() for n in value.split(",")]
    elif prop == "signed":
        fd.signed = _ints(value)
    elif prop == "predictor":
        fd.predictor = _ints(value)
    elif prop == "encoding":
        fd.encoding = _ints(value)


def as_array(values: list[int], n: int) -> np.ndarray:
    a = np.zeros(max(n, 1), dtype=np.int64)
    a[: len(values)] = values[:n]
    return a
