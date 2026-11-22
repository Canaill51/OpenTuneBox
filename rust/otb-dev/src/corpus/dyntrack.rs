//! `otb-dev corpus dyntrack` : suivi des pics du notch dynamique rejoué (`otb_analysis::dyn_notch`, lot 22c du plan)
//! comparé aux centres que le firmware enregistre dans le debug `FFT_FREQ`. Un passage écrit une ligne par session et par
//! notch ; `--resume` en fait le tableau par version du firmware et cadence du log.
//!
//! Période : celle de l'assistant (`analysed`).

use super::json::{as_f64, dump, num, parse};
use super::{Args, Output, analysed, drone_id, log_files, logs_root, map_files, rel};
use otb_analysis::dyn_notch::{CentreError, check_tracking};
use otb_log::flightlog::{FlightLog, load};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

fn err_json(e: &CentreError) -> Value {
    json!({
        "med": num(e.median_abs_hz), "p90": num(e.p90_abs_hz), "bias": num(e.median_hz), "bin": num(e.within_bin),
    })
}

fn session_rows(lg: &FlightLog, f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let s = &lg.settings;
    let Some(sel) = analysed(lg) else { return Ok(Vec::new()) };
    let Some(c) = check_tracking(lg, sel)? else {
        return Ok(Vec::new());
    };
    let relf = rel(f, root);
    let t = &c.tracker;
    let base = json!({
        "file": relf,
        "sess": lg.session,
        "drone": drone_id(&s.craft_name, &relf),
        "version": s.version.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("."),
        "family": s.family(),
        "pid_hz": num(s.pid_rate_hz()),
        "log_hz": num(s.log_rate_hz()),
        "axis": c.axis,
        "count": t.count,
        "range": format!("{}-{}", s.dyn_notch_min_hz, s.dyn_notch_max_hz),
        "q": s.dyn_notch_q,
        "sample_count": t.sample_count,
        "bin_hz": num(t.resolution_hz as f64),
        "floor": format!("{:?}", t.floor),
    });
    Ok(c.notches
        .iter()
        .map(|n| {
            let mut r = base.clone();
            let m = r.as_object_mut().expect("objet");
            m.insert("notch".into(), n.notch.into());
            m.insert("samples".into(), n.samples.into());
            m.insert("recorded_hz".into(), num(n.recorded_hz));
            m.insert("debug".into(), err_json(&n.from_debug));
            m.insert("model".into(), n.from_model.as_ref().map_or(Value::Null, err_json));
            m.insert("initial".into(), err_json(&n.initial));
            r
        })
        .collect())
}

fn measure_file(f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        out.extend(session_rows(&lg, f, root)?);
    }
    Ok(out)
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    if v.len() % 2 == 1 {
        v[m]
    } else {
        (v[m - 1] + v[m]) / 2.0
    }
}

fn get(r: &Map<String, Value>, src: &str, k: &str) -> Option<f64> {
    r.get(src)?.as_object()?.get(k).and_then(as_f64)
}

/// Tableau : par (famille, log à chaque boucle PID ou non), pour chaque source de centres (suivi depuis le debug, depuis
/// le modèle, centres initiaux), médiane sur les notchs de l'écart absolu médian, du 90e centile, et part à moins d'une case.
fn resume_text(rows: &[Value]) -> String {
    type Key = (String, bool);
    let mut groups: BTreeMap<Key, Vec<&Map<String, Value>>> = BTreeMap::new();
    for r in rows.iter().filter_map(Value::as_object) {
        let pid = r.get("pid_hz").and_then(as_f64).unwrap_or(0.0);
        let log = r.get("log_hz").and_then(as_f64).unwrap_or(0.0);
        let fam = r.get("family").and_then(Value::as_str).unwrap_or("?").to_string();
        groups.entry((fam, log >= pid * 0.999)).or_default().push(r);
    }
    let mut out = String::from(
        "| Famille | Log | Notchs (sessions) | Source | |écart| médian | 90e centile | à moins d'une case |\n|---|---|---|---|---|---|---|\n",
    );
    for ((fam, full), rs) in &groups {
        let sessions: std::collections::BTreeSet<(String, i64)> = rs
            .iter()
            .map(|r| {
                (
                    r.get("file").and_then(Value::as_str).unwrap_or("").to_string(),
                    r.get("sess").and_then(Value::as_i64).unwrap_or(0),
                )
            })
            .collect();
        for src in ["debug", "model", "initial"] {
            let col = |k: &str| -> (f64, usize) {
                let mut v: Vec<f64> = rs.iter().filter_map(|r| get(r, src, k)).collect();
                let n = v.len();
                (median(&mut v), n)
            };
            let ((med, n), (p90, _), (bin, _)) = (col("med"), col("p90"), col("bin"));
            if n == 0 {
                continue;
            }
            out.push_str(&format!(
                "| {fam} | {} | {n} ({}) | {src} | {med:.1} Hz | {p90:.1} Hz | {:.0} % |\n",
                if *full { "chaque boucle" } else { "1/2 ou moins" },
                sessions.len(),
                bin * 100.0
            ));
        }
    }
    out
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["-j", "--limit", "--dir"], &["--resume"])?;
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
            let res = map_files(|f| measure_file(f, &root), &files, a.number("-j")?, &root);
            let rows: Vec<Value> = res.into_iter().flat_map(|(_, rows)| rows).collect();
            std::fs::write(path, dump(&Value::Array(rows.clone()))?).map_err(|e| format!("{path} : {e}"))?;
            Ok(Output::ok(resume_text(&rows)))
        }
        _ => Err(super::USAGE.into()),
    }
}
