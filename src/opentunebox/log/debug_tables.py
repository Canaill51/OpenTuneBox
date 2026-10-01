"""Noms des modes de débogage selon la version de Betaflight (voir docs/spec-betaflight.md §1)."""

from __future__ import annotations

_BASE_4X = [
    "NONE", "CYCLETIME", "BATTERY", "GYRO_FILTERED", "ACCELEROMETER", "PIDLOOP", "GYRO_SCALED",
    "RC_INTERPOLATION", "ANGLERATE", "ESC_SENSOR", "SCHEDULER", "STACK", "ESC_SENSOR_RPM",
    "ESC_SENSOR_TMP", "ALTITUDE", "FFT", "FFT_TIME", "FFT_FREQ", "RX_FRSKY_SPI", "RX_SFHSS_SPI",
    "GYRO_RAW", "DUAL_GYRO_RAW", "DUAL_GYRO_DIFF", "MAX7456_SIGNAL", "MAX7456_SPICLOCK", "SBUS",
    "FPORT", "RANGEFINDER", "RANGEFINDER_QUALITY",
]
_BASE_2025 = [
    "NONE", "CYCLETIME", "BATTERY", "GYRO_FILTERED", "ACCELEROMETER", "PIDLOOP",
    "RC_INTERPOLATION", "ANGLERATE", "ESC_SENSOR", "SCHEDULER", "STACK", "ESC_SENSOR_RPM",
    "ESC_SENSOR_TMP", "ALTITUDE", "FFT", "FFT_TIME", "FFT_FREQ", "RX_FRSKY_SPI", "RX_SFHSS_SPI",
    "GYRO_RAW", "MULTI_GYRO_RAW", "MULTI_GYRO_DIFF", "MAX7456_SIGNAL", "MAX7456_SPICLOCK", "SBUS",
    "FPORT", "RANGEFINDER", "RANGEFINDER_QUALITY", "OPTICALFLOW",
]
# 29 … 74 : identiques dans toutes les versions (D_MIN → D_MAX et quelques renommages gérés plus bas)
_MIDDLE = [
    "LIDAR_TF", "ADC_INTERNAL", "RUNAWAY_TAKEOFF", "SDIO", "CURRENT_SENSOR", "USB", "SMARTAUDIO",
    "RTH", "ITERM_RELAX", "ACRO_TRAINER", "RC_SMOOTHING", "RX_SIGNAL_LOSS", "RC_SMOOTHING_RATE",
    "ANTI_GRAVITY", "DYN_LPF", "RX_SPEKTRUM_SPI", "DSHOT_RPM_TELEMETRY", "RPM_FILTER", "D_MIN",
    "AC_CORRECTION", "AC_ERROR", "DUAL_GYRO_SCALED", "DSHOT_RPM_ERRORS",
    "CRSF_LINK_STATISTICS_UPLINK", "CRSF_LINK_STATISTICS_PWR", "CRSF_LINK_STATISTICS_DOWN", "BARO",
    "GPS_RESCUE_THROTTLE_PID", "DYN_IDLE", "FEEDFORWARD_LIMIT", "FEEDFORWARD", "BLACKBOX_OUTPUT",
    "GYRO_SAMPLE", "RX_TIMING", "D_LPF", "VTX_TRAMP", "GHST", "GHST_MSP", "SCHEDULER_DETERMINISM",
    "TIMING_ACCURACY", "RX_EXPRESSLRS_SPI", "RX_EXPRESSLRS_PHASELOCK", "RX_STATE_TIME",
    "GPS_RESCUE_VELOCITY", "GPS_RESCUE_HEADING", "GPS_RESCUE_TRACKING",
]
_RENAMES_2025 = {"D_MIN": "D_MAX", "DUAL_GYRO_SCALED": "MULTI_GYRO_SCALED",
                 "GPS_RESCUE_THROTTLE_PID": "AUTOPILOT_ALTITUDE"}
_TAIL_44 = ["ATTITUDE", "VTX_MSP", "GPS_DOP", "FAILSAFE", "DSHOT_TELEMETRY_COUNTS"]
_TAIL_45 = [
    "GPS_CONNECTION", "ATTITUDE", "VTX_MSP", "GPS_DOP", "FAILSAFE", "GYRO_CALIBRATION", "ANGLE_MODE",
    "ANGLE_TARGET", "CURRENT_ANGLE", "DSHOT_TELEMETRY_COUNTS", "RPM_LIMIT", "RC_STATS", "MAG_CALIB",
    "MAG_TASK_RATE", "EZLANDING",
]
_TAIL_2025 = _TAIL_45 + [
    "TPA", "S_TERM", "SPA", "TASK", "GIMBAL", "WING_SETPOINT", "AUTOPILOT_POSITION", "CHIRP",
    "FLASH_TEST_PRBS", "MAVLINK_TELEMETRY",
]
_TAIL_2026 = _TAIL_45 + [
    "TPA", "S_TERM", "SPA", "TASK", "GIMBAL", "WING_SETPOINT", "CHIRP", "FLASH_TEST_PRBS",
    "MAVLINK_TELEMETRY", "AUTOPILOT_PID", "POSITION_NAV", "AUTOPILOT_STOP",
    # master / versions suivantes
    "PITOT", "POSITION_EST", "AUTOPILOT_HEADING", "RX_REDPINE_SPI", "LAUNCH", "VELOCITY_EST",
    "SAG_COMPENSATION", "UPT1", "FLIGHT_PLAN",
]

TABLES: dict[str, list[str]] = {
    "4.4": _BASE_4X + _MIDDLE + _TAIL_44,
    "4.5": _BASE_4X + _MIDDLE + _TAIL_45,
    "2025.12": _BASE_2025 + [_RENAMES_2025.get(n, n) for n in _MIDDLE] + _TAIL_2025,
    "2026.6": _BASE_2025 + [_RENAMES_2025.get(n, n) for n in _MIDDLE] + _TAIL_2026,
}


def family(version: tuple[int, ...]) -> str:
    """Famille de version utilisée pour les tables (4.4, 4.5, 2025.12, 2026.6)."""
    if not version:
        return "2026.6"
    major = version[0]
    minor = version[1] if len(version) > 1 else 0
    if major < 2000:
        if (major, minor) <= (4, 4):
            return "4.4"
        return "4.5"
    if (major, minor) < (2026, 6):
        return "2025.12"
    return "2026.6"


def debug_mode_name(index: int | None, version: tuple[int, ...], explicit: str | None = None) -> str:
    """Nom du mode de débogage. `explicit` = clé d'en-tête debug_mode_name si présente."""
    if explicit:
        return explicit.strip().upper()
    if index is None:
        return "NONE"
    table = TABLES[family(version)]
    if 0 <= index < len(table):
        return table[index]
    return f"DEBUG_{index}"
