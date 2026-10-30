//! Empreintes d'un `FlightLog`, au format de `scripts/golden_flightlog.py:flightlog_record` : `"dtype|forme|sha256"` par
//! tableau (octets petit-boutistes en ordre C), `fs` en bits, statistiques, colonnes PID, propriétés dérivées.

use otb_log::flightlog::{FlightLog, Rows};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn digest(dtype: &str, shape: String, bytes: &[u8]) -> Value {
    let hex: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    json!(format!("{dtype}|{shape}|{hex}"))
}

fn rows(r: &Rows) -> Value {
    let bytes: Vec<u8> = r.data.iter().flat_map(|v| v.to_le_bytes()).collect();
    digest("float32", format!("({}, {})", r.k, r.n), &bytes)
}

fn vec_f32(v: &[f32]) -> Value {
    digest(
        "float32",
        format!("({},)", v.len()),
        &v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>(),
    )
}

fn f64_bits(x: f64) -> String {
    format!("f64:{:016x}", x.to_bits())
}

/// Tous les champs de comparaison exacte (les échantillons de `d_term_prefilter` se comparent à part, avec une tolérance).
pub fn flightlog_digest(lg: &FlightLog) -> Value {
    let opt_rows = |r: &Option<Rows>| r.as_ref().map_or(Value::Null, rows);
    let opt_vec = |v: &Option<Vec<f32>>| v.as_ref().map_or(Value::Null, |v| vec_f32(v));
    let s = &lg.stats;
    json!({
        "session": lg.session, "n": lg.n(), "fs": f64_bits(lg.fs),
        "t": digest("float64", format!("({},)", lg.t.len()), &lg.t.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()),
        "gyro": rows(&lg.gyro), "gyro_unfilt": opt_rows(&lg.gyro_unfilt), "setpoint": rows(&lg.setpoint),
        "rc_command": rows(&lg.rc_command), "p_term": rows(&lg.p_term), "i_term": rows(&lg.i_term),
        "d_term": rows(&lg.d_term), "f_term": rows(&lg.f_term), "throttle": vec_f32(&lg.throttle),
        "motors": rows(&lg.motors), "motor_hz": opt_rows(&lg.motor_hz), "debug": rows(&lg.debug),
        "vbat": opt_vec(&lg.vbat), "current": opt_vec(&lg.current), "acc": opt_rows(&lg.acc),
        "mode_flags": lg.mode_flags.as_ref().map_or(Value::Null, |f| {
            digest("int32", format!("({},)", f.len()), &f.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>())
        }),
        "events": digest("int64", format!("({}, {})", lg.events.rows, lg.events.cols),
            &lg.events.data.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()),
        "stats": {
            "bytes": s.bytes, "corrupt_frames": s.corrupt_frames, "desync_frames": s.desync_frames, "i_frames": s.i_frames,
            "implausible_frames": lg.implausible_frames, "log_end_seen": s.log_end_seen, "p_frames": s.p_frames,
            "skipped_iterations": s.skipped_iterations,
        },
        "pid_columns": lg.pid_columns.iter().collect::<Vec<_>>(),
        "pid_sum": rows(&lg.pid_sum()), "pid_error": rows(&lg.pid_error()),
        "armed": digest("bool", format!("({},)", lg.n()), &lg.armed().iter().map(|&b| b as u8).collect::<Vec<u8>>()),
        "cells": lg.cells(),
        "segments": lg.segments(4.0).iter().map(|r| [r.start, r.end]).collect::<Vec<_>>(),
    })
}

/// Écart maximal des échantillons de `d_term_prefilter` (rapporté au plus grand échantillon de l'axe) entre Python et Rust.
pub fn prefilter_deviation(want: &Value, lg: &FlightLog) -> Result<f64, String> {
    let stride = want["stride"].as_u64().ok_or("stride")? as usize;
    let got = lg.d_term_prefilter();
    if want["shape"] != json!([got.k, got.n]) {
        return Err(format!("forme {} ≠ [{}, {}]", want["shape"], got.k, got.n));
    }
    let mut worst = 0.0f64;
    for ax in 0..3 {
        let w: Vec<f64> = want["samples"][ax]
            .as_array()
            .ok_or("samples")?
            .iter()
            .map(|h| f32::from_bits(u32::from_str_radix(h.as_str().unwrap(), 16).unwrap()) as f64)
            .collect();
        let g: Vec<f64> = got.row(ax).iter().step_by(stride).map(|&v| v as f64).collect();
        if w.len() != g.len() {
            return Err(format!("{} échantillons ≠ {}", w.len(), g.len()));
        }
        let scale = w.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        for (a, b) in w.iter().zip(&g) {
            if scale > 0.0 {
                worst = worst.max((a - b).abs() / scale);
            } else if a != b {
                return Err("échantillons non nuls pour un axe nul".into());
            }
        }
    }
    Ok(worst)
}

/// Clés qui diffèrent (hors `d_term_prefilter`).
pub fn diff(want: &Value, got: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in got.as_object().unwrap() {
        if want.get(k) != Some(v) {
            out.push(format!(
                "{k} : Python {}, Rust {v}",
                want.get(k).unwrap_or(&Value::Null)
            ));
        }
    }
    out
}
