"""FlightLog : une session de vol en unités physiques, prête pour les analyses.

Conventions :
- temps `t` en secondes depuis la première trame ; `fs` = cadence d'enregistrement (Hz) ;
- tableaux par axe de forme (3, n) : 0 = roll, 1 = pitch, 2 = yaw ;
- gyro, consigne en °/s ; termes PID en unités « somme PID » (1000 = autorité totale du mixeur) ;
- gaz et moteurs en % ; fréquences moteur en Hz (depuis l'eRPM).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from functools import cached_property
from pathlib import Path

import numpy as np

from ..analysis.loop import PidTerms
from ..decoder.reader import DecodedSession, decode_file
from .settings import Settings, parse_settings

AXES = ("Roll", "Pitch", "Yaw")
ANGLE_HORIZON_MASK = 0b110  # boîtes ANGLE (bit 1) et HORIZON (bit 2), mêmes rangs dans toutes les versions
ANGLE_MODE_MASK = 0b010  # ANGLE seul : le feedforward de la boucle de vitesse y est à 0 (pas en HORIZON)
MAX_LEVEL_SHARE = 0.2  # au-delà, consigne calculée par l'autolevel : instrument = commande radio


@dataclass
class FlightLog:
    path: Path
    session: int
    settings: Settings
    t: np.ndarray
    fs: float
    gyro: np.ndarray
    gyro_unfilt: np.ndarray | None
    setpoint: np.ndarray
    rc_command: np.ndarray
    p_term: np.ndarray
    i_term: np.ndarray
    d_term: np.ndarray
    f_term: np.ndarray
    throttle: np.ndarray  # % (0–100)
    motors: np.ndarray  # (nb moteurs, n) en %
    motor_hz: np.ndarray | None  # (nb moteurs, n) fréquence de rotation en Hz, si eRPM
    debug: np.ndarray  # (nb champs debug, n) valeurs brutes
    vbat: np.ndarray | None  # V
    current: np.ndarray | None  # A
    acc: np.ndarray | None  # g
    mode_flags: np.ndarray | None  # flightModeFlags (masque des boîtes actives)
    events: np.ndarray  # (e, 4) : type, ligne, donnée1, donnée2
    stats: dict = field(default_factory=dict)
    pid_columns: frozenset[str] = frozenset()  # champs axisP[0]… présents dans le log

    # ------------------------------------------------------------ propriétés dérivées
    @property
    def n(self) -> int:
        return self.t.shape[0]

    @property
    def duration(self) -> float:
        return float(self.t[-1] - self.t[0]) if self.n > 1 else 0.0

    @property
    def name(self) -> str:
        suffix = f".{self.session:02d}" if self.session > 1 else ""
        return f"{self.path.stem}{suffix}"

    @property
    def n_motors(self) -> int:
        return self.motors.shape[0]

    @cached_property
    def pid_sum(self) -> np.ndarray:
        return self.p_term + self.i_term + self.d_term + self.f_term

    @cached_property
    def pid_error(self) -> np.ndarray:
        """Erreur PID, convention PIDtoolbox : gyro − consigne (°/s)."""
        return self.gyro - self.setpoint

    @cached_property
    def d_term_prefilter(self) -> np.ndarray:
        """Terme D avant ses filtres passe-bas, reconstruit à partir du gyro filtré.

        D = Kd · (−dgyro/dt) : on dérive le gyro filtré, puis on ajuste le gain par moindres carrés
        sur les composantes lentes (là où les filtres du D n'atténuent presque rien). Cela absorbe
        D max, TPA, etc., comme le fait PIDtoolbox (rapport médian), mais de façon plus stable.
        """
        out = np.zeros_like(self.d_term)
        for ax in range(3):
            d = self.d_term[ax]
            if not np.any(d):
                continue
            raw = -np.gradient(self.gyro[ax]) * self.fs
            k = _slow_gain(raw, d, self.fs)
            out[ax] = raw * k
        return out

    @cached_property
    def cells(self) -> int | None:
        """Nombre de cellules de la batterie (formule du firmware), ou None sans tension connue."""
        return cell_count(self.settings, self.vbat)

    @cached_property
    def armed(self) -> np.ndarray:
        """Vrai quand le drone est armé (bit ARM des modes), ou partout si inconnu."""
        if self.mode_flags is None:
            return np.ones(self.n, dtype=bool)
        return (self.mode_flags & 1).astype(bool)

    def term_present(self, term: str, ax: int) -> bool:
        """Le terme « P », « I », « D » ou « F » de l'axe est-il enregistré (sinon il vaut 0 partout) ?"""
        return f"axis{term}[{ax}]" in self.pid_columns

    def pid_terms(self, ax: int, sel: slice = slice(None)) -> PidTerms:
        """Termes PID de l'axe sur `sel`, pour mesurer la boucle ouverte (`analysis/loop.py`).

        `note` explique pourquoi la mesure est impossible (terme de gain non nul absent du log).
        En mode angle ou horizon plus de 20 % du temps, la consigne dépend de l'attitude du drone et
        n'est plus extérieure à la boucle : l'instrument devient la commande radio (manche).
        """
        pid = self.settings.pids[ax] if ax < len(self.settings.pids) else None
        need = {"P": True, "I": True, "D": pid is None or max(pid.D, pid.Dmax) > 0, "F": pid is None or pid.F > 0}
        missing = [t for t, needed in need.items() if needed and not self.term_present(t, ax)]
        note = ""
        if missing:
            s = "s" if len(missing) > 1 else ""
            note = f"terme{s} {', '.join(missing)} non enregistré{s}"
        instrument = None
        if self.mode_flags is not None:
            level = (self.mode_flags[sel] & ANGLE_HORIZON_MASK) != 0
            if level.size and level.mean() > MAX_LEVEL_SHARE:
                instrument = self.rc_command[ax, sel]
                chirp = self.chirp_excitation(ax, sel)
                if chirp is not None:
                    instrument = instrument + chirp
        return PidTerms(self.p_term[ax, sel], self.i_term[ax, sel], self.d_term[ax, sel], self.f_term[ax, sel],
                        note, instrument)

    def chirp_excitation(self, ax: int, sel: slice = slice(None)) -> np.ndarray | None:
        """Excitation du chirp de Betaflight sur l'axe `ax` (°/s, avant son filtre d'avance de phase), ou None
        sans le debug CHIRP. Le firmware l'ajoute à la consigne avant le mode angle (`flight/pid.c`) : en mode
        angle, la commande radio ne la contient pas, il faut l'ajouter à l'instrument. Debug 1 = axe excité
        (−1 : aucun), debug 3 = excitation × 1000 ; amplitude `chirp_amplitude_<axe>` de l'en-tête."""
        if self.debug_name() != "CHIRP" or self.debug.shape[0] < 4 or ax > 2:
            return None
        amp = float(self.settings.raw.get(f"chirp_amplitude_{AXES[ax].lower()}", 0) or 0)
        active = self.debug[1, sel] == ax
        if amp <= 0 or not active.any():
            return None
        return np.where(active, self.debug[3, sel] / 1000.0 * amp, 0.0)

    def debug_name(self) -> str:
        return self.settings.debug_mode

    def segments(self, max_gap_factor: float = 4.0) -> list[slice]:
        """Morceaux contigus (sans trou d'enregistrement) du log."""
        if self.n < 2:
            return [slice(0, self.n)]
        dt = np.diff(self.t)
        gaps = np.where(dt > max_gap_factor / self.fs)[0]
        bounds = [0, *(gaps + 1).tolist(), self.n]
        return [slice(a, b) for a, b in zip(bounds[:-1], bounds[1:]) if b - a > 1]

    def breaks(self, sel: slice = slice(None), max_gap_factor: float = 4.0) -> np.ndarray:
        """Raccords de `sel` : indices (comptés depuis le début de `sel`) des échantillons qui suivent un trou.
        Les analyses n'y posent aucune fenêtre (D-020) : sans cela, chaque raccord ajoute un faux bruit (L-027)."""
        dt = np.diff(self.t[sel])
        return np.flatnonzero(dt > max_gap_factor / self.fs) + 1

    def recording_gaps(self, sel: slice = slice(None), max_gap_factor: float = 4.0) -> tuple[int, float]:
        """(nombre de trous, secondes perdues) dans `sel` : mêmes trous que `segments`."""
        dt = np.diff(self.t[sel])
        gaps = dt[dt > max_gap_factor / self.fs]
        return int(gaps.size), float(np.sum(gaps - 1.0 / self.fs))

    def index_of(self, time_s: float) -> int:
        return int(np.clip(np.searchsorted(self.t, time_s), 0, max(0, self.n - 1)))

    def slice_time(self, t0: float, t1: float) -> slice:
        return slice(self.index_of(t0), self.index_of(t1) + 1)

    def summary(self) -> str:
        s = self.settings
        pid = " | ".join(f"{a[0]} {p.text()}" for a, p in zip(AXES, s.pids))
        return (
            f"{self.name} — {s.firmware}\n"
            f"  durée {self.duration:.1f} s, {self.n} trames, enregistrement {self.fs:.0f} Hz "
            f"(PID {s.pid_rate_hz:.0f} Hz, gyro {s.gyro_rate_hz:.0f} Hz)\n"
            f"  debug {s.debug_mode}, {self.n_motors} moteurs, eRPM {'oui' if self.motor_hz is not None else 'non'},"
            f" gyro non filtré {'oui' if self.gyro_unfilt is not None else 'non'}\n"
            f"  PID (P,I,D,Dmax,F) : {pid}"
        )


def cell_count(settings, vbat):
    """Nombre de cellules par la formule du firmware (sensors/battery.c : tension / vbatmaxcellvoltage + 1),
    avec la tension au branchement (`vbatref`), sinon la première tension loggée."""
    v = settings.get_int("vbatref", 0) / 100.0
    if v <= 0 and vbat is not None and vbat.size:
        v = float(vbat[0])
    if v <= 0.5:
        return None
    cv = settings.get_ints("vbatcellvoltage")  # min, alerte, max (centièmes de volt)
    vmax = cv[2] / 100.0 if len(cv) == 3 and cv[2] > 0 else 4.30  # VBAT_CELL_VOLTAGE_DEFAULT_MAX
    return min(int(v / vmax) + 1, 24)


def _slow_gain(raw: np.ndarray, d: np.ndarray, fs: float) -> float:
    """Gain k minimisant |d − k·raw| sur les composantes < ~30 Hz (moyenne glissante)."""
    w = max(3, int(fs / 60))
    kern = np.ones(w) / w
    a = np.convolve(raw, kern, mode="same")
    b = np.convolve(d, kern, mode="same")
    den = float(np.dot(a, a))
    if den <= 0:
        return 0.0
    return float(np.dot(a, b) / den)


# ---------------------------------------------------------------- construction


def load(path: str | Path) -> list[FlightLog]:
    """Charge toutes les sessions d'un fichier .bbl/.bfl."""
    path = Path(path)
    logs = [from_session(path, s) for s in decode_file(path) if s.main.shape[0] > 1]
    return [lg for lg in logs if lg is not None]


def _drop_implausible(sess: DecodedSession, hr: float) -> int:
    """Écarte les trames aux valeurs impossibles (données abîmées acceptées par le format).

    Les champs signés sont d'abord ramenés aux entiers 32 bits comme dans le firmware. Une trame est
    rejetée si le gyro ou la consigne dépasse 4000 °/s, ou un terme PID ±65535 : ce ne peut être
    qu'une trame corrompue, qui fausserait spectres et réponses indicielles.
    """
    main = sess.main
    if main.shape[0] == 0:
        return 0
    names = sess.main_names
    signed_cols = [i for i, n in enumerate(names) if n not in ("loopIteration", "time")]
    if signed_cols:
        sub = main[:, signed_cols]
        main[:, signed_cols] = ((sub + 2**31) % 2**32) - 2**31
    bad = np.zeros(main.shape[0], dtype=bool)
    for i, n in enumerate(names):
        if n.startswith(("gyroADC[", "gyroUnfilt[")) or (n.startswith("setpoint[") and n != "setpoint[3]"):
            bad |= np.abs(main[:, i]) > 4000 * hr
        elif n.startswith(("axisP[", "axisI[", "axisD[", "axisF[")):
            bad |= np.abs(main[:, i]) > 65535
    if bad.any():
        keep = ~bad
        sess.main = main[keep]
        sess.main_kind = sess.main_kind[keep]
        sess.slow = sess.slow[keep]
    return int(bad.sum())


def from_session(path: Path, sess: DecodedSession) -> FlightLog | None:
    """FlightLog d'une session, ou None si elle ne contient aucune trame exploitable."""
    st = parse_settings(sess.header.raw)
    hr = 10.0 if st.high_resolution else 1.0
    n0 = sess.main.shape[0]
    sess.stats["implausible_frames"] = _drop_implausible(sess, hr)
    if sess.main.shape[0] < 2 or sess.stats["implausible_frames"] > n0 / 2:
        return None  # session essentiellement corrompue

    def col(name: str) -> np.ndarray | None:
        c = sess.column(name)
        return None if c is None else c.astype(np.float64)

    def axes(prefix: str, scale: float = 1.0, count: int = 3) -> np.ndarray | None:
        cols = [col(f"{prefix}[{i}]") for i in range(count)]
        if all(c is None for c in cols):
            return None
        n = sess.main.shape[0]
        return np.vstack([c / scale if c is not None else np.zeros(n) for c in cols])

    time_us = sess.main[:, 1].astype(np.float64)
    t = (time_us - time_us[0]) * 1e-6
    dt = np.diff(time_us)
    dt = dt[dt > 0]
    fs = 1e6 / float(np.median(dt)) if dt.size else st.log_rate_hz
    n = t.shape[0]
    zeros3 = np.zeros((3, n))

    gyro = axes("gyroADC", hr)
    gyro_unfilt = axes("gyroUnfilt", hr)
    debug = axes("debug", 1.0, 8)
    if debug is None:
        debug = np.zeros((0, n))
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
        rc = np.zeros((4, n))
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
        motors = np.zeros((0, n))

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
        mode_flags=flags.astype(np.int64) if flags is not None else None,
        events=sess.events,
        stats=sess.stats,
        pid_columns=frozenset(n for n in sess.main_names if n.startswith(("axisP[", "axisI[", "axisD[", "axisF["))),
    )
