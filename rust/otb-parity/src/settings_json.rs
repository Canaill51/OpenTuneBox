//! `Settings` en JSON, au format de `scripts/golden_settings.py:settings_json` (flottants en bits : `"f64:<hexa>"`).

use otb_log::settings::{Lowpass, Settings};
use serde_json::{Value, json};

fn f64_bits(x: f64) -> String {
    format!("f64:{:016x}", x.to_bits())
}

fn lowpass(lp: &Lowpass) -> Value {
    json!({
        "type": lp.kind, "static_hz": lp.static_hz, "dyn_min_hz": lp.dyn_min_hz, "dyn_max_hz": lp.dyn_max_hz,
        "expo": lp.expo, "enabled": lp.enabled(), "dynamic": lp.dynamic(),
    })
}

pub fn settings_json(s: &Settings) -> Value {
    let sliders: serde_json::Map<String, Value> = s
        .sliders()
        .into_iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect();
    json!({
        "firmware": s.firmware, "firmware_name": s.firmware_name, "version": s.version, "board": s.board,
        "craft_name": s.craft_name, "looptime_us": s.looptime_us, "pid_denom": s.pid_denom,
        "p_interval": [s.p_interval.0, s.p_interval.1], "debug_mode": s.debug_mode, "debug_index": s.debug_index,
        "pids": s.pids.iter().map(|p| [p.p, p.i, p.d, p.dmax, p.f]).collect::<Vec<_>>(),
        "gyro_lpf1": lowpass(&s.gyro_lpf1), "gyro_lpf2": lowpass(&s.gyro_lpf2),
        "gyro_notches": s.gyro_notches.iter().map(|n| [n.0, n.1]).collect::<Vec<_>>(),
        "dterm_lpf1": lowpass(&s.dterm_lpf1), "dterm_lpf2": lowpass(&s.dterm_lpf2),
        "dterm_notch": [s.dterm_notch.0, s.dterm_notch.1],
        "yaw_lowpass_hz": s.yaw_lowpass_hz, "dyn_notch_count": s.dyn_notch_count, "dyn_notch_q": s.dyn_notch_q,
        "dyn_notch_min_hz": s.dyn_notch_min_hz, "dyn_notch_max_hz": s.dyn_notch_max_hz,
        "rpm_harmonics": s.rpm_harmonics, "rpm_weights": s.rpm_weights, "rpm_q": s.rpm_q,
        "rpm_min_hz": s.rpm_min_hz, "rpm_fade_hz": s.rpm_fade_hz, "rpm_lpf_hz": s.rpm_lpf_hz,
        "motor_poles": s.motor_poles, "dshot_bidir": s.dshot_bidir, "motor_protocol": s.motor_protocol,
        "motor_output": [s.motor_output.0, s.motor_output.1], "minthrottle": s.minthrottle, "maxthrottle": s.maxthrottle,
        "high_resolution": s.high_resolution, "acc_1g": s.acc_1g, "motor_kv": s.motor_kv,
        "gyro_rate_hz": f64_bits(s.gyro_rate_hz()), "pid_rate_hz": f64_bits(s.pid_rate_hz()),
        "log_rate_hz": f64_bits(s.log_rate_hz()), "family": s.family(), "is_dshot": s.is_dshot(), "sliders": sliders,
    })
}

/// Clés qui diffèrent entre deux objets JSON de réglages (vide si identiques).
pub fn diff(want: &Value, got: &Value) -> Vec<String> {
    let (w, g) = (want.as_object().unwrap(), got.as_object().unwrap());
    let mut out: Vec<String> = w
        .iter()
        .filter(|(k, v)| g.get(*k) != Some(*v))
        .map(|(k, v)| format!("{k} : Python {v}, Rust {}", g.get(k).unwrap_or(&Value::Null)))
        .collect();
    out.extend(
        g.keys()
            .filter(|k| !w.contains_key(*k))
            .map(|k| format!("{k} : clé en trop côté Rust")),
    );
    out
}
