//! Couche « assistant » des références (D-035, phase 5a) : `golden.py record-advisor` écrit un `.npz` par session analysée par
//! l'assistant (`DOSSIER/advisor/<fichier>.<session>.npz`, mêmes noms de clés que `golden.flatten`, `/` écrit `|`) ; ici chaque
//! session est rouverte par le décodeur Rust, refaite par `otb-advisor`, puis comparée **clé par clé**.
//!
//! Les textes (titres, détails, actions, explications) sont comparés au caractère près. Depuis la phase 5a-3c, `analyze` est complet :
//! **tous** les constats (`report|findings|…`, `report|sorted|…`) et toutes les mesures (`report|measures|…`) sont comparés. Seules les
//! clés d'une autre partie de l'assistant (`noise|…`, `tuning|…`, phases suivantes) sont ignorées et comptées.

use crate::analysis_layer::{DELAY_TOL, Flat, Item, SLIDERS_TOL, compare_item, put_flight_stats, put_opt, stem};
use crate::value::{Array, load_arrays};
use otb_advisor::rules::{self, Finding};
use otb_analysis::period;
use otb_log::flightlog::FlightLog;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Préfixes des clés de Python déjà portées (une clé de Python qui commence par l'un d'eux doit exister côté Rust, et réciproquement).
/// Les constats (`report|findings|…`, `report|sorted|…`) ont leur propre règle : tous sont comparés.
pub const PORTED: [&str; 8] = [
    "report|profile",
    "report|reason",
    "report|period",
    "report|steps[",
    "report|measures|hover_motor_hz",
    "report|measures|log_rate_needed_hz",
    "report|measures|aliased",
    "report|measures|steps|",
];

/// Autres mesures portées en 5a-2b, puis en 5a-3b / 5a-3c (une seule liste pour que le résumé les nomme).
pub const PORTED_MEASURES: [&str; 15] = [
    "delays",
    "gyro_noise_db",
    "dterm_noise_db",
    "low_band_db",
    "gyro_peak_ptb_db",
    "dterm_peak_ptb_db",
    "stats",
    "pid_error_sd",
    "iterm_mean",
    "sliders",
    "hover_lines",
    "frame_resonance",
    "dyn_notch",
    "motor_harmonic_left_db",
    "motor_harmonic_raw_db",
];

/// Tolérance des mesures de bruit en dB (moyennes de bande, sommets lissés : spectres de `realfft` contre pocketfft, L-048), écart absolu
/// rapporté à max(|valeur|, 1). **Mesurée sur les 630 sessions** : 2,2e-14 au pire (sommets proches de 0 dB, où `log10` amplifie l'arrondi
/// de la FFT) ; échecs à 1e-14 (2 clés), aucun à 3e-14. Marge ×45. Une mauvaise bande ou un mauvais lissage dépasse 1e-3.
pub const NOISE_TOL: f64 = 1e-12;

/// Tolérance des mesures de `filter_rules` et du vol stationnaire (niveaux et largeurs de raies, excès de la raie fixe, Q nécessaire :
/// spectres de `realfft` contre pocketfft, L-048), écart absolu rapporté à max(|valeur|, 1). **Mesurée sur les 630 sessions** : 2,3e-13 au
/// pire (hauteur d'une raie dans le gyro filtré, proche de 0 dB où `log10` amplifie l'arrondi de la FFT). Marge ×43. Une mauvaise bande ou
/// un mauvais lissage dépasse 1e-6.
pub const FILTER_TOL: f64 = 1e-11;

/// L'assistant analyse-t-il cette session ? (mêmes critères que `golden_advisor.advisor_record` : au moins 8 s, période d'au moins 5 s.)
fn analysed(lg: &FlightLog) -> Option<std::ops::Range<usize>> {
    if lg.duration() < 8.0 {
        return None;
    }
    let (t0, t1) = period::auto_period(lg, 8.0, 0.5).ok()?;
    let sel = lg.slice_time(t0, t1);
    (sel.end.saturating_sub(sel.start) as f64 >= lg.fs * 5.0).then_some(sel)
}

fn put_list(f: &mut Flat, key: &str, v: &[f64], tol: f64) {
    for (i, x) in v.iter().enumerate() {
        f.insert(format!("{key}[{i}]"), Item::T(*x, tol, 1.0));
    }
}

/// Les champs d'un constat sous les noms de Python (`<prefix>|key`, `|actions[j]`…).
fn put_finding(f: &mut Flat, prefix: &str, x: &Finding) {
    let s = |v: &str| Item::S(v.to_string());
    f.insert(format!("{prefix}|key"), s(&x.key));
    f.insert(format!("{prefix}|category"), s(&x.category));
    f.insert(format!("{prefix}|severity"), s(&x.severity));
    f.insert(format!("{prefix}|title"), s(&x.title));
    f.insert(format!("{prefix}|detail"), s(&x.detail));
    f.insert(format!("{prefix}|axis"), s(x.axis.as_deref().unwrap_or("None")));
    f.insert(format!("{prefix}|n_actions"), Item::I(x.actions.len() as i64));
    for (j, t) in x.actions.iter().enumerate() {
        f.insert(format!("{prefix}|actions[{j}]"), s(t));
    }
    f.insert(format!("{prefix}|n_settings"), Item::I(x.settings.len() as i64));
    for (j, t) in x.settings.iter().enumerate() {
        f.insert(format!("{prefix}|settings[{j}]"), s(t));
    }
}

/// Mesures de `filter_rules` (5a-3c) et du vol stationnaire (5a-3b) sous les noms de Python. Une clé absente de Python (eRPM absent,
/// pas de vol stationnaire) est absente ici aussi : `None` n'insère rien, sauf `frame_resonance` et `dyn_notch` où `None` est la valeur
/// `None` de Python (écrite « None »). Une liste vide n'écrit aucune clé, des deux côtés.
fn put_filter_measures(f: &mut Flat, m: &rules::Measures) {
    let k = "report|measures";
    for (name, v) in [
        ("motor_harmonic_left_db", m.motor_harmonic_left_db),
        ("motor_harmonic_raw_db", m.motor_harmonic_raw_db),
    ] {
        if let Some(v) = v {
            f.insert(format!("{k}|{name}"), Item::T(v, FILTER_TOL, 1.0));
        }
    }
    match &m.frame_resonance {
        Some(r) => {
            f.insert(format!("{k}|frame_resonance[0]"), Item::F(r.freq_hz));
            f.insert(format!("{k}|frame_resonance[1]"), Item::T(r.excess_db, FILTER_TOL, 1.0));
            f.insert(format!("{k}|frame_resonance[2]"), Item::B(r.near_motor));
        }
        None => {
            f.insert(format!("{k}|frame_resonance"), Item::S("None".into()));
        }
    }
    match &m.dyn_notch {
        Some(d) => {
            f.insert(format!("{k}|dyn_notch|count"), Item::I(d.count));
            f.insert(format!("{k}|dyn_notch|min_hz"), Item::I(d.min_hz));
            f.insert(format!("{k}|dyn_notch|max_hz"), Item::I(d.max_hz));
            for (i, v) in d.peaks.iter().enumerate() {
                f.insert(format!("{k}|dyn_notch|peaks[{i}]"), Item::F(*v));
            }
            put_list(f, &format!("{k}|dyn_notch|widths"), &d.widths, FILTER_TOL);
            put_list(f, &format!("{k}|dyn_notch|heights"), &d.heights, FILTER_TOL);
            put_list(f, &format!("{k}|dyn_notch|left"), &d.left, FILTER_TOL);
            match d.q_need {
                Some(q) => f.insert(format!("{k}|dyn_notch|q_need"), Item::T(q, FILTER_TOL, 1.0)),
                None => f.insert(format!("{k}|dyn_notch|q_need"), Item::S("None".into())),
            };
        }
        None => {
            f.insert(format!("{k}|dyn_notch"), Item::S("None".into()));
        }
    }
    if let Some(lines) = &m.hover_lines {
        for (i, ln) in lines.iter().enumerate() {
            f.insert(format!("{k}|hover_lines[{i}].freq_hz"), Item::F(ln.freq_hz));
            f.insert(
                format!("{k}|hover_lines[{i}].raw_db"),
                Item::T(ln.raw_db, FILTER_TOL, 1.0),
            );
            f.insert(
                format!("{k}|hover_lines[{i}].left_db"),
                Item::T(ln.left_db, FILTER_TOL, 1.0),
            );
        }
    }
}

/// Ce que produit l'assistant pour une session, sous les noms de Python, et le rapport lui-même (ses constats sont rangés à part).
fn ported(lg: &FlightLog, sel: std::ops::Range<usize>) -> Result<(Flat, rules::Report), String> {
    let mut f = Flat::new();
    let rep = rules::analyze(lg, Some(sel), None)?;
    f.insert("report|profile".into(), Item::S(rep.profile.key.into()));
    f.insert("report|reason".into(), Item::S(rep.profile_reason.clone()));
    f.insert("report|period[0]".into(), Item::F(rep.period.0));
    f.insert("report|period[1]".into(), Item::F(rep.period.1));
    for (i, st) in rep.steps.iter().enumerate() {
        f.insert(format!("report|steps[{i}]"), Item::S(st.axis.clone()));
    }
    let m = &rep.measures;
    let miss = |what: &str| format!("mesure {what} absente du rapport");
    put_opt(&mut f, "report|measures|hover_motor_hz", m.hover_motor_hz);
    let (b, h) = m.log_rate_needed_hz.ok_or_else(|| miss("log_rate_needed_hz"))?;
    f.insert("report|measures|log_rate_needed_hz[0]".into(), Item::F(b));
    f.insert("report|measures|log_rate_needed_hz[1]".into(), Item::F(h));
    f.insert(
        "report|measures|aliased".into(),
        Item::B(m.aliased.ok_or_else(|| miss("aliased"))?),
    );
    for (axis, sm) in m.steps.as_ref().ok_or_else(|| miss("steps"))? {
        for (name, v) in [
            ("peak", sm.peak),
            ("peak_ms", sm.peak_ms),
            ("latency_ms", sm.latency_ms),
            ("rise_ms", sm.rise_ms),
            ("settling_ms", sm.settling_ms),
            ("overshoot_pct", sm.overshoot_pct),
            ("ripple", sm.ripple),
            ("ripple_std", sm.ripple_std),
            ("ripple_ms", sm.ripple_ms),
            ("waves", sm.waves),
            ("waves_ratio", sm.waves_ratio),
        ] {
            f.insert(format!("report|measures|steps|{axis}.{name}"), Item::F(v));
        }
    }
    let d = m.delays.as_ref().ok_or_else(|| miss("delays"))?;
    let unit = 1000.0 / lg.fs; // une période d'échantillonnage, en ms
    f.insert(
        "report|measures|delays.gyro_ms".into(),
        Item::T(d.gyro_ms, DELAY_TOL, unit),
    );
    f.insert(
        "report|measures|delays.dterm_ms".into(),
        Item::T(d.dterm_ms, DELAY_TOL, unit),
    );
    for (a, v) in d.sp_gyro_ms.iter().enumerate() {
        f.insert(
            format!("report|measures|delays.sp_gyro_ms[{a}]"),
            Item::T(*v, DELAY_TOL, unit),
        );
    }
    f.insert("report|measures|delays.stick_p95_dps".into(), Item::F(d.stick_p95_dps));
    for (name, v) in [
        ("gyro_noise_db", &m.gyro_noise_db),
        ("dterm_noise_db", &m.dterm_noise_db),
        ("low_band_db", &m.low_band_db),
    ] {
        put_list(
            &mut f,
            &format!("report|measures|{name}"),
            v.as_ref().ok_or_else(|| miss(name))?,
            NOISE_TOL,
        );
    }
    for (name, v) in [
        ("gyro_peak_ptb_db", m.gyro_peak_ptb_db),
        ("dterm_peak_ptb_db", m.dterm_peak_ptb_db),
    ] {
        f.insert(
            format!("report|measures|{name}"),
            Item::T(v.ok_or_else(|| miss(name))?, NOISE_TOL, 1.0),
        );
    }
    put_filter_measures(&mut f, m);
    put_flight_stats(
        &mut f,
        "report|measures|stats",
        m.stats.as_ref().ok_or_else(|| miss("stats"))?,
    );
    for (i, v) in m
        .pid_error_sd
        .as_ref()
        .ok_or_else(|| miss("pid_error_sd"))?
        .iter()
        .enumerate()
    {
        f.insert(format!("report|measures|pid_error_sd[{i}]"), Item::F(*v));
    }
    for (i, v) in m
        .iterm_mean
        .as_ref()
        .ok_or_else(|| miss("iterm_mean"))?
        .iter()
        .enumerate()
    {
        f.insert(format!("report|measures|iterm_mean[{i}]"), Item::F(*v));
    }
    let sv = m.sliders.as_ref().ok_or_else(|| miss("sliders"))?;
    for (k, v) in &sv.values {
        f.insert(
            format!("report|measures|sliders.values|{k}"),
            Item::T(*v, SLIDERS_TOL, 1.0),
        );
    }
    f.insert("report|measures|sliders.from_header".into(), Item::B(sv.from_header));
    f.insert("report|measures|sliders.consistent".into(), Item::B(sv.consistent));
    Ok((f, rep))
}

/// Préfixes de clés (`report|measures|…`) portés en plus de [`PORTED`].
fn is_ported_measure(key: &str) -> bool {
    key.strip_prefix("report|measures|")
        .is_some_and(|rest| PORTED_MEASURES.iter().any(|p| rest.starts_with(p)))
}

/// Mesure écrite par `put_filter_measures` ?
fn is_filter_measure(key: &str) -> bool {
    ["hover_lines", "frame_resonance", "dyn_notch", "motor_harmonic"]
        .iter()
        .any(|p| key.starts_with(&format!("report|measures|{p}")))
}

/// `report|<liste>|items[i]|…` : l'indice `i` d'une clé de constat de Python.
fn finding_index(key: &str, list: &str) -> Option<usize> {
    let rest = key.strip_prefix(&format!("report|{list}|items["))?;
    rest.split(']').next()?.parse().ok()
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
    /// Plus grand écart normalisé des seules mesures de `filter_rules` et du vol stationnaire (tolérance [`FILTER_TOL`]).
    pub worst_filter: f64,
    pub failures: Vec<String>,
    /// Constats comparés (tous ceux de Python contre ceux de Rust), par clé.
    pub compared_findings: BTreeMap<String, usize>,
}

impl AdvisorReport {
    fn merge(&mut self, o: AdvisorReport) {
        self.sessions += o.sessions;
        self.keys += o.keys;
        self.skipped += o.skipped;
        self.worst = self.worst.max(o.worst);
        self.worst_filter = self.worst_filter.max(o.worst_filter);
        self.failures.extend(o.failures);
        for (k, n) in o.compared_findings {
            *self.compared_findings.entry(k).or_insert(0) += n;
        }
    }
}

/// Indices (dans la liste `list` de Python) des constats : tous sont comparés ; la liste `findings` les compte par clé.
fn kept_indices(py: &HashMap<String, Array>, list: &str, rep: &mut AdvisorReport) -> Result<Vec<usize>, String> {
    let n = py
        .get(&format!("report|{list}|n"))
        .ok_or_else(|| format!("report|{list}|n absente de Python"))?
        .get_f64(0) as usize;
    let mut kept = Vec::new();
    for i in 0..n {
        let key = py
            .get(&format!("report|{list}|items[{i}]|key"))
            .and_then(Array::as_string)
            .ok_or_else(|| format!("report|{list}|items[{i}]|key illisible"))?;
        kept.push(i);
        if list == "findings" {
            *rep.compared_findings.entry(key).or_insert(0) += 1;
        }
    }
    Ok(kept)
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
    let (mut got, report) = match ported(lg, sel) {
        Ok(g) => g,
        Err(e) => {
            return rep
                .failures
                .push(format!("{who} : Rust lève ({e}), Python rend un résultat"));
        }
    };
    // constats : ceux de Python, dans l'ordre, contre ceux de Rust ; les éléments de Rust prennent l'indice de Python (mêmes noms de clés)
    let mut handled: Vec<String> = Vec::new();
    for (list, rust) in [("findings", report.findings.clone()), ("sorted", report.sorted())] {
        let kept = match kept_indices(&py, list, rep) {
            Ok(k) => k,
            Err(e) => return rep.failures.push(format!("{who} : {e}")),
        };
        if rust.len() != kept.len() {
            let keys = |it: Vec<&str>| it.join(", ");
            rep.failures.push(format!(
                "{who} : {list} : {} constats côté Rust ({}), {} côté Python",
                rust.len(),
                keys(rust.iter().map(|x| x.key.as_str()).collect()),
                kept.len()
            ));
            continue;
        }
        for (x, &pi) in rust.iter().zip(&kept) {
            let prefix = format!("report|{list}|items[{pi}]");
            put_finding(&mut got, &prefix, x);
            handled.push(format!("{prefix}|"));
        }
    }
    for (k, item) in &got {
        match py.get(k) {
            None => rep.failures.push(format!("{who} : {k} absente de Python")),
            Some(w) => {
                rep.keys += 1;
                let mut e = 0.0;
                let m = compare_item(k, item, w, &mut e);
                rep.worst = rep.worst.max(e);
                if is_filter_measure(k) {
                    rep.worst_filter = rep.worst_filter.max(e);
                }
                if let Some(m) = m {
                    rep.failures.push(format!("{who} : {m}"));
                }
            }
        }
    }
    for k in py.keys() {
        let in_findings = finding_index(k, "findings").is_some() || finding_index(k, "sorted").is_some();
        if in_findings {
            if handled.iter().any(|p| k.starts_with(p)) {
                if !got.contains_key(k) {
                    rep.failures.push(format!("{who} : {k} absente de Rust"));
                }
            } else {
                rep.skipped += 1;
            }
        } else if k == "report|findings|n" || k == "report|sorted|n" {
            rep.skipped += 1;
        } else if PORTED.iter().any(|p| k.starts_with(p)) || is_ported_measure(k) {
            if !got.contains_key(k) {
                rep.failures.push(format!("{who} : {k} absente de Rust"));
            }
        } else {
            rep.skipped += 1;
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
                    report.lock().unwrap().merge(local);
                }
            });
        }
    });
    let mut report = report.into_inner().unwrap();
    // chaque référence doit avoir été rejouée : une session absente du côté Rust ne doit pas passer en silence
    let refs = std::fs::read_dir(dossier.join("advisor"))
        .map_err(|e| format!("advisor/ : {e}"))?
        .filter(|e| {
            e.as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "npz"))
        })
        .count();
    if refs != report.sessions {
        report.failures.push(format!(
            "{refs} références dans advisor/, {} sessions rejouées",
            report.sessions
        ));
    }
    Ok(report)
}
