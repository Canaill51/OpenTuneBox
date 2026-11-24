//! `otb-dev corpus chaindiff` : écart entre la chaîne de filtres actuelle et celle à l'arithmétique du firmware
//! (`otb_analysis::firmware_chain`, lot 24c du plan, D-062), session par session, **avant** de brancher quoi que ce soit.
//! Les deux rejouent les réglages du log sur son gyro non filtré (`filter_replay::chain_gap`) ; on compare leurs spectres
//! exacts (sans l'arrondi du Blackbox, L-108), gyro et terme D, roulis et tangage, par bande, et l'écart dans le temps.
//! Une ligne par session ; `--resume` en fait le tableau par version et selon que le log enregistre chaque boucle PID.
//!
//! Période : celle de l'assistant (`analysed`). Positif : la chaîne du firmware laisse passer plus que l'actuelle.

use super::json::{as_f64, dump, num, parse};
use super::{Args, Output, analysed, drone_id, log_files, logs_root, map_files, rel};
use otb_analysis::filter_replay::chain_gap;
use otb_analysis::firmware_chain::Release;
use otb_log::flightlog::{FlightLog, load};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

/// Bandes comparées (Hz) ; la dernière s'arrête à 0,45 × cadence du log.
const BANDS: [(f64, f64); 4] = [(5.0, 20.0), (20.0, 100.0), (100.0, 250.0), (250.0, 500.0)];

/// Moyenne de `b − a` (dB) sur [`lo`, `hi`[ et plus grand |b − a| ; NaN sans fréquence dans la bande.
fn band(f: &[f64], a: &[f64], b: &[f64], lo: f64, hi: f64) -> (f64, f64) {
    let v: Vec<f64> = f
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(x, _)| **x >= lo && **x < hi)
        .map(|(_, (p, q))| q - p)
        .collect();
    if v.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    (
        v.iter().sum::<f64>() / v.len() as f64,
        v.iter().map(|x| x.abs()).fold(0.0, f64::max),
    )
}

fn mean(v: &[f64]) -> f64 {
    let v: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn session_row(lg: &FlightLog, f: &Path, root: &Path) -> Result<Option<Value>, String> {
    let s = &lg.settings;
    let Some(sel) = analysed(lg) else { return Ok(None) };
    if lg.gyro_unfilt.is_none() {
        return Ok(None);
    }
    let relf = rel(f, root);
    let mut row = json!({
        "file": relf,
        "sess": lg.session,
        "drone": drone_id(&s.craft_name, &relf),
        "version": s.version.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("."),
        "release": Release::of(s).map_or("avant 4.3".to_string(), |r| format!("{r:?}")),
        "log_hz": num(s.log_rate_hz()),
        "pid_hz": num(s.pid_rate_hz()),
        "gyro_hz": num(s.gyro_rate_hz()),
    });
    let m = row.as_object_mut().expect("objet");
    if Release::of(s).is_none() {
        m.insert("skipped".into(), "version d'avant la 4.3".into());
        return Ok(Some(row));
    }
    let t = Instant::now();
    let Some(g) = chain_gap(lg, sel, 300.0)? else {
        return Ok(None);
    };
    m.insert("seconds".into(), num(t.elapsed().as_secs_f64()));
    let top = 0.45 * lg.fs;
    let mut gaps = Map::new();
    for (lo, nominal_hi) in BANDS {
        let hi = nominal_hi.min(top);
        if hi <= lo {
            continue;
        }
        let gy: Vec<(f64, f64)> = g
            .gyro_db
            .iter()
            .take(2)
            .map(|(a, b)| band(&g.freqs, a, b, lo, hi))
            .collect();
        let d: Vec<(f64, f64)> = g
            .dterm_db
            .iter()
            .take(2)
            .flatten()
            .map(|(a, b)| band(&g.freqs, a, b, lo, hi))
            .collect();
        let key = |sig: &str| format!("{sig}_{lo:.0}_{nominal_hi:.0}");
        gaps.insert(key("gyro"), num(mean(&gy.iter().map(|x| x.0).collect::<Vec<_>>())));
        gaps.insert(key("gyro_max"), num(gy.iter().map(|x| x.1).fold(f64::NAN, f64::max)));
        gaps.insert(key("d"), num(mean(&d.iter().map(|x| x.0).collect::<Vec<_>>())));
        gaps.insert(key("d_max"), num(d.iter().map(|x| x.1).fold(f64::NAN, f64::max)));
    }
    m.insert("gaps".into(), Value::Object(gaps));
    m.insert(
        "gyro_rel_rms".into(),
        num(mean(&g.gyro_rel_rms[..g.gyro_rel_rms.len().min(2)])),
    );
    Ok(Some(row))
}

fn measure_file(f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        out.extend(session_row(&lg, f, root)?);
    }
    Ok(out)
}

fn percentile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

/// Tableau : par groupe (version, chaque boucle PID ou non), pour chaque signal et bande, |écart| médian, 90e centile et
/// maximum, et moyenne signée ; puis l'écart dans le temps et les sessions aux plus grands écarts sous 250 Hz.
fn resume_text(rows: &[Value]) -> String {
    let mut groups: BTreeMap<String, Vec<&Map<String, Value>>> = BTreeMap::new();
    let mut skipped = 0;
    for r in rows.iter().filter_map(Value::as_object) {
        if r.contains_key("skipped") {
            skipped += 1;
            continue;
        }
        let hz = r.get("log_hz").and_then(as_f64).unwrap_or(0.0);
        let pid = r.get("pid_hz").and_then(as_f64).unwrap_or(0.0);
        let key = format!(
            "{} ; {}",
            r.get("release").and_then(Value::as_str).unwrap_or("?"),
            if hz >= pid * 0.999 {
                "chaque boucle"
            } else {
                "1/2 ou moins"
            }
        );
        groups.entry(key).or_default().push(r);
    }
    let mut out = String::from(
        "| Logs | Sessions | Signal, bande | |écart| médian | 90e centile | maximum | moyenne signée |\n|---|---|---|---|---|---|---|\n",
    );
    for (key, rs) in &groups {
        let mut cols: Vec<String> = rs
            .iter()
            .filter_map(|r| r.get("gaps").and_then(Value::as_object))
            .flat_map(|e| e.keys().cloned())
            .filter(|k| !k.contains("_max"))
            .collect();
        cols.sort();
        cols.dedup();
        for c in cols {
            let v: Vec<f64> = rs
                .iter()
                .filter_map(|r| r.get("gaps")?.as_object()?.get(&c).and_then(as_f64))
                .filter(|x| x.is_finite())
                .collect();
            if v.is_empty() {
                continue;
            }
            let signed = v.iter().sum::<f64>() / v.len() as f64;
            let mut abs: Vec<f64> = v.iter().map(|x| x.abs()).collect();
            out.push_str(&format!(
                "| {key} | {} | {c} | {:.3} dB | {:.3} dB | {:.3} dB | {signed:+.3} dB |\n",
                v.len(),
                percentile(&mut abs.clone(), 0.5),
                percentile(&mut abs.clone(), 0.9),
                percentile(&mut abs, 1.0)
            ));
        }
        let mut rms: Vec<f64> = rs
            .iter()
            .filter_map(|r| r.get("gyro_rel_rms").and_then(as_f64))
            .filter(|x| x.is_finite())
            .collect();
        if !rms.is_empty() {
            out.push_str(&format!(
                "| {key} | {} | gyro, écart efficace / signal | {:.2e} | {:.2e} | {:.2e} | |\n",
                rms.len(),
                percentile(&mut rms.clone(), 0.5),
                percentile(&mut rms.clone(), 0.9),
                percentile(&mut rms, 1.0)
            ));
        }
    }
    // sessions aux plus grands écarts moyens sous 250 Hz
    let mut worst: Vec<(f64, String)> = rows
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|r| {
            let g = r.get("gaps")?.as_object()?;
            let w = g
                .iter()
                .filter(|(k, _)| !k.contains("_max") && !k.ends_with("_500"))
                .filter_map(|(_, v)| as_f64(v))
                .filter(|x| x.is_finite())
                .map(f64::abs)
                .fold(0.0, f64::max);
            let file = r.get("file")?.as_str()?;
            let sess = r.get("sess").map(|v| v.to_string()).unwrap_or_default();
            let release = r.get("release").and_then(Value::as_str).unwrap_or("?");
            Some((w, format!("{file} #{sess} ({release})")))
        })
        .collect();
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.push_str("\nPlus grands écarts moyens sous 250 Hz :\n");
    for (w, what) in worst.iter().take(10) {
        out.push_str(&format!("- {w:.3} dB : {what}\n"));
    }
    let mut secs: Vec<f64> = rows.iter().filter_map(|r| r.get("seconds").and_then(as_f64)).collect();
    out.push_str(&format!(
        "\nSessions d'avant la 4.3 (non comparées) : {skipped}\nDurée des deux rejeux par session : médiane {:.2} s, maximum {:.2} s\n",
        percentile(&mut secs.clone(), 0.5),
        percentile(&mut secs, 1.0)
    ));
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
