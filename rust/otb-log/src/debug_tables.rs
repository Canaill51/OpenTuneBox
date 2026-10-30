//! Noms des modes de débogage selon la version de Betaflight (port de `log/debug_tables.py`, voir `docs/spec-betaflight.md` §1).

use std::sync::LazyLock;

const BASE_4X: &[&str] = &[
    "NONE",
    "CYCLETIME",
    "BATTERY",
    "GYRO_FILTERED",
    "ACCELEROMETER",
    "PIDLOOP",
    "GYRO_SCALED",
    "RC_INTERPOLATION",
    "ANGLERATE",
    "ESC_SENSOR",
    "SCHEDULER",
    "STACK",
    "ESC_SENSOR_RPM",
    "ESC_SENSOR_TMP",
    "ALTITUDE",
    "FFT",
    "FFT_TIME",
    "FFT_FREQ",
    "RX_FRSKY_SPI",
    "RX_SFHSS_SPI",
    "GYRO_RAW",
    "DUAL_GYRO_RAW",
    "DUAL_GYRO_DIFF",
    "MAX7456_SIGNAL",
    "MAX7456_SPICLOCK",
    "SBUS",
    "FPORT",
    "RANGEFINDER",
    "RANGEFINDER_QUALITY",
];
const BASE_2025: &[&str] = &[
    "NONE",
    "CYCLETIME",
    "BATTERY",
    "GYRO_FILTERED",
    "ACCELEROMETER",
    "PIDLOOP",
    "RC_INTERPOLATION",
    "ANGLERATE",
    "ESC_SENSOR",
    "SCHEDULER",
    "STACK",
    "ESC_SENSOR_RPM",
    "ESC_SENSOR_TMP",
    "ALTITUDE",
    "FFT",
    "FFT_TIME",
    "FFT_FREQ",
    "RX_FRSKY_SPI",
    "RX_SFHSS_SPI",
    "GYRO_RAW",
    "MULTI_GYRO_RAW",
    "MULTI_GYRO_DIFF",
    "MAX7456_SIGNAL",
    "MAX7456_SPICLOCK",
    "SBUS",
    "FPORT",
    "RANGEFINDER",
    "RANGEFINDER_QUALITY",
    "OPTICALFLOW",
];
/// 29 … 74 : identiques dans toutes les versions (D_MIN → D_MAX et quelques renommages gérés plus bas).
const MIDDLE: &[&str] = &[
    "LIDAR_TF",
    "ADC_INTERNAL",
    "RUNAWAY_TAKEOFF",
    "SDIO",
    "CURRENT_SENSOR",
    "USB",
    "SMARTAUDIO",
    "RTH",
    "ITERM_RELAX",
    "ACRO_TRAINER",
    "RC_SMOOTHING",
    "RX_SIGNAL_LOSS",
    "RC_SMOOTHING_RATE",
    "ANTI_GRAVITY",
    "DYN_LPF",
    "RX_SPEKTRUM_SPI",
    "DSHOT_RPM_TELEMETRY",
    "RPM_FILTER",
    "D_MIN",
    "AC_CORRECTION",
    "AC_ERROR",
    "DUAL_GYRO_SCALED",
    "DSHOT_RPM_ERRORS",
    "CRSF_LINK_STATISTICS_UPLINK",
    "CRSF_LINK_STATISTICS_PWR",
    "CRSF_LINK_STATISTICS_DOWN",
    "BARO",
    "GPS_RESCUE_THROTTLE_PID",
    "DYN_IDLE",
    "FEEDFORWARD_LIMIT",
    "FEEDFORWARD",
    "BLACKBOX_OUTPUT",
    "GYRO_SAMPLE",
    "RX_TIMING",
    "D_LPF",
    "VTX_TRAMP",
    "GHST",
    "GHST_MSP",
    "SCHEDULER_DETERMINISM",
    "TIMING_ACCURACY",
    "RX_EXPRESSLRS_SPI",
    "RX_EXPRESSLRS_PHASELOCK",
    "RX_STATE_TIME",
    "GPS_RESCUE_VELOCITY",
    "GPS_RESCUE_HEADING",
    "GPS_RESCUE_TRACKING",
];
const RENAMES_2025: &[(&str, &str)] = &[
    ("D_MIN", "D_MAX"),
    ("DUAL_GYRO_SCALED", "MULTI_GYRO_SCALED"),
    ("GPS_RESCUE_THROTTLE_PID", "AUTOPILOT_ALTITUDE"),
];
const TAIL_44: &[&str] = &["ATTITUDE", "VTX_MSP", "GPS_DOP", "FAILSAFE", "DSHOT_TELEMETRY_COUNTS"];
const TAIL_45: &[&str] = &[
    "GPS_CONNECTION",
    "ATTITUDE",
    "VTX_MSP",
    "GPS_DOP",
    "FAILSAFE",
    "GYRO_CALIBRATION",
    "ANGLE_MODE",
    "ANGLE_TARGET",
    "CURRENT_ANGLE",
    "DSHOT_TELEMETRY_COUNTS",
    "RPM_LIMIT",
    "RC_STATS",
    "MAG_CALIB",
    "MAG_TASK_RATE",
    "EZLANDING",
];
const TAIL_2025_EXTRA: &[&str] = &[
    "TPA",
    "S_TERM",
    "SPA",
    "TASK",
    "GIMBAL",
    "WING_SETPOINT",
    "AUTOPILOT_POSITION",
    "CHIRP",
    "FLASH_TEST_PRBS",
    "MAVLINK_TELEMETRY",
];
const TAIL_2026_EXTRA: &[&str] = &[
    "TPA",
    "S_TERM",
    "SPA",
    "TASK",
    "GIMBAL",
    "WING_SETPOINT",
    "CHIRP",
    "FLASH_TEST_PRBS",
    "MAVLINK_TELEMETRY",
    "AUTOPILOT_PID",
    "POSITION_NAV",
    "AUTOPILOT_STOP",
    // master / versions suivantes
    "PITOT",
    "POSITION_EST",
    "AUTOPILOT_HEADING",
    "RX_REDPINE_SPI",
    "LAUNCH",
    "VELOCITY_EST",
    "SAG_COMPENSATION",
    "UPT1",
    "FLIGHT_PLAN",
];

fn renamed(name: &'static str) -> &'static str {
    RENAMES_2025
        .iter()
        .find(|(from, _)| *from == name)
        .map_or(name, |(_, to)| to)
}

fn build(parts: &[&[&'static str]]) -> Vec<&'static str> {
    parts.concat()
}

static TABLES: LazyLock<[(&'static str, Vec<&'static str>); 4]> = LazyLock::new(|| {
    let middle_2025: Vec<&'static str> = MIDDLE.iter().map(|n| renamed(n)).collect();
    [
        ("4.4", build(&[BASE_4X, MIDDLE, TAIL_44])),
        ("4.5", build(&[BASE_4X, MIDDLE, TAIL_45])),
        ("2025.12", build(&[BASE_2025, &middle_2025, TAIL_45, TAIL_2025_EXTRA])),
        ("2026.6", build(&[BASE_2025, &middle_2025, TAIL_45, TAIL_2026_EXTRA])),
    ]
});

/// Table d'une famille (`"4.4"`, `"4.5"`, `"2025.12"`, `"2026.6"`) ; une autre famille est un défaut du code appelant
/// (Python lève `KeyError`).
pub fn table(family: &str) -> &'static [&'static str] {
    TABLES
        .iter()
        .find(|(f, _)| *f == family)
        .map(|(_, t)| t.as_slice())
        .unwrap_or_else(|| panic!("famille inconnue : {family}"))
}

/// Famille de version utilisée pour les tables (4.4, 4.5, 2025.12, 2026.6).
pub fn family(version: &[i64]) -> &'static str {
    let Some(&major) = version.first() else { return "2026.6" };
    let minor = version.get(1).copied().unwrap_or(0);
    if major < 2000 {
        return if (major, minor) <= (4, 4) { "4.4" } else { "4.5" };
    }
    if (major, minor) < (2026, 6) {
        "2025.12"
    } else {
        "2026.6"
    }
}

/// Nom du mode de débogage. `explicit` = clé d'en-tête `debug_mode_name` si présente (non vide).
pub fn debug_mode_name(index: Option<i64>, version: &[i64], explicit: Option<&str>) -> String {
    if let Some(e) = explicit.filter(|e| !e.is_empty()) {
        return e.trim().to_uppercase();
    }
    let Some(i) = index else { return "NONE".into() };
    let t = table(family(version));
    match usize::try_from(i).ok().and_then(|i| t.get(i)) {
        Some(name) => (*name).to_string(),
        None => format!("DEBUG_{i}"),
    }
}
