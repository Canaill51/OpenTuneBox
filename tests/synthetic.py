"""Logs synthétiques pour les tests sans corpus : une session décodée (`make_session`) et un vol complet en
unités physiques (`make_flightlog`), assez réalistes pour exercer les analyses, l'assistant et les pages de
l'interface (consigne en paliers, gyro retardé et bruité, raie moteur, télémétrie RPM, termes PID)."""

from pathlib import Path

import numpy as np
from scipy.signal import lfilter

from opentunebox.decoder.header import LogHeader
from opentunebox.decoder.reader import DecodedSession
from opentunebox.log.flightlog import FlightLog
from opentunebox.log.settings import parse_settings


def make_session(columns: dict[str, list], header: dict[str, str] | None = None,
                 slow: dict[str, list] | None = None) -> DecodedSession:
    """Session décodée fabriquée à la main : `columns` = colonnes principales (loopIteration et time ajoutés
    s'ils manquent), `header` = lignes H brutes, `slow` = colonnes lentes (flightModeFlags…)."""
    cols = dict(columns)
    n = len(next(iter(cols.values())))
    cols.setdefault("loopIteration", list(range(n)))
    cols.setdefault("time", [1_000_000 + 500 * i for i in range(n)])  # 2 kHz
    names = ["loopIteration", "time"] + [k for k in cols if k not in ("loopIteration", "time")]
    main = np.array([cols[k] for k in names], dtype=np.int64).T
    slow = slow or {}
    slow_arr = np.array([slow[k] for k in slow], dtype=np.int64).T if slow else np.zeros((n, 0), dtype=np.int64)
    return DecodedSession(
        index=1, header=LogHeader(dict(header or {}), {}, 0, 0), main_names=names, main=main,
        main_kind=np.zeros(n, dtype=np.int64), slow_names=list(slow), slow=slow_arr,
        gps_names=[], gps=np.zeros((0, 0)), gps_at=np.zeros(0, dtype=np.int64), events=np.zeros((0, 4), dtype=np.int64),
    )


# En-tête d'un 5" en Betaflight 4.5 (gyro 8 kHz, PID 8 kHz, filtres par défaut du configurateur)
BASE_HEADER = {
    "Firmware revision": "Betaflight 4.5.1 (abcd1234) STM32F7X2",
    "Craft name": "Synth 5",
    "looptime": "125", "pid_process_denom": "1", "motor_poles": "14",
    "rollPID": "45,80,40", "pitchPID": "47,84,46", "yawPID": "45,80,0", "d_min": "30,34,0",
    "ff_weight": "120,125,120",
    "gyro_lpf1_type": "0", "gyro_lpf1_static_hz": "0", "gyro_lpf1_dyn_hz": "250,500", "gyro_lpf1_dyn_expo": "5",
    "gyro_lpf2_type": "0", "gyro_lpf2_static_hz": "500",
    "gyro_notch_hz": "0,0", "gyro_notch_cutoff": "0,0",
    "dterm_lpf1_type": "0", "dterm_lpf1_static_hz": "75", "dterm_lpf1_dyn_hz": "75,150", "dterm_lpf1_dyn_expo": "5",
    "dterm_lpf2_type": "0", "dterm_lpf2_static_hz": "150",
    "dterm_notch_hz": "0", "dterm_notch_cutoff": "0",
    "dyn_notch_count": "3", "dyn_notch_q": "300", "dyn_notch_min_hz": "100", "dyn_notch_max_hz": "600",
    "rpm_filter_harmonics": "3", "rpm_filter_weights": "100,100,100", "rpm_filter_q": "500",
    "rpm_filter_min_hz": "100", "rpm_filter_fade_range_hz": "50",
    "simplified_pids_mode": "2", "simplified_master_multiplier": "100", "simplified_i_gain": "100",
    "simplified_d_gain": "100", "simplified_pi_gain": "100", "simplified_dmax_gain": "100",
    "simplified_feedforward_gain": "100", "simplified_pitch_d_gain": "100", "simplified_pitch_pi_gain": "100",
    "simplified_dterm_filter": "1", "simplified_dterm_filter_multiplier": "100",
    "simplified_gyro_filter": "1", "simplified_gyro_filter_multiplier": "100",
}


def make_flightlog(seconds: float = 12.0, fs: float = 2000.0, name: str = "synth", moving: bool = True,
                   unfilt: bool = True, rpm: bool = True, motor_hz: float = 220.0, mode_flags: int = 0b1,
                   seed: int = 0, **header) -> FlightLog:
    """Vol synthétique d'un 5" : paliers de consigne ±100–400 °/s sur roll et pitch (`moving`), gyro = consigne
    passée dans un PT1 à 30 Hz, retardée de 8 ms, plus bruit blanc et raie moteur ; gaz 35–65 %, quatre
    moteurs à `motor_hz` × (0,7 + 0,6 × gaz) si `rpm`. `header` surcharge `BASE_HEADER` (valeur None = clé
    retirée, p. ex. `d_min=None, d_max="40,42,0"` pour la convention 2025.12)."""
    rng = np.random.default_rng(seed)
    n = int(seconds * fs)
    t = np.arange(n) / fs
    sp = np.zeros((3, n))
    if moving:
        for ax in (0, 1):
            k = int(0.5 * fs)
            while k < n:
                hold, gap = int(rng.uniform(0.2, 0.5) * fs), int(rng.uniform(0.3, 0.8) * fs)
                sp[ax, k:k + hold] = rng.choice([-1.0, 1.0]) * rng.uniform(100, 400)
                k += hold + gap
    w = 2 * np.pi * 30.0 / fs
    k1 = w / (w + 1.0)
    delay = int(round(0.008 * fs))
    gyro = np.zeros_like(sp)
    for ax in range(3):
        y = lfilter([k1], [1.0, -(1.0 - k1)], sp[ax])
        gyro[ax, delay:] = y[: n - delay]
    throttle = 50.0 + 15.0 * np.sin(2 * np.pi * 0.1 * t)
    mhz_mean = motor_hz * (0.7 + 0.6 * throttle / 100.0)
    line = 3.0 * np.sin(2 * np.pi * np.cumsum(mhz_mean) / fs)  # raie qui suit le régime moteur
    gyro = gyro + rng.normal(0, 2.0, (3, n)) + line
    gyro_unfilt = gyro + rng.normal(0, 4.0, (3, n)) + 6.0 * np.sin(4 * np.pi * np.cumsum(mhz_mean) / fs) if unfilt else None
    motors = np.clip(np.vstack([throttle + rng.normal(0, 1.0, n) for _ in range(4)]), 0, 100)
    motor_hz_arr = np.vstack([mhz_mean + rng.normal(0, 1.0, n) for _ in range(4)]) if rpm else None
    err = sp - gyro
    p_term = 0.032029 * 45 * err
    i_term = np.cumsum(0.244381 * 80 * err / fs, axis=1)
    i_term -= i_term.mean(axis=1, keepdims=True)
    d_term = -0.000529 * 40 * np.gradient(gyro, axis=1) * fs
    d_term[2] = 0.0
    f_term = 0.013754 * 1.2 * np.gradient(sp, axis=1) * fs
    rc = np.vstack([sp / 2.0, 1000.0 + 10.0 * throttle])
    raw = {**BASE_HEADER, "Craft name": header.pop("craft_name", BASE_HEADER["Craft name"])}
    raw.update({k: v for k, v in header.items()})
    raw = {k: v for k, v in raw.items() if v is not None}
    return FlightLog(
        Path(f"{name}.bbl"), 1, parse_settings(raw), t, fs, gyro, gyro_unfilt, sp, rc, p_term, i_term, d_term,
        f_term, throttle, motors, motor_hz_arr, np.zeros((0, n)), None, None, None, np.full(n, mode_flags),
        np.zeros((0, 4)), {}, frozenset(f"axis{k}[{a}]" for k in "PIDF" for a in range(3)),
    )
