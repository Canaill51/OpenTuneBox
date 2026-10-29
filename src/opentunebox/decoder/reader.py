"""Lecture d'un fichier Blackbox complet → sessions décodées (valeurs brutes entières)."""

from __future__ import annotations

from dataclasses import dataclass, field
import os
from pathlib import Path

import numpy as np

from .header import (
    CTX_DATA_VERSION,
    CTX_HOME0_IDX,
    CTX_HOME1_IDX,
    CTX_MINTHROTTLE,
    CTX_MOTOR0_IDX,
    CTX_MOTOR_MIN,
    CTX_SIZE,
    CTX_VBATREF,
    FrameDef,
    LogHeader,
    as_array,
    find_log_starts,
    parse_header,
)


@dataclass
class DecodedSession:
    """Une session décodée : valeurs brutes (unités du firmware), sans conversion."""

    index: int  # numéro de session dans le fichier (1, 2, …)
    header: LogHeader
    main_names: list[str]
    main: np.ndarray  # (n, champs) int64 : trames I/P valides, dans l'ordre
    main_kind: np.ndarray  # 0 = I, 1 = P
    slow_names: list[str]
    slow: np.ndarray  # (n, champs lents) : dernière trame S connue à chaque ligne principale
    gps_names: list[str]
    gps: np.ndarray
    gps_at: np.ndarray
    events: np.ndarray  # (e, 4) : type, ligne principale, donnée1, donnée2
    stats: dict = field(default_factory=dict)

    def column(self, name: str) -> np.ndarray | None:
        if name in self.main_names:
            return self.main[:, self.main_names.index(name)]
        if name in self.slow_names:
            return self.slow[:, self.slow_names.index(name)]
        return None


def decode_stream_backend():
    """Implémentation du décodage : `OTB_BACKEND=rust` (extension `opentunebox._native`) (défaut) ou `python` (numba,
    gardé le temps de la migration, D-031). Lue à chaque appel : les tests de parité passent de l'une à l'autre."""
    backend = os.environ.get("OTB_BACKEND", "rust")
    if backend == "rust":
        from .. import _native
        return _native.decode_stream
    if backend != "python":
        raise ValueError(f"OTB_BACKEND inconnu : {backend!r} (python ou rust)")
    try:
        from . import core  # numba : chargé seulement à la demande (tests de parité), pas dans l'installateur
    except ImportError as e:
        raise ValueError("OTB_BACKEND=python : numba n'est pas installé (extra « dev », ou absent de l'exécutable)") from e

    return core.decode_stream


def list_sessions(path: str | Path) -> list[LogHeader]:
    """En-têtes de toutes les sessions d'un fichier (rapide, sans décoder les trames)."""
    data = Path(path).read_bytes()
    starts = find_log_starts(data)
    out = []
    for k, s in enumerate(starts):
        e = starts[k + 1] if k + 1 < len(starts) else len(data)
        out.append(parse_header(data, s, e))
    return out


def decode_file(path: str | Path, only: int | None = None) -> list[DecodedSession]:
    """Décode toutes les sessions (ou seulement la n° `only`, à partir de 1)."""
    data = Path(path).read_bytes()
    buf = np.frombuffer(data, dtype=np.uint8)
    starts = find_log_starts(data)
    sessions = []
    for k, s in enumerate(starts):
        if only is not None and k + 1 != only:
            continue
        e = starts[k + 1] if k + 1 < len(starts) else len(data)
        header = parse_header(data, s, e)
        sess = _decode_session(buf, header, k + 1)
        if sess is not None:
            sessions.append(sess)
    return sessions


def _defs(fd: FrameDef | None):
    if fd is None or not fd.complete():
        z = np.zeros(1, dtype=np.int64)
        return z, z, 0, False
    n = fd.count
    return as_array(fd.predictor, n), as_array(fd.encoding, n), n, True


def _decode_session(buf: np.ndarray, header: LogHeader, index: int) -> DecodedSession | None:
    fi = header.frames.get("I")
    if fi is None or not fi.complete() or fi.count < 2:  # itération et temps sont indispensables (D-031)
        return None
    fp = header.frames.get("P")
    i_pred, i_enc, n_main, _ = _defs(fi)
    p_pred, p_enc, _, has_p = _defs(fp)
    s_pred, s_enc, n_slow, has_s = _defs(header.frames.get("S"))
    g_pred, g_enc, n_gps, has_g = _defs(header.frames.get("G"))
    h_pred, h_enc, n_home, has_h = _defs(header.frames.get("H"))
    fh = header.frames.get("H")

    ctx = np.zeros(CTX_SIZE, dtype=np.int64)
    ctx[CTX_MINTHROTTLE] = header.minthrottle
    ctx[CTX_MOTOR0_IDX] = max(0, fi.index("motor[0]"))
    ctx[CTX_VBATREF] = header.vbatref
    ctx[CTX_MOTOR_MIN] = header.motor_output[0]
    ctx[CTX_HOME0_IDX] = max(0, fh.index("GPS_home[0]")) if fh else 0
    ctx[CTX_HOME1_IDX] = max(0, fh.index("GPS_home[1]")) if fh else 0
    ctx[CTX_DATA_VERSION] = header.data_version

    p_num, p_denom = header.p_interval
    nbytes = header.data_end - header.data_start
    est_rows = max(1024, nbytes // 20)

    main, kind, slow, slow_at, gps, gps_at, events, st = decode_stream_backend()(
        buf,
        header.data_start,
        header.data_end,
        i_pred,
        i_enc,
        p_pred,
        p_enc,
        n_main,
        s_pred,
        s_enc,
        n_slow,
        g_pred,
        g_enc,
        n_gps,
        h_pred,
        h_enc,
        n_home,
        has_p,
        has_s,
        has_g,
        has_h,
        header.i_interval,
        p_num,
        p_denom,
        ctx,
        est_rows,
    )

    slow_names = header.frames["S"].names if has_s else []
    slow_full = _forward_fill(slow, slow_at, main.shape[0], max(n_slow, 1))[:, : len(slow_names)]
    gps_names = header.frames["G"].names if has_g else []

    stats = {
        "corrupt_frames": int(st[0]),
        "desync_frames": int(st[1]),
        "skipped_iterations": int(st[2]),
        "log_end_seen": bool(st[3]),
        "bytes": int(st[4]),
        "i_frames": int(st[5]),
        "p_frames": int(st[6]),
    }
    return DecodedSession(
        index=index,
        header=header,
        main_names=list(fi.names),
        main=main,
        main_kind=kind,
        slow_names=list(slow_names),
        slow=slow_full,
        gps_names=list(gps_names),
        gps=gps[:, : len(gps_names)],
        gps_at=gps_at,
        events=events,
        stats=stats,
    )


def _forward_fill(slow: np.ndarray, at: np.ndarray, n: int, width: int) -> np.ndarray:
    """Pour chaque ligne principale, la dernière trame S reçue avant elle (zéros avant la première)."""
    out = np.zeros((n, width), dtype=np.int64)
    if slow.shape[0] == 0 or n == 0:
        return out
    # indice de la dernière trame S dont la position « at » est ≤ ligne
    idx = np.searchsorted(at, np.arange(n), side="right") - 1
    ok = idx >= 0
    out[ok] = slow[idx[ok]]
    return out
