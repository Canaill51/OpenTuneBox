//! `otb-dev corpus replay` : le simulateur de la page Filtres (`otb_analysis::filter_replay`, lot 22e du plan, D-058) juge
//! lui-même son modèle : spectre du rejeu des réglages du log (« simulé actuel ») contre le spectre enregistré, gyro et
//! terme D, roulis et tangage, par bande de fréquences. Une ligne par session ; `--resume` en fait le tableau selon que le
//! log enregistre chaque boucle PID ou non (sinon le gyro non filtré est sous-échantillonné sans anti-repliement, W-067).
//!
//! Période : celle de l'assistant (`analysed`). Chaque ligne garde aussi l'étendue des gaz (10e et 90e centiles, %) et si le
//! LPF1 du D est dynamique : l'ordre filtrer / dériver du terme D ne compte que quand sa coupure suit les gaz (lot 33).

use super::json::{as_f64, dump, num, parse};
use super::{Args, Output, analysed, drone_id, log_files, logs_root, map_files, rel};
use otb_analysis::filter_replay::{BeforeAfter, before_after_with};
use otb_analysis::firmware_chain::Engine;
use otb_log::flightlog::{FlightLog, load};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

/// Bandes comparées (Hz) ; la dernière s'arrête à 0,45 × cadence du log.
const BANDS: [(f64, f64); 3] = [(20.0, 100.0), (100.0, 250.0), (250.0, 500.0)];

/// Moyenne de `a − b` (dB) sur [`lo`, `hi`[ ; NaN sans fréquence dans la bande.
fn band(f: &[f64], a: &[f64], b: &[f64], lo: f64, hi: f64) -> f64 {
    let v: Vec<f64> = f
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(x, _)| **x >= lo && **x < hi)
        .map(|(_, (p, q))| p - q)
        .collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Écarts par bande, rejeu actuel − log (dB), moyenne du roulis et du tangage : `gyro_<bas>_<haut>` et `d_<bas>_<haut>`. La
/// dernière bande s'arrête à 0,45 × `fs` (cadence du log).
pub(super) fn errors(r: &BeforeAfter, fs: f64) -> Map<String, Value> {
    let top = 0.45 * fs;
    let mut errs = Map::new();
    for (lo, nominal_hi) in BANDS {
        // bande nommée d'après sa borne nominale, coupée à 0,45 × cadence du log
        let hi = nominal_hi.min(top);
        if hi <= lo {
            continue;
        }
        let mean = |v: Vec<f64>| -> f64 {
            let v: Vec<f64> = v.into_iter().filter(|x| x.is_finite()).collect();
            if v.is_empty() {
                f64::NAN
            } else {
                v.iter().sum::<f64>() / v.len() as f64
            }
        };
        let g = mean(
            (0..2)
                .map(|a| band(&r.freqs, &r.gyro[a].current_db, &r.gyro[a].logged_db, lo, hi))
                .collect(),
        );
        let d = mean(
            (0..2)
                .filter_map(|a| r.dterm[a].as_ref())
                .map(|d| band(&r.freqs, &d.current_db, &d.logged_db, lo, hi))
                .collect(),
        );
        errs.insert(format!("gyro_{lo:.0}_{nominal_hi:.0}"), num(g));
        errs.insert(format!("d_{lo:.0}_{nominal_hi:.0}"), num(d));
    }
    errs
}

fn session_row(lg: &FlightLog, f: &Path, root: &Path, engine: Engine) -> Result<Option<Value>, String> {
    let s = &lg.settings;
    let Some(sel) = analysed(lg) else { return Ok(None) };
    let t = Instant::now();
    let Some(r) = before_after_with(lg, sel.clone(), s, s, 300.0, engine)? else {
        return Ok(None);
    };
    let secs = t.elapsed().as_secs_f64();
    let errs = errors(&r, lg.fs);
    // gaz de la période (%) : la coupure du LPF1 D dynamique les suit, et l'ordre filtrer / dériver ne compte qu'alors (W-072)
    let mut thr: Vec<f64> = lg.throttle[sel.clone()].iter().map(|&v| v as f64).collect();
    let (thr_p10, thr_p90) = (percentile(&mut thr, 0.1), percentile(&mut thr, 0.9));
    let relf = rel(f, root);
    Ok(Some(json!({
        "file": relf,
        "sess": lg.session,
        "drone": drone_id(&s.craft_name, &relf),
        "family": s.family(),
        "debug": s.debug_mode,
        "log_hz": num(s.log_rate_hz()),
        "pid_hz": num(s.pid_rate_hz()),
        "seconds": num(secs),
        "d_lpf1_dynamic": s.dterm_lpf1.dynamic(),
        "throttle_p10": num(thr_p10),
        "throttle_p90": num(thr_p90),
        "errors": errs,
    })))
}

fn measure_file(f: &Path, root: &Path, engine: Engine) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        out.extend(session_row(&lg, f, root, engine)?);
    }
    Ok(out)
}

pub(super) fn percentile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

/// Tableau : par groupe (log à chaque boucle PID ou non, à 1,6 kHz ou plus ou non : D-057), pour chaque signal et chaque bande, médiane et 90e
/// centile de |rejeu actuel − log|, et moyenne signée (le rejeu filtre plus que le firmware si elle est négative).
fn resume_text(rows: &[Value]) -> String {
    let mut groups: BTreeMap<String, Vec<&Map<String, Value>>> = BTreeMap::new();
    for r in rows.iter().filter_map(Value::as_object) {
        let hz = r.get("log_hz").and_then(as_f64).unwrap_or(0.0);
        let pid = r.get("pid_hz").and_then(as_f64).unwrap_or(0.0);
        let key = format!(
            "{} ; {}",
            if hz >= pid * 0.999 {
                "chaque boucle"
            } else {
                "1/2 ou moins"
            },
            if hz >= 1600.0 { "≥ 1,6 kHz" } else { "< 1,6 kHz" }
        );
        groups.entry(key).or_default().push(r);
    }
    let mut out = String::from(
        "| Logs | Sessions | Signal, bande | |écart| médian | 90e centile | moyenne signée |\n|---|---|---|---|---|---|\n",
    );
    for (key, rs) in &groups {
        let mut cols: Vec<String> = rs
            .iter()
            .filter_map(|r| r.get("errors").and_then(Value::as_object))
            .flat_map(|e| e.keys().cloned())
            .collect();
        cols.sort();
        cols.dedup();
        for c in cols {
            let v: Vec<f64> = rs
                .iter()
                .filter_map(|r| r.get("errors")?.as_object()?.get(&c).and_then(as_f64))
                .filter(|x| x.is_finite())
                .collect();
            if v.is_empty() {
                continue;
            }
            let signed = v.iter().sum::<f64>() / v.len() as f64;
            let mut abs: Vec<f64> = v.iter().map(|x| x.abs()).collect();
            out.push_str(&format!(
                "| {key} | {} | {c} | {:.2} dB | {:.2} dB | {signed:+.2} dB |\n",
                v.len(),
                percentile(&mut abs, 0.5),
                percentile(&mut abs, 0.9)
            ));
        }
    }
    let mut secs: Vec<f64> = rows.iter().filter_map(|r| r.get("seconds").and_then(as_f64)).collect();
    out.push_str(&format!(
        "\nDurée du rejeu par session : médiane {:.2} s, maximum {:.2} s\n",
        percentile(&mut secs.clone(), 0.5),
        percentile(&mut secs, 1.0)
    ));
    out
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["-j", "--limit", "--dir", "--chain"], &["--resume"])?;
    let engine = Engine::parse(a.text("--chain").unwrap_or("model"))?;
    match (a.has("--resume"), a.positional.as_slice()) {
        (true, [path]) => {
            let t = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
            let rows = parse(&t)?;
            Ok(Output::ok(resume_text(rows.as_array().map_or(&[][..], |v| v))))
        }
        (false, [path]) => {
            let root = logs_root();
            let dir = a.text("--dir").map_or(root.clone(), |d| root.join(d));
            let mut files = log_files(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
            if let Some(n) = a.number("--limit")? {
                files.truncate(n);
            }
            let res = map_files(|f| measure_file(f, &root, engine), &files, a.number("-j")?, &root);
            let rows: Vec<Value> = res.into_iter().flat_map(|(_, rows)| rows).collect();
            std::fs::write(path, dump(&Value::Array(rows.clone()))?).map_err(|e| format!("{path} : {e}"))?;
            Ok(Output::ok(resume_text(&rows)))
        }
        _ => Err(super::USAGE.into()),
    }
}
