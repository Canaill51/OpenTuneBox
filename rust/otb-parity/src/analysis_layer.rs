//! Couche « analyses » des références (D-035, phase 4c) : `golden.py record-analysis` écrit un `.npz` par session du corpus
//! (`DOSSIER/analysis/<fichier>.<session>.npz`) ; ici chaque session est rouverte par le décodeur Rust et refaite par les
//! modules portés, puis comparée **clé par clé** (mêmes noms que `golden.flatten`, `/` écrit `|`).
//!
//! Les sections non encore portées (`delay`, `motors`, `step`, `series`…) sont ignorées et comptées ; une section en erreur côté
//! Python (`<section>|_error`) aussi. Une clé présente d'un côté seulement est un écart.

use crate::value::{Array, load_arrays};
use otb_analysis::{datasets, period, stats};
use otb_log::flightlog::FlightLog;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Clone)]
enum Item {
    F(f64),
    I(i64),
    B(bool),
    S(String),
    A(Vec<f64>),
}

type Flat = BTreeMap<String, Item>;

/// Sections de la couche déjà portées en Rust (les autres sont ignorées).
pub const PORTED: [&str; 3] = ["period", "datasets", "stats"];

fn put_opt(f: &mut Flat, key: &str, v: Option<f64>) {
    f.insert(key.to_string(), v.map_or(Item::S("None".into()), Item::F));
}

fn put_ms(f: &mut Flat, key: &str, m: &stats::MeanSd) {
    f.insert(format!("{key}.mean"), Item::F(m.mean));
    f.insert(format!("{key}.sd"), Item::F(m.sd));
    f.insert(format!("{key}.max"), Item::F(m.max));
}

fn put_opt_ms(f: &mut Flat, key: &str, m: &Option<stats::MeanSd>) {
    match m {
        Some(m) => put_ms(f, key, m),
        None => {
            f.insert(key.to_string(), Item::S("None".into()));
        }
    }
}

fn put_pair(f: &mut Flat, key: &str, v: &Option<(f64, f64)>) {
    match v {
        Some((a, b)) => {
            f.insert(format!("{key}[0]"), Item::F(*a));
            f.insert(format!("{key}[1]"), Item::F(*b));
        }
        None => {
            f.insert(key.to_string(), Item::S("None".into()));
        }
    }
}

/// Résultats des modules portés pour une session, sous les noms de Python.
fn ported(lg: &FlightLog) -> Result<Flat, String> {
    let mut f = Flat::new();
    // period
    let (t0, t1) = period::auto_period(lg, 8.0, 0.5)?;
    let sel = lg.slice_time(t0, t1);
    f.insert("period|auto_period[0]".into(), Item::F(t0));
    f.insert("period|auto_period[1]".into(), Item::F(t1));
    put_opt(&mut f, "period|crash_time", period::crash_time(lg, 300.0, 60.0));
    let parts = period::flight_parts(lg, t0, t1, 1.0, period::MOVING_SETPOINT_DPS)?;
    put_pair(&mut f, "period|flight_parts|hover", &parts.hover);
    put_pair(&mut f, "period|flight_parts|moving", &parts.moving);
    f.insert("period|sel[0]".into(), Item::I(sel.start as i64));
    f.insert("period|sel[1]".into(), Item::I(sel.end as i64));
    // datasets
    for (i, k) in datasets::available(lg).iter().enumerate() {
        f.insert(format!("datasets|available[{i}]"), Item::S((*k).to_string()));
    }
    for (i, (key, _)) in datasets::DATASETS.iter().enumerate() {
        for ax in 0..3 {
            f.insert(
                format!("datasets|has_axis[{i}][{ax}]"),
                Item::B(datasets::has_axis(lg, key, ax)?),
            );
        }
    }
    // stats
    let s = stats::flight_stats(lg, sel.clone());
    f.insert("stats|flight.duration_s".into(), Item::F(s.duration_s));
    for (name, centres, hist) in &s.stick_hist {
        f.insert(format!("stats|flight.stick_hist|{name}[0]"), Item::A(centres.clone()));
        f.insert(format!("stats|flight.stick_hist|{name}[1]"), Item::A(hist.clone()));
    }
    for (name, v) in &s.terms {
        for (a, m) in v.iter().enumerate() {
            put_ms(&mut f, &format!("stats|flight.terms|{name}[{a}]"), m);
        }
    }
    for (k, m) in s.motors.iter().enumerate() {
        put_ms(&mut f, &format!("stats|flight.motors[{k}]"), m);
    }
    f.insert(
        "stats|flight.motor_saturation_pct".into(),
        Item::F(s.motor_saturation_pct),
    );
    f.insert(
        "stats|flight.motor_imbalance_pct".into(),
        Item::F(s.motor_imbalance_pct),
    );
    put_ms(&mut f, "stats|flight.throttle", &s.throttle);
    put_opt_ms(&mut f, "stats|flight.vbat", &s.vbat);
    put_opt(&mut f, "stats|flight.vbat_min", s.vbat_min);
    put_opt_ms(&mut f, "stats|flight.current", &s.current);
    put_opt(&mut f, "stats|flight.mah", s.mah);
    let e = stats::pid_error_stats(lg, sel, 100.0, 50.0);
    f.insert("stats|pid_error.edges".into(), Item::A(e.edges.clone()));
    f.insert("stats|pid_error.stick_bins".into(), Item::A(e.stick_bins.clone()));
    for a in 0..3 {
        f.insert(format!("stats|pid_error.hist[{a}]"), Item::A(e.hist[a].clone()));
        f.insert(format!("stats|pid_error.sd[{a}]"), Item::F(e.sd[a]));
        f.insert(format!("stats|pid_error.mean_abs[{a}]"), Item::F(e.mean_abs[a]));
        f.insert(
            format!("stats|pid_error.err_vs_stick[{a}]"),
            Item::A(e.err_vs_stick[a].clone()),
        );
    }
    Ok(f)
}

fn same_bits(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn compare_item(key: &str, got: &Item, want: &Array) -> Option<String> {
    let bad = |what: String| Some(format!("{key} : {what}"));
    match got {
        Item::F(g) => {
            if want.kind != 'f' || want.size != 8 || want.len() != 1 {
                return bad(format!("Rust {g:e}, Python {}{} ×{}", want.kind, want.size, want.len()));
            }
            let w = want.get_f64(0);
            (!same_bits(*g, w)).then(|| format!("{key} : Rust {g:e} / Python {w:e}"))
        }
        Item::I(g) => {
            let ok = matches!(want.kind, 'i' | 'u') && want.len() == 1 && want.get_f64(0) == *g as f64;
            (!ok).then(|| format!("{key} : Rust {g} / Python {}", want.get_f64(0)))
        }
        Item::B(g) => {
            let ok = want.kind == 'b' && want.len() == 1 && (want.get_f64(0) != 0.0) == *g;
            (!ok).then(|| format!("{key} : Rust {g}"))
        }
        Item::S(g) => (want.as_string().as_ref() != Some(g))
            .then(|| format!("{key} : Rust {g:?} / Python {:?}", want.as_string())),
        Item::A(g) => {
            if want.kind != 'f' || want.size != 8 || want.len() != g.len() {
                return bad(format!(
                    "{} valeurs (Python {}{} ×{})",
                    g.len(),
                    want.kind,
                    want.size,
                    want.len()
                ));
            }
            (0..g.len())
                .find(|&i| !same_bits(g[i], want.get_f64(i)))
                .map(|i| format!("{key} : élément {i} : Rust {:e} / Python {:e}", g[i], want.get_f64(i)))
        }
    }
}

/// Section d'une clé : le texte avant le premier `|`, `.` ou `[`.
fn section_of(key: &str) -> &str {
    key.split(['|', '.', '[']).next().unwrap_or(key)
}

pub struct AnalysisReport {
    /// Sessions comparées.
    pub sessions: usize,
    /// Clés comparées au bit près.
    pub keys: usize,
    /// Sections de Python en erreur (non comparées).
    pub python_errors: usize,
    pub failures: Vec<String>,
}

fn stem(rel: &str, session: i64) -> String {
    format!("{}.{session}.npz", rel.replace('/', "__"))
}

fn check_one(dossier: &Path, rel: &str, lg: &FlightLog, rep: &mut AnalysisReport) {
    let file = dossier.join("analysis").join(stem(rel, lg.session as i64));
    let py: HashMap<String, Array> = match load_arrays(&file) {
        Ok(a) => a,
        Err(e) => return rep.failures.push(format!("{rel} session {} : {e}", lg.session)),
    };
    let got = match ported(lg) {
        Ok(g) => g,
        Err(e) => {
            return rep
                .failures
                .push(format!("{rel} session {} : Rust échoue : {e}", lg.session));
        }
    };
    let broken: Vec<&str> = PORTED
        .iter()
        .copied()
        .filter(|s| py.contains_key(&format!("{s}|_error")))
        .collect();
    rep.python_errors += broken.len();
    rep.sessions += 1;
    for (k, item) in &got {
        if broken.contains(&section_of(k)) {
            continue;
        }
        match py.get(&k.replace('/', "|")) {
            None => rep
                .failures
                .push(format!("{rel} session {} : {k} absente de Python", lg.session)),
            Some(w) => {
                rep.keys += 1;
                if let Some(m) = compare_item(k, item, w) {
                    rep.failures.push(format!("{rel} session {} : {m}", lg.session));
                }
            }
        }
    }
    for k in py.keys() {
        let s = section_of(k);
        if PORTED.contains(&s) && !broken.contains(&s) && !k.ends_with("|_error") && !got.contains_key(k) {
            rep.failures
                .push(format!("{rel} session {} : {k} absente de Rust", lg.session));
        }
    }
}

/// Compare les sections portées aux références `dossier/analysis/` (sessions listées par `dossier/flightlog.json`).
pub fn check_analysis(dossier: &Path, logs_root: &Path, threads: usize) -> Result<AnalysisReport, String> {
    let text = std::fs::read_to_string(dossier.join("flightlog.json")).map_err(|e| format!("flightlog.json : {e}"))?;
    let reference: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let files: Vec<&String> = reference.as_object().ok_or("flightlog.json")?.keys().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let report = std::sync::Mutex::new(AnalysisReport {
        sessions: 0,
        keys: 0,
        python_errors: 0,
        failures: vec![],
    });
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(rel) = files.get(i) else { break };
                    let mut local = AnalysisReport {
                        sessions: 0,
                        keys: 0,
                        python_errors: 0,
                        failures: vec![],
                    };
                    match otb_log::flightlog::load(&logs_root.join(rel)) {
                        Err(e) => local.failures.push(format!("{rel} : lecture : {e}")),
                        Ok(logs) => {
                            for lg in &logs {
                                check_one(dossier, rel, lg, &mut local);
                            }
                        }
                    }
                    let mut r = report.lock().unwrap();
                    r.sessions += local.sessions;
                    r.keys += local.keys;
                    r.python_errors += local.python_errors;
                    r.failures.extend(local.failures);
                }
            });
        }
    });
    Ok(report.into_inner().unwrap())
}
