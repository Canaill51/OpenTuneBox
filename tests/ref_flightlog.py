"""Code d'origine de `log/flightlog.py:_drop_implausible` et `from_session` (numpy), gardé TEL QUEL comme étalon des
tests de parité avec leur port Rust (`rust/otb-log`, `_native.drop_implausible` / `_native.rows_f32`) : D-031, phase 3a.
Ne pas le modifier. `from_session_ref` et `drop_implausible_ref` sont les anciennes fonctions, seulement renommées."""

from pathlib import Path

import numpy as np

from opentunebox.decoder.reader import DecodedSession
from opentunebox.log.flightlog import SAMPLE_DTYPE, FlightLog
from opentunebox.log.settings import parse_settings


def drop_implausible_ref(sess: DecodedSession, hr: float) -> int:
    """Écarte les trames aux valeurs impossibles (données abîmées acceptées par le format).

    Les champs signés sont d'abord ramenés aux entiers 32 bits comme dans le firmware. Une trame est
    rejetée si le gyro ou la consigne dépasse 4000 °/s, ou un terme PID ±65535 : ce ne peut être
    qu'une trame corrompue, qui fausserait spectres et réponses indicielles.
    """
    main = sess.main
    if main.shape[0] == 0:
        return 0
    names = sess.main_names
    bad = np.zeros(main.shape[0], dtype=bool)
    for i, n in enumerate(names):
        if n in ("loopIteration", "time"):
            continue
        c = main[:, i]
        # colonne par colonne et seulement si besoin : ramener tout le tableau d'un coup en ferait quatre copies
        if c.min() < -2**31 or c.max() >= 2**31:
            c[:] = ((c + 2**31) % 2**32) - 2**31
        if n.startswith(("gyroADC[", "gyroUnfilt[")) or (n.startswith("setpoint[") and n != "setpoint[3]"):
            bad |= np.abs(c) > 4000 * hr
        elif n.startswith(("axisP[", "axisI[", "axisD[", "axisF[")):
            bad |= np.abs(c) > 65535
    if bad.any():
        keep = ~bad
        sess.main = main[keep]
        sess.main_kind = sess.main_kind[keep]
        sess.slow = sess.slow[keep]
    return int(bad.sum())


def from_session_ref(path: Path, sess: DecodedSession) -> FlightLog | None:
    """FlightLog d'une session, ou None si elle ne contient aucune trame exploitable."""
    st = parse_settings(sess.header.raw)
    hr = 10.0 if st.high_resolution else 1.0
    n0 = sess.main.shape[0]
    sess.stats["implausible_frames"] = drop_implausible_ref(sess, hr)
    if sess.main.shape[0] < 2 or sess.stats["implausible_frames"] > n0 / 2:
        return None  # session essentiellement corrompue

    def col(name: str) -> np.ndarray | None:
        c = sess.column(name)
        return None if c is None else c.astype(SAMPLE_DTYPE)

    def axes(prefix: str, scale: float = 1.0, count: int = 3) -> np.ndarray | None:
        cols = [sess.column(f"{prefix}[{i}]") for i in range(count)]
        if all(c is None for c in cols):
            return None
        out = np.zeros((count, sess.main.shape[0]), dtype=SAMPLE_DTYPE)  # rempli ligne par ligne : une seule copie des données
        for i, c in enumerate(cols):
            if c is not None:
                out[i] = c
                if scale != 1.0:
                    out[i] /= scale
        return out

    time_us = sess.main[:, 1].astype(np.float64)
    t = (time_us - time_us[0]) * 1e-6
    dt = np.diff(time_us)
    dt = dt[dt > 0]
    fs = 1e6 / float(np.median(dt)) if dt.size else st.log_rate_hz
    n = t.shape[0]
    zeros3 = np.zeros((3, n), dtype=SAMPLE_DTYPE)

    gyro = axes("gyroADC", hr)
    gyro_unfilt = axes("gyroUnfilt", hr)
    debug = axes("debug", 1.0, 8)
    if debug is None:
        debug = np.zeros((0, n), dtype=SAMPLE_DTYPE)
    else:
        present = [i for i in range(8) if sess.column(f"debug[{i}]") is not None]
        debug = debug[: (max(present) + 1 if present else 0)]
    if gyro_unfilt is None and st.debug_mode == "GYRO_SCALED" and debug.shape[0] >= 3:
        gyro_unfilt = debug[:3] / hr

    setpoint = axes("setpoint", 1.0, 4)
    if setpoint is not None:
        # seuls les axes sont en haute résolution ; setpoint[3] = gaz du mixeur × 1000, jamais × hr (spec §7)
        throttle = setpoint[3] / 10.0
        setpoint = setpoint[:3] / hr
        if sess.column("setpoint[3]") is None:
            throttle = None
    else:
        setpoint = zeros3.copy()
        throttle = None
    rc = axes("rcCommand", 1.0, 4)
    if rc is None:
        rc = np.zeros((4, n), dtype=SAMPLE_DTYPE)
    # les quatre voies rcCommand sont en haute résolution aussi, gaz compris (blackbox.c, spec §7)
    rc /= hr
    if throttle is None:
        throttle = np.clip((rc[3] - 1000.0) / 10.0, 0, 100)

    # moteurs
    motor_cols = []
    i = 0
    while sess.column(f"motor[{i}]") is not None:
        motor_cols.append(col(f"motor[{i}]"))
        i += 1
    if motor_cols:
        raw_m = np.vstack(motor_cols)
        if st.is_dshot or st.motor_output[1] > 2000:
            lo, hi = 48.0, 2047.0
        else:
            lo, hi = float(st.motor_output[0]), float(st.motor_output[1])
        motors = np.clip((raw_m - lo) / max(1.0, hi - lo) * 100.0, -5, 105)
    else:
        motors = np.zeros((0, n), dtype=SAMPLE_DTYPE)

    erpm = []
    for k in range(len(motor_cols)):
        c = col(f"eRPM[{k}]")
        if c is None:
            erpm = []
            break
        erpm.append(c)
    motor_hz = None
    if erpm:
        motor_hz = np.vstack(erpm) * 100.0 / 60.0 / (st.motor_poles / 2.0)
    elif st.debug_mode == "DSHOT_RPM_TELEMETRY" and debug.shape[0] >= len(motor_cols) > 0:
        motor_hz = debug[: len(motor_cols)] * 100.0 / 60.0 / (st.motor_poles / 2.0)

    vbat = col("vbatLatest")
    cur = col("amperageLatest")
    acc = axes("accSmooth", float(st.acc_1g))
    flags = sess.column("flightModeFlags")

    yaw_d = col("axisD[2]")
    d_term = axes("axisD") if sess.column("axisD[0]") is not None else zeros3.copy()
    if yaw_d is None and d_term is not None:
        d_term[2] = 0.0

    return FlightLog(
        path=path,
        session=sess.index,
        settings=st,
        t=t,
        fs=fs,
        gyro=gyro if gyro is not None else zeros3.copy(),
        gyro_unfilt=gyro_unfilt,
        setpoint=setpoint,
        rc_command=rc,
        p_term=axes("axisP") if sess.column("axisP[0]") is not None else zeros3.copy(),
        i_term=axes("axisI") if sess.column("axisI[0]") is not None else zeros3.copy(),
        d_term=d_term,
        f_term=axes("axisF") if sess.column("axisF[0]") is not None else zeros3.copy(),
        throttle=throttle,
        motors=motors,
        motor_hz=motor_hz,
        debug=debug,
        vbat=vbat / 100.0 if vbat is not None else None,
        current=cur / 100.0 if cur is not None else None,
        acc=acc,
        mode_flags=flags.astype(np.int32) if flags is not None else None,  # masque de boîtes, 32 bits dans le firmware
        events=sess.events,
        stats=sess.stats,
        pid_columns=frozenset(n for n in sess.main_names if n.startswith(("axisP[", "axisI[", "axisD[", "axisF["))),
    )
