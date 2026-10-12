"""Fabrique de fichiers Blackbox (.bbl) pour tester le décodeur sans corpus.

On écrit l'en-tête (« H … ») et des trames I / P / S / E encodées comme le firmware, à partir d'une
définition de champs (prédicteur et encodage par trame). `Writer` suit les prédicteurs pour produire les
bons deltas, et `session()` enchaîne I et P selon `I interval` / `P interval` comme Betaflight.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from opentunebox.decoder.header import (
    ENC_NULL,
    ENC_SIGNED_VB,
    ENC_UNSIGNED_VB,
    LOG_START_MARKER,
    PRED_AVERAGE_2,
    PRED_INC,
    PRED_PREVIOUS,
    PRED_STRAIGHT_LINE,
    PRED_ZERO,
)

# ---------------------------------------------------------------- encodeurs élémentaires


def enc_uvb(v: int) -> bytes:
    """Entier non signé à longueur variable (7 bits utiles par octet)."""
    out = bytearray()
    v &= 0xFFFFFFFF
    while v >= 0x80:
        out.append((v & 0x7F) | 0x80)
        v >>= 7
    out.append(v)
    return bytes(out)


def enc_svb(v: int) -> bytes:
    """Entier signé : zigzag puis uvb."""
    return enc_uvb(((v << 1) ^ (v >> 31)) & 0xFFFFFFFF)


# ---------------------------------------------------------------- définition des champs


@dataclass
class Field:
    name: str
    signed: int
    pred_i: int
    enc_i: int
    pred_p: int
    enc_p: int


def gyro_fields(extra: list[str] = ()) -> list[Field]:
    """Champs d'un log minimal : itération, temps, gyro 3 axes (+ `extra`, signés, prédits par la valeur
    précédente), avec les prédicteurs qu'utilise Betaflight pour ces champs."""
    fields = [
        Field("loopIteration", 0, PRED_ZERO, ENC_UNSIGNED_VB, PRED_INC, ENC_NULL),
        Field("time", 0, PRED_ZERO, ENC_UNSIGNED_VB, PRED_STRAIGHT_LINE, ENC_SIGNED_VB),
    ]
    for ax in range(3):
        fields.append(Field(f"gyroADC[{ax}]", 1, PRED_ZERO, ENC_SIGNED_VB, PRED_PREVIOUS, ENC_SIGNED_VB))
    for name in extra:
        fields.append(Field(name, 1, PRED_ZERO, ENC_SIGNED_VB, PRED_PREVIOUS, ENC_SIGNED_VB))
    return fields


def header_bytes(fields: list[Field], i_interval: int = 32, p_interval: str = "1/1",
                 raw: dict[str, str] | None = None, slow: list[str] = ()) -> bytes:
    """Lignes « H … » d'une session : produit, version, intervalles, définitions I et P (et S si `slow`),
    puis les paires clé:valeur de `raw` (Firmware revision, looptime, …)."""
    lines = [LOG_START_MARKER, b"H Data version:2", f"H I interval:{i_interval}".encode(),
             f"H P interval:{p_interval}".encode()]
    join = lambda vals: ",".join(str(v) for v in vals)  # noqa: E731
    lines += [
        f"H Field I name:{join(f.name for f in fields)}".encode(),
        f"H Field I signed:{join(f.signed for f in fields)}".encode(),
        f"H Field I predictor:{join(f.pred_i for f in fields)}".encode(),
        f"H Field I encoding:{join(f.enc_i for f in fields)}".encode(),
        f"H Field P predictor:{join(f.pred_p for f in fields)}".encode(),
        f"H Field P encoding:{join(f.enc_p for f in fields)}".encode(),
    ]
    if slow:
        lines += [
            f"H Field S name:{join(slow)}".encode(),
            f"H Field S signed:{join(0 for _ in slow)}".encode(),
            f"H Field S predictor:{join(PRED_ZERO for _ in slow)}".encode(),
            f"H Field S encoding:{join(ENC_UNSIGNED_VB for _ in slow)}".encode(),
        ]
    for k, v in (raw or {}).items():
        lines.append(f"H {k}:{v}".encode())
    return b"\n".join(lines) + b"\n"


# ---------------------------------------------------------------- trames


def should_have_frame(idx: int, i_interval: int, p_num: int, p_denom: int) -> bool:
    """Même règle que le firmware (`blackboxShouldLogPFrame`) et que `core._should_have_frame`."""
    return ((idx % i_interval) + p_num - 1) % p_denom < p_num


class Writer:
    """Encode les trames I et P d'une session en suivant les prédicteurs de la définition."""

    def __init__(self, fields: list[Field]) -> None:
        self.fields = fields
        self.prev: list[int] | None = None
        self.prev2: list[int] | None = None

    def iframe(self, values: list[int]) -> bytes:
        out = bytearray(b"I")
        for f, v in zip(self.fields, values):
            assert f.pred_i == PRED_ZERO, "la fabrique n'écrit que des trames I sans prédicteur"
            out += enc_uvb(v) if f.enc_i == ENC_UNSIGNED_VB else enc_svb(v)
        self.prev = self.prev2 = list(values)
        return bytes(out)

    def pframe(self, values: list[int]) -> bytes:
        assert self.prev is not None, "une trame I doit précéder la première trame P"
        out = bytearray(b"P")
        for i, (f, v) in enumerate(zip(self.fields, values)):
            p, pv, pv2 = f.pred_p, self.prev[i], self.prev2[i]
            if p == PRED_INC:
                continue  # rien d'écrit : l'itération se déduit des trames sautées
            if p == PRED_PREVIOUS:
                base = pv
            elif p == PRED_STRAIGHT_LINE:
                base = 2 * pv - pv2
            elif p == PRED_AVERAGE_2:
                s = pv + pv2
                base = abs(s) // 2 * (1 if s >= 0 else -1)  # troncature vers zéro, comme en C
            elif p == PRED_ZERO:
                base = 0
            else:
                raise ValueError(f"prédicteur P non géré par la fabrique : {p}")
            delta = v - base
            out += enc_svb(delta) if f.enc_p == ENC_SIGNED_VB else enc_uvb(delta)
        self.prev2, self.prev = self.prev, list(values)
        return bytes(out)


def sframe(values: list[int]) -> bytes:
    return b"S" + b"".join(enc_uvb(v) for v in values)


def event(kind: int, *payload: int) -> bytes:
    """Événement E : type puis valeurs uvb (`EVENT_FLIGHT_MODE` : drapeaux, anciens drapeaux…)."""
    return b"E" + bytes([kind]) + b"".join(enc_uvb(v) for v in payload)


def event_log_end() -> bytes:
    return b"E\xffEnd of log\x00"


def session(fields: list[Field], rows: list[list[int]], i_interval: int = 32, p_num: int = 1,
            p_denom: int = 1, raw: dict[str, str] | None = None, log_end: bool = True,
            slow: list[str] = (), slow_rows: dict[int, list[int]] | None = None) -> bytes:
    """Session complète : en-tête puis, pour chaque ligne de `rows` (qui commence par l'itération), une trame
    I aux multiples de `i_interval`, une trame P quand le firmware en écrirait une, rien sinon ; une trame S
    avant la ligne d'index k si `slow_rows[k]`."""
    p_interval = f"{p_num}/{p_denom}" if p_denom != 1 or p_num != 1 else "1/1"
    out = bytearray(header_bytes(fields, i_interval, p_interval, raw, slow))
    w = Writer(fields)
    for k, row in enumerate(rows):
        if slow_rows and k in slow_rows:
            out += sframe(slow_rows[k])
        it = row[0]
        if it % i_interval == 0:
            out += w.iframe(row)
        elif should_have_frame(it, i_interval, p_num, p_denom):
            out += w.pframe(row)
    if log_end:
        out += event_log_end()
    return bytes(out)


def rows_2khz(n: int, i_interval: int = 32, p_num: int = 1, p_denom: int = 1, gyro: int = 0,
              t0: int = 1_000_000, dt: int = 125) -> list[list[int]]:
    """Lignes [itération, temps, gyro×3] pour les `n` premières itérations d'une boucle à 8 kHz (dt = 125 µs),
    seulement celles que le firmware enregistrerait (`rows` de `session`)."""
    return [[k, t0 + k * dt, gyro + k, -gyro - k, 7] for k in range(n)
            if k % i_interval == 0 or should_have_frame(k, i_interval, p_num, p_denom)]


def write(tmp_path: Path, data: bytes, name: str = "synth.bbl") -> Path:
    p = tmp_path / name
    p.write_bytes(data)
    return p
