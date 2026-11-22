//! `otb-dev corpus stages` : contrôle du modèle de filtres étage par étage sur le corpus (lot 22a du plan,
//! `otb_analysis::filter_stages`). Un passage écrit les mesures brutes (une ligne par session, étage et axe, avec ses
//! bandes) ; `--resume` en fait le tableau par étage, version du firmware et type de filtre.
//!
//! Période : celle de l'assistant (`analysed`). Logs sous 1,6 kHz sautés (D-057).

use super::json::{as_f64, dump, num, parse};
use super::{Args, Output, analysed, drone_id, log_files, logs_root, map_files, rel};
use otb_analysis::filter_stages::{MIN_LOG_RATE_HZ, Stage, check_stages};
use otb_log::flightlog::{FlightLog, load};
use otb_log::settings::{Lowpass, Settings};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

/// Écart (dB) au-delà duquel le plan demande de s'arrêter, sous [`ALERT_MAX_HZ`], sur un log à chaque boucle PID.
const ALERT_DB: f64 = 1.0;
const ALERT_MAX_HZ: f64 = 250.0;

fn lpf(lp: &Lowpass) -> String {
    if !lp.enabled() {
        "off".into()
    } else if lp.dynamic() {
        format!("{} dyn", lp.kind)
    } else {
        lp.kind.into()
    }
}

/// Types des filtres que l'étage contient (pour grouper les écarts par type de filtre).
fn filters(stage: Stage, s: &Settings) -> String {
    let notches = s.gyro_notches.iter().take(2).filter(|(c, _)| *c > 0).count();
    match stage {
        Stage::Clock => "-".into(),
        Stage::Lpf2 => lpf(&s.gyro_lpf2),
        Stage::Rpm => format!("rpm×{}", s.rpm_harmonics),
        Stage::StaticLpf1 => format!("{} + notch×{notches}", lpf(&s.gyro_lpf1)),
        Stage::GyroBlock => format!(
            "{} / rpm×{} / {} + notch×{notches}",
            lpf(&s.gyro_lpf2),
            s.rpm_harmonics,
            lpf(&s.gyro_lpf1)
        ),
        Stage::DynNotch => format!("dyn×{}", s.dyn_notch_count),
        Stage::Dterm => format!(
            "{} / {} / notch {}",
            lpf(&s.dterm_lpf1),
            lpf(&s.dterm_lpf2),
            if s.dterm_notch.0 > 0 { "on" } else { "off" }
        ),
    }
}

fn session_rows(lg: &FlightLog, f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let s = &lg.settings;
    let Some(sel) = analysed(lg) else { return Ok(Vec::new()) };
    let relf = rel(f, root);
    let base = json!({
        "file": relf,
        "sess": lg.session,
        "drone": drone_id(&s.craft_name, &relf),
        "version": s.version.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("."),
        "family": s.family(),
        "debug": lg.debug_name(),
        "gyro_hz": num(s.gyro_rate_hz()),
        "pid_hz": num(s.pid_rate_hz()),
        "log_hz": num(s.log_rate_hz()),
        "measured_hz": num(lg.fs),
        "high_res": s.high_resolution,
    });
    let mut rows = Vec::new();
    if s.log_rate_hz() < MIN_LOG_RATE_HZ {
        return Ok(rows);
    }
    let res = check_stages(lg, sel)?;
    for c in &res.checks {
        let mut r = base.clone();
        let m = r.as_object_mut().expect("objet");
        m.insert("stage".into(), c.stage.key().into());
        m.insert("filters".into(), filters(c.stage, s).into());
        m.insert("axis".into(), c.axis.into());
        m.insert("clock_ms".into(), num(c.clock_offset_ms));
        m.insert("gain".into(), num(c.gain_factor));
        m.insert("windows".into(), c.windows.into());
        m.insert("residual".into(), num(c.residual));
        let bands: Vec<Value> = c
            .bands
            .iter()
            .map(|b| {
                json!({
                    "lo": num(b.lo_hz), "hi": num(b.hi_hz), "bins": b.bins,
                    "meas_db": num(b.gain_measured_db), "model_db": num(b.gain_model_db),
                    "err_db": num(b.error_db), "delay_ms": num(b.delay_ms),
                    "coh": num(b.coherence), "residual": num(b.residual), "ok": b.conclusive,
                })
            })
            .collect();
        m.insert("bands".into(), Value::Array(bands));
        rows.push(r);
    }
    for (st, why) in &res.skipped {
        let mut r = base.clone();
        let m = r.as_object_mut().expect("objet");
        m.insert("stage".into(), st.key().into());
        m.insert("skipped".into(), why.as_str().into());
        rows.push(r);
    }
    Ok(rows)
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

fn text(r: &Map<String, Value>, k: &str) -> String {
    match r.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// Tableau : pour chaque (étage, famille, cadence complète ou non, filtres), et chaque bande, la médiane et le maximum de
/// |écart| sur les bandes concluantes ; puis les alertes du critère d'arrêt et les raisons des étages sautés.
fn resume_text(rows: &[Value]) -> String {
    type Key = (String, String, bool, String);
    /// Par bande : |écarts| et retards des bandes concluantes, nombre total de bandes.
    type Bands = BTreeMap<(i64, i64), (Vec<f64>, Vec<f64>, usize)>;
    let mut groups: BTreeMap<Key, Bands> = BTreeMap::new();
    let mut sessions: BTreeMap<Key, usize> = BTreeMap::new();
    let mut alerts = Vec::new();
    let mut skipped: BTreeMap<(String, String), usize> = BTreeMap::new();
    for r in rows.iter().filter_map(Value::as_object) {
        let stage = text(r, "stage");
        if let Some(why) = r.get("skipped").and_then(Value::as_str) {
            *skipped.entry((stage, why.to_string())).or_default() += 1;
            continue;
        }
        let (pid, log) = (
            r.get("pid_hz").and_then(as_f64).unwrap_or(0.0),
            r.get("log_hz").and_then(as_f64).unwrap_or(0.0),
        );
        let full = log >= pid * 0.999;
        let key = (stage.clone(), text(r, "family"), full, text(r, "filters"));
        *sessions.entry(key.clone()).or_default() += 1;
        let g = groups.entry(key).or_default();
        for b in r.get("bands").and_then(Value::as_array).into_iter().flatten() {
            let get = |k: &str| b.get(k).and_then(as_f64).unwrap_or(f64::NAN);
            let (lo, hi, err, delay) = (get("lo"), get("hi"), get("err_db"), get("delay_ms"));
            let e = g.entry((lo as i64, hi as i64)).or_default();
            e.2 += 1;
            if b.get("ok").and_then(Value::as_bool) == Some(true) && err.is_finite() {
                e.0.push(err.abs());
                if delay.is_finite() {
                    e.1.push(delay);
                }
                if full && hi <= ALERT_MAX_HZ && err.abs() > ALERT_DB {
                    alerts.push(format!(
                        "{} #{} {} axe {} {lo:.0}–{hi:.0} Hz : {err:+.2} dB",
                        text(r, "file"),
                        text(r, "sess"),
                        stage,
                        text(r, "axis")
                    ));
                }
            }
        }
    }
    let mut out = String::from(
        "étage | famille | chaque boucle PID | filtres | n | bande Hz : médiane |écart| dB (max ; retard médian ms ; bandes concluantes/total)\n",
    );
    for (key, bands) in &groups {
        let mut line = format!(
            "{} | {} | {} | {} | {}",
            key.0,
            key.1,
            if key.2 { "oui" } else { "non" },
            key.3,
            sessions[key]
        );
        for ((lo, hi), (errs, delays, total)) in bands {
            let mut e = errs.clone();
            let max = e.iter().copied().fold(f64::NAN, f64::max);
            let med = median(&mut e);
            let dl = median(&mut delays.clone());
            line += &format!(" | {lo}–{hi} : {med:.2} ({max:.2} ; {dl:.3} ; {}/{total})", errs.len());
        }
        out += &line;
        out.push('\n');
    }
    out += &format!(
        "\nAlertes (|écart| > {ALERT_DB} dB sous {ALERT_MAX_HZ} Hz, log à chaque boucle PID, bande concluante) : {}\n",
        alerts.len()
    );
    for a in &alerts {
        out += a;
        out.push('\n');
    }
    out += "\nÉtages sautés :\n";
    for ((st, why), n) in &skipped {
        out += &format!("{st} : {why} ({n})\n");
    }
    out
}

/// `corpus stages SORTIE.json [--dir SOUS-DOSSIER] [--limit N] [-j N]` ; `corpus stages --resume SORTIE.json`.
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
