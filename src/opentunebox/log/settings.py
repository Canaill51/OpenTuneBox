"""Réglages Betaflight lus dans l'en-tête, normalisés quelle que soit la version.

Différences gérées (voir docs/spec-betaflight.md §4) :
- 4.4/4.5 : rollPID D = D maximum, `d_min` = D de base ;
  2025.12+ : rollPID D = D de base, `d_max` = D maximum.
- ralenti : `dshot_idle_value` (4.x) / `motor_idle` (2025.12+).
- curseur D max : `simplified_dmax_gain` (4.x) / `simplified_d_max_gain` (2025.12+).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from .debug_tables import debug_mode_name, family

FILTER_TYPES = {0: "PT1", 1: "BIQUAD", 2: "PT2", 3: "PT3"}
RATES_TYPES = {0: "BETAFLIGHT", 1: "RACEFLIGHT", 2: "KISS", 3: "ACTUAL", 4: "QUICK"}
# motor_pwm_protocol : 5 et plus = DShot / ProShot
PWM_DSHOT_MIN = 5


def _ints(v: str | None) -> list[int]:
    if not v:
        return []
    out = []
    for p in v.split(","):
        p = p.strip()
        try:
            out.append(int(p, 16) if p.lower().startswith("0x") else int(float(p)))
        except ValueError:
            pass
    return out


def _int(v: str | None, default: int = 0) -> int:
    vals = _ints(v)
    return vals[0] if vals else default


@dataclass
class AxisPID:
    P: int = 0
    I: int = 0
    D: int = 0  # D de base (niveau bas)
    Dmax: int = 0  # D maximum (boost D max / D min)
    F: int = 0

    def text(self) -> str:
        return f"{self.P},{self.I},{self.D},{self.Dmax},{self.F}"


@dataclass
class LowpassSetting:
    type: str = "PT1"
    static_hz: int = 0
    dyn_min_hz: int = 0
    dyn_max_hz: int = 0
    expo: int = 5

    @property
    def enabled(self) -> bool:
        return self.static_hz > 0 or self.dyn_min_hz > 0

    @property
    def dynamic(self) -> bool:
        return self.dyn_min_hz > 0


@dataclass
class Settings:
    raw: dict[str, str]
    firmware: str = ""
    firmware_name: str = "Betaflight"
    version: tuple[int, ...] = ()
    board: str = ""
    craft_name: str = ""
    looptime_us: int = 125
    pid_denom: int = 1
    p_interval: tuple[int, int] = (1, 1)
    debug_mode: str = "NONE"
    debug_index: int = 0
    pids: list[AxisPID] = field(default_factory=lambda: [AxisPID(), AxisPID(), AxisPID()])
    gyro_lpf1: LowpassSetting = field(default_factory=LowpassSetting)
    gyro_lpf2: LowpassSetting = field(default_factory=LowpassSetting)
    gyro_notches: list[tuple[int, int]] = field(default_factory=list)  # (centre, coupure)
    dterm_lpf1: LowpassSetting = field(default_factory=LowpassSetting)
    dterm_lpf2: LowpassSetting = field(default_factory=LowpassSetting)
    dterm_notch: tuple[int, int] = (0, 0)
    yaw_lowpass_hz: int = 0
    dyn_notch_count: int = 0
    dyn_notch_q: int = 0
    dyn_notch_min_hz: int = 0
    dyn_notch_max_hz: int = 0
    rpm_harmonics: int = 0
    rpm_weights: list[int] = field(default_factory=list)
    rpm_q: int = 500
    rpm_min_hz: int = 100
    rpm_fade_hz: int = 50
    rpm_lpf_hz: int = 150
    motor_poles: int = 14
    dshot_bidir: bool = False
    motor_protocol: int = 0
    motor_output: tuple[int, int] = (48, 2047)
    minthrottle: int = 1000
    maxthrottle: int = 2000
    high_resolution: bool = False
    acc_1g: int = 2048
    motor_kv: int = 0

    # ------------------------------------------------------------ dérivés
    @property
    def gyro_rate_hz(self) -> float:
        return 1e6 / max(1, self.looptime_us)

    @property
    def pid_rate_hz(self) -> float:
        return self.gyro_rate_hz / max(1, self.pid_denom)

    @property
    def log_rate_hz(self) -> float:
        num, den = self.p_interval
        return self.pid_rate_hz * num / max(1, den)

    @property
    def family(self) -> str:
        return family(self.version)

    @property
    def is_dshot(self) -> bool:
        return self.motor_protocol >= PWM_DSHOT_MIN

    def get(self, key: str, default: str | None = None) -> str | None:
        return self.raw.get(key, default)

    def get_int(self, key: str, default: int = 0) -> int:
        return _int(self.raw.get(key), default)

    def get_ints(self, key: str) -> list[int]:
        return _ints(self.raw.get(key))

    def sliders(self) -> dict[str, int]:
        """Positions des curseurs du réglage simplifié (en %, 100 = défaut)."""
        keys = {
            "mode": "simplified_pids_mode",
            "master": "simplified_master_multiplier",
            "i_gain": "simplified_i_gain",
            "d_gain": "simplified_d_gain",
            "pi_gain": "simplified_pi_gain",
            "ff_gain": "simplified_feedforward_gain",
            "pitch_d": "simplified_pitch_d_gain",
            "pitch_pi": "simplified_pitch_pi_gain",
            "dmax_gain": "simplified_d_max_gain" if "simplified_d_max_gain" in self.raw else "simplified_dmax_gain",
            "dterm_filter": "simplified_dterm_filter",
            "dterm_mult": "simplified_dterm_filter_multiplier",
            "gyro_filter": "simplified_gyro_filter",
            "gyro_mult": "simplified_gyro_filter_multiplier",
        }
        return {k: self.get_int(v, -1) for k, v in keys.items() if v in self.raw}


_FW_RE = re.compile(r"^(\w+)\s+(\d+)\.(\d+)\.(\d+)")


def parse_settings(raw: dict[str, str]) -> Settings:
    s = Settings(raw=raw)
    fw = raw.get("Firmware revision", "").strip()
    s.firmware = fw
    m = _FW_RE.match(fw)
    if m:
        s.firmware_name = m.group(1)
        s.version = (int(m.group(2)), int(m.group(3)), int(m.group(4)))
        s.board = fw.split()[-1] if ")" in fw else ""
    s.craft_name = raw.get("Craft name", "").strip()
    s.looptime_us = max(1, _int(raw.get("looptime"), 125))
    s.pid_denom = max(1, _int(raw.get("pid_process_denom"), 1))
    pv = raw.get("P interval", "1")
    if "/" in pv:
        a, b = pv.split("/", 1)
        s.p_interval = (max(1, _int(a, 1)), max(1, _int(b, 1)))
    else:
        s.p_interval = (1, max(1, _int(pv, 1)))
    s.debug_index = _int(raw.get("debug_mode"), 0)
    s.debug_mode = debug_mode_name(s.debug_index, s.version, raw.get("debug_mode_name"))

    # PID
    new_dmax = family(s.version) in ("2025.12", "2026.6")
    ff = _ints(raw.get("ff_weight"))
    dmin = _ints(raw.get("d_min"))
    dmax = _ints(raw.get("d_max"))
    for ax, key in enumerate(("rollPID", "pitchPID", "yawPID")):
        v = _ints(raw.get(key)) + [0, 0, 0]
        p = AxisPID(P=v[0], I=v[1], D=v[2], Dmax=v[2], F=ff[ax] if ax < len(ff) else 0)
        if new_dmax:
            if ax < len(dmax) and dmax[ax] > p.D:
                p.Dmax = dmax[ax]
        else:
            if ax < len(dmin) and 0 < dmin[ax] < p.D:
                p.D = dmin[ax]
        s.pids[ax] = p

    # Filtres gyro et D
    s.gyro_lpf1 = _lowpass(raw, "gyro_lpf1")
    s.gyro_lpf2 = _lowpass(raw, "gyro_lpf2")
    hz = _ints(raw.get("gyro_notch_hz"))
    cut = _ints(raw.get("gyro_notch_cutoff"))
    s.gyro_notches = [(h, cut[i] if i < len(cut) else 0) for i, h in enumerate(hz) if h > 0]
    s.dterm_lpf1 = _lowpass(raw, "dterm_lpf1")
    s.dterm_lpf2 = _lowpass(raw, "dterm_lpf2")
    s.dterm_notch = (_int(raw.get("dterm_notch_hz")), _int(raw.get("dterm_notch_cutoff")))
    s.yaw_lowpass_hz = _int(raw.get("yaw_lowpass_hz"))
    s.dyn_notch_count = _int(raw.get("dyn_notch_count"))
    s.dyn_notch_q = _int(raw.get("dyn_notch_q"))
    s.dyn_notch_min_hz = _int(raw.get("dyn_notch_min_hz"))
    s.dyn_notch_max_hz = _int(raw.get("dyn_notch_max_hz"))
    s.rpm_harmonics = _int(raw.get("rpm_filter_harmonics"))
    s.rpm_weights = _ints(raw.get("rpm_filter_weights")) or [100] * max(1, s.rpm_harmonics)
    s.rpm_q = _int(raw.get("rpm_filter_q"), 500)
    s.rpm_min_hz = _int(raw.get("rpm_filter_min_hz"), 100)
    s.rpm_fade_hz = _int(raw.get("rpm_filter_fade_range_hz"), 50)
    s.rpm_lpf_hz = _int(raw.get("rpm_filter_lpf_hz"), 150)

    # Moteurs
    s.motor_poles = max(2, _int(raw.get("motor_poles"), 14))
    s.dshot_bidir = _int(raw.get("dshot_bidir")) == 1
    s.motor_protocol = _int(raw.get("motor_pwm_protocol"), 6)
    mo = _ints(raw.get("motorOutput"))
    if len(mo) >= 2:
        s.motor_output = (mo[0], mo[1])
    s.minthrottle = _int(raw.get("minthrottle"), 1000)
    s.maxthrottle = _int(raw.get("maxthrottle"), 2000)
    s.high_resolution = _int(raw.get("blackbox_high_resolution")) == 1
    s.acc_1g = max(1, _int(raw.get("acc_1G"), 2048))
    s.motor_kv = _int(raw.get("motor_kv"))
    return s


def _lowpass(raw: dict[str, str], prefix: str) -> LowpassSetting:
    dyn = _ints(raw.get(f"{prefix}_dyn_hz"))
    return LowpassSetting(
        type=FILTER_TYPES.get(_int(raw.get(f"{prefix}_type")), "PT1"),
        static_hz=_int(raw.get(f"{prefix}_static_hz")),
        dyn_min_hz=dyn[0] if dyn else 0,
        dyn_max_hz=dyn[1] if len(dyn) > 1 else 0,
        expo=_int(raw.get(f"{prefix}_dyn_expo"), 5),
    )
