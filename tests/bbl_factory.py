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


# ---------------------------------------------------------------- encodeurs groupés (trames I/P de Betaflight)


def _widths_tag2(values: list[int]) -> bytes:
    """Sélecteur 3 des TAG2_3S32 / TAG2_3SVARIABLE : largeur 8, 16, 24 ou 32 bits par valeur (2 bits chacune,
    la première valeur dans les bits de poids faible), puis les octets petit-boutiste."""
    lead, payload = 0xC0, bytearray()
    for k, v in enumerate(values):
        nbytes = next(b for b in (1, 2, 3, 4) if -(1 << (8 * b - 1)) <= v < (1 << (8 * b - 1)))
        lead |= (nbytes - 1) << (2 * k)
        payload += (v & ((1 << (8 * nbytes)) - 1)).to_bytes(nbytes, "little")
    return bytes([lead]) + bytes(payload)


def _fits(v: int, bits: int) -> bool:
    return -(1 << (bits - 1)) <= v < (1 << (bits - 1))


def enc_tag2_3svariable(values: list[int]) -> bytes:
    """TAG2_3SVARIABLE (blackbox_encoding.c, `blackboxWriteTag2_3SVariable`) : 2/2/2, 5/5/4, 8/7/7 bits ou
    largeurs par valeur."""
    v0, v1, v2 = values
    if all(_fits(v, 2) for v in values):
        return bytes([((v0 & 3) << 4) | ((v1 & 3) << 2) | (v2 & 3)])
    if _fits(v0, 5) and _fits(v1, 5) and _fits(v2, 4):
        return bytes([0x40 | ((v0 & 0x1F) << 1) | ((v1 >> 4) & 1), ((v1 & 0x0F) << 4) | (v2 & 0x0F)])
    if _fits(v0, 8) and _fits(v1, 7) and _fits(v2, 7):
        return bytes([0x80 | ((v0 >> 2) & 0x3F), ((v0 & 3) << 6) | ((v1 >> 1) & 0x3F), ((v1 & 1) << 7) | (v2 & 0x7F)])
    return _widths_tag2(values)


def enc_tag2_3s32(values: list[int]) -> bytes:
    """TAG2_3S32 : 2, 4 ou 6 bits par valeur, sinon largeurs par valeur."""
    v0, v1, v2 = values
    if all(_fits(v, 2) for v in values):
        return bytes([((v0 & 3) << 4) | ((v1 & 3) << 2) | (v2 & 3)])
    if all(_fits(v, 4) for v in values):
        return bytes([0x40 | (v0 & 0x0F), ((v1 & 0x0F) << 4) | (v2 & 0x0F)])
    if all(_fits(v, 6) for v in values):
        return bytes([0x80 | (v0 & 0x3F), v1 & 0x3F, v2 & 0x3F])
    return _widths_tag2(values)


def enc_tag8_4s16_v2(values: list[int]) -> bytes:
    """TAG8_4S16 (Data version 2) : sélecteur 2 bits par valeur (0 = zéro, 1 = 4 bits, 2 = 8 bits, 3 = 16 bits),
    puis les quartets à la suite, poids fort d'abord (`blackboxWriteTag8_4S16`)."""
    selector, nibbles = 0, []
    for k, v in enumerate(values):
        if v == 0:
            s = 0
        elif _fits(v, 4):
            s, nibbles = 1, nibbles + [v & 0xF]
        elif _fits(v, 8):
            s, nibbles = 2, nibbles + [(v >> 4) & 0xF, v & 0xF]
        else:
            s, nibbles = 3, nibbles + [(v >> 12) & 0xF, (v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF]
        selector |= s << (2 * k)
    if len(nibbles) % 2:
        nibbles.append(0)
    return bytes([selector] + [(nibbles[i] << 4) | nibbles[i + 1] for i in range(0, len(nibbles), 2)])


def enc_tag8_4s16_v1(values: list[int]) -> bytes:
    """TAG8_4S16 (Data version 1, Cleanflight 2014) : même sélecteur ; un champ de 4 bits emporte le suivant dans
    le quartet haut du même octet (quel que soit son sélecteur), comme le lit `blackbox_decode`."""
    selector, out, k = 0, bytearray(), 0
    while k < 4:
        v = values[k]
        if v == 0:
            s = 0
        elif _fits(v, 4):
            s = 1
            nxt = values[k + 1] if k + 1 < 4 else 0
            assert _fits(nxt, 4), "après un champ de 4 bits, le suivant tient aussi sur 4 bits"
            out.append((v & 0xF) | ((nxt & 0xF) << 4))
            selector |= 1 << (2 * k)
            if k + 1 < 4:
                selector |= 1 << (2 * (k + 1))
            k += 2
            continue
        elif _fits(v, 8):
            s = 2
            out.append(v & 0xFF)
        else:
            s = 3
            out += (v & 0xFFFF).to_bytes(2, "little")
        selector |= s << (2 * k)
        k += 1
    return bytes([selector]) + bytes(out)


def enc_tag8_8svb(values: list[int]) -> bytes:
    """TAG8_8SVB : un octet de présence (bit k = valeur k non nulle), puis les valeurs non nulles en svb ; une
    seule valeur : svb direct."""
    if len(values) == 1:
        return enc_svb(values[0])
    header = sum(1 << k for k, v in enumerate(values) if v)
    return bytes([header]) + b"".join(enc_svb(v) for v in values if v)


def enc_neg_14bit(v: int) -> bytes:
    """NEG_14BIT : −v sur 14 bits en uvb (champ `vbatLatest` des anciens firmwares)."""
    return enc_uvb((-v) & 0x3FFF)
