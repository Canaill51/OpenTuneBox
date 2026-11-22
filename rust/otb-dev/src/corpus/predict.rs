//! `otb-dev corpus predict AVANT APRÈS` : vérification en vol du simulateur de filtres (lot 22f du plan, W-060). Les
//! réglages de filtres du vol APRÈS sont appliqués au vol AVANT ; la prédiction est comparée au vol APRÈS, gyro et terme D,
//! roulis et tangage, par bande de fréquences, pour les deux méthodes :
//! - « rejeu » : la page Filtres (`filter_replay::before_after`, D-058) ;
//! - « rapport » : rapport des |H|² (`bf_filters::whatif_spectrum`, D-008), la méthode de l'assistant jusqu'à D-059.
//!
//! Chaque colonne est un **changement** en dB par rapport au vol AVANT (spectre moyen de la période analysée) :
//! prédit par chaque méthode, mesuré dans le vol APRÈS, et changement du gyro **non filtré** (bruit d'avant les filtres).
//! Le simulateur garde le bruit d'avant filtres du vol AVANT : ce dernier changement, il ne peut pas le prévoir. Il mêle la
//! boucle fermée (filtrer autrement change ce que les moteurs reçoivent, donc le bruit qu'ils font) et ce qui varie d'un vol
//! à l'autre (batterie, hélices, gaz). Deux vols aux mêmes réglages donnent l'ordre de grandeur de cette variation.

use super::{Args, Output, analysed};
use otb_analysis::bf_filters::{Target, WhatIfCache, whatif_spectrum};
use otb_analysis::filter_replay::before_after;
use otb_analysis::spectral::{Mode, py_round, seg_spectra, segments, to_db};
use otb_log::flightlog::{FlightLog, load};
use otb_log::settings::Settings;
use std::fmt::Write;
use std::ops::Range;

/// Bandes comparées (Hz), comme `corpus replay`.
const BANDS: [(f64, f64); 3] = [(20.0, 100.0), (100.0, 250.0), (250.0, 500.0)];
/// Fenêtres des spectres (ms), comme la page Filtres et l'assistant.
const SEG_MS: f64 = 300.0;

/// Réglages `a` avec les filtres de `b` (passe-bas, notchs statiques, notch dynamique, filtre RPM).
pub fn with_filters_of(a: &Settings, b: &Settings) -> Settings {
    let mut s = a.clone();
    s.gyro_lpf1 = b.gyro_lpf1.clone();
    s.gyro_lpf2 = b.gyro_lpf2.clone();
    s.gyro_notches = b.gyro_notches.clone();
    s.dterm_lpf1 = b.dterm_lpf1.clone();
    s.dterm_lpf2 = b.dterm_lpf2.clone();
    s.dterm_notch = b.dterm_notch;
    s.yaw_lowpass_hz = b.yaw_lowpass_hz;
    s.dyn_notch_count = b.dyn_notch_count;
    s.dyn_notch_q = b.dyn_notch_q;
    s.dyn_notch_min_hz = b.dyn_notch_min_hz;
    s.dyn_notch_max_hz = b.dyn_notch_max_hz;
    s.rpm_harmonics = b.rpm_harmonics;
    s.rpm_weights = b.rpm_weights.clone();
    s.rpm_q = b.rpm_q;
    s.rpm_min_hz = b.rpm_min_hz;
    s.rpm_fade_hz = b.rpm_fade_hz;
    s.rpm_lpf_hz = b.rpm_lpf_hz;
    s
}

/// Ce qui change hors filtres entre deux vols et que la prédiction ignore (cadence, PID, résolution du log).
pub fn other_changes(a: &Settings, b: &Settings) -> Vec<String> {
    let mut v = Vec::new();
    if a.looptime_us != b.looptime_us || a.pid_denom != b.pid_denom {
        v.push("cadence gyro ou PID".into());
    }
    if a.p_interval != b.p_interval {
        v.push("cadence du log".into());
    }
    if a.pids != b.pids {
        v.push("gains PID".into());
    }
    if a.high_resolution != b.high_resolution {
        v.push("haute résolution du log".into());
    }
    if a.version != b.version {
        v.push("version du firmware".into());
    }
    v
}

/// Fréquences (Hz) et spectre moyen (dB).
type Spectrum = (Vec<f64>, Vec<f64>);

/// Spectre moyen (dB) d'un signal sur la période `sel`, fenêtres de [`SEG_MS`].
fn mean_spectrum(log: &FlightLog, x: &[f32], sel: &Range<usize>) -> Result<Option<Spectrum>, String> {
    let l = 32.max(py_round(SEG_MS * log.fs / 1000.0)?) as usize;
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let x: Vec<f64> = x[sel.clone()].iter().map(|&v| v as f64).collect();
    let segs = segments(&x, l, l / 2, Some(&br));
    if segs.nseg == 0 {
        return Ok(None);
    }
    let (f, p) = seg_spectra(&segs, log.fs, Mode::Psd)?;
    let nf = f.len();
    let mut m = vec![0.0; nf];
    for row in p.chunks(nf) {
        m.iter_mut().zip(row).for_each(|(a, b)| *a += b);
    }
    let n = (p.len() / nf.max(1)) as f64;
    Ok(Some((f, to_db(&m.iter().map(|a| a / n).collect::<Vec<_>>()))))
}

/// Moyenne de `a − b` (dB) sur [`lo`, `hi`[.
fn band(f: &[f64], a: &[f64], b: &[f64], lo: f64, hi: f64) -> f64 {
    let v: Vec<f64> = (0..f.len().min(a.len()).min(b.len()))
        .filter(|&i| f[i] >= lo && f[i] < hi && a[i].is_finite() && b[i].is_finite())
        .map(|i| a[i] - b[i])
        .collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn mean(v: &[f32]) -> f64 {
    v.iter().map(|&x| x as f64).sum::<f64>() / v.len().max(1) as f64
}

fn first_analysed(path: &str) -> Result<(FlightLog, Range<usize>), String> {
    for lg in load(std::path::Path::new(path)).map_err(|e| format!("{path} : {e}"))? {
        if let Some(sel) = analysed(&lg) {
            return Ok((lg, sel));
        }
    }
    Err(format!("{path} : aucune session analysable"))
}

/// Un signal comparé : (nom, changements par bande `[prédit rejeu, prédit rapport, mesuré, non filtré]`).
type Row = (String, Vec<[f64; 4]>);

fn compare(before: &str, after: &str) -> Result<String, String> {
    let (a, sa) = first_analysed(before)?;
    let (b, sb) = first_analysed(after)?;
    if a.gyro_unfilt.is_none() || b.gyro_unfilt.is_none() {
        return Err("gyro non filtré absent d'un des deux vols".into());
    }
    if (a.fs - b.fs).abs() > 1.0 {
        return Err(format!("cadences de log différentes : {:.0} et {:.0} Hz", a.fs, b.fs));
    }
    let modf = with_filters_of(&a.settings, &b.settings);
    let Some(ba) = before_after(&a, sa.clone(), &a.settings, &modf, SEG_MS)? else {
        return Err(format!("{before} : rejeu impossible"));
    };
    let mut cache = WhatIfCache::default();
    let prefilter = a.d_term_prefilter()?;
    let pre_b = b.d_term_prefilter()?;
    let mut rows: Vec<Row> = Vec::new();
    for ax in 0..2 {
        let axis = ["roulis", "tangage"][ax];
        let wide = |r: &[f32]| r.iter().map(|&v| v as f64).collect::<Vec<_>>();
        let (ua, ub) = (a.gyro_unfilt.as_ref().unwrap(), b.gyro_unfilt.as_ref().unwrap());
        let (Some((f, gb)), Some((_, una)), Some((_, unb))) = (
            mean_spectrum(&b, b.gyro.row(ax), &sb)?,
            mean_spectrum(&a, ua.row(ax), &sa)?,
            mean_spectrum(&b, ub.row(ax), &sb)?,
        ) else {
            return Err("période trop courte".into());
        };
        let old = whatif_spectrum(
            &wide(a.gyro.row(ax)),
            Some(&wide(ua.row(ax))),
            &a,
            sa.clone(),
            &a.settings,
            &modf,
            Target::Gyro,
            SEG_MS,
            &mut cache,
        )?;
        let g = &ba.gyro[ax];
        let mut v = Vec::new();
        for (lo, hi) in BANDS {
            let hi = hi.min(0.45 * a.fs);
            v.push([
                band(&ba.freqs, &g.modified_db, &g.logged_db, lo, hi),
                band(&old.freqs, &old.modified_db, &old.current_db, lo, hi),
                band(&f, &gb, &g.logged_db, lo, hi),
                band(&f, &unb, &una, lo, hi),
            ]);
        }
        rows.push((format!("gyro {axis}"), v));
        let Some(d) = &ba.dterm[ax] else { continue };
        let (Some((f, db)), Some((_, pa)), Some((_, pb))) = (
            mean_spectrum(&b, b.d_term.row(ax), &sb)?,
            mean_spectrum(&a, prefilter.row(ax), &sa)?,
            mean_spectrum(&b, pre_b.row(ax), &sb)?,
        ) else {
            continue;
        };
        let old = whatif_spectrum(
            &wide(a.d_term.row(ax)),
            Some(&wide(prefilter.row(ax))),
            &a,
            sa.clone(),
            &a.settings,
            &modf,
            Target::Dterm,
            SEG_MS,
            &mut cache,
        )?;
        let mut v = Vec::new();
        for (lo, hi) in BANDS {
            let hi = hi.min(0.45 * a.fs);
            v.push([
                band(&ba.freqs, &d.modified_db, &d.logged_db, lo, hi),
                band(&old.freqs, &old.modified_db, &old.current_db, lo, hi),
                band(&f, &db, &d.logged_db, lo, hi),
                band(&f, &pb, &pa, lo, hi),
            ]);
        }
        rows.push((format!("D {axis}"), v));
    }
    let mut out = String::new();
    let per = |lg: &FlightLog, sel: &Range<usize>| {
        format!(
            "{:.1}–{:.1} s, gaz moyen {:.0}",
            sel.start as f64 / lg.fs,
            sel.end as f64 / lg.fs,
            mean(&lg.throttle[sel.clone()])
        )
    };
    writeln!(out, "AVANT : {before} ({})", per(&a, &sa)).ok();
    writeln!(out, "APRÈS : {after} ({})", per(&b, &sb)).ok();
    if modf == a.settings {
        writeln!(
            out,
            "Filtres identiques : prédiction nulle, l'écart mesuré est la variation d'un vol à l'autre."
        )
        .ok();
    }
    let others = other_changes(&a.settings, &b.settings);
    if !others.is_empty() {
        writeln!(
            out,
            "⚠️ Change aussi hors filtres (ignoré par la prédiction) : {}",
            others.join(", ")
        )
        .ok();
    }
    writeln!(
        out,
        "\nChangement par rapport au vol AVANT (dB) : rejeu | rapport | mesuré | non filtré (D : avant ses filtres)"
    )
    .ok();
    for (lo, hi) in BANDS {
        writeln!(out, "\n{lo:.0}–{hi:.0} Hz").ok();
        let i = BANDS.iter().position(|x| x.0 == lo).unwrap_or(0);
        for (name, v) in &rows {
            let [r, o, m, u] = v[i];
            writeln!(out, "  {name:<12} {r:+6.2} | {o:+6.2} | {m:+6.2} | {u:+6.2}").ok();
        }
    }
    Ok(out)
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &[], &[])?;
    match a.positional.as_slice() {
        [before, after] => compare(before, after).map(Output::ok),
        _ => Err("Usage : otb-dev corpus predict AVANT.bbl APRÈS.bbl".into()),
    }
}
