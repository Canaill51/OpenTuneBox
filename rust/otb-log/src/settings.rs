//! Réglages Betaflight lus dans l'en-tête, normalisés quelle que soit la version (port de `log/settings.py`).
//!
//! Différences gérées (voir `docs/spec-betaflight.md` §4) :
//! - 4.4/4.5 : `rollPID` D = D maximum, `d_min` = D de base ; 2025.12+ : `rollPID` D = D de base, `d_max` = D maximum ;
//! - ralenti : `dshot_idle_value` (4.x) / `motor_idle` (2025.12+) ;
//! - curseur D max : `simplified_dmax_gain` (4.x) / `simplified_d_max_gain` (2025.12+).
//!
//! ⚠️ Les nombres de l'en-tête se lisent comme en Python (`int(float(p))`, `0x…` en base 16) : voir `otb_decoder::header`.
//! Écarts de robustesse assumés : `inf` (Python plante) et un entier hors de `i64` rendent la valeur ignorée ou saturée.

use std::collections::BTreeMap;

use otb_decoder::header::{py_float, py_hex, py_space, py_strip};

use crate::debug_tables::{debug_mode_name, family};

/// motor_pwm_protocol : 5 et plus = DShot / ProShot.
pub const PWM_DSHOT_MIN: i64 = 5;

fn filter_type(n: i64) -> Option<&'static str> {
    match n {
        0 => Some("PT1"),
        1 => Some("BIQUAD"),
        2 => Some("PT2"),
        3 => Some("PT3"),
        _ => None,
    }
}

/// `_ints` : entiers séparés par des virgules (`0x…` en base 16, sinon `int(float(p))`), illisibles ignorés.
pub fn ints(v: Option<&str>) -> Vec<i64> {
    let Some(v) = v.filter(|v| !v.is_empty()) else {
        return vec![];
    };
    v.split(',')
        .filter_map(|p| {
            let p = py_strip(p);
            if p.to_ascii_lowercase().starts_with("0x") {
                py_hex(p)
            } else {
                py_float(p).filter(|f| f.is_finite()).map(|f| f as i64)
            }
        })
        .collect()
}

/// `_int` : premier entier lisible de la valeur, sinon `default`.
pub fn int(v: Option<&str>, default: i64) -> i64 {
    ints(v).first().copied().unwrap_or(default)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AxisPid {
    pub p: i64,
    pub i: i64,
    /// D de base (niveau bas).
    pub d: i64,
    /// D maximum (boost D max / D min).
    pub dmax: i64,
    pub f: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lowpass {
    /// `"PT1"`, `"BIQUAD"`, `"PT2"` ou `"PT3"`.
    pub kind: &'static str,
    pub static_hz: i64,
    pub dyn_min_hz: i64,
    pub dyn_max_hz: i64,
    pub expo: i64,
}

impl Default for Lowpass {
    fn default() -> Self {
        Lowpass {
            kind: "PT1",
            static_hz: 0,
            dyn_min_hz: 0,
            dyn_max_hz: 0,
            expo: 5,
        }
    }
}

impl Lowpass {
    pub fn enabled(&self) -> bool {
        self.static_hz > 0 || self.dyn_min_hz > 0
    }

    pub fn dynamic(&self) -> bool {
        self.dyn_min_hz > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub raw: BTreeMap<String, String>,
    pub firmware: String,
    pub firmware_name: String,
    pub version: Vec<i64>,
    pub board: String,
    pub craft_name: String,
    pub looptime_us: i64,
    pub pid_denom: i64,
    pub p_interval: (i64, i64),
    pub debug_mode: String,
    pub debug_index: i64,
    pub pids: [AxisPid; 3],
    pub gyro_lpf1: Lowpass,
    pub gyro_lpf2: Lowpass,
    /// (centre, coupure)
    pub gyro_notches: Vec<(i64, i64)>,
    pub dterm_lpf1: Lowpass,
    pub dterm_lpf2: Lowpass,
    pub dterm_notch: (i64, i64),
    pub yaw_lowpass_hz: i64,
    pub dyn_notch_count: i64,
    pub dyn_notch_q: i64,
    pub dyn_notch_min_hz: i64,
    pub dyn_notch_max_hz: i64,
    pub rpm_harmonics: i64,
    pub rpm_weights: Vec<i64>,
    pub rpm_q: i64,
    pub rpm_min_hz: i64,
    pub rpm_fade_hz: i64,
    pub rpm_lpf_hz: i64,
    pub motor_poles: i64,
    pub dshot_bidir: bool,
    pub motor_protocol: i64,
    pub motor_output: (i64, i64),
    pub minthrottle: i64,
    pub maxthrottle: i64,
    pub high_resolution: bool,
    pub acc_1g: i64,
    pub motor_kv: i64,
}

impl Default for Settings {
    /// Valeurs par défaut du dataclass Python, avant la lecture de l'en-tête.
    fn default() -> Self {
        Settings {
            raw: BTreeMap::new(),
            firmware: String::new(),
            firmware_name: "Betaflight".into(),
            version: vec![],
            board: String::new(),
            craft_name: String::new(),
            looptime_us: 125,
            pid_denom: 1,
            p_interval: (1, 1),
            debug_mode: "NONE".into(),
            debug_index: 0,
            pids: [AxisPid::default(); 3],
            gyro_lpf1: Lowpass::default(),
            gyro_lpf2: Lowpass::default(),
            gyro_notches: vec![],
            dterm_lpf1: Lowpass::default(),
            dterm_lpf2: Lowpass::default(),
            dterm_notch: (0, 0),
            yaw_lowpass_hz: 0,
            dyn_notch_count: 0,
            dyn_notch_q: 0,
            dyn_notch_min_hz: 0,
            dyn_notch_max_hz: 0,
            rpm_harmonics: 0,
            rpm_weights: vec![],
            rpm_q: 500,
            rpm_min_hz: 100,
            rpm_fade_hz: 50,
            rpm_lpf_hz: 150,
            motor_poles: 14,
            dshot_bidir: false,
            motor_protocol: 0,
            motor_output: (48, 2047),
            minthrottle: 1000,
            maxthrottle: 2000,
            high_resolution: false,
            acc_1g: 2048,
            motor_kv: 0,
        }
    }
}

impl Settings {
    pub fn gyro_rate_hz(&self) -> f64 {
        1e6 / self.looptime_us.max(1) as f64
    }

    pub fn pid_rate_hz(&self) -> f64 {
        self.gyro_rate_hz() / self.pid_denom.max(1) as f64
    }

    pub fn log_rate_hz(&self) -> f64 {
        let (num, den) = self.p_interval;
        self.pid_rate_hz() * num as f64 / den.max(1) as f64
    }

    pub fn family(&self) -> &'static str {
        family(&self.version)
    }

    pub fn is_dshot(&self) -> bool {
        self.motor_protocol >= PWM_DSHOT_MIN
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.raw.get(key).map(String::as_str)
    }

    pub fn get_int(&self, key: &str, default: i64) -> i64 {
        int(self.get(key), default)
    }

    pub fn get_ints(&self, key: &str) -> Vec<i64> {
        ints(self.get(key))
    }

    /// Positions des curseurs du réglage simplifié (en %, 100 = défaut) : seulement ceux que l'en-tête contient.
    pub fn sliders(&self) -> Vec<(&'static str, i64)> {
        let dmax_key = if self.raw.contains_key("simplified_d_max_gain") {
            "simplified_d_max_gain"
        } else {
            "simplified_dmax_gain"
        };
        [
            ("mode", "simplified_pids_mode"),
            ("master", "simplified_master_multiplier"),
            ("i_gain", "simplified_i_gain"),
            ("d_gain", "simplified_d_gain"),
            ("pi_gain", "simplified_pi_gain"),
            ("ff_gain", "simplified_feedforward_gain"),
            ("pitch_d", "simplified_pitch_d_gain"),
            ("pitch_pi", "simplified_pitch_pi_gain"),
            ("dmax_gain", dmax_key),
            ("dterm_filter", "simplified_dterm_filter"),
            ("dterm_mult", "simplified_dterm_filter_multiplier"),
            ("gyro_filter", "simplified_gyro_filter"),
            ("gyro_mult", "simplified_gyro_filter_multiplier"),
        ]
        .into_iter()
        .filter(|(_, key)| self.raw.contains_key(*key))
        .map(|(name, key)| (name, self.get_int(key, -1)))
        .collect()
    }
}

/// Entier décimal en tête de `s` (au moins un chiffre ASCII), saturé à `i64::MAX` ; `(valeur, reste)`.
fn leading_int(s: &str) -> Option<(i64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some((s[..end].parse().unwrap_or(i64::MAX), &s[end..]))
}

/// `^(\w+)\s+(\d+)\.(\d+)\.(\d+)` : nom du firmware et version (`\w` et `\s` au sens d'Unicode, comme `re` de Python).
fn firmware_version(fw: &str) -> Option<(String, [i64; 3])> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let name_end = fw.find(|c: char| !word(c)).unwrap_or(fw.len());
    if name_end == 0 {
        return None;
    }
    let rest = &fw[name_end..];
    let after_space = rest.trim_start_matches(py_space);
    if after_space.len() == rest.len() {
        return None;
    }
    let (a, rest) = leading_int(after_space)?;
    let (b, rest) = leading_int(rest.strip_prefix('.')?)?;
    let (c, _) = leading_int(rest.strip_prefix('.')?)?;
    Some((fw[..name_end].to_string(), [a, b, c]))
}

fn lowpass(raw: &BTreeMap<String, String>, prefix: &str) -> Lowpass {
    let get = |suffix: &str| raw.get(&format!("{prefix}_{suffix}")).map(String::as_str);
    let dyn_hz = ints(get("dyn_hz"));
    Lowpass {
        kind: filter_type(int(get("type"), 0)).unwrap_or("PT1"),
        static_hz: int(get("static_hz"), 0),
        dyn_min_hz: dyn_hz.first().copied().unwrap_or(0),
        dyn_max_hz: dyn_hz.get(1).copied().unwrap_or(0),
        expo: int(get("dyn_expo"), 5),
    }
}

pub fn parse_settings(raw: &BTreeMap<String, String>) -> Settings {
    let mut s = Settings {
        raw: raw.clone(),
        ..Settings::default()
    };
    let g = |key: &str| raw.get(key).map(String::as_str);
    let fw = py_strip(g("Firmware revision").unwrap_or("")).to_string();
    if let Some((name, v)) = firmware_version(&fw) {
        s.firmware_name = name;
        s.version = v.to_vec();
        s.board = if fw.contains(')') {
            fw.split(py_space).rfind(|t| !t.is_empty()).unwrap_or("").to_string()
        } else {
            String::new()
        };
    }
    s.firmware = fw;
    s.craft_name = py_strip(g("Craft name").unwrap_or("")).to_string();
    s.looptime_us = int(g("looptime"), 125).max(1);
    s.pid_denom = int(g("pid_process_denom"), 1).max(1);
    let pv = g("P interval").unwrap_or("1");
    s.p_interval = match pv.split_once('/') {
        Some((a, b)) => (int(Some(a), 1).max(1), int(Some(b), 1).max(1)),
        None => (1, int(Some(pv), 1).max(1)),
    };
    s.debug_index = int(g("debug_mode"), 0);
    s.debug_mode = debug_mode_name(Some(s.debug_index), &s.version, g("debug_mode_name"));

    // PID
    let new_dmax = matches!(family(&s.version), "2025.12" | "2026.6");
    let ff = ints(g("ff_weight"));
    let dmin = ints(g("d_min"));
    let dmax = ints(g("d_max"));
    for (ax, key) in ["rollPID", "pitchPID", "yawPID"].into_iter().enumerate() {
        let mut v = ints(g(key));
        v.extend([0, 0, 0]);
        let mut p = AxisPid {
            p: v[0],
            i: v[1],
            d: v[2],
            dmax: v[2],
            f: ff.get(ax).copied().unwrap_or(0),
        };
        if new_dmax {
            if let Some(&m) = dmax.get(ax).filter(|&&m| m > p.d) {
                p.dmax = m;
            }
        } else if let Some(&m) = dmin.get(ax).filter(|&&m| 0 < m && m < p.d) {
            p.d = m;
        }
        s.pids[ax] = p;
    }

    // Filtres gyro et D
    s.gyro_lpf1 = lowpass(raw, "gyro_lpf1");
    s.gyro_lpf2 = lowpass(raw, "gyro_lpf2");
    let hz = ints(g("gyro_notch_hz"));
    let cut = ints(g("gyro_notch_cutoff"));
    s.gyro_notches = hz
        .iter()
        .enumerate()
        .filter(|(_, h)| **h > 0)
        .map(|(i, h)| (*h, cut.get(i).copied().unwrap_or(0)))
        .collect();
    s.dterm_lpf1 = lowpass(raw, "dterm_lpf1");
    s.dterm_lpf2 = lowpass(raw, "dterm_lpf2");
    s.dterm_notch = (int(g("dterm_notch_hz"), 0), int(g("dterm_notch_cutoff"), 0));
    s.yaw_lowpass_hz = int(g("yaw_lowpass_hz"), 0);
    s.dyn_notch_count = int(g("dyn_notch_count"), 0);
    s.dyn_notch_q = int(g("dyn_notch_q"), 0);
    s.dyn_notch_min_hz = int(g("dyn_notch_min_hz"), 0);
    s.dyn_notch_max_hz = int(g("dyn_notch_max_hz"), 0);
    s.rpm_harmonics = int(g("rpm_filter_harmonics"), 0);
    let weights = ints(g("rpm_filter_weights"));
    s.rpm_weights = if weights.is_empty() {
        vec![100; s.rpm_harmonics.clamp(1, 1 << 20) as usize]
    } else {
        weights
    };
    s.rpm_q = int(g("rpm_filter_q"), 500);
    s.rpm_min_hz = int(g("rpm_filter_min_hz"), 100);
    s.rpm_fade_hz = int(g("rpm_filter_fade_range_hz"), 50);
    s.rpm_lpf_hz = int(g("rpm_filter_lpf_hz"), 150);

    // Moteurs
    s.motor_poles = int(g("motor_poles"), 14).max(2);
    s.dshot_bidir = int(g("dshot_bidir"), 0) == 1;
    s.motor_protocol = int(g("motor_pwm_protocol"), 6);
    if let [a, b, ..] = ints(g("motorOutput")).as_slice() {
        s.motor_output = (*a, *b);
    }
    s.minthrottle = int(g("minthrottle"), 1000);
    s.maxthrottle = int(g("maxthrottle"), 2000);
    s.high_resolution = int(g("blackbox_high_resolution"), 0) == 1;
    s.acc_1g = int(g("acc_1G"), 2048).max(1);
    s.motor_kv = int(g("motor_kv"), 0);
    s
}
