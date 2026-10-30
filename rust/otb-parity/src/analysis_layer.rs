//! Couche « analyses » des références (D-035, phase 4c) : `golden.py record-analysis` écrit un `.npz` par session du corpus
//! (`DOSSIER/analysis/<fichier>.<session>.npz`) ; ici chaque session est rouverte par le décodeur Rust et refaite par les
//! modules portés, puis comparée **clé par clé** (mêmes noms que `golden.flatten`, `/` écrit `|`).
//!
//! Les sections non encore portées (`delay`, `motors`, `step`, `series`…) sont ignorées et comptées ; une section en erreur côté
//! Python (`<section>|_error`) aussi. Une clé présente d'un côté seulement est un écart.

use crate::value::{Array, load_arrays};
use otb_analysis::{datasets, delay, motors, period, stats};
use otb_log::flightlog::{FlightLog, Rows, clamp_range};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Clone)]
enum Item {
    F(f64),
    /// Tableau `float32` (bits `f32`).
    A32(Vec<f32>),
    /// Tableau d'entiers.
    AI(Vec<i64>),
    /// Tableau de flottants comparé à une tolérance relative au plus grand élément fini de la référence.
    TA(Vec<f64>, f64),
    /// Tableau de flottants comparé **case par case**, en relatif à chaque valeur de référence (spectres : les petites cases comptent).
    TR(Vec<f64>, f64),
    /// Flottant comparé à une tolérance : `(valeur, tolérance, unité)` ; écart = |Rust − Python| / max(|Python|, unité).
    T(f64, f64, f64),
    I(i64),
    B(bool),
    S(String),
    A(Vec<f64>),
}

type Flat = BTreeMap<String, Item>;

/// Sections de la couche déjà portées en Rust (les autres sont ignorées).
pub const PORTED: [&str; 5] = ["period", "datasets", "stats", "delay", "motors"];

/// Tolérance des valeurs en dB et des spectres des fonctions de `motors` (FFT de `realfft`, `log10`, `hypot`, `pow` : L-048). **Mesurée sur
/// les 668 sessions** : 1,5e-11 au pire (cas écrits : 7,1e-12, écart-type de valeurs en dB, qui amplifie l'arrondi de la FFT). Marge ×70 ;
/// les erreurs de logique que l'on veut voir (mauvaise bande, mauvais seuil) dépassent 1e-6.
pub const MOTORS_TOL: f64 = 1e-9;

/// Nombre maximal de largeurs de raies exemptées (L-055) : une seule est attendue sur le corpus ; davantage est un écart à examiner.
pub const MAX_EXEMPT: usize = 1;

/// Tolérance des retards (FFT, `hypot`, `atan2`, produits complexes en FMA : L-048, L-051), relative à la valeur ou, si elle est
/// plus petite, à une période d'échantillonnage. **Mesurée sur les 668 sessions** : 2,6e-10 au pire (retard consigne → gyro du lacet
/// de `HORUS/stock.bbl`, pic d'intercorrélation plat : l'interpolation parabolique divise par `a − 2b + d`, différence de trois
/// valeurs voisines qui amplifie l'arrondi de la FFT). Marge ×40. Un saut d'indice du pic vaudrait au moins une période
/// d'échantillonnage (≥ 0,1 ms), six ordres de grandeur au-dessus : la tolérance le refuse.
pub const DELAY_TOL: f64 = 1e-8;

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

type Section = (&'static str, Result<Flat, String>);

/// Résultats des modules portés pour une session, par section et sous les noms de Python.
fn ported(lg: &FlightLog) -> Vec<Section> {
    let period = period_section(lg);
    // la période de vol ; si `auto_period` échoue, Python garde `slice(None)` pour les sections suivantes
    let sel = period::auto_period(lg, 8.0, 0.5)
        .map(|(t0, t1)| lg.slice_time(t0, t1))
        .unwrap_or(0..lg.n());
    vec![
        ("period", period),
        ("datasets", datasets_section(lg)),
        ("stats", Ok(stats_section(lg, sel.clone()))),
        ("delay", delay_section(lg, sel.clone())),
        ("motors", motors_section(lg, sel)),
    ]
}

fn period_section(lg: &FlightLog) -> Result<Flat, String> {
    let mut f = Flat::new();
    let (t0, t1) = period::auto_period(lg, 8.0, 0.5)?;
    let sel = lg.slice_time(t0, t1);
    f.insert("period|auto_period[0]".into(), Item::F(t0));
    f.insert("period|auto_period[1]".into(), Item::F(t1));
    put_opt(&mut f, "period|crash_time", period::crash_time(lg, 300.0, 60.0)?);
    let parts = period::flight_parts(lg, t0, t1, 1.0, period::MOVING_SETPOINT_DPS)?;
    put_pair(&mut f, "period|flight_parts|hover", &parts.hover);
    put_pair(&mut f, "period|flight_parts|moving", &parts.moving);
    f.insert("period|sel[0]".into(), Item::I(sel.start as i64));
    f.insert("period|sel[1]".into(), Item::I(sel.end as i64));
    Ok(f)
}

fn datasets_section(lg: &FlightLog) -> Result<Flat, String> {
    let mut f = Flat::new();
    for (i, k) in datasets::available(lg)?.iter().enumerate() {
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
    Ok(f)
}

fn delay_section(lg: &FlightLog, sel: std::ops::Range<usize>) -> Result<Flat, String> {
    let d = delay::compute_delays(lg, sel)?;
    let unit = 1000.0 / lg.fs; // une période d'échantillonnage, en ms
    let mut f = Flat::new();
    f.insert("delay.gyro_ms".into(), Item::T(d.gyro_ms, DELAY_TOL, unit));
    f.insert("delay.dterm_ms".into(), Item::T(d.dterm_ms, DELAY_TOL, unit));
    for (a, v) in d.sp_gyro_ms.iter().enumerate() {
        f.insert(format!("delay.sp_gyro_ms[{a}]"), Item::T(*v, DELAY_TOL, unit));
    }
    f.insert("delay.stick_p95_dps".into(), Item::F(d.stick_p95_dps));
    Ok(f)
}

fn put_arr(f: &mut Flat, key: &str, v: Option<Vec<f64>>) {
    f.insert(key.to_string(), v.map_or(Item::S("None".into()), Item::A));
}

/// Colonnes `sel` d'un tableau `(k, n)`.
fn sub_rows(r: &Rows, sel: &std::ops::Range<usize>) -> Rows {
    let mut out = Rows::zeros(r.k, sel.len());
    for k in 0..r.k {
        out.row_mut(k).copy_from_slice(&r.row(k)[sel.clone()]);
    }
    out
}

/// Section `motors` : ce que fait l'assistant avec la télémétrie eRPM (bruit moteur, harmoniques, cibles du notch dynamique, raies).
fn motors_section(lg: &FlightLog, sel: std::ops::Range<usize>) -> Result<Flat, String> {
    let mut f = Flat::new();
    let (mhz, est) = motors::motor_hz(lg, None);
    match &mhz {
        Some(m) => f.insert("motors|motor_hz".into(), Item::A32(m.data.clone())),
        None => f.insert("motors|motor_hz".into(), Item::S("None".into())),
    };
    f.insert("motors|estimated".into(), Item::B(est));
    put_opt(&mut f, "motors|hover", motors::hover_motor_hz(lg));
    put_arr(&mut f, "motors|dyn", motors::dyn_notch_centres(lg).map(|v| v.concat()));
    let (Some(mhz_log), Some(gu)) = (&lg.motor_hz, &lg.gyro_unfilt) else {
        return Ok(f);
    };
    let (st, fs) = (&lg.settings, lg.fs);
    let sel = clamp_range(sel.start, sel.end, lg.n());
    let br: Vec<i64> = lg.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let mh = sub_rows(mhz_log, &sel);
    let wide = |r: &[f32]| -> Vec<f64> { r[sel.clone()].iter().map(|&v| v as f64).collect() };
    let raw: Vec<Vec<f64>> = (0..2).map(|a| wide(gu.row(a))).collect();
    let flt: Vec<Vec<f64>> = (0..2).map(|a| wide(lg.gyro.row(a))).collect();
    let tol = MOTORS_TOL;
    for a in 0..2 {
        for (j, x) in [&flt[a], &raw[a]].into_iter().enumerate() {
            let n = motors::motor_noise(x, &mh, fs, 3, 300.0, 20.0, Some(&br))?;
            let key = format!("motors|noise[{a}][{j}]");
            f.insert(format!("{key}.harmonics"), Item::AI(n.harmonics));
            f.insert(format!("{key}.mean_db"), Item::TA(n.mean_db, tol));
            f.insert(format!("{key}.std_db"), Item::TA(n.std_db, tol));
        }
        for (j, x) in [&raw[a], &flt[a]].into_iter().enumerate() {
            let p = motors::harmonic_prominence(x, &mh, fs, 3, 300.0, 20.0, Some(&br))?;
            f.insert(format!("motors|prominence[{a}][{j}]"), Item::TA(p, tol));
        }
    }
    // cibles du notch dynamique ; ⚠️ `band` mélange des entiers et des flottants comme en Python (`min(int, float)`)
    let lo_i = (st.dyn_notch_min_hz - 37).max(20);
    let hi_i = st.dyn_notch_max_hz + 37;
    let band = (lo_i as f64, hi_i as f64);
    let hi_is_int = 0.95 * fs / 2.0 >= hi_i as f64;
    let mut targets = Vec::new();
    for (a, x) in raw.iter().enumerate() {
        let t = motors::dyn_notch_targets(x, Some(&mh), fs, band, Some(st), 300.0, 18.5, Some(&br))?;
        let key = format!("motors|targets[{a}]");
        f.insert(format!("{key}.freqs"), Item::A(t.freqs.clone()));
        f.insert(format!("{key}.ratios_db"), Item::TA(t.ratios_db.clone(), tol));
        // la bande RENDUE par Rust, convertie en entier là où Python garde un entier
        f.insert(format!("{key}.band[0]"), Item::I(t.band.0 as i64));
        f.insert(
            format!("{key}.band[1]"),
            if hi_is_int {
                Item::I(t.band.1 as i64)
            } else {
                Item::F(t.band.1)
            },
        );
        targets.push(t);
    }
    // `max(t, key=lambda x: x.freqs.size)` : le premier des plus longs
    let best = if targets[1].freqs.len() > targets[0].freqs.len() {
        &targets[1]
    } else {
        &targets[0]
    };
    let mut peaks: Vec<f64> = best.freqs.clone();
    peaks.extend([133.0, 421.5]);
    for (i, p) in peaks.iter().enumerate() {
        f.insert(format!("motors|peaks[{i}]"), Item::F(*p));
    }
    for a in 0..2 {
        let sh = motors::line_shapes(&raw[a], &mh, fs, &peaks, st.rpm_harmonics, 300.0, Some(&br), Some(st))?;
        for (i, (w, h)) in sh.iter().enumerate() {
            f.insert(format!("motors|shapes[{a}][{i}][0]"), Item::T(*w, tol, 1.0));
            f.insert(format!("motors|shapes[{a}][{i}][1]"), Item::T(*h, tol, 1.0));
        }
        for (i, h) in motors::line_heights(&flt[a], fs, &peaks, 300.0, Some(&br))?
            .iter()
            .enumerate()
        {
            f.insert(format!("motors|heights[{a}][{i}]"), Item::T(*h, tol, 1.0));
        }
        let lines = motors::spectrum_lines(&raw[a], &flt[a], &mh, fs, (20.0, 900.0), 6.0, 8, 300.0, Some(&br))?;
        for (i, l) in lines.iter().enumerate() {
            let key = format!("motors|lines[{a}][{i}]");
            f.insert(format!("{key}.freq_hz"), Item::F(l.freq_hz));
            f.insert(format!("{key}.raw_db"), Item::T(l.raw_db, tol, 1.0));
            f.insert(format!("{key}.left_db"), Item::T(l.left_db, tol, 1.0));
        }
        let (ff, ss) = motors::rpm_filtered_spectrum(&raw[a], Some(&mh), fs, Some(st), 300.0, Some(&br))?;
        f.insert(format!("motors|filtered[{a}][0]"), Item::A(ff));
        f.insert(format!("motors|filtered[{a}][1]"), Item::TR(ss, tol));
    }
    Ok(f)
}

fn stats_section(lg: &FlightLog, sel: std::ops::Range<usize>) -> Flat {
    let mut f = Flat::new();
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
    f
}

fn same_bits(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn compare_item(key: &str, got: &Item, want: &Array, worst: &mut f64) -> Option<String> {
    let bad = |what: String| Some(format!("{key} : {what}"));
    match got {
        Item::F(g) => {
            if want.kind != 'f' || want.size != 8 || want.len() != 1 {
                return bad(format!("Rust {g:e}, Python {}{} ×{}", want.kind, want.size, want.len()));
            }
            let w = want.get_f64(0);
            (!same_bits(*g, w)).then(|| format!("{key} : Rust {g:e} / Python {w:e}"))
        }
        Item::T(g, tol, unit) => {
            if want.kind != 'f' || want.size != 8 || want.len() != 1 {
                return bad(format!("Rust {g:e}, Python {}{} ×{}", want.kind, want.size, want.len()));
            }
            let w = want.get_f64(0);
            if !g.is_finite() || !w.is_finite() {
                return (!same_bits(*g, w)).then(|| format!("{key} : Rust {g:e} / Python {w:e}"));
            }
            let err = (g - w).abs() / w.abs().max(*unit);
            *worst = worst.max(err);
            (err > *tol).then(|| format!("{key} : Rust {g:e} / Python {w:e} (écart {err:e} > {tol:e})"))
        }
        Item::A32(g) => {
            if want.kind != 'f' || want.size != 4 || want.len() != g.len() {
                return bad(format!(
                    "{} valeurs float32 (Python {}{} ×{})",
                    g.len(),
                    want.kind,
                    want.size,
                    want.len()
                ));
            }
            (0..g.len())
                .find(|&i| {
                    let w = want.get_f64(i) as f32;
                    g[i].to_bits() != w.to_bits() && !(g[i].is_nan() && w.is_nan())
                })
                .map(|i| format!("{key} : élément {i} : Rust {:e} / Python {:e}", g[i], want.get_f64(i)))
        }
        Item::AI(g) => {
            if !matches!(want.kind, 'i' | 'u') || want.len() != g.len() {
                return bad(format!(
                    "{} entiers (Python {}{} ×{})",
                    g.len(),
                    want.kind,
                    want.size,
                    want.len()
                ));
            }
            (0..g.len())
                .find(|&i| want.get_f64(i) != g[i] as f64)
                .map(|i| format!("{key} : élément {i} : Rust {}", g[i]))
        }
        Item::TA(g, tol) => {
            if want.kind != 'f' || want.size != 8 || want.len() != g.len() {
                return bad(format!(
                    "{} valeurs (Python {}{} ×{})",
                    g.len(),
                    want.kind,
                    want.size,
                    want.len()
                ));
            }
            let top = (0..g.len())
                .map(|i| want.get_f64(i))
                .filter(|v| v.is_finite())
                .fold(0.0f64, |m, v| m.max(v.abs()))
                .max(1e-300);
            for (i, gi) in g.iter().enumerate() {
                let w = want.get_f64(i);
                if !gi.is_finite() || !w.is_finite() {
                    if !same_bits(*gi, w) {
                        return Some(format!("{key} : élément {i} : Rust {gi:e} / Python {w:e}"));
                    }
                    continue;
                }
                let err = (gi - w).abs() / top;
                *worst = worst.max(err);
                if err > *tol {
                    return Some(format!(
                        "{key} : élément {i} : Rust {gi:e} / Python {w:e} (écart {err:e} > {tol:e})"
                    ));
                }
            }
            None
        }
        Item::TR(g, tol) => {
            if want.kind != 'f' || want.size != 8 || want.len() != g.len() {
                return bad(format!(
                    "{} valeurs (Python {}{} ×{})",
                    g.len(),
                    want.kind,
                    want.size,
                    want.len()
                ));
            }
            for (i, gi) in g.iter().enumerate() {
                let w = want.get_f64(i);
                if !gi.is_finite() || !w.is_finite() {
                    if !same_bits(*gi, w) {
                        return Some(format!("{key} : élément {i} : Rust {gi:e} / Python {w:e}"));
                    }
                    continue;
                }
                let err = if w == 0.0 {
                    (gi - w).abs()
                } else {
                    (gi - w).abs() / w.abs()
                };
                *worst = worst.max(err);
                if err > *tol {
                    return Some(format!(
                        "{key} : élément {i} : Rust {gi:e} / Python {w:e} (écart relatif {err:e} > {tol:e})"
                    ));
                }
            }
            None
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
    /// Sections où Python lève (Rust doit lever aussi).
    pub python_errors: usize,
    /// Largeurs de raie ignorées : la raie n'existe pas (hauteur de référence < 1e-6 dB), la largeur est alors une fonction
    /// discontinue du dernier bit du spectre (L-055).
    pub exempt: usize,
    /// Plus grand écart normalisé des valeurs comparées à une tolérance (`Item::T`).
    pub worst: std::collections::BTreeMap<String, f64>,
    pub failures: Vec<String>,
}

fn stem(rel: &str, session: i64) -> String {
    format!("{}.{session}.npz", rel.replace('/', "__"))
}

fn check_one(dossier: &Path, rel: &str, lg: &FlightLog, rep: &mut AnalysisReport) {
    let who = format!("{rel} session {}", lg.session);
    let file = dossier.join("analysis").join(stem(rel, lg.session as i64));
    let py: HashMap<String, Array> = match load_arrays(&file) {
        Ok(a) => a,
        Err(e) => return rep.failures.push(format!("{who} : {e}")),
    };
    rep.sessions += 1;
    for (section, result) in ported(lg) {
        // Python a levé dans cette section : Rust doit lever aussi (et réciproquement)
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
                    // largeur d'une raie inexistante (hauteur de référence ≈ 0 dB) : exemptée et comptée (L-055)
                    if let Some(base) = k.strip_suffix("[0]").filter(|b| b.starts_with("motors|shapes["))
                        && py.get(&format!("{base}[1]")).is_some_and(|h| h.get_f64(0).abs() < 1e-6)
                    {
                        rep.exempt += 1;
                        continue;
                    }
                    rep.keys += 1;
                    let mut w_err = 0.0;
                    let m = compare_item(k, item, w, &mut w_err);
                    let e = rep.worst.entry(section.to_string()).or_insert(0.0);
                    *e = e.max(w_err);
                    if let Some(m) = m {
                        rep.failures.push(format!("{who} : {m}"));
                    }
                }
            }
        }
        for k in py.keys() {
            if section_of(k) == section && !k.ends_with("|_error") && !got.contains_key(k) {
                rep.failures.push(format!("{who} : {k} absente de Rust"));
            }
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
        exempt: 0,
        worst: Default::default(),
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
                        exempt: 0,
                        worst: Default::default(),
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
                    r.exempt += local.exempt;
                    for (k, v) in local.worst {
                        let e = r.worst.entry(k).or_insert(0.0);
                        *e = e.max(v);
                    }
                    r.failures.extend(local.failures);
                }
            });
        }
    });
    let mut report = report.into_inner().unwrap();
    if report.exempt > MAX_EXEMPT {
        report.failures.push(format!(
            "{} largeurs de raies exemptées (au plus {MAX_EXEMPT} attendue : L-055)",
            report.exempt
        ));
    }
    // chaque référence doit avoir été rejouée : une session absente du côté Rust ne doit pas passer en silence
    let refs = std::fs::read_dir(dossier.join("analysis"))
        .map_err(|e| format!("analysis/ : {e}"))?
        .filter(|e| {
            e.as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "npz"))
        })
        .count();
    if refs != report.sessions {
        report.failures.push(format!(
            "{refs} références dans analysis/, {} sessions rejouées",
            report.sessions
        ));
    }
    Ok(report)
}
