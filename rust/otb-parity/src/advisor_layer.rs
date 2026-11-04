//! Couche « assistant » des références (D-035, phase 5a) : `golden.py record-advisor` écrit un `.npz` par session analysée par
//! l'assistant (`DOSSIER/advisor/<fichier>.<session>.npz`, mêmes noms de clés que `golden.flatten`, `/` écrit `|`) ; ici chaque
//! session est rouverte par le décodeur Rust, refaite par `otb-advisor`, puis comparée **clé par clé**.
//!
//! Les textes (titres, détails, actions, explications) sont comparés au caractère près. Depuis la phase 5a-3c, `analyze` est complet :
//! **tous** les constats (`report|findings|…`, `report|sorted|…`) et toutes les mesures (`report|measures|…`) sont comparés. Depuis la
//! phase 5a-4, le diagnostic du bruit (`noise::noise_report`, clés `noise|…` : bande, niveaux par axe, harmoniques, constats rangés par partie,
//! blocs de filtres avec observations, réglages et verdicts, stratégie) l'est aussi, textes au caractère près ; une clé `noise|…` de Python
//! absente de Rust, ou l'inverse, est un échec. La réponse indicielle par séries (`step_tuning`, 5a-5) a sa propre couche, un fichier par
//! dossier (`tuning_layer.rs`, `DOSSIER/advisor_tuning/`) : une clé `tuning|…` dans un fichier de session est donc un échec ici. Seules les clés
//! d'identification (`_session`, `_n`, `_sel`) et les nombres de constats (comparés par la longueur des listes) sont comptées sans comparaison ;
//! toute autre clé de Python doit exister côté Rust.

use crate::analysis_layer::{DELAY_TOL, Flat, Item, SLIDERS_TOL, compare_item, put_flight_stats, put_opt, stem};
use crate::value::{Array, load_arrays};
use otb_advisor::noise::{self, AxisNoise, FilterBlock};
use otb_advisor::rules::{self, Finding};
use otb_analysis::period;
use otb_log::flightlog::FlightLog;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Préfixes des clés de Python déjà portées (une clé de Python qui commence par l'un d'eux doit exister côté Rust, et réciproquement).
/// Les constats (`report|findings|…`, `report|sorted|…`) ont leur propre règle : tous sont comparés. `noise|` : tout le diagnostic du bruit.
pub const PORTED: [&str; 9] = [
    "noise|",
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
pub(crate) fn analysed(lg: &FlightLog) -> Option<std::ops::Range<usize>> {
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
pub(crate) fn put_finding(f: &mut Flat, prefix: &str, x: &Finding) {
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

/// Niveaux de bruit d'une liste d'axes (`noise|gyro[i]|…`, `noise|dterm[i]|…`) sous les noms de Python (`vars(AxisNoise)`).
fn put_axes(f: &mut Flat, name: &str, v: &[AxisNoise]) {
    f.insert(format!("noise|n_{name}"), Item::I(v.len() as i64));
    for (i, x) in v.iter().enumerate() {
        let p = format!("noise|{name}[{i}]");
        f.insert(format!("{p}|axis"), Item::S(x.axis.clone()));
        f.insert(format!("{p}|before_db"), Item::T(x.before_db, NOISE_TOL, 1.0));
        f.insert(format!("{p}|after_db"), Item::T(x.after_db, NOISE_TOL, 1.0));
        f.insert(format!("{p}|peak_hz"), Item::F(x.peak_hz));
        f.insert(format!("{p}|reduction_db"), Item::T(x.reduction_db, NOISE_TOL, 1.0));
    }
}

/// `findings_list` de Python : `<prefix>|n`, puis `<prefix>|items[i]|…`.
fn put_findings(f: &mut Flat, prefix: &str, v: &[Finding]) {
    f.insert(format!("{prefix}|n"), Item::I(v.len() as i64));
    for (i, x) in v.iter().enumerate() {
        put_finding(f, &format!("{prefix}|items[{i}]"), x);
    }
}

/// `block_dict` de Python (`None` s'écrit « None »).
fn put_block(f: &mut Flat, prefix: &str, b: Option<&FilterBlock>) {
    let Some(b) = b else {
        f.insert(prefix.to_string(), Item::S("None".into()));
        return;
    };
    f.insert(format!("{prefix}|title"), Item::S(b.title.clone()));
    f.insert(format!("{prefix}|settings"), Item::S(b.settings.clone()));
    f.insert(format!("{prefix}|n_observations"), Item::I(b.observations.len() as i64));
    for (j, o) in b.observations.iter().enumerate() {
        f.insert(format!("{prefix}|observations[{j}]"), Item::S(o.clone()));
    }
    put_findings(f, &format!("{prefix}|findings"), &b.findings);
    match &b.verdict {
        Some(v) => put_finding(f, &format!("{prefix}|verdict"), v),
        None => {
            f.insert(format!("{prefix}|verdict"), Item::S("None".into()));
        }
    }
}

/// Le diagnostic du bruit sous les noms de Python (`noise_dict` de `scripts/golden_advisor.py`, aplati par `golden.flatten`).
fn put_noise(f: &mut Flat, nr: &noise::NoiseReport) {
    f.insert("noise|band[0]".into(), Item::F(nr.band.0));
    f.insert("noise|band[1]".into(), Item::F(nr.band.1));
    f.insert("noise|moving".into(), Item::B(nr.moving));
    f.insert("noise|has_unfiltered".into(), Item::B(nr.has_unfiltered));
    put_axes(f, "gyro", &nr.gyro);
    put_axes(f, "dterm", &nr.dterm);
    match &nr.harmonics {
        Some((raw, left)) => {
            // même grandeur que `motor_harmonic_*_db` (hauteur d'une harmonique au-dessus du bruit voisin, proche de 0 dB où `log10`
            // amplifie l'arrondi de la FFT) : même tolérance. **Mesuré** : 1,3e-12 au pire sur les 630 sessions (un élément dépasse
            // NOISE_TOL, faite pour les moyennes de bande et les sommets).
            f.insert("noise|harmonics[0]".into(), Item::TU(raw.clone(), FILTER_TOL, 1.0));
            f.insert("noise|harmonics[1]".into(), Item::TU(left.clone(), FILTER_TOL, 1.0));
        }
        None => {
            f.insert("noise|harmonics".into(), Item::S("None".into()));
        }
    }
    put_findings(f, "noise|conditions", &nr.conditions);
    put_findings(f, "noise|gyro_findings", &nr.gyro_findings);
    put_findings(f, "noise|dterm_findings", &nr.dterm_findings);
    put_findings(f, "noise|strategy", &nr.strategy);
    f.insert("noise|n_blocks".into(), Item::I(nr.gyro_blocks.len() as i64));
    for (i, b) in nr.gyro_blocks.iter().enumerate() {
        put_block(f, &format!("noise|gyro_blocks[{i}]"), Some(b));
    }
    put_findings(f, "noise|gyro_other", &nr.gyro_other);
    put_block(f, "noise|dterm_block", nr.dterm_block.as_ref());
}

/// Ce que produit l'assistant pour une session, sous les noms de Python, et le rapport lui-même (ses constats sont rangés à part).
fn ported(lg: &FlightLog, sel: std::ops::Range<usize>) -> Result<(Flat, rules::Report), String> {
    let mut f = Flat::new();
    // le diagnostic du bruit refait `analyze` (profil détecté) : son rapport est celui qu'on compare
    let nr = noise::noise_report(lg, Some(sel), None)?;
    let rep = nr.report.clone();
    put_noise(&mut f, &nr);
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

/// Clés d'identification de la session : les seules que la couche ne compare pas (elle les compte). Depuis 5a-5, plus aucune partie de
/// l'assistant n'est ignorée : la réponse indicielle est jugée par `tuning_layer.rs`.
fn is_identification(key: &str) -> bool {
    matches!(key, "_session" | "_n" | "_sel")
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
    /// Clés non comparées une à une : identification de la session et nombres de constats (comparés par la longueur des listes).
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
    if py.contains_key("noise|_error") {
        rep.failures.push(format!(
            "{who} : Python lève dans le diagnostic du bruit (cas non prévu)"
        ));
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
        } else if k.starts_with("report|") {
            // toute autre clé du rapport doit être classée (PORTED / PORTED_MEASURES) ET produite par Rust : jamais de saut silencieux
            if !(PORTED.iter().any(|p| k.starts_with(p)) || is_ported_measure(k)) {
                rep.failures.push(format!(
                    "{who} : {k} : clé du rapport de Python non classée parmi les clés portées"
                ));
            } else if !got.contains_key(k) {
                rep.failures.push(format!("{who} : {k} absente de Rust"));
            }
        } else if k.starts_with("noise|") {
            // tout le diagnostic du bruit est porté : une clé de Python que Rust n'écrit pas est un écart
            if !got.contains_key(k) {
                rep.failures.push(format!("{who} : {k} absente de Rust"));
            }
        } else if is_identification(k) {
            rep.skipped += 1;
        } else {
            rep.failures
                .push(format!("{who} : {k} : clé de Python inconnue de la couche"));
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
