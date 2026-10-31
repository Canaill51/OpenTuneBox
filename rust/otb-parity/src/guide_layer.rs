//! Couche « guide » des références (D-035, phase 5b-4) : `golden.py record-guide` écrit un `.npz` par session
//! (`DOSSIER/guide/<fichier>.<session>.npz`, mêmes noms de clés que `golden.flatten`, `/` écrit `|`) ; ici chaque session est rouverte par le
//! décodeur Rust, passée par `otb-advisor::log_check` (`check_log` rapide et exact, `filter_targets`), puis comparée **clé par clé**.
//!
//! Les textes (titres, détails, corrections, commandes) sont comparés au caractère près. Toute clé de Python doit exister côté Rust, et
//! réciproquement. Une section où Python lève (`<section>|_error`) doit faire lever Rust aussi.

use crate::advisor_layer::NOISE_TOL;
use crate::analysis_layer::{DELAY_TOL, Flat, Item, SLIDERS_TOL, compare_item, stem};
use crate::value::{Array, load_arrays};
use otb_advisor::log_check::{self, LogCheck};
use otb_advisor::profiles::detect_profile;
use otb_log::flightlog::FlightLog;
use std::collections::HashMap;
use std::path::Path;

fn s(v: &str) -> Item {
    Item::S(v.to_string())
}

fn put_pair(f: &mut Flat, key: &str, p: Option<(f64, f64)>) {
    match p {
        Some((a, b)) => {
            f.insert(format!("{key}[0]"), Item::F(a));
            f.insert(format!("{key}[1]"), Item::F(b));
        }
        None => {
            f.insert(key.to_string(), s("None"));
        }
    }
}

fn put_list(f: &mut Flat, key: &str, v: &[String]) {
    for (i, x) in v.iter().enumerate() {
        f.insert(format!("{key}[{i}]"), s(x));
    }
}

/// `check_dict` de `scripts/golden_guide.py` : tout ce que rend `check_log`, sous les noms de Python.
fn put_check(f: &mut Flat, p: &str, ck: &LogCheck, fs: f64) {
    f.insert(format!("{p}|label"), s(&ck.label));
    f.insert(format!("{p}|moving"), Item::B(ck.moving));
    put_pair(f, &format!("{p}|hover"), ck.hover);
    put_pair(f, &format!("{p}|moving_part"), ck.moving_part);
    f.insert(format!("{p}|n_items"), Item::I(ck.items.len() as i64));
    for (i, it) in ck.items.iter().enumerate() {
        let k = format!("{p}|items[{i}]");
        f.insert(format!("{k}|key"), s(&it.key));
        f.insert(format!("{k}|title"), s(&it.title));
        f.insert(format!("{k}|status"), s(it.status));
        f.insert(format!("{k}|detail"), s(&it.detail));
        f.insert(format!("{k}|n_fixes"), Item::I(it.fixes.len() as i64));
        for (j, fx) in it.fixes.iter().enumerate() {
            f.insert(format!("{k}|fixes[{j}]|where"), s(&fx.where_));
            f.insert(format!("{k}|fixes[{j}]|cli"), s(&fx.cli));
        }
    }
    f.insert(format!("{p}|n_usages"), Item::I(ck.usages.len() as i64));
    for (i, u) in ck.usages.iter().enumerate() {
        let k = format!("{p}|usages[{i}]");
        f.insert(format!("{k}|key"), s(&u.key));
        f.insert(format!("{k}|label"), s(&u.label));
        f.insert(format!("{k}|ok"), Item::B(u.ok));
        f.insert(format!("{k}|reason"), s(&u.reason));
        f.insert(format!("{k}|shown"), Item::B(u.shown));
    }
    f.insert(format!("{p}|n_windows"), Item::I(ck.windows.len() as i64));
    for (i, w) in ck.windows.iter().enumerate() {
        f.insert(format!("{p}|windows[{i}]"), Item::I(*w as i64));
    }
    f.insert(format!("{p}|exact"), Item::B(ck.exact));
    for (k, v) in &ck.sliders {
        f.insert(format!("{p}|sliders|{k}"), Item::T(*v, SLIDERS_TOL, 1.0));
    }
    f.insert(format!("{p}|role"), s(ck.role));
    f.insert(format!("{p}|n_sp_delay"), Item::I(ck.sp_delay_ms.len() as i64));
    for (i, v) in ck.sp_delay_ms.iter().enumerate() {
        f.insert(format!("{p}|sp_delay[{i}]"), Item::T(*v, DELAY_TOL, 1000.0 / fs));
    }
    for (name, role) in [
        ("None", None),
        ("filters", Some(log_check::FILTERS)),
        ("pid", Some(log_check::PID)),
    ] {
        let items: Vec<String> = ck.items_for(role).iter().map(|i| i.key.clone()).collect();
        put_list(f, &format!("{p}|items_for|{name}"), &items);
        let usages: Vec<String> = ck.usages_for(role).iter().map(|u| u.key.clone()).collect();
        put_list(f, &format!("{p}|usages_for|{name}"), &usages);
    }
    let cli = ck.cli_commands();
    f.insert(format!("{p}|n_cli"), Item::I(cli.len() as i64));
    put_list(f, &format!("{p}|cli"), &cli);
}

/// `targets_dict` de `scripts/golden_guide.py`.
fn put_targets(f: &mut Flat, p: &str, ft: Option<&log_check::FilterTargets>, fs: f64) {
    let Some(ft) = ft else {
        f.insert(p.to_string(), s("None"));
        return;
    };
    f.insert(format!("{p}|gyro_ptb_db"), Item::T(ft.gyro_ptb_db, NOISE_TOL, 1.0));
    f.insert(format!("{p}|dterm_ptb_db"), Item::T(ft.dterm_ptb_db, NOISE_TOL, 1.0));
    f.insert(
        format!("{p}|gyro_delay_ms"),
        Item::T(ft.gyro_delay_ms, DELAY_TOL, 1000.0 / fs),
    );
    f.insert(
        format!("{p}|dterm_delay_ms"),
        Item::T(ft.dterm_delay_ms, DELAY_TOL, 1000.0 / fs),
    );
    put_pair(f, &format!("{p}|goal"), ft.delay_goal_ms);
    let checks = ft.checks();
    f.insert(format!("{p}|n_checks"), Item::I(checks.len() as i64));
    for (i, (name, ok, text)) in checks.iter().enumerate() {
        f.insert(format!("{p}|checks[{i}]|name"), s(name));
        f.insert(format!("{p}|checks[{i}]|ok"), ok.map_or_else(|| s("None"), Item::B));
        f.insert(format!("{p}|checks[{i}]|text"), s(text));
    }
    f.insert(format!("{p}|reached"), ft.reached().map_or_else(|| s("None"), Item::B));
}

/// Ce que Rust produit pour une session : `(section, résultat)`, dans l'ordre de `guide_record` de Python.
fn ported(lg: &FlightLog) -> Vec<(&'static str, Result<Flat, String>)> {
    let fs = lg.fs;
    let name = lg.name();
    let one = |exact: bool, section: &'static str| {
        let mut f = Flat::new();
        log_check::check_log(lg, None, &name, exact).map(|ck| {
            put_check(&mut f, section, &ck, fs);
            f
        })
    };
    let targets = || {
        let ck = log_check::check_log(lg, None, &name, false)?;
        let inch = detect_profile(lg).0.nominal_inch;
        let ft = log_check::filter_targets(lg, ck.hover, ck.moving_part, inch)?;
        let mut f = Flat::new();
        put_targets(&mut f, "targets", ft.as_ref(), fs);
        Ok(f)
    };
    vec![
        ("check", one(false, "check")),
        ("check_exact", one(true, "check_exact")),
        ("targets", targets()),
    ]
}

#[derive(Default)]
pub struct GuideReport {
    /// Sessions comparées.
    pub sessions: usize,
    /// Clés comparées.
    pub keys: usize,
    /// Sections où Python et Rust lèvent tous les deux.
    pub python_errors: usize,
    /// Plus grand écart normalisé des valeurs comparées à une tolérance.
    pub worst: f64,
    pub failures: Vec<String>,
}

impl GuideReport {
    fn merge(&mut self, o: GuideReport) {
        self.sessions += o.sessions;
        self.keys += o.keys;
        self.python_errors += o.python_errors;
        self.worst = self.worst.max(o.worst);
        self.failures.extend(o.failures);
    }
}

fn check_one(dossier: &Path, rel: &str, lg: &FlightLog, rep: &mut GuideReport) {
    let who = format!("{rel} session {}", lg.session);
    let file = dossier.join("guide").join(stem(rel, lg.session as i64));
    let py: HashMap<String, Array> = match load_arrays(&file) {
        Ok(a) => a,
        Err(e) => return rep.failures.push(format!("{who} : {e}")),
    };
    rep.sessions += 1;
    for (section, result) in ported(lg) {
        // Python a levé dans cette section : Rust doit lever aussi
        if py.contains_key(&format!("{section}|_error")) {
            rep.python_errors += 1;
            if result.is_ok() {
                rep.failures
                    .push(format!("{who} : {section} : Python lève, Rust rend un résultat"));
            }
            continue;
        }
        let got = match result {
            Ok(g) => g,
            Err(e) => {
                rep.failures
                    .push(format!("{who} : {section} : Rust lève ({e}), Python rend un résultat"));
                continue;
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
        let prefix = format!("{section}|");
        for k in py.keys() {
            // `check|…` ne doit pas capter `check_exact|…` : le séparateur fait partie du préfixe
            if (k.starts_with(&prefix) || k == section) && !got.contains_key(k) {
                rep.failures.push(format!("{who} : {k} absente de Rust"));
            }
        }
    }
    for k in py.keys() {
        let known = ["check|", "check_exact|", "targets|"].iter().any(|p| k.starts_with(p))
            || k == "targets"
            || matches!(k.as_str(), "_session" | "_n");
        if !known {
            rep.failures
                .push(format!("{who} : {k} : clé de Python inconnue de la couche"));
        }
    }
}

/// Compare les sessions aux références `dossier/guide/` (fichiers listés par `dossier/flightlog.json`).
pub fn check_guide(dossier: &Path, logs_root: &Path, threads: usize) -> Result<GuideReport, String> {
    let text = std::fs::read_to_string(dossier.join("flightlog.json")).map_err(|e| format!("flightlog.json : {e}"))?;
    let reference: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let files: Vec<&String> = reference.as_object().ok_or("flightlog.json")?.keys().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let report = std::sync::Mutex::new(GuideReport::default());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(rel) = files.get(i) else { break };
                    let mut local = GuideReport::default();
                    match otb_log::flightlog::load(&logs_root.join(rel)) {
                        Err(e) => local.failures.push(format!("{rel} : lecture : {e}")),
                        Ok(logs) => {
                            for lg in &logs {
                                check_one(dossier, rel, lg, &mut local);
                            }
                        }
                    }
                    report.lock().unwrap().merge(local);
                }
            });
        }
    });
    let mut report = report.into_inner().unwrap();
    // chaque référence doit avoir été rejouée : une session absente du côté Rust ne doit pas passer en silence
    let refs = std::fs::read_dir(dossier.join("guide"))
        .map_err(|e| format!("guide/ : {e}"))?
        .filter(|e| {
            e.as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "npz"))
        })
        .count();
    if refs != report.sessions {
        report.failures.push(format!(
            "{refs} références dans guide/, {} sessions rejouées",
            report.sessions
        ));
    }
    Ok(report)
}
