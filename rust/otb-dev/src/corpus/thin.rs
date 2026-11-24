//! `otb-dev corpus thin` : d'où vient l'écart du terme D rejoué sur les logs ralentis (W-083, point 1).
//!
//! Un log enregistré à **chaque** boucle PID est ramené à une boucle sur `--factor` (2 par défaut) en gardant un échantillon
//! sur N de chaque signal : c'est ce qu'aurait écrit le Blackbox avec ce `P interval`, sans anti-repliement. Sur ce log
//! réduit, le D enregistré reste le vrai D du firmware ; on lui compare plusieurs rejeux, chacun jugé comme `corpus replay`
//! (spectre du rejeu « actuel » − spectre enregistré, roulis et tangage, par bande, période de l'assistant) :
//!
//! - `model_full`, `firmware_full` : les deux chaînes sur le log complet (repère) ;
//! - `model_thin`, `firmware_thin` : les deux chaînes sur le log réduit, comme sur un vrai log ralenti ;
//! - `oracle` : la chaîne du firmware rejouée sur le log **complet**, dont on garde une boucle sur N — le gyro à la cadence
//!   PID est le vrai, rien n'est reconstruit ; ce que le log réduit permet de mieux ;
//! - `firmware_gyro_log_d` : gyro de `firmware_thin`, terme D de la chaîne actuelle (dérivé à la cadence du log) ;
//! - `oracle_gyro_log_d` : gyro d'`oracle`, terme D de la chaîne actuelle.
//!
//! `firmware_thin` ne diffère d'`oracle` que par ce que le log réduit a perdu : l'entrée reconstruite à la cadence PID depuis un
//! gyro non filtré sous-échantillonné, et les centres du notch dynamique suivis sur ce signal-là.

use super::json::{as_f64, dump, parse};
use super::replay::{errors, percentile};
use super::{Args, Output, analysed, drone_id, log_files, logs_root, map_files, rel};
use otb_analysis::bf_filters::Matrix64;
use otb_analysis::filter_replay::{Replay, before_after_from, replay_dterm, replay_with};
use otb_analysis::firmware_chain::Engine;
use otb_log::flightlog::{FlightLog, Rows, load};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;

/// Durée d'une fenêtre des spectres (ms), celle de `corpus replay`.
const SEG_MS: f64 = 300.0;

/// Variantes, dans l'ordre du tableau.
const VARIANTS: [&str; 7] = [
    "model_full",
    "firmware_full",
    "model_thin",
    "firmware_thin",
    "oracle",
    "firmware_gyro_log_d",
    "oracle_gyro_log_d",
];

/// Un échantillon sur `n` : le dernier de chaque groupe, comme `firmware_chain::every` (celui qu'enregistre le log).
fn pick<T: Copy>(v: &[T], n: usize) -> Vec<T> {
    v.chunks_exact(n).map(|c| c[n - 1]).collect()
}

fn pick_rows(r: &Rows, n: usize) -> Rows {
    let m = r.n / n;
    let mut data = Vec::with_capacity(r.k * m);
    for i in 0..r.k {
        data.extend(pick(r.row(i), n));
    }
    Rows { k: r.k, n: m, data }
}

/// `lg` ramené à une boucle sur `n` (`P interval` divisé d'autant). Les événements ne sont pas repris (le rejeu ne les lit pas).
fn thinned(lg: &FlightLog, n: usize) -> FlightLog {
    let mut t = lg.clone();
    t.t = pick(&lg.t, n);
    t.fs = lg.fs / n as f64;
    for (dst, src) in [
        (&mut t.gyro, &lg.gyro),
        (&mut t.setpoint, &lg.setpoint),
        (&mut t.rc_command, &lg.rc_command),
        (&mut t.p_term, &lg.p_term),
        (&mut t.i_term, &lg.i_term),
        (&mut t.d_term, &lg.d_term),
        (&mut t.f_term, &lg.f_term),
        (&mut t.motors, &lg.motors),
        (&mut t.debug, &lg.debug),
    ] {
        *dst = pick_rows(src, n);
    }
    t.gyro_unfilt = lg.gyro_unfilt.as_ref().map(|r| pick_rows(r, n));
    t.motor_hz = lg.motor_hz.as_ref().map(|r| pick_rows(r, n));
    t.acc = lg.acc.as_ref().map(|r| pick_rows(r, n));
    t.throttle = pick(&lg.throttle, n);
    t.vbat = lg.vbat.as_deref().map(|v| pick(v, n));
    t.current = lg.current.as_deref().map(|v| pick(v, n));
    t.mode_flags = lg.mode_flags.as_deref().map(|v| pick(v, n));
    let (num, den) = lg.settings.p_interval;
    t.settings.p_interval = (num, den * n as i64);
    t
}

/// `rep` (rejeu du log complet) ramené à une boucle sur `n`.
fn thinned_replay(rep: &Replay, n: usize) -> Replay {
    let m = rep.dterm.n / n;
    let mut data = Vec::with_capacity(rep.dterm.k * m);
    for a in 0..rep.dterm.k {
        data.extend(pick(rep.dterm.row(a), n));
    }
    Replay {
        gyro: rep.gyro.iter().map(|g| pick(g, n)).collect(),
        dterm: Matrix64 {
            k: rep.dterm.k,
            n: m,
            data,
        },
        engine: rep.engine,
        gyro_pid: Vec::new(),
    }
}

/// Écarts par bande de `rep` (rejeu des réglages du log) contre le log `lg`, période `sel`.
fn judged(lg: &FlightLog, sel: &Range<usize>, rep: &Replay) -> Result<Option<Map<String, Value>>, String> {
    let s = &lg.settings;
    Ok(before_after_from(lg, sel.clone(), s, rep, s, SEG_MS)?.map(|r| errors(&r, lg.fs)))
}

fn session_row(lg: &FlightLog, f: &Path, root: &Path, n: usize) -> Result<Option<Value>, String> {
    let s = &lg.settings;
    // seulement les logs enregistrés à chaque boucle PID
    if s.log_rate_hz() < s.pid_rate_hz() * 0.999 {
        return Ok(None);
    }
    let Some(sel) = analysed(lg) else { return Ok(None) };
    let (Some(model_full), Some(fw_full)) = (
        replay_with(lg, s, Engine::Model)?,
        replay_with(lg, s, Engine::Firmware)?,
    ) else {
        return Ok(None);
    };
    let tl = thinned(lg, n);
    let ts = &tl.settings;
    let sel_t = sel.start / n..sel.end / n;
    let (Some(model_thin), Some(fw_thin)) = (
        replay_with(&tl, ts, Engine::Model)?,
        replay_with(&tl, ts, Engine::Firmware)?,
    ) else {
        return Ok(None);
    };
    let oracle = thinned_replay(&fw_full, n);
    let with_log_d = |gyro: &[Vec<f64>]| -> Result<Replay, String> {
        Ok(Replay {
            gyro: gyro.to_vec(),
            dterm: replay_dterm(&tl, ts, gyro)?,
            engine: Engine::Model,
            gyro_pid: Vec::new(),
        })
    };
    let fw_log_d = with_log_d(&fw_thin.gyro)?;
    let oracle_log_d = with_log_d(&oracle.gyro)?;
    let cases: [(&str, &FlightLog, &Range<usize>, &Replay); 7] = [
        ("model_full", lg, &sel, &model_full),
        ("firmware_full", lg, &sel, &fw_full),
        ("model_thin", &tl, &sel_t, &model_thin),
        ("firmware_thin", &tl, &sel_t, &fw_thin),
        ("oracle", &tl, &sel_t, &oracle),
        ("firmware_gyro_log_d", &tl, &sel_t, &fw_log_d),
        ("oracle_gyro_log_d", &tl, &sel_t, &oracle_log_d),
    ];
    let mut variants = Map::new();
    for (name, log, period, rep) in cases {
        let Some(e) = judged(log, period, rep)? else {
            return Ok(None);
        };
        variants.insert(name.into(), Value::Object(e));
    }
    let relf = rel(f, root);
    Ok(Some(json!({
        "file": relf,
        "sess": lg.session,
        "drone": drone_id(&s.craft_name, &relf),
        "family": s.family(),
        "pid_hz": s.pid_rate_hz(),
        "thin_log_hz": ts.log_rate_hz(),
        "factor": n,
        "variants": variants,
    })))
}

fn measure_file(f: &Path, root: &Path, n: usize) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        out.extend(session_row(&lg, f, root, n)?);
    }
    Ok(out)
}

/// Tableau : pour chaque variante et chaque bande, |écart| médian au log (moyenne signée) ; une ligne par variante.
fn resume_text(rows: &[Value]) -> String {
    let rs: Vec<&Map<String, Value>> = rows.iter().filter_map(Value::as_object).collect();
    let drones: BTreeMap<&str, usize> =
        rs.iter()
            .filter_map(|r| r.get("drone").and_then(Value::as_str))
            .fold(BTreeMap::new(), |mut m, d| {
                *m.entry(d).or_default() += 1;
                m
            });
    let mut cols: Vec<String> = rs
        .iter()
        .filter_map(|r| r.get("variants")?.get("firmware_thin")?.as_object())
        .flat_map(|e| e.keys().cloned())
        .collect();
    cols.sort_by_key(|c| {
        (
            !c.starts_with("gyro"),
            c.split('_').nth(1).and_then(|x| x.parse::<u32>().ok()),
        )
    });
    cols.dedup();
    let mut out = format!(
        "{} sessions, {} drones ({}) ; |écart| médian au log (moyenne signée), dB\n\n| Variante | {} |\n|---|{}\n",
        rs.len(),
        drones.len(),
        drones
            .iter()
            .map(|(d, k)| format!("{d} : {k}"))
            .collect::<Vec<_>>()
            .join(", "),
        cols.join(" | "),
        "---|".repeat(cols.len())
    );
    for v in VARIANTS {
        let mut cells = Vec::new();
        for c in &cols {
            let x: Vec<f64> = rs
                .iter()
                .filter_map(|r| r.get("variants")?.get(v)?.get(c).and_then(as_f64))
                .filter(|x| x.is_finite())
                .collect();
            if x.is_empty() {
                cells.push("—".to_string());
                continue;
            }
            let signed = x.iter().sum::<f64>() / x.len() as f64;
            let mut abs: Vec<f64> = x.iter().map(|y| y.abs()).collect();
            cells.push(format!("{:.2} ({signed:+.2})", percentile(&mut abs, 0.5)));
        }
        out.push_str(&format!("| {v} | {} |\n", cells.join(" | ")));
    }
    out
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["-j", "--limit", "--dir", "--factor"], &["--resume"])?;
    match (a.has("--resume"), a.positional.as_slice()) {
        (true, [path]) => {
            let t = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
            let rows = parse(&t)?;
            Ok(Output::ok(resume_text(rows.as_array().map_or(&[][..], |v| v))))
        }
        (false, [path]) => {
            let n = a.number("--factor")?.unwrap_or(2);
            if n < 2 {
                return Err("--factor : au moins 2".into());
            }
            let root = logs_root();
            let dir = a.text("--dir").map_or(root.clone(), |d| root.join(d));
            let mut files = log_files(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
            if let Some(k) = a.number("--limit")? {
                files.truncate(k);
            }
            let res = map_files(|f| measure_file(f, &root, n), &files, a.number("-j")?, &root);
            let rows: Vec<Value> = res.into_iter().flat_map(|(_, rows)| rows).collect();
            std::fs::write(path, dump(&Value::Array(rows.clone()))?).map_err(|e| format!("{path} : {e}"))?;
            Ok(Output::ok(resume_text(&rows)))
        }
        _ => Err(super::USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn jeno5() -> FlightLog {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
        load(&p).expect("jeno5_5s.bbl").remove(0)
    }

    #[test]
    fn le_log_reduit_garde_le_dernier_echantillon_de_chaque_groupe() {
        let lg = jeno5();
        let t = thinned(&lg, 2);
        assert!((t.settings.log_rate_hz() - lg.settings.log_rate_hz() / 2.0).abs() < 1e-9);
        assert!((t.fs - lg.fs / 2.0).abs() < 1e-9);
        let n = lg.n() / 2;
        assert_eq!(t.n(), n);
        for k in [0, 1, n / 2, n - 1] {
            assert_eq!(t.t[k], lg.t[2 * k + 1]);
            assert_eq!(t.d_term.row(0)[k], lg.d_term.row(0)[2 * k + 1]);
            let (gu, gt) = (lg.gyro_unfilt.as_ref().unwrap(), t.gyro_unfilt.as_ref().unwrap());
            assert_eq!(gt.row(1)[k], gu.row(1)[2 * k + 1]);
            assert_eq!(t.throttle[k], lg.throttle[2 * k + 1]);
            assert_eq!(t.debug.row(0)[k], lg.debug.row(0)[2 * k + 1]);
        }
        assert_eq!(t.gyro.n, n);
        assert_eq!(t.motors.n, n);
    }
}
