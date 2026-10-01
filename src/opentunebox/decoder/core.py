"""Décodage binaire des trames Blackbox, compilé par numba.

Machine à états fidèle au décodeur de référence (blackbox-tools / Blackbox Explorer) :
- une trame n'est validée que lorsque l'octet qui la suit ressemble au début d'une nouvelle trame
  (ou à la fin du fichier) et qu'elle fait au plus 256 octets ; sinon on reprend la lecture un octet
  après son début (resynchronisation) ;
- une trame I valide resynchronise le flux principal ; les trames P ne sont acceptées que si le flux
  est valide et que le temps / l'itération n'ont pas sauté trop loin.
"""

from __future__ import annotations

import numpy as np
from numba import njit

from .header import (
    ENC_NEG_14BIT,
    ENC_NULL,
    ENC_SIGNED_VB,
    ENC_TAG2_3S32,
    ENC_TAG2_3SVARIABLE,
    ENC_TAG8_4S16,
    ENC_TAG8_8SVB,
    ENC_UNSIGNED_VB,
    PRED_1500,
    PRED_AVERAGE_2,
    PRED_HOME_COORD,
    PRED_HOME_COORD_1,
    PRED_INC,
    PRED_LAST_MAIN_FRAME_TIME,
    PRED_MINMOTOR,
    PRED_MINTHROTTLE,
    PRED_MOTOR_0,
    PRED_PREVIOUS,
    PRED_STRAIGHT_LINE,
    PRED_VBATREF,
)

MAX_FRAME_LENGTH = 256
MAX_TIME_JUMP = 10 * 1000000
MAX_ITERATION_JUMP = 500 * 10

FIELD_ITERATION = 0
FIELD_TIME = 1

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

# Indices dans le tableau d'état du flux `st`
ST_POS = 0
ST_EOF = 1
ST_END = 2

EVENT_SYNC_BEEP = 0
EVENT_AUTOTUNE_CYCLE_START = 10
EVENT_AUTOTUNE_CYCLE_RESULT = 11
EVENT_AUTOTUNE_TARGETS = 12
EVENT_INFLIGHT_ADJUSTMENT = 13
EVENT_LOGGING_RESUME = 14
EVENT_DISARM = 15
EVENT_GTUNE_CYCLE_RESULT = 20
EVENT_FLIGHT_MODE = 30
EVENT_TWITCH_TEST = 40
EVENT_LOG_END = 255

KIND_I = 0
KIND_P = 1


# ---------------------------------------------------------------- lecture d'octets


@njit(cache=True)
def _read_byte(buf, st):
    pos = st[ST_POS]
    if pos >= st[ST_END]:
        st[ST_EOF] = 1
        return -1
    st[ST_POS] = pos + 1
    return np.int64(buf[pos])


@njit(cache=True)
def _to_int32(v):
    v = v & 0xFFFFFFFF
    if v >= 0x80000000:
        v -= 0x100000000
    return v


@njit(cache=True)
def _sign_extend(v, bits):
    v = v & ((1 << bits) - 1)
    if v & (1 << (bits - 1)):
        v -= 1 << bits
    return v


@njit(cache=True)
def _read_uvb(buf, st):
    result = np.int64(0)
    shift = 0
    for _ in range(5):
        b = _read_byte(buf, st)
        if b < 0:
            return np.int64(0)
        result = result | ((b & 0x7F) << shift)
        if b < 128:
            return result & 0xFFFFFFFF
        shift += 7
    return np.int64(0)


@njit(cache=True)
def _read_svb(buf, st):
    u = _read_uvb(buf, st)
    return _to_int32((u >> 1) ^ (-(u & 1)))


@njit(cache=True)
def _read_tag2_3s32(buf, st, values):
    lead = _read_byte(buf, st)
    sel = (lead >> 6) & 3
    if sel == 0:
        values[0] = _sign_extend((lead >> 4) & 0x03, 2)
        values[1] = _sign_extend((lead >> 2) & 0x03, 2)
        values[2] = _sign_extend(lead & 0x03, 2)
    elif sel == 1:
        values[0] = _sign_extend(lead & 0x0F, 4)
        lead = _read_byte(buf, st)
        values[1] = _sign_extend(lead >> 4, 4)
        values[2] = _sign_extend(lead & 0x0F, 4)
    elif sel == 2:
        values[0] = _sign_extend(lead & 0x3F, 6)
        lead = _read_byte(buf, st)
        values[1] = _sign_extend(lead & 0x3F, 6)
        lead = _read_byte(buf, st)
        values[2] = _sign_extend(lead & 0x3F, 6)
    else:
        _read_tag2_wide(buf, st, values, lead)


@njit(cache=True)
def _read_tag2_wide(buf, st, values, lead):
    for i in range(3):
        w = lead & 0x03
        if w == 0:
            values[i] = _sign_extend(_read_byte(buf, st), 8)
        elif w == 1:
            b1 = _read_byte(buf, st)
            b2 = _read_byte(buf, st)
            values[i] = _sign_extend(b1 | (b2 << 8), 16)
        elif w == 2:
            b1 = _read_byte(buf, st)
            b2 = _read_byte(buf, st)
            b3 = _read_byte(buf, st)
            values[i] = _sign_extend(b1 | (b2 << 8) | (b3 << 16), 24)
        else:
            b1 = _read_byte(buf, st)
            b2 = _read_byte(buf, st)
            b3 = _read_byte(buf, st)
            b4 = _read_byte(buf, st)
            values[i] = _to_int32(b1 | (b2 << 8) | (b3 << 16) | (b4 << 24))
        lead >>= 2


@njit(cache=True)
def _read_tag2_3svariable(buf, st, values):
    lead = _read_byte(buf, st)
    sel = (lead >> 6) & 3
    if sel == 0:
        values[0] = _sign_extend((lead >> 4) & 0x03, 2)
        values[1] = _sign_extend((lead >> 2) & 0x03, 2)
        values[2] = _sign_extend(lead & 0x03, 2)
    elif sel == 1:
        values[0] = _sign_extend((lead & 0x3E) >> 1, 5)
        lead2 = _read_byte(buf, st)
        values[1] = _sign_extend(((lead & 0x01) << 4) | ((lead2 & 0xF0) >> 4), 5)
        values[2] = _sign_extend(lead2 & 0x0F, 4)
    elif sel == 2:
        lead2 = _read_byte(buf, st)
        values[0] = _sign_extend(((lead & 0x3F) << 2) | ((lead2 & 0xC0) >> 6), 8)
        lead3 = _read_byte(buf, st)
        values[1] = _sign_extend(((lead2 & 0x3F) << 1) | ((lead3 & 0x80) >> 7), 7)
        values[2] = _sign_extend(lead3 & 0x7F, 7)
    else:
        _read_tag2_wide(buf, st, values, lead)


@njit(cache=True)
def _read_tag8_4s16_v1(buf, st, values):
    selector = _read_byte(buf, st)
    i = 0
    while i < 4:
        s = selector & 0x03
        if s == 0:
            values[i] = 0
        elif s == 1:
            c = _read_byte(buf, st)
            values[i] = _sign_extend(c & 0x0F, 4)
            i += 1
            selector >>= 2
            if i < 4:
                values[i] = _sign_extend(c >> 4, 4)
        elif s == 2:
            values[i] = _sign_extend(_read_byte(buf, st), 8)
        else:
            c1 = _read_byte(buf, st)
            c2 = _read_byte(buf, st)
            values[i] = _sign_extend(c1 | (c2 << 8), 16)
        selector >>= 2
        i += 1


@njit(cache=True)
def _read_tag8_4s16_v2(buf, st, values):
    selector = _read_byte(buf, st)
    nibble = 0
    buffer = np.int64(0)
    for i in range(4):
        s = selector & 0x03
        if s == 0:
            values[i] = 0
        elif s == 1:
            if nibble == 0:
                buffer = _read_byte(buf, st)
                values[i] = _sign_extend(buffer >> 4, 4)
                nibble = 1
            else:
                values[i] = _sign_extend(buffer & 0x0F, 4)
                nibble = 0
        elif s == 2:
            if nibble == 0:
                values[i] = _sign_extend(_read_byte(buf, st), 8)
            else:
                c1 = (buffer & 0x0F) << 4
                buffer = _read_byte(buf, st)
                c1 |= buffer >> 4
                values[i] = _sign_extend(c1, 8)
        else:
            if nibble == 0:
                c1 = _read_byte(buf, st)
                c2 = _read_byte(buf, st)
                values[i] = _sign_extend((c1 << 8) | c2, 16)
            else:
                c1 = _read_byte(buf, st)
                c2 = _read_byte(buf, st)
                values[i] = _sign_extend(((buffer & 0x0F) << 12) | (c1 << 4) | (c2 >> 4), 16)
                buffer = c2
        selector >>= 2


@njit(cache=True)
def _read_tag8_8svb(buf, st, values, count):
    if count == 1:
        values[0] = _read_svb(buf, st)
    else:
        header = _read_byte(buf, st)
        for i in range(count):
            if header & 0x01:
                values[i] = _read_svb(buf, st)
            else:
                values[i] = 0
            header >>= 1


# ---------------------------------------------------------------- prédiction


@njit(cache=True)
def _predict(i, p, v, cur, prev, prev2, has_prev, ctx, home):
    if p == 0:
        return v
    if p == PRED_PREVIOUS:
        if has_prev:
            return v + prev[i]
        return v
    if p == PRED_STRAIGHT_LINE:
        if has_prev:
            return v + 2 * prev[i] - prev2[i]
        return v
    if p == PRED_AVERAGE_2:
        if has_prev:
            s = prev[i] + prev2[i]
            # division entière tronquée vers zéro, comme en C
            q = abs(s) // 2
            return v + (q if s >= 0 else -q)
        return v
    if p == PRED_MINTHROTTLE:
        return _to_int32(v) + ctx[CTX_MINTHROTTLE]
    if p == PRED_MINMOTOR:
        return _to_int32(v) + ctx[CTX_MOTOR_MIN]
    if p == PRED_MOTOR_0:
        return v + cur[ctx[CTX_MOTOR0_IDX]]
    if p == PRED_1500:
        return v + 1500
    if p == PRED_VBATREF:
        return v + ctx[CTX_VBATREF]
    if p == PRED_HOME_COORD:
        return v + home[ctx[CTX_HOME0_IDX]]
    if p == PRED_HOME_COORD_1:
        return v + home[ctx[CTX_HOME1_IDX]]
    if p == PRED_LAST_MAIN_FRAME_TIME:
        if ctx[CTX_HAS_LAST_TIME]:
            return v + ctx[CTX_LAST_TIME]
        return v
    return v


@njit(cache=True)
def _parse_frame(buf, st, pred, enc, n, cur, prev, prev2, has_prev, skipped, ctx, home, values):
    i = 0
    while i < n:
        p = pred[i]
        if p == PRED_INC:
            cur[i] = skipped + 1
            if has_prev:
                cur[i] += prev[i]
            i += 1
            continue
        e = enc[i]
        if e == ENC_SIGNED_VB:
            v = _read_svb(buf, st)
        elif e == ENC_UNSIGNED_VB:
            v = _read_uvb(buf, st)
        elif e == ENC_NEG_14BIT:
            v = -_sign_extend(_read_uvb(buf, st), 14)
        elif e == ENC_TAG8_4S16:
            if ctx[CTX_DATA_VERSION] < 2:
                _read_tag8_4s16_v1(buf, st, values)
            else:
                _read_tag8_4s16_v2(buf, st, values)
            for j in range(4):
                if i < n:
                    cur[i] = _predict(i, pred[i], values[j], cur, prev, prev2, has_prev, ctx, home)
                i += 1
            continue
        elif e == ENC_TAG2_3S32:
            _read_tag2_3s32(buf, st, values)
            for j in range(3):
                if i < n:
                    cur[i] = _predict(i, pred[i], values[j], cur, prev, prev2, has_prev, ctx, home)
                i += 1
            continue
        elif e == ENC_TAG2_3SVARIABLE:
            _read_tag2_3svariable(buf, st, values)
            for j in range(3):
                if i < n:
                    cur[i] = _predict(i, pred[i], values[j], cur, prev, prev2, has_prev, ctx, home)
                i += 1
            continue
        elif e == ENC_TAG8_8SVB:
            j = i + 1
            while j < i + 8 and j < n:
                if enc[j] != ENC_TAG8_8SVB:
                    break
                j += 1
            count = j - i
            _read_tag8_8svb(buf, st, values, count)
            for k in range(count):
                cur[i] = _predict(i, pred[i], values[k], cur, prev, prev2, has_prev, ctx, home)
                i += 1
            continue
        elif e == ENC_NULL:
            v = 0
        else:
            # encodage inconnu : on lit rien (la trame sera probablement rejetée)
            v = 0
        cur[i] = _predict(i, p, v, cur, prev, prev2, has_prev, ctx, home)
        i += 1


# ---------------------------------------------------------------- tableaux extensibles


@njit(cache=True)
def _grow2d(a, nrows):
    if nrows < a.shape[0]:
        return a
    b = np.zeros((max(16, a.shape[0] * 2), a.shape[1]), dtype=a.dtype)
    b[: a.shape[0]] = a
    return b


@njit(cache=True)
def _grow1d(a, n):
    if n < a.shape[0]:
        return a
    b = np.zeros(max(16, a.shape[0] * 2), dtype=a.dtype)
    b[: a.shape[0]] = a
    return b


@njit(cache=True)
def _should_have_frame(idx, i_interval, p_num, p_denom):
    return ((idx % i_interval) + p_num - 1) % p_denom < p_num


@njit(cache=True)
def _is_marker(c):
    # I P G H S E
    return c == 73 or c == 80 or c == 71 or c == 72 or c == 83 or c == 69


# ---------------------------------------------------------------- boucle principale


@njit(cache=True)
def decode_stream(
    buf,
    start,
    end,
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
    i_interval,
    p_num,
    p_denom,
    ctx,
    est_rows,
):
    """Décode les trames d'une session.

    Retourne :
      main (n×n_main int64), main_kind (I=0/P=1), slow (m×n_slow), slow_at (indice de la ligne
      principale suivante), gps (k×n_gps), gps_at, events (e×4 : type, ligne, donnée1, donnée2),
      stats (8 : trames corrompues, désynchros, itérations sautées volontairement, fin de log vue,
      octets lus, trames I, trames P, inutilisé).
    """
    st = np.zeros(3, dtype=np.int64)
    st[ST_POS] = start
    st[ST_END] = end

    nm = max(n_main, 1)
    main = np.zeros((max(16, est_rows), nm), dtype=np.int64)
    kind = np.zeros(max(16, est_rows), dtype=np.int8)
    n_rows = 0
    slow = np.zeros((64, max(n_slow, 1)), dtype=np.int64)
    slow_at = np.zeros(64, dtype=np.int64)
    n_slow_rows = 0
    gps = np.zeros((64, max(n_gps, 1)), dtype=np.int64)
    gps_at = np.zeros(64, dtype=np.int64)
    n_gps_rows = 0
    events = np.zeros((64, 4), dtype=np.int64)
    n_events = 0
    stats = np.zeros(8, dtype=np.int64)

    cur = np.zeros(nm, dtype=np.int64)
    prev = np.zeros(nm, dtype=np.int64)
    prev2 = np.zeros(nm, dtype=np.int64)
    has_prev = False
    last_slow = np.zeros(max(n_slow, 1), dtype=np.int64)
    last_gps = np.zeros(max(n_gps, 1), dtype=np.int64)
    home0 = np.zeros(max(n_home, 1), dtype=np.int64)
    home1 = np.zeros(max(n_home, 1), dtype=np.int64)
    home_valid = False
    dummy = np.zeros(max(n_home, 1), dtype=np.int64)
    values = np.zeros(8, dtype=np.int64)

    stream_valid = False
    last_iter = np.int64(-1)
    last_time = np.int64(-1)
    last_skipped = np.int64(0)
    last_type = -1
    frame_start = start
    premature_eof = False

    # événement en cours
    ev_ok = False
    ev_type = 0
    ev_d1 = np.int64(0)
    ev_d2 = np.int64(0)

    while True:
        cmd = _read_byte(buf, st)

        if last_type != -1:
            size = st[ST_POS] - frame_start
            looks = _is_marker(cmd) or ((not premature_eof) and cmd == -1)
            if size <= MAX_FRAME_LENGTH and looks:
                accepted = True
                if last_type == 73:  # I
                    accept = True
                    if last_iter != -1:
                        it = cur[FIELD_ITERATION]
                        tm = cur[FIELD_TIME]
                        accept = (
                            it >= last_iter
                            and it < last_iter + MAX_ITERATION_JUMP
                            and tm >= last_time
                            and tm < last_time + MAX_TIME_JUMP
                        )
                    if accept:
                        if last_iter != -1:
                            # itérations volontairement absentes jusqu'à cette trame
                            k = last_iter + 1
                            while k < cur[FIELD_ITERATION]:
                                if not _should_have_frame(k, i_interval, p_num, p_denom):
                                    stats[2] += 1
                                k += 1
                        last_iter = cur[FIELD_ITERATION]
                        last_time = cur[FIELD_TIME]
                        stream_valid = True
                    else:
                        stream_valid = False
                    if stream_valid:
                        main = _grow2d(main, n_rows)
                        kind = _grow1d(kind, n_rows)
                        main[n_rows, :] = cur
                        kind[n_rows] = 0
                        n_rows += 1
                        stats[5] += 1
                    else:
                        accepted = False
                    prev[:] = cur
                    prev2[:] = cur
                    has_prev = True
                elif last_type == 80:  # P
                    if stream_valid and (
                        cur[FIELD_TIME] > last_time + MAX_TIME_JUMP
                        or cur[FIELD_ITERATION] > last_iter + MAX_ITERATION_JUMP
                    ):
                        stream_valid = False
                    if stream_valid:
                        last_iter = cur[FIELD_ITERATION]
                        last_time = cur[FIELD_TIME]
                        stats[2] += last_skipped
                        main = _grow2d(main, n_rows)
                        kind = _grow1d(kind, n_rows)
                        main[n_rows, :] = cur
                        kind[n_rows] = 1
                        n_rows += 1
                        stats[6] += 1
                        prev2[:] = prev
                        prev[:] = cur
                    else:
                        accepted = False
                elif last_type == 83:  # S
                    slow = _grow2d(slow, n_slow_rows)
                    slow_at = _grow1d(slow_at, n_slow_rows)
                    slow[n_slow_rows, :] = last_slow
                    slow_at[n_slow_rows] = n_rows
                    n_slow_rows += 1
                elif last_type == 72:  # H
                    home1[:] = home0
                    home_valid = True
                elif last_type == 71:  # G
                    if home_valid:
                        gps = _grow2d(gps, n_gps_rows)
                        gps_at = _grow1d(gps_at, n_gps_rows)
                        gps[n_gps_rows, :] = last_gps
                        gps_at[n_gps_rows] = n_rows
                        n_gps_rows += 1
                    else:
                        accepted = False
                elif last_type == 69:  # E
                    if ev_ok:
                        if ev_type == EVENT_LOGGING_RESUME:
                            last_iter = ev_d1
                            last_time = ev_d2
                        events = _grow2d(events, n_events)
                        events[n_events, 0] = ev_type
                        events[n_events, 1] = n_rows
                        events[n_events, 2] = ev_d1
                        events[n_events, 3] = ev_d2
                        n_events += 1
                        if ev_type == EVENT_LOG_END:
                            stats[3] = 1
                    else:
                        accepted = False
                if not accepted:
                    stats[1] += 1
            else:
                # trame précédente corrompue : on se resynchronise un octet plus loin
                stream_valid = False
                stats[0] += 1
                st[ST_POS] = frame_start + 1
                st[ST_EOF] = 0
                last_type = -1
                premature_eof = False
                continue

        if cmd == -1:
            break

        frame_start = st[ST_POS] - 1
        defined = (
            cmd == 69
            or cmd == 73
            or (cmd == 80 and has_p)
            or (cmd == 83 and has_s)
            or (cmd == 71 and has_g)
            or (cmd == 72 and has_h)
        )
        if _is_marker(cmd) and defined:
            last_type = cmd
            if cmd == 73:
                ctx[CTX_HAS_LAST_TIME] = 1 if has_prev else 0
                ctx[CTX_LAST_TIME] = prev[FIELD_TIME]
                _parse_frame(buf, st, i_pred, i_enc, n_main, cur, prev, prev2, has_prev, 0, ctx, dummy, values)
            elif cmd == 80:
                last_skipped = 0
                if last_iter != -1:
                    k = last_iter + 1
                    while not _should_have_frame(k, i_interval, p_num, p_denom):
                        last_skipped += 1
                        k += 1
                ctx[CTX_HAS_LAST_TIME] = 1 if has_prev else 0
                ctx[CTX_LAST_TIME] = prev[FIELD_TIME]
                _parse_frame(
                    buf, st, p_pred, p_enc, n_main, cur, prev, prev2, has_prev, last_skipped, ctx, dummy, values
                )
            elif cmd == 83:
                _parse_frame(buf, st, s_pred, s_enc, n_slow, last_slow, last_slow, last_slow, False, 0, ctx, dummy, values)
            elif cmd == 72:
                _parse_frame(buf, st, h_pred, h_enc, n_home, home0, home0, home0, False, 0, ctx, dummy, values)
            elif cmd == 71:
                ctx[CTX_HAS_LAST_TIME] = 1 if has_prev else 0
                ctx[CTX_LAST_TIME] = prev[FIELD_TIME]
                _parse_frame(buf, st, g_pred, g_enc, n_gps, last_gps, last_gps, last_gps, False, 0, ctx, home1, values)
            else:  # E
                ev_ok, ev_type, ev_d1, ev_d2 = _parse_event(buf, st)
            if st[ST_EOF]:
                premature_eof = True
        else:
            stream_valid = False
            last_type = -1

    stats[4] = st[ST_POS] - start
    return (
        main[:n_rows],
        kind[:n_rows],
        slow[:n_slow_rows],
        slow_at[:n_slow_rows],
        gps[:n_gps_rows],
        gps_at[:n_gps_rows],
        events[:n_events],
        stats,
    )


@njit(cache=True)
def _parse_event(buf, st):
    t = _read_byte(buf, st)
    d1 = np.int64(0)
    d2 = np.int64(0)
    if t == EVENT_SYNC_BEEP:
        d1 = _read_uvb(buf, st)
    elif t == EVENT_FLIGHT_MODE:
        d1 = _read_uvb(buf, st)
        d2 = _read_uvb(buf, st)
    elif t == EVENT_DISARM:
        d1 = _read_uvb(buf, st)
    elif t == EVENT_AUTOTUNE_CYCLE_START:
        for _ in range(5):
            _read_byte(buf, st)
    elif t == EVENT_AUTOTUNE_CYCLE_RESULT:
        for _ in range(4):
            _read_byte(buf, st)
    elif t == EVENT_AUTOTUNE_TARGETS:
        for _ in range(8):
            _read_byte(buf, st)
    elif t == EVENT_GTUNE_CYCLE_RESULT:
        _read_byte(buf, st)
        d1 = _read_svb(buf, st)
        _read_byte(buf, st)
        _read_byte(buf, st)
    elif t == EVENT_INFLIGHT_ADJUSTMENT:
        f = _read_byte(buf, st)
        d1 = f & 127
        if f < 128:
            d2 = _read_svb(buf, st)
        else:
            b1 = _read_byte(buf, st)
            b2 = _read_byte(buf, st)
            b3 = _read_byte(buf, st)
            b4 = _read_byte(buf, st)
            d2 = b1 | (b2 << 8) | (b3 << 16) | (b4 << 24)
    elif t == EVENT_TWITCH_TEST:
        d1 = _read_byte(buf, st)
        b1 = _read_byte(buf, st)
        b2 = _read_byte(buf, st)
        b3 = _read_byte(buf, st)
        b4 = _read_byte(buf, st)
        d2 = b1 | (b2 << 8) | (b3 << 16) | (b4 << 24)
    elif t == EVENT_LOGGING_RESUME:
        d1 = _read_uvb(buf, st)
        d2 = _read_uvb(buf, st)
    elif t == EVENT_LOG_END:
        # « End of log\0 » : sinon ce n'était pas un vrai événement de fin
        msg = (69, 110, 100, 32, 111, 102, 32, 108, 111, 103, 0)
        ok = True
        for c in msg:
            if _read_byte(buf, st) != c:
                ok = False
        if ok:
            st[ST_END] = st[ST_POS]
            return True, t, d1, d2
        return False, t, d1, d2
    else:
        return False, t, d1, d2
    return True, t, d1, d2
