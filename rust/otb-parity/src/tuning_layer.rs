//! Couche « réponse indicielle par séries » des références (D-035, phase 5a-5) : `golden.py record-tuning` écrit un `.npz` par dossier
//! (`DOSSIER/advisor_tuning/<dossier>.npz`, `Cedric.npz` pour le premier niveau, `Cedric__Air65.npz` pour un dossier de drone ; mêmes noms
//! de clés que `golden.flatten`, `/` écrit `|`, racine `tuning|…`).
//!
//! Ici les fichiers que chaque référence déclare (`tuning|files[i]`, chemins triés) sont rouverts par le décodeur Rust, chaque session que
//! l'assistant analyse est refaite par `step_series::flight_step` (déjà prouvé identique au Python par la couche `analysis`), les vols sont
//! rangés dans l'ordre de Python (sessions dans l'ordre du log, nom = `FlightLog::name`), passent par `step_tuning::step_tuning` et sont
//! comparés **clé par clé** :
//! conditions, blocs (titre, série, lignes, constat, vol conseillé), textes au caractère près. Python qui plante (`tuning|_error`) ⇒ Rust
//! doit planter aussi, et réciproquement ; chaque référence doit être rejouée.

use crate::advisor_layer::{analysed, put_finding};
use crate::analysis_layer::{Flat, Item, compare_item};
use crate::value::{Array, load_arrays};
use otb_advisor::step_tuning::{self, TuningReport};
use otb_analysis::step_series::{FlightStep, flight_step};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Mutex;

/// Une session d'un fichier : son numéro et son vol, ou l'erreur de `flight_step`.
type Session = (i64, Result<FlightStep, String>);

#[derive(Default)]
pub struct TuningLayerReport {
    /// Dossiers rejoués (premier niveau et dossiers de drone).
    pub groups: usize,
    /// Dossiers où Python (et Rust) lève.
    pub errors_replayed: usize,
    /// Sessions rangées dans les dossiers du premier niveau.
    pub sessions: usize,
    /// Clés comparées.
    pub keys: usize,
    /// Dossiers qui produisent au moins un bloc de série (Damping, Pitch D., Master, Pitch T. ou Feedforward).
    pub with_series: usize,
    /// Blocs de série, par curseur.
    pub series: BTreeMap<String, usize>,
    /// Constats comparés (conditions et constats de blocs), par clé.
    pub findings: BTreeMap<String, usize>,
    pub failures: Vec<String>,
}

/// Dossiers d'un log : son dossier du premier niveau (`Cedric`) et, plus bas, son dossier de drone (`Cedric/Air65`) —
/// `golden_advisor.tuning_groups`.
fn groups_of(rel: &str) -> Vec<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut out = Vec::new();
    if parts.len() > 1 && parts[0] != "golden" {
        out.push(parts[0].to_string());
        if parts.len() > 2 {
            out.push(format!("{}/{}", parts[0], parts[1]));
        }
    }
    out
}

/// `tuning_dict` de `scripts/golden_advisor.py`, aplati comme `golden.flatten` (`/` écrit `|`).
fn flat_of(files: &[&str], flights: &[(&str, i64, &FlightStep)], rep: &TuningReport) -> Flat {
    let mut f = Flat::new();
    let s = |v: &str| Item::S(v.to_string());
    f.insert("tuning|n_flights".into(), Item::I(flights.len() as i64));
    for (i, r) in files.iter().enumerate() {
        f.insert(format!("tuning|files[{i}]"), s(r));
    }
    for (i, (file, session, fl)) in flights.iter().enumerate() {
        let p = format!("tuning|flights[{i}]");
        f.insert(format!("{p}|file"), s(file));
        f.insert(format!("{p}|label"), s(&fl.label));
        f.insert(format!("{p}|session"), Item::I(*session));
        for (j, n) in fl.n.iter().enumerate() {
            f.insert(format!("{p}|n[{j}]"), Item::I(*n as i64));
        }
    }
    f.insert("tuning|conditions|n".into(), Item::I(rep.conditions.len() as i64));
    for (i, c) in rep.conditions.iter().enumerate() {
        put_finding(&mut f, &format!("tuning|conditions|items[{i}]"), c);
    }
    f.insert("tuning|n_blocks".into(), Item::I(rep.blocks.len() as i64));
    for (i, b) in rep.blocks.iter().enumerate() {
        let p = format!("tuning|blocks[{i}]");
        f.insert(format!("{p}|title"), s(&b.title));
        match &b.series {
            None => {
                f.insert(format!("{p}|series"), s("None"));
            }
            Some(sr) => {
                f.insert(format!("{p}|series|key"), s(sr.key));
                for (j, v) in sr.flights.iter().enumerate() {
                    f.insert(format!("{p}|series|flights[{j}]"), Item::I(*v as i64));
                }
                for (j, v) in sr.values.iter().enumerate() {
                    f.insert(format!("{p}|series|values[{j}]"), Item::F(*v));
                }
            }
        }
        f.insert(format!("{p}|n_lines"), Item::I(b.lines.len() as i64));
        for (j, l) in b.lines.iter().enumerate() {
            f.insert(format!("{p}|lines[{j}]"), s(l));
        }
        match &b.finding {
            None => {
                f.insert(format!("{p}|finding"), s("None"));
            }
            Some(x) => put_finding(&mut f, &format!("{p}|finding"), x),
        }
        match b.best {
            None => {
                f.insert(format!("{p}|best"), s("None"));
            }
            Some(v) => {
                f.insert(format!("{p}|best"), Item::I(v as i64));
            }
        }
    }
    f
}

/// Les vols de chaque fichier (sessions analysées par l'assistant), calculés en parallèle ; `loops` est vidé comme côté Python.
fn compute_flights(
    files: &[String],
    logs_root: &Path,
    threads: usize,
    failures: &mut Vec<String>,
) -> BTreeMap<String, Vec<Session>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Mutex<BTreeMap<String, Vec<Session>>> = Mutex::new(BTreeMap::new());
    let fails: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(rel) = files.get(i) else { break };
                    match otb_log::flightlog::load(&logs_root.join(rel)) {
                        Err(e) => fails.lock().unwrap().push(format!("{rel} : lecture : {e}")),
                        Ok(logs) => {
                            let mut v: Vec<Session> = Vec::new();
                            for lg in &logs {
                                let Some(sel) = analysed(lg) else { continue };
                                let r = flight_step(lg, sel, &lg.name()).map(|mut f| {
                                    f.loops.clear();
                                    f
                                });
                                v.push((lg.session as i64, r));
                            }
                            out.lock().unwrap().insert(rel.clone(), v);
                        }
                    }
                }
            });
        }
    });
    failures.extend(fails.into_inner().unwrap());
    out.into_inner().unwrap()
}

/// Fichiers d'une référence (`tuning|files[i]`, dans l'ordre de Python).
fn files_of(py: &HashMap<String, Array>) -> Vec<String> {
    (0..)
        .map_while(|i| py.get(&format!("tuning|files[{i}]")).and_then(Array::as_string))
        .collect()
}

/// Rejoue la réponse indicielle par séries de chaque dossier et la compare aux références `dossier/advisor_tuning/`.
///
/// Les fichiers d'un dossier sont ceux que sa référence déclare (`tuning|files[i]`) : la liste de `flightlog.json` peut être plus ancienne
/// que le corpus. Elle sert à vérifier que toute session analysée y est rangée, dans chacun de ses dossiers (une référence supprimée ou
/// jamais écrite est un échec).
pub fn check_tuning(dossier: &Path, logs_root: &Path, threads: usize) -> Result<TuningLayerReport, String> {
    let text = std::fs::read_to_string(dossier.join("flightlog.json")).map_err(|e| format!("flightlog.json : {e}"))?;
    let reference: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let known: Vec<String> = reference.as_object().ok_or("flightlog.json")?.keys().cloned().collect();
    let dir = dossier.join("advisor_tuning");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("advisor_tuning/ : {e}"))?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".npz"))
        .collect();
    names.sort();
    let mut rep = TuningLayerReport::default();
    let mut failures = Vec::new();
    let mut refs: BTreeMap<String, HashMap<String, Array>> = BTreeMap::new();
    for n in &names {
        match load_arrays(&dir.join(n)) {
            Ok(a) => {
                refs.insert(n.clone(), a);
            }
            Err(e) => failures.push(format!("{n} : {e}")),
        }
    }
    if refs.is_empty() {
        failures.push("advisor_tuning/ : aucune référence".into());
    }
    // tous les fichiers de `flightlog.json` et tous ceux que les références déclarent, calculés une fois
    let mut files: Vec<String> = refs.values().flat_map(files_of).chain(known.iter().cloned()).collect();
    files.sort();
    files.dedup();
    let by_file = compute_flights(&files, logs_root, threads, &mut failures);
    // toute session analysée de `flightlog.json` est rangée dans la référence de chacun de ses dossiers (premier niveau et drone)
    for rel in &known {
        let analysed_here = by_file.get(rel).is_some_and(|v| !v.is_empty());
        if !analysed_here {
            continue;
        }
        for g in groups_of(rel) {
            let n = format!("{}.npz", g.replace('/', "__"));
            let listed = refs.get(&n).is_some_and(|a| files_of(a).contains(rel));
            if !listed {
                failures.push(format!("{rel} : session analysée absente de la référence {n}"));
            }
        }
    }
    for (name, py) in &refs {
        let g = name.trim_end_matches(".npz").replace("__", "/");
        let rels = files_of(py);
        if rels.is_empty() {
            failures.push(format!("{name} : aucun fichier déclaré (tuning|files)"));
            continue;
        }
        let mut items: Vec<(&str, i64, &Result<FlightStep, String>)> = Vec::new();
        for rel in &rels {
            if !groups_of(rel).contains(&g) {
                failures.push(format!("{name} : {rel} n'appartient pas à ce dossier"));
            }
            if let Some(v) = by_file.get(rel) {
                items.extend(v.iter().map(|(s, r)| (rel.as_str(), *s, r)));
            }
        }
        rep.groups += 1;
        if !g.contains('/') {
            rep.sessions += items.len();
        }
        let py_error = py.contains_key("tuning|_error");
        // Rust : une erreur de `flight_step` ou de `step_tuning` ; Python : `tuning|_error` dans les deux cas
        let mut flights: Vec<FlightStep> = Vec::new();
        let mut flight_error: Option<String> = None;
        for (rel, s, r) in &items {
            match r {
                Ok(f) => flights.push(f.clone()),
                Err(e) => {
                    flight_error.get_or_insert(format!("{rel} session {s} : {e}"));
                }
            }
        }
        let tuned = match flight_error {
            Some(e) => Err(e),
            None => step_tuning::step_tuning(&flights),
        };
        let rust_report = match (tuned, py_error) {
            (Err(_), true) => {
                rep.errors_replayed += 1;
                continue;
            }
            (Err(e), false) => {
                failures.push(format!("{g} : Rust lève ({e}), Python rend un résultat"));
                continue;
            }
            (Ok(_), true) => {
                failures.push(format!("{g} : Python lève, Rust rend un résultat"));
                continue;
            }
            (Ok(r), false) => r,
        };
        let listed: Vec<(&str, i64, &FlightStep)> = items
            .iter()
            .zip(&flights)
            .map(|((rel, s, _), f)| (*rel, *s, f))
            .collect();
        let rel_refs: Vec<&str> = rels.iter().map(String::as_str).collect();
        let got = flat_of(&rel_refs, &listed, &rust_report);
        for (k, item) in &got {
            match py.get(k) {
                None => failures.push(format!("{g} : {k} absente de Python")),
                Some(w) => {
                    rep.keys += 1;
                    let mut e = 0.0;
                    if let Some(m) = compare_item(k, item, w, &mut e) {
                        failures.push(format!("{g} : {m}"));
                    }
                }
            }
        }
        for k in py.keys() {
            if !got.contains_key(k) {
                failures.push(format!("{g} : {k} absente de Rust"));
            }
        }
        if !rust_report.blocks.iter().all(|b| b.series.is_none()) {
            rep.with_series += 1;
        }
        for b in &rust_report.blocks {
            if let Some(sr) = &b.series {
                *rep.series.entry(sr.key.to_string()).or_insert(0) += 1;
            }
        }
        for f in rust_report
            .conditions
            .iter()
            .chain(rust_report.blocks.iter().filter_map(|b| b.finding.as_ref()))
        {
            *rep.findings.entry(f.key.clone()).or_insert(0) += 1;
        }
    }
    rep.failures = failures;
    Ok(rep)
}
