//! Couche « assistant » des références (D-035, phase 5a) : `golden.py record-advisor` écrit un `.npz` par session analysée par
//! l'assistant (`DOSSIER/advisor/<fichier>.<session>.npz`, mêmes noms de clés que `golden.flatten`, `/` écrit `|`) ; ici chaque
//! session est rouverte par le décodeur Rust, refaite par `otb-advisor`, puis comparée **clé par clé**.
//!
//! Les textes (titres, détails, actions, explications) sont comparés au caractère près. Les clés pas encore portées sont ignorées
//! et comptées : [`PORTED`] liste les préfixes déjà rejoués, à étendre à chaque étape de la phase 5a.

use crate::analysis_layer::{Flat, Item, compare_item, stem};
use crate::value::{Array, load_arrays};
use otb_advisor::profiles;
use otb_analysis::period;
use otb_log::flightlog::FlightLog;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Préfixes des clés de Python déjà portées (une clé de Python qui commence par l'un d'eux doit exister côté Rust, et réciproquement).
pub const PORTED: [&str; 2] = ["report|profile", "report|reason"];

/// L'assistant analyse-t-il cette session ? (mêmes critères que `golden_advisor.advisor_record` : au moins 8 s, période d'au moins 5 s.)
fn analysed(lg: &FlightLog) -> Option<std::ops::Range<usize>> {
    if lg.duration() < 8.0 {
        return None;
    }
    let (t0, t1) = period::auto_period(lg, 8.0, 0.5).ok()?;
    let sel = lg.slice_time(t0, t1);
    (sel.end.saturating_sub(sel.start) as f64 >= lg.fs * 5.0).then_some(sel)
}

/// Ce que produit l'assistant pour une session, sous les noms de Python.
fn ported(lg: &FlightLog, _sel: std::ops::Range<usize>) -> Result<Flat, String> {
    let mut f = Flat::new();
    let (profile, reason) = profiles::detect_profile(lg);
    f.insert("report|profile".into(), Item::S(profile.key.into()));
    f.insert("report|reason".into(), Item::S(reason));
    Ok(f)
}

fn is_ported(key: &str) -> bool {
    PORTED.iter().any(|p| key.starts_with(p))
}

#[derive(Default)]
pub struct AdvisorReport {
    /// Sessions comparées.
    pub sessions: usize,
    /// Clés comparées.
    pub keys: usize,
    /// Clés de Python pas encore portées (ignorées).
    pub skipped: usize,
    /// Plus grand écart normalisé des valeurs comparées à une tolérance.
    pub worst: f64,
    pub failures: Vec<String>,
}

fn check_one(dossier: &Path, rel: &str, lg: &FlightLog, rep: &mut AdvisorReport) {
    let who = format!("{rel} session {}", lg.session);
    let file = dossier.join("advisor").join(stem(rel, lg.session as i64));
    let Some(sel) = analysed(lg) else {
        if file.exists() {
            rep.failures
                .push(format!("{who} : Python analyse cette session, pas Rust"));
        }
        return;
    };
    let py: HashMap<String, Array> = match load_arrays(&file) {
        Ok(a) => a,
        Err(e) => return rep.failures.push(format!("{who} : {e}")),
    };
    rep.sessions += 1;
    if py.contains_key("report|_error") {
        rep.failures
            .push(format!("{who} : Python lève dans l'assistant (cas non prévu)"));
        return;
    }
    let got = match ported(lg, sel) {
        Ok(g) => g,
        Err(e) => {
            return rep
                .failures
                .push(format!("{who} : Rust lève ({e}), Python rend un résultat"));
        }
    };
    for (k, item) in &got {
        match py.get(k) {
            None => rep.failures.push(format!("{who} : {k} absente de Python")),
            Some(w) => {
                rep.keys += 1;
                let mut e = 0.0;
                let m = compare_item(k, item, w, &mut e);
                rep.worst = rep.worst.max(e);
                if let Some(m) = m {
                    rep.failures.push(format!("{who} : {m}"));
                }
            }
        }
    }
    for k in py.keys() {
        if !is_ported(k) {
            rep.skipped += 1;
        } else if !got.contains_key(k) {
            rep.failures.push(format!("{who} : {k} absente de Rust"));
        }
    }
}

/// Compare les clés portées aux références `dossier/advisor/` (sessions listées par `dossier/flightlog.json`).
pub fn check_advisor(dossier: &Path, logs_root: &Path, threads: usize) -> Result<AdvisorReport, String> {
    let text = std::fs::read_to_string(dossier.join("flightlog.json")).map_err(|e| format!("flightlog.json : {e}"))?;
    let reference: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let files: Vec<&String> = reference.as_object().ok_or("flightlog.json")?.keys().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let report = std::sync::Mutex::new(AdvisorReport::default());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(rel) = files.get(i) else { break };
                    let mut local = AdvisorReport::default();
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
                    r.skipped += local.skipped;
                    r.worst = r.worst.max(local.worst);
                    r.failures.extend(local.failures);
                }
            });
        }
    });
    let report = report.into_inner().unwrap();
    // chaque référence doit avoir été rejouée : une session absente du côté Rust ne doit pas passer en silence
    let refs = std::fs::read_dir(dossier.join("advisor"))
        .map_err(|e| format!("advisor/ : {e}"))?
        .filter(|e| {
            e.as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "npz"))
        })
        .count();
    let mut report = report;
    if refs != report.sessions {
        report.failures.push(format!(
            "{refs} références dans advisor/, {} sessions rejouées",
            report.sessions
        ));
    }
    Ok(report)
}
