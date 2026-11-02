//! Parité des calculs des outils avec Python (phases 6-2 à 6-4) : rejoue, avec les réglages par défaut, `compute` des
//! sept outils portés (Spectre, Fréquence × gaz, Fréquence × temps, Réponse indicielle, Curseurs PID, Simulateur de filtres,
//! Assistant) sur les 20 journaux de
//! `golden.py record` (`DOSSIER/tools/NNN.npz`) et compare **chaque clé** de Python.
//!
//! `cargo run --release -p otb-gui --example check_tools -- $OTB_LOGS/golden [--tol X]`
//!
//! Décisions discrètes (clés présentes, formes, comptes, textes, booléens, emplacement des NaN) : identiques, sans tolérance.
//! Valeurs continues : écart rapporté au plus grand élément fini de la référence ≤ tolérance de la famille (mesurées avant
//! d'être posées : voir `docs/migration-rust.md`, 6-2). Toute clé de Python d'un outil porté doit être rejouée.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use otb_advisor::profiles::CraftProfile;
use otb_advisor::rules::{Finding, Measures};
use otb_analysis::open_loop::LoopMetrics;
use otb_analysis::stats::MeanSd;
use otb_analysis::step::{StepMetrics, StepResult};
use otb_analysis::step_series::Sig;
use otb_gui::state::AppState;
use otb_gui::tool::{LogInput, Tool};
use otb_gui::tools::advisor::{AdvisorOutput, AdvisorTool, Options as AdvisorOptions};
use otb_gui::tools::filter_sim::{FilterSimTool, Options as FilterSimOptions};
use otb_gui::tools::filter_sim_calc::FilterSimResult;
use otb_gui::tools::freq_throttle::{FreqThrottleOutput, FreqThrottleTool};
use otb_gui::tools::freq_time::{FreqTimeOutput, FreqTimeTool};
use otb_gui::tools::pid_slider::{PidOutput, PidSliderTool};
use otb_gui::tools::spectral::{SpectralOutput, SpectralTool};
use otb_gui::tools::step::{Options as StepOptions, StepOutput, StepTool};
use otb_gui::tools::step_common::StepOpts;
use otb_log::settings::{Lowpass, Settings};
use otb_parity::value::{Array, load_arrays};

#[derive(Default)]
struct Report {
    fails: Vec<String>,
    /// Famille → (plus grand écart relatif au maximum de la référence, nombre de valeurs comparées).
    dev: BTreeMap<String, (f64, usize)>,
    used: HashSet<String>,
}

type Refs = HashMap<String, Array>;

impl Report {
    fn fail(&mut self, file: usize, msg: String) {
        if self.fails.len() < 40 {
            self.fails.push(format!("fichier {file:03} : {msg}"));
        }
    }

    fn get<'a>(&mut self, refs: &'a Refs, file: usize, key: &str) -> Option<&'a Array> {
        self.used.insert(key.to_string());
        let a = refs.get(key);
        if a.is_none() {
            self.fail(file, format!("clé absente de la référence : {key}"));
        }
        a
    }

    fn text(&mut self, refs: &Refs, file: usize, key: &str, got: &str) {
        if let Some(a) = self.get(refs, file, key)
            && a.as_string().as_deref() != Some(got)
        {
            self.fail(file, format!("{key} : {:?} ≠ {got:?}", a.as_string()));
        }
    }

    fn flag(&mut self, refs: &Refs, file: usize, key: &str, got: bool) {
        if let Some(a) = self.get(refs, file, key)
            && (a.len() != 1 || (a.get_f64(0) != 0.0) != got)
        {
            self.fail(file, format!("{key} : booléen différent"));
        }
    }

    fn ints(&mut self, refs: &Refs, file: usize, key: &str, got: &[i64]) {
        if let Some(a) = self.get(refs, file, key) {
            let r: Vec<i64> = (0..a.len()).map(|i| a.get_f64(i) as i64).collect();
            if r != got {
                self.fail(file, format!("{key} : comptes différents"));
            }
        }
    }

    /// Tableau de flottants : même forme (`shape`), mêmes NaN, écart ≤ `tol` × plus grand élément fini de la référence.
    #[allow(clippy::too_many_arguments)]
    fn floats(&mut self, refs: &Refs, file: usize, key: &str, family: &str, shape: &[usize], got: &[f64], tol: f64) {
        let Some(a) = self.get(refs, file, key) else { return };
        let want: &[usize] = if shape.is_empty() { &[] } else { shape };
        if a.shape != want {
            self.fail(file, format!("{key} : forme {:?} ≠ {want:?}", a.shape));
            return;
        }
        let n = a.len();
        if n != got.len() {
            self.fail(file, format!("{key} : {n} valeurs ≠ {}", got.len()));
            return;
        }
        let max = (0..n)
            .map(|i| a.get_f64(i))
            .filter(|x| x.is_finite())
            .fold(0.0f64, |m, x| m.max(x.abs()));
        let mut worst = 0.0f64;
        let mut first_bad = None;
        for (i, &g) in got.iter().enumerate() {
            let r = a.get_f64(i);
            if r.is_nan() != g.is_nan() || (r.is_infinite() && r != g) {
                first_bad.get_or_insert((i, r, g));
                continue;
            }
            if r.is_finite() {
                let d = (r - g).abs() / if max > 0.0 { max } else { 1.0 };
                worst = worst.max(d);
                if d > tol {
                    first_bad.get_or_insert((i, r, g));
                }
            }
        }
        let e = self.dev.entry(family.to_string()).or_insert((0.0, 0));
        e.0 = e.0.max(worst);
        e.1 += n;
        if let Some((i, r, g)) = first_bad {
            self.fail(
                file,
                format!("{key} : indice {i}, Python {r:e} ≠ Rust {g:e} (tolérance {tol:e})"),
            );
        }
    }

    fn scalar(&mut self, refs: &Refs, file: usize, key: &str, family: &str, got: f64, tol: f64) {
        self.floats(refs, file, key, family, &[], &[got], tol);
    }

    /// Valeur seule : écart rapporté à max(|référence|, 1) (grandeurs en dB ou en ms proches de 0, où un écart relatif n'a pas de sens).
    fn num(&mut self, refs: &Refs, file: usize, key: &str, family: &str, got: f64, tol: f64) {
        let Some(a) = self.get(refs, file, key) else { return };
        if a.shape != [] as [usize; 0] || a.len() != 1 {
            self.fail(file, format!("{key} : forme {:?} au lieu d'une valeur", a.shape));
            return;
        }
        let r = a.get_f64(0);
        if r.is_nan() != got.is_nan() || (r.is_infinite() && r != got) {
            self.fail(file, format!("{key} : Python {r:e} ≠ Rust {got:e}"));
            return;
        }
        if r.is_finite() {
            let d = (r - got).abs() / r.abs().max(1.0);
            let e = self.dev.entry(family.to_string()).or_insert((0.0, 0));
            e.0 = e.0.max(d);
            e.1 += 1;
            if d > tol {
                self.fail(
                    file,
                    format!("{key} : Python {r:e} ≠ Rust {got:e} (écart {d:e} > {tol:e})"),
                );
            }
        } else {
            let e = self.dev.entry(family.to_string()).or_insert((0.0, 0));
            e.1 += 1;
        }
    }

    /// Valeur optionnelle : `None` s'écrit « None » côté Python.
    fn opt_num(&mut self, refs: &Refs, file: usize, key: &str, family: &str, got: Option<f64>, tol: f64) {
        match got {
            Some(v) => self.num(refs, file, key, family, v, tol),
            None => self.text(refs, file, key, "None"),
        }
    }

    /// Aucune clé `<prefix>` de plus que ce que Rust a produit (la liste Python n'est pas plus longue).
    fn no_more(&mut self, refs: &Refs, file: usize, key: &str) {
        if refs.contains_key(key) {
            self.fail(file, format!("{key} : Python a plus d'éléments que Rust"));
        }
    }

    /// Tableau de complexes `(re, im)` : même longueur, écart de chaque partie ≤ `tol` × plus grand module fini de la référence.
    fn complexes(&mut self, refs: &Refs, file: usize, key: &str, family: &str, got: &[(f64, f64)], tol: f64) {
        let Some(a) = self.get(refs, file, key) else { return };
        if a.kind != 'c' || a.shape != [got.len()] {
            self.fail(
                file,
                format!(
                    "{key} : forme {:?} ou type {} différents ({} valeurs)",
                    a.shape,
                    a.kind,
                    got.len()
                ),
            );
            return;
        }
        let flat: Vec<f64> = got.iter().flat_map(|c| [c.0, c.1]).collect();
        let max = (0..a.n_floats())
            .map(|i| a.get_f64(i))
            .filter(|x| x.is_finite())
            .fold(0.0f64, |m, x| m.max(x.abs()));
        let mut worst = 0.0f64;
        let mut first_bad = None;
        for (i, &g) in flat.iter().enumerate() {
            let r = a.get_f64(i);
            if r.is_nan() != g.is_nan() || (r.is_infinite() && r != g) {
                first_bad.get_or_insert((i, r, g));
            } else if r.is_finite() {
                let d = (r - g).abs() / if max > 0.0 { max } else { 1.0 };
                worst = worst.max(d);
                if d > tol {
                    first_bad.get_or_insert((i, r, g));
                }
            }
        }
        let e = self.dev.entry(family.to_string()).or_insert((0.0, 0));
        e.0 = e.0.max(worst);
        e.1 += flat.len();
        if let Some((i, r, g)) = first_bad {
            self.fail(
                file,
                format!("{key} : indice {i}, Python {r:e} ≠ Rust {g:e} (tolérance {tol:e})"),
            );
        }
    }
}

fn tuple_key(k: &str, ax: usize) -> String {
    format!("('{k}', {ax})")
}

struct Tols {
    /// Axes de fréquence et de temps (calculés par `linspace` / `rfftfreq`).
    grid: f64,
    /// Spectres, cartes et niveaux en dB (FFT, `log10`).
    db: f64,
    /// Retards (méthode de phase).
    delay: f64,
    /// Traces moteur réduites en `float32` par numpy, moyennes en `float64` ici : mesuré 2,8e-7 (6-2), marge ×7.
    trace: f64,
    /// Réponses indicielles, moyennes, écarts-types et métriques (même noyau des deux côtés) : mesuré 1,1e-13 au pire (6-3), marge ×900.
    step: f64,
    /// Boucle ouverte (`L`, `T`, cohérence, marges : `log`, `atan2`, FFT ; `L` amplifie les annulations) : mesuré 2,4e-10 au pire sur
    /// les 20 journaux (6-3), marge ×40.
    open_loop: f64,
    /// Positions des curseurs estimées depuis les PID (SVD de Jacobi à la place de `lstsq`).
    slider: f64,
    /// Mesures de bruit de l'Assistant (moyennes de bande, sommets : FFT, `log10`), écart rapporté à max(|valeur|, 1).
    noise: f64,
    /// Mesures de filtres de l'Assistant (raies, notch dynamique, harmoniques moteur), même normalisation.
    filter: f64,
    /// Statistiques du vol, erreurs PID, moyennes des I (calculs en `float32` des deux côtés).
    stats: f64,
}

fn check_spectral(rep: &mut Report, refs: &Refs, i: usize, out: &SpectralOutput, t: &Tols) {
    rep.text(refs, i, "spectral.mode", if out.psd { "psd" } else { "amp" });
    for (k, key) in out.keys.iter().enumerate() {
        rep.text(refs, i, &format!("spectral.keys[{k}]"), key);
    }
    let f = &out.files[0];
    let p = "spectral.files[0]";
    // `spectra|(K, A)[0]` = fréquences, `[1]` = valeurs
    let mut ids: Vec<(&&str, usize)> = Vec::new();
    for ((k, ax), (fr, v)) in &f.spectra {
        let name = tuple_key(k, *ax);
        rep.floats(
            refs,
            i,
            &format!("{p}.spectra|{name}[0]"),
            "spectre.freqs",
            &[fr.len()],
            fr,
            t.grid,
        );
        rep.floats(
            refs,
            i,
            &format!("{p}.spectra|{name}[1]"),
            "spectre.valeurs",
            &[v.len()],
            v,
            t.db,
        );
        ids.push((k, *ax));
    }
    for ((k, ax), (m, s)) in &f.noise {
        let name = tuple_key(k, *ax);
        rep.floats(
            refs,
            i,
            &format!("{p}.noise|{name}[0]"),
            "bruit moteur",
            &[m.len()],
            m,
            t.db,
        );
        rep.floats(
            refs,
            i,
            &format!("{p}.noise|{name}[1]"),
            "bruit moteur",
            &[s.len()],
            s,
            t.db,
        );
    }
    // aucune courbe de plus ni de moins que Python
    let wanted = |kind: &str| -> HashSet<String> {
        refs.keys()
            .filter(|k| k.starts_with(&format!("{p}.{kind}|")))
            .map(|k| k.rsplit_once('[').map_or(k.clone(), |x| x.0.to_string()))
            .collect()
    };
    for (kind, n) in [("spectra", f.spectra.len()), ("noise", f.noise.len())] {
        if wanted(kind).len() != n {
            rep.fail(i, format!("{kind} : {} courbes ≠ {n}", wanted(kind).len()));
        }
    }
    rep.scalar(refs, i, &format!("{p}.gyro_ms"), "retards", f.gyro_ms, t.delay);
    rep.scalar(refs, i, &format!("{p}.dterm_ms"), "retards", f.dterm_ms, t.delay);
    rep.flag(refs, i, &format!("{p}.delays_moving"), f.delays_moving);
    for (m, v) in f.motor_sorted.iter().enumerate() {
        rep.floats(
            refs,
            i,
            &format!("{p}.motor_sorted[{m}]"),
            "moteurs triés",
            &[v.len()],
            v,
            0.0,
        );
    }
    for (m, v) in f.notch_sorted.iter().enumerate() {
        rep.floats(
            refs,
            i,
            &format!("{p}.notch_sorted[{m}]"),
            "notchs triés",
            &[v.len()],
            v,
            0.0,
        );
    }
    rep.text(refs, i, &format!("{p}.rpm_text"), &f.rpm_text);
    rep.scalar(refs, i, &format!("{p}.nyquist"), "nyquist", f.nyquist, 0.0);
    rep.text(refs, i, &format!("{p}.diagnosis"), "None");
    for (n, c) in [
        ("motor_sorted", f.motor_sorted.len()),
        ("notch_sorted", f.notch_sorted.len()),
    ] {
        let have = (0..64).filter(|k| refs.contains_key(&format!("{p}.{n}[{k}]"))).count();
        if have != c {
            rep.fail(i, format!("{n} : {have} listes ≠ {c}"));
        }
    }
}

fn check_freq_throttle(rep: &mut Report, refs: &Refs, i: usize, out: &FreqThrottleOutput, t: &Tols) {
    rep.text(refs, i, "freq_throttle.mode", if out.psd { "psd" } else { "amp" });
    rep.text(refs, i, "freq_throttle.xaxis", "throttle");
    for (c, col) in out.columns.iter().enumerate() {
        let p = format!("freq_throttle.columns[{c}]");
        let Some(col) = col else {
            rep.text(refs, i, &p, "None");
            continue;
        };
        rep.text(refs, i, &format!("{p}.key"), col.key);
        rep.scalar(refs, i, &format!("{p}.x_range[0]"), "plage x", col.x_range.0, 0.0);
        rep.scalar(refs, i, &format!("{p}.x_range[1]"), "plage x", col.x_range.1, 0.0);
        let have = (0..3)
            .filter(|a| refs.contains_key(&format!("{p}.maps|{a}.freqs")))
            .count();
        if have != col.maps.len() {
            rep.fail(i, format!("{p} : {have} cartes ≠ {}", col.maps.len()));
        }
        for (ax, m) in &col.maps {
            let q = format!("{p}.maps|{ax}");
            rep.floats(
                refs,
                i,
                &format!("{q}.freqs"),
                "carte.freqs",
                &[m.freqs.len()],
                &m.freqs,
                t.grid,
            );
            rep.floats(
                refs,
                i,
                &format!("{q}.bins"),
                "carte.bins",
                &[m.bins.len()],
                &m.bins,
                t.grid,
            );
            rep.floats(
                refs,
                i,
                &format!("{q}.z"),
                "carte.z",
                &[m.freqs.len(), m.bins.len()],
                &m.z,
                t.db,
            );
            rep.ints(refs, i, &format!("{q}.counts"), &m.counts);
            rep.text(refs, i, &format!("{q}.mode"), if out.psd { "psd" } else { "amp" });
        }
    }
}

fn check_freq_time(rep: &mut Report, refs: &Refs, i: usize, out: &FreqTimeOutput, t: &Tols) {
    rep.text(refs, i, "freq_time.key", out.key);
    rep.scalar(refs, i, "freq_time.nyquist", "nyquist", out.nyquist, 0.0);
    let have = (0..3)
        .filter(|a| refs.contains_key(&format!("freq_time.maps|{a}.freqs")))
        .count();
    if have != out.maps.len() {
        rep.fail(i, format!("freq_time : {have} cartes ≠ {}", out.maps.len()));
    }
    for (ax, m) in &out.maps {
        let q = format!("freq_time.maps|{ax}");
        rep.floats(
            refs,
            i,
            &format!("{q}.times"),
            "spectro.temps",
            &[m.times.len()],
            &m.times,
            t.grid,
        );
        rep.floats(
            refs,
            i,
            &format!("{q}.freqs"),
            "spectro.freqs",
            &[m.freqs.len()],
            &m.freqs,
            t.grid,
        );
        rep.floats(
            refs,
            i,
            &format!("{q}.z"),
            "spectro.z",
            &[m.freqs.len(), m.times.len()],
            &m.z,
            t.db,
        );
    }
    if let Some(mt) = &out.motor_t {
        rep.floats(refs, i, "freq_time.motor_t", "traces.temps", &[mt.len()], mt, t.grid);
    } else {
        rep.text(refs, i, "freq_time.motor_t", "None");
    }
    for (m, hz) in out.motor_hz.iter().enumerate() {
        rep.floats(
            refs,
            i,
            &format!("freq_time.motor_hz[{m}]"),
            "traces.moteurs",
            &[hz.len()],
            hz,
            t.trace,
        );
    }
    for (n, (tt, hz)) in out.notches.iter().enumerate() {
        rep.floats(
            refs,
            i,
            &format!("freq_time.notches[{n}][0]"),
            "traces.temps",
            &[tt.len()],
            tt,
            t.grid,
        );
        rep.floats(
            refs,
            i,
            &format!("freq_time.notches[{n}][1]"),
            "traces.notchs",
            &[hz.len()],
            hz,
            t.trace,
        );
    }
    let have = (0..64)
        .filter(|k| refs.contains_key(&format!("freq_time.motor_hz[{k}]")))
        .count();
    if have != out.motor_hz.len() {
        rep.fail(
            i,
            format!("freq_time.motor_hz : {have} listes ≠ {}", out.motor_hz.len()),
        );
    }
    let have = (0..64)
        .filter(|k| refs.contains_key(&format!("freq_time.notches[{k}][0]")))
        .count();
    if have != out.notches.len() {
        rep.fail(i, format!("freq_time.notches : {have} listes ≠ {}", out.notches.len()));
    }
}

// ------------------------------------------------------------------ Réponse indicielle et Curseurs PID (6-3)

fn hex(c: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn check_metrics(rep: &mut Report, refs: &Refs, i: usize, p: &str, m: &StepMetrics, t: &Tols) {
    for (k, v) in [
        ("peak", m.peak),
        ("peak_ms", m.peak_ms),
        ("latency_ms", m.latency_ms),
        ("rise_ms", m.rise_ms),
        ("settling_ms", m.settling_ms),
        ("overshoot_pct", m.overshoot_pct),
        ("ripple", m.ripple),
        ("ripple_std", m.ripple_std),
        ("ripple_ms", m.ripple_ms),
        ("waves", m.waves),
        ("waves_ratio", m.waves_ratio),
    ] {
        rep.scalar(refs, i, &format!("{p}.{k}"), "métriques de la réponse", v, t.step);
    }
}

/// Une boucle ouverte (`res.loop` d'un axe, ou `flight.loops[k]`).
fn check_loop(rep: &mut Report, refs: &Refs, i: usize, p: &str, lp: &LoopMetrics, t: &Tols) {
    rep.text(refs, i, &format!("{p}.method"), lp.method);
    for (k, n) in lp.notes.iter().enumerate() {
        rep.text(refs, i, &format!("{p}.notes[{k}]"), n);
    }
    rep.floats(
        refs,
        i,
        &format!("{p}.band_hz[0]"),
        "grilles",
        &[],
        &[lp.band_hz.0],
        t.grid,
    );
    rep.floats(
        refs,
        i,
        &format!("{p}.band_hz[1]"),
        "grilles",
        &[],
        &[lp.band_hz.1],
        t.grid,
    );
    for (k, v) in [
        ("bandwidth_hz", lp.bandwidth_hz),
        ("tracking_peak_db", lp.tracking_peak_db),
        ("sensitivity_peak_db", lp.sensitivity_peak_db),
        ("sensitivity_peak_hz", lp.sensitivity_peak_hz),
        ("crossover_hz", lp.crossover_hz),
        ("phase_margin_deg", lp.phase_margin_deg),
        ("gain_margin_db", lp.gain_margin_db),
        ("d_ratio", lp.d_ratio),
    ] {
        rep.scalar(refs, i, &format!("{p}.{k}"), "marges de la boucle", v, t.open_loop);
    }
    rep.flag(
        refs,
        i,
        &format!("{p}.sensitivity_peak_at_edge"),
        lp.sensitivity_peak_at_edge,
    );
    rep.floats(
        refs,
        i,
        &format!("{p}.freqs"),
        "grilles",
        &[lp.freqs.len()],
        &lp.freqs,
        t.grid,
    );
    rep.floats(
        refs,
        i,
        &format!("{p}.coherence"),
        "cohérence",
        &[lp.coherence.len()],
        &lp.coherence,
        t.open_loop,
    );
    rep.complexes(refs, i, &format!("{p}.T"), "boucle ouverte L et T", &lp.t, t.open_loop);
    rep.complexes(refs, i, &format!("{p}.L"), "boucle ouverte L et T", &lp.l, t.open_loop);
    let valid: Vec<i64> = lp.valid.iter().map(|&v| i64::from(v)).collect();
    rep.ints(refs, i, &format!("{p}.valid"), &valid);
    for (k, v) in &lp.std {
        rep.scalar(
            refs,
            i,
            &format!("{p}.std|{k}"),
            "écarts-types jackknife",
            *v,
            t.open_loop,
        );
    }
}

fn sig_keys(prefix: &str, s: &Sig, out: &mut Vec<(String, i64)>) {
    match s {
        Sig::I(v) => out.push((prefix.to_string(), *v)),
        Sig::T(items) => {
            for (j, e) in items.iter().enumerate() {
                sig_keys(&format!("{prefix}[{j}]"), e, out);
            }
        }
    }
}

/// Un `StepResult` (`step[0].axes[k].res`, `advisor[1].steps[k].result`) : mêmes clés des deux côtés.
fn check_step_result(rep: &mut Report, refs: &Refs, i: usize, q: &str, r: &StepResult, t: &Tols) {
    rep.ints(refs, i, &format!("{q}.n"), &[r.n as i64]);
    rep.floats(
        refs,
        i,
        &format!("{q}.t_ms"),
        "grilles",
        &[r.t_ms.len()],
        &r.t_ms,
        t.grid,
    );
    rep.floats(
        refs,
        i,
        &format!("{q}.mean"),
        "réponses",
        &[r.mean.len()],
        &r.mean,
        t.step,
    );
    rep.floats(refs, i, &format!("{q}.std"), "réponses", &[r.std.len()], &r.std, t.step);
    let shape = [r.n, r.t_ms.len()];
    rep.floats(
        refs,
        i,
        &format!("{q}.responses"),
        "réponses",
        &shape,
        &r.responses,
        t.step,
    );
    rep.floats(
        refs,
        i,
        &format!("{q}.max_inputs"),
        "consignes max",
        &[r.max_inputs.len()],
        &r.max_inputs,
        0.0,
    );
    check_metrics(rep, refs, i, &format!("{q}.metrics"), &r.metrics, t);
    check_loop(rep, refs, i, &format!("{q}.loop"), &r.loop_, t);
}

fn check_step(rep: &mut Report, refs: &Refs, i: usize, out: &StepOutput, t: &Tols) {
    let Some(f) = out.files.first() else {
        rep.fail(i, "réponse indicielle : aucun fichier".into());
        return;
    };
    let (d, p) = (&f.data, "step[0]");
    rep.text(refs, i, &format!("{p}.label"), &f.label);
    rep.text(refs, i, &format!("{p}.color"), &hex(f.color));
    rep.text(refs, i, &format!("{p}.family"), &d.family);
    rep.ints(refs, i, &format!("{p}.mode"), &[d.mode]);
    for (ax, pid) in d.pids.iter().enumerate() {
        for (k, v) in [
            ("P", pid.p),
            ("I", pid.i),
            ("D", pid.d),
            ("Dmax", pid.dmax),
            ("F", pid.f),
        ] {
            rep.ints(refs, i, &format!("{p}.pids[{ax}].{k}"), &[v]);
        }
    }
    for (k, v) in &d.sliders.values {
        rep.scalar(refs, i, &format!("{p}.sliders.values|{k}"), "curseurs", *v, t.slider);
    }
    rep.flag(refs, i, &format!("{p}.sliders.from_header"), d.sliders.from_header);
    rep.flag(refs, i, &format!("{p}.sliders.consistent"), d.sliders.consistent);
    for (ax, a) in d.axes.iter().enumerate() {
        let q = format!("{p}.axes[{ax}]");
        check_step_result(rep, refs, i, &format!("{q}.res"), &a.res, t);
        rep.floats(
            refs,
            i,
            &format!("{q}.seg_peak"),
            "pic et latence par fenêtre",
            &[a.seg_peak.len()],
            &a.seg_peak,
            t.step,
        );
        rep.floats(
            refs,
            i,
            &format!("{q}.seg_latency"),
            "pic et latence par fenêtre",
            &[a.seg_latency.len()],
            &a.seg_latency,
            t.step,
        );
        rep.scalar(refs, i, &format!("{q}.sp_delay_ms"), "retards", a.sp_delay_ms, t.delay);
    }
    // le diagnostic : roll et pitch
    let fl = &d.flight;
    let q = format!("{p}.flight");
    rep.text(refs, i, &format!("{q}.label"), &f.label);
    for (k, v) in &fl.sliders {
        rep.scalar(refs, i, &format!("{q}.sliders|{k}"), "curseurs", *v, t.slider);
    }
    let mut sig = Vec::new();
    sig_keys(&format!("{q}.filters"), &fl.filters, &mut sig);
    for (k, v) in &sig {
        rep.ints(refs, i, k, &[*v]);
    }
    rep.ints(refs, i, &format!("{q}.conditions.ff[0]"), &[fl.conditions.ff.0]);
    rep.ints(refs, i, &format!("{q}.conditions.ff[1]"), &[fl.conditions.ff.1]);
    rep.scalar(
        refs,
        i,
        &format!("{q}.conditions.angle_share"),
        "conditions de vol",
        fl.conditions.angle_share,
        t.step,
    );
    rep.scalar(
        refs,
        i,
        &format!("{q}.conditions.i_slider"),
        "conditions de vol",
        fl.conditions.i_slider,
        t.step,
    );
    for k in 0..fl.metrics.len() {
        check_metrics(rep, refs, i, &format!("{q}.metrics[{k}]"), &fl.metrics[k], t);
        rep.ints(refs, i, &format!("{q}.n[{k}]"), &[fl.n[k] as i64]);
        rep.scalar(
            refs,
            i,
            &format!("{q}.sp_delay_ms[{k}]"),
            "retards",
            fl.sp_delay_ms[k],
            t.delay,
        );
        check_loop(rep, refs, i, &format!("{q}.loops[{k}]"), &fl.loops[k], t);
    }
}

fn check_sliders(rep: &mut Report, refs: &Refs, i: usize, out: &PidOutput, t: &Tols) {
    let Some(f) = out.files.first() else {
        rep.fail(i, "curseurs PID : aucun fichier".into());
        return;
    };
    let p = "sliders[0]";
    rep.text(refs, i, &format!("{p}.label"), &f.label);
    rep.text(refs, i, &format!("{p}.color"), &hex(f.color));
    rep.ints(refs, i, &format!("{p}.mode"), &[f.mode]);
    for (k, v) in &f.view.values {
        rep.scalar(refs, i, &format!("{p}.view.values|{k}"), "curseurs", *v, t.slider);
    }
    rep.flag(refs, i, &format!("{p}.view.from_header"), f.view.from_header);
    rep.flag(refs, i, &format!("{p}.view.consistent"), f.view.consistent);
}

// ------------------------------------------------------------------ Simulateur de filtres et Assistant (6-4c, 6-4a)

fn check_lowpass(rep: &mut Report, refs: &Refs, i: usize, p: &str, l: &Lowpass) {
    rep.text(refs, i, &format!("{p}.type"), l.kind);
    for (k, v) in [
        ("static_hz", l.static_hz),
        ("dyn_min_hz", l.dyn_min_hz),
        ("dyn_max_hz", l.dyn_max_hz),
        ("expo", l.expo),
    ] {
        rep.ints(refs, i, &format!("{p}.{k}"), &[v]);
    }
}

/// Tous les champs de `Settings` (`filter_sim.base`, `filter_sim.mod`), y compris l'en-tête brut (`raw|nom`).
fn check_settings(rep: &mut Report, refs: &Refs, i: usize, p: &str, s: &Settings) {
    for (k, v) in [
        ("firmware", &s.firmware),
        ("firmware_name", &s.firmware_name),
        ("board", &s.board),
        ("craft_name", &s.craft_name),
        ("debug_mode", &s.debug_mode),
    ] {
        rep.text(refs, i, &format!("{p}.{k}"), v);
    }
    for (k, v) in s.version.iter().enumerate() {
        rep.ints(refs, i, &format!("{p}.version[{k}]"), &[*v]);
    }
    rep.no_more(refs, i, &format!("{p}.version[{}]", s.version.len()));
    for (k, v) in [
        ("looptime_us", s.looptime_us),
        ("pid_denom", s.pid_denom),
        ("debug_index", s.debug_index),
        ("yaw_lowpass_hz", s.yaw_lowpass_hz),
        ("dyn_notch_count", s.dyn_notch_count),
        ("dyn_notch_q", s.dyn_notch_q),
        ("dyn_notch_min_hz", s.dyn_notch_min_hz),
        ("dyn_notch_max_hz", s.dyn_notch_max_hz),
        ("rpm_harmonics", s.rpm_harmonics),
        ("rpm_q", s.rpm_q),
        ("rpm_min_hz", s.rpm_min_hz),
        ("rpm_fade_hz", s.rpm_fade_hz),
        ("rpm_lpf_hz", s.rpm_lpf_hz),
        ("motor_poles", s.motor_poles),
        ("motor_protocol", s.motor_protocol),
        ("minthrottle", s.minthrottle),
        ("maxthrottle", s.maxthrottle),
        ("acc_1g", s.acc_1g),
        ("motor_kv", s.motor_kv),
    ] {
        rep.ints(refs, i, &format!("{p}.{k}"), &[v]);
    }
    for (k, v) in [
        ("p_interval", s.p_interval),
        ("dterm_notch", s.dterm_notch),
        ("motor_output", s.motor_output),
    ] {
        rep.ints(refs, i, &format!("{p}.{k}[0]"), &[v.0]);
        rep.ints(refs, i, &format!("{p}.{k}[1]"), &[v.1]);
    }
    rep.flag(refs, i, &format!("{p}.dshot_bidir"), s.dshot_bidir);
    rep.flag(refs, i, &format!("{p}.high_resolution"), s.high_resolution);
    for (ax, pid) in s.pids.iter().enumerate() {
        for (k, v) in [
            ("P", pid.p),
            ("I", pid.i),
            ("D", pid.d),
            ("Dmax", pid.dmax),
            ("F", pid.f),
        ] {
            rep.ints(refs, i, &format!("{p}.pids[{ax}].{k}"), &[v]);
        }
    }
    for (k, l) in [
        ("gyro_lpf1", &s.gyro_lpf1),
        ("gyro_lpf2", &s.gyro_lpf2),
        ("dterm_lpf1", &s.dterm_lpf1),
        ("dterm_lpf2", &s.dterm_lpf2),
    ] {
        check_lowpass(rep, refs, i, &format!("{p}.{k}"), l);
    }
    for (k, (c, cut)) in s.gyro_notches.iter().enumerate() {
        rep.ints(refs, i, &format!("{p}.gyro_notches[{k}][0]"), &[*c]);
        rep.ints(refs, i, &format!("{p}.gyro_notches[{k}][1]"), &[*cut]);
    }
    rep.no_more(refs, i, &format!("{p}.gyro_notches[{}][0]", s.gyro_notches.len()));
    for (k, v) in s.rpm_weights.iter().enumerate() {
        rep.ints(refs, i, &format!("{p}.rpm_weights[{k}]"), &[*v]);
    }
    rep.no_more(refs, i, &format!("{p}.rpm_weights[{}]", s.rpm_weights.len()));
    // en-tête brut : mêmes noms, mêmes textes
    for (k, v) in &s.raw {
        rep.text(refs, i, &format!("{p}.raw|{k}"), v);
    }
    let have = refs.keys().filter(|k| k.starts_with(&format!("{p}.raw|"))).count();
    if have != s.raw.len() {
        rep.fail(i, format!("{p}.raw : {have} lignes d'en-tête ≠ {}", s.raw.len()));
    }
}

fn check_filter_sim(rep: &mut Report, refs: &Refs, i: usize, li: &LogInput, r: &FilterSimResult, t: &Tols) {
    let p = "filter_sim";
    rep.text(refs, i, &format!("{p}.label"), &li.label);
    rep.text(refs, i, &format!("{p}.color"), &hex(li.color));
    rep.scalar(refs, i, &format!("{p}.fs"), "fréquence du log", r.fs, 0.0);
    rep.flag(refs, i, &format!("{p}.has_unfilt"), r.has_unfilt);
    rep.floats(
        refs,
        i,
        &format!("{p}.freqs"),
        "simulateur.freqs",
        &[r.freqs.len()],
        &r.freqs,
        t.grid,
    );
    for (ax, v) in r.d_scale.iter().enumerate() {
        rep.scalar(refs, i, &format!("{p}.d_scale[{ax}]"), "échelle du D", *v, 0.0);
    }
    rep.no_more(refs, i, &format!("{p}.d_scale[{}]", r.d_scale.len()));
    for (k, axes) in &r.spectra {
        for (ax, sp) in axes.iter().enumerate() {
            let key = format!("{p}.spectra|{k}[{ax}]");
            match sp {
                Some(v) => rep.floats(refs, i, &key, "simulateur.spectres", &[v.len()], v, t.db),
                None => rep.text(refs, i, &key, "None"),
            }
        }
    }
    check_settings(rep, refs, i, &format!("{p}.base"), &r.base);
    check_settings(rep, refs, i, &format!("{p}.mod"), &r.modf);
}

fn check_profile(rep: &mut Report, refs: &Refs, i: usize, p: &str, c: &CraftProfile) {
    rep.text(refs, i, &format!("{p}.key"), c.key);
    rep.text(refs, i, &format!("{p}.label"), c.label);
    for (k, v) in [
        ("hover_hz", c.hover_hz),
        ("resonance_hz", c.resonance_hz),
        ("latency_ms", c.latency_ms),
    ] {
        rep.scalar(refs, i, &format!("{p}.{k}[0]"), "profil", v.0, 0.0);
        rep.scalar(refs, i, &format!("{p}.{k}[1]"), "profil", v.1, 0.0);
    }
    for (k, v) in [
        ("gyro_delay_max_ms", c.gyro_delay_max_ms),
        ("dterm_delay_max_ms", c.dterm_delay_max_ms),
        ("gyro_noise_high_db", c.gyro_noise_high_db),
        ("dterm_noise_high_db", c.dterm_noise_high_db),
        ("low_band_high_db", c.low_band_high_db),
        ("motor_left_high_db", c.motor_left_high_db),
        ("imbalance_high_pct", c.imbalance_high_pct),
        ("iterm_offset_high", c.iterm_offset_high),
        ("saturation_high_pct", c.saturation_high_pct),
        ("roll_pitch_gap_high", c.roll_pitch_gap_high),
        ("resonance_high_db", c.resonance_high_db),
    ] {
        rep.scalar(refs, i, &format!("{p}.{k}"), "profil", v, 0.0);
    }
    rep.flag(refs, i, &format!("{p}.provisional"), c.provisional);
    match c.nominal_inch {
        Some(v) => rep.scalar(refs, i, &format!("{p}.nominal_inch"), "profil", v, 0.0),
        None => rep.text(refs, i, &format!("{p}.nominal_inch"), "None"),
    }
}

fn check_finding(rep: &mut Report, refs: &Refs, i: usize, p: &str, f: &Finding) {
    for (k, v) in [
        ("key", &f.key),
        ("category", &f.category),
        ("severity", &f.severity),
        ("title", &f.title),
        ("detail", &f.detail),
    ] {
        rep.text(refs, i, &format!("{p}.{k}"), v);
    }
    rep.text(refs, i, &format!("{p}.axis"), f.axis.as_deref().unwrap_or("None"));
    for (j, a) in f.actions.iter().enumerate() {
        rep.text(refs, i, &format!("{p}.actions[{j}]"), a);
    }
    rep.no_more(refs, i, &format!("{p}.actions[{}]", f.actions.len()));
    for (j, a) in f.settings.iter().enumerate() {
        rep.text(refs, i, &format!("{p}.settings[{j}]"), a);
    }
    rep.no_more(refs, i, &format!("{p}.settings[{}]", f.settings.len()));
}

/// Un `MeanSd` (`<p>.mean`, `.sd`, `.max`).
fn check_mean_sd(rep: &mut Report, refs: &Refs, i: usize, p: &str, m: &MeanSd, t: &Tols) {
    rep.num(refs, i, &format!("{p}.mean"), "mesures : statistiques", m.mean, t.stats);
    rep.num(refs, i, &format!("{p}.sd"), "mesures : statistiques", m.sd, t.stats);
    rep.num(refs, i, &format!("{p}.max"), "mesures : statistiques", m.max, t.stats);
}

fn check_measures(rep: &mut Report, refs: &Refs, i: usize, m: &Measures, t: &Tols) {
    let k = "advisor[1].measures";
    let fam_noise = "mesures : bruit";
    let fam_filter = "mesures : filtres";
    rep.opt_num(
        refs,
        i,
        &format!("{k}|hover_motor_hz"),
        "mesures : régime",
        m.hover_motor_hz,
        t.stats,
    );
    if let Some((b, h)) = m.log_rate_needed_hz {
        rep.num(
            refs,
            i,
            &format!("{k}|log_rate_needed_hz[0]"),
            "mesures : régime",
            b,
            t.stats,
        );
        rep.num(
            refs,
            i,
            &format!("{k}|log_rate_needed_hz[1]"),
            "mesures : régime",
            h,
            t.stats,
        );
    } else {
        rep.fail(i, "mesure log_rate_needed_hz absente de Rust".into());
    }
    match m.aliased {
        Some(a) => rep.flag(refs, i, &format!("{k}|aliased"), a),
        None => rep.fail(i, "mesure aliased absente de Rust".into()),
    }
    for (axis, sm) in m.steps.iter().flatten() {
        check_metrics(rep, refs, i, &format!("{k}|steps|{axis}"), sm, t);
    }
    if let Some(d) = &m.delays {
        rep.num(refs, i, &format!("{k}|delays.gyro_ms"), "retards", d.gyro_ms, t.delay);
        rep.num(refs, i, &format!("{k}|delays.dterm_ms"), "retards", d.dterm_ms, t.delay);
        for (a, v) in d.sp_gyro_ms.iter().enumerate() {
            rep.num(refs, i, &format!("{k}|delays.sp_gyro_ms[{a}]"), "retards", *v, t.delay);
        }
        rep.num(
            refs,
            i,
            &format!("{k}|delays.stick_p95_dps"),
            "retards",
            d.stick_p95_dps,
            t.stats,
        );
    } else {
        rep.fail(i, "mesure delays absente de Rust".into());
    }
    for (name, v) in [
        ("gyro_noise_db", &m.gyro_noise_db),
        ("dterm_noise_db", &m.dterm_noise_db),
        ("low_band_db", &m.low_band_db),
    ] {
        for (a, x) in v.iter().flatten().enumerate() {
            rep.num(refs, i, &format!("{k}|{name}[{a}]"), fam_noise, *x, t.noise);
        }
        rep.no_more(refs, i, &format!("{k}|{name}[{}]", v.as_ref().map_or(0, Vec::len)));
    }
    rep.opt_num(
        refs,
        i,
        &format!("{k}|gyro_peak_ptb_db"),
        fam_noise,
        m.gyro_peak_ptb_db,
        t.noise,
    );
    rep.opt_num(
        refs,
        i,
        &format!("{k}|dterm_peak_ptb_db"),
        fam_noise,
        m.dterm_peak_ptb_db,
        t.noise,
    );
    // mesures de filtres : absentes de Python quand Rust les a à `None` (sauf frame_resonance et dyn_notch, écrits « None »)
    for (name, v) in [
        ("motor_harmonic_left_db", m.motor_harmonic_left_db),
        ("motor_harmonic_raw_db", m.motor_harmonic_raw_db),
    ] {
        if let Some(v) = v {
            rep.num(refs, i, &format!("{k}|{name}"), fam_filter, v, t.filter);
        } else {
            rep.no_more(refs, i, &format!("{k}|{name}"));
        }
    }
    match &m.frame_resonance {
        Some(r) => {
            rep.num(
                refs,
                i,
                &format!("{k}|frame_resonance[0]"),
                fam_filter,
                r.freq_hz,
                t.filter,
            );
            rep.num(
                refs,
                i,
                &format!("{k}|frame_resonance[1]"),
                fam_filter,
                r.excess_db,
                t.filter,
            );
            rep.flag(refs, i, &format!("{k}|frame_resonance[2]"), r.near_motor);
        }
        None => rep.text(refs, i, &format!("{k}|frame_resonance"), "None"),
    }
    match &m.dyn_notch {
        Some(d) => {
            rep.ints(refs, i, &format!("{k}|dyn_notch|count"), &[d.count]);
            rep.ints(refs, i, &format!("{k}|dyn_notch|min_hz"), &[d.min_hz]);
            rep.ints(refs, i, &format!("{k}|dyn_notch|max_hz"), &[d.max_hz]);
            for (name, v) in [
                ("peaks", &d.peaks),
                ("widths", &d.widths),
                ("heights", &d.heights),
                ("left", &d.left),
            ] {
                for (a, x) in v.iter().enumerate() {
                    rep.num(refs, i, &format!("{k}|dyn_notch|{name}[{a}]"), fam_filter, *x, t.filter);
                }
                rep.no_more(refs, i, &format!("{k}|dyn_notch|{name}[{}]", v.len()));
            }
            rep.opt_num(
                refs,
                i,
                &format!("{k}|dyn_notch|q_need"),
                fam_filter,
                d.q_need,
                t.filter,
            );
        }
        None => rep.text(refs, i, &format!("{k}|dyn_notch"), "None"),
    }
    for (a, ln) in m.hover_lines.iter().flatten().enumerate() {
        rep.num(
            refs,
            i,
            &format!("{k}|hover_lines[{a}].freq_hz"),
            fam_filter,
            ln.freq_hz,
            t.filter,
        );
        rep.num(
            refs,
            i,
            &format!("{k}|hover_lines[{a}].raw_db"),
            fam_filter,
            ln.raw_db,
            t.filter,
        );
        rep.num(
            refs,
            i,
            &format!("{k}|hover_lines[{a}].left_db"),
            fam_filter,
            ln.left_db,
            t.filter,
        );
    }
    rep.no_more(
        refs,
        i,
        &format!(
            "{k}|hover_lines[{}].freq_hz",
            m.hover_lines.as_ref().map_or(0, Vec::len)
        ),
    );
    if let Some(s) = &m.stats {
        let q = format!("{k}|stats");
        rep.num(
            refs,
            i,
            &format!("{q}.duration_s"),
            "mesures : statistiques",
            s.duration_s,
            t.stats,
        );
        for (name, centres, hist) in &s.stick_hist {
            for (n, v) in [centres, hist].into_iter().enumerate() {
                rep.floats(
                    refs,
                    i,
                    &format!("{q}.stick_hist|{name}[{n}]"),
                    "mesures : histogrammes",
                    &[v.len()],
                    v,
                    t.stats,
                );
            }
        }
        for (name, v) in &s.terms {
            for (a, ms) in v.iter().enumerate() {
                check_mean_sd(rep, refs, i, &format!("{q}.terms|{name}[{a}]"), ms, t);
            }
        }
        for (a, ms) in s.motors.iter().enumerate() {
            check_mean_sd(rep, refs, i, &format!("{q}.motors[{a}]"), ms, t);
        }
        rep.no_more(refs, i, &format!("{q}.motors[{}].mean", s.motors.len()));
        rep.num(
            refs,
            i,
            &format!("{q}.motor_saturation_pct"),
            "mesures : statistiques",
            s.motor_saturation_pct,
            t.stats,
        );
        rep.num(
            refs,
            i,
            &format!("{q}.motor_imbalance_pct"),
            "mesures : statistiques",
            s.motor_imbalance_pct,
            t.stats,
        );
        check_mean_sd(rep, refs, i, &format!("{q}.throttle"), &s.throttle, t);
        for (name, v) in [("vbat", &s.vbat), ("current", &s.current)] {
            match v {
                Some(ms) => check_mean_sd(rep, refs, i, &format!("{q}.{name}"), ms, t),
                None => rep.text(refs, i, &format!("{q}.{name}"), "None"),
            }
        }
        rep.opt_num(
            refs,
            i,
            &format!("{q}.vbat_min"),
            "mesures : statistiques",
            s.vbat_min,
            t.stats,
        );
        rep.opt_num(refs, i, &format!("{q}.mah"), "mesures : statistiques", s.mah, t.stats);
    } else {
        rep.fail(i, "mesure stats absente de Rust".into());
    }
    for (name, v) in [("pid_error_sd", &m.pid_error_sd), ("iterm_mean", &m.iterm_mean)] {
        for (a, x) in v.iter().flatten().enumerate() {
            rep.num(
                refs,
                i,
                &format!("{k}|{name}[{a}]"),
                "mesures : statistiques",
                *x,
                t.stats,
            );
        }
        rep.no_more(refs, i, &format!("{k}|{name}[{}]", v.as_ref().map_or(0, Vec::len)));
    }
    if let Some(sv) = &m.sliders {
        for (name, v) in &sv.values {
            rep.num(refs, i, &format!("{k}|sliders.values|{name}"), "curseurs", *v, t.slider);
        }
        rep.flag(refs, i, &format!("{k}|sliders.from_header"), sv.from_header);
        rep.flag(refs, i, &format!("{k}|sliders.consistent"), sv.consistent);
    } else {
        rep.fail(i, "mesure sliders absente de Rust".into());
    }
}

fn check_advisor(rep: &mut Report, refs: &Refs, i: usize, li: &LogInput, out: &AdvisorOutput, t: &Tols) {
    // `advisor[0]` : l'entrée du log (période, couleur, nom) ; son `.log.` est prouvé par `otb-parity check-flightlog`
    rep.text(refs, i, "advisor[0].label", &out.label);
    rep.text(refs, i, "advisor[0].color", &hex(out.color));
    rep.scalar(refs, i, "advisor[0].period[0]", "périodes", li.period.0, t.grid);
    rep.scalar(refs, i, "advisor[0].period[1]", "périodes", li.period.1, t.grid);
    let r = &out.report;
    let p = "advisor[1]";
    check_profile(rep, refs, i, &format!("{p}.profile"), &r.profile);
    rep.text(refs, i, &format!("{p}.profile_reason"), &r.profile_reason);
    rep.scalar(refs, i, &format!("{p}.period[0]"), "périodes", r.period.0, t.grid);
    rep.scalar(refs, i, &format!("{p}.period[1]"), "périodes", r.period.1, t.grid);
    for (n, f) in r.findings.iter().enumerate() {
        check_finding(rep, refs, i, &format!("{p}.findings[{n}]"), f);
    }
    rep.no_more(refs, i, &format!("{p}.findings[{}].key", r.findings.len()));
    for (n, st) in r.steps.iter().enumerate() {
        rep.text(refs, i, &format!("{p}.steps[{n}].axis"), &st.axis);
        check_step_result(rep, refs, i, &format!("{p}.steps[{n}].result"), &st.result, t);
    }
    rep.no_more(refs, i, &format!("{p}.steps[{}].axis", r.steps.len()));
    check_measures(rep, refs, i, &r.measures, t);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("dossier des références (…/golden)"));
    let mut tol_override: Option<f64> = None;
    while let Some(a) = args.next() {
        if a == "--tol" {
            tol_override = args.next().and_then(|v| v.parse().ok());
        }
    }
    // tolérances posées d'après la mesure (docs/migration-rust.md, 6-2) ; `--tol X` les remplace toutes (pour mesurer)
    let mut t = Tols {
        grid: 1e-12,
        db: 1e-9,
        delay: 1e-9,
        trace: 2e-6,
        step: 1e-10,
        open_loop: 1e-8,
        slider: 1e-11,
        noise: 1e-12,
        filter: 1e-11,
        stats: 0.0,
    };
    if let Some(x) = tol_override {
        t = Tols {
            grid: x,
            db: x,
            delay: x,
            trace: x,
            step: x,
            open_loop: x,
            slider: x,
            noise: x,
            filter: x,
            stats: x,
        };
    }
    let root: &Path = dir.parent().expect("dossier parent des logs");
    let list: Vec<String> = serde_json::from_str(&std::fs::read_to_string(dir.join("tools.json")).expect("tools.json"))
        .expect("tools.json illisible");
    let mut rep = Report::default();
    let (mut compared, mut ignored) = (0usize, 0usize);
    for (i, rel) in list.iter().enumerate() {
        let refs = load_arrays(&dir.join("tools").join(format!("{i:03}.npz"))).expect("référence");
        let session = refs["_session"].get_f64(0) as i64;
        let logs = otb_log::flightlog::load(&root.join(rel)).unwrap_or_else(|e| panic!("{rel} : {e}"));
        let Some(log) = logs.into_iter().find(|l| l.session as i64 == session) else {
            rep.fail(i, format!("session {session} introuvable dans {rel}"));
            continue;
        };
        let mut st = AppState::new();
        st.add(log);
        let t0 = std::time::Instant::now();

        let spec = SpectralTool::new();
        match SpectralTool::compute(spec.options(), st.inputs(&[0])) {
            Ok(out) => check_spectral(&mut rep, &refs, i, &out, &t),
            Err(e) => rep.fail(i, format!("spectre : erreur {e}")),
        }
        let mut fgaz = FreqThrottleTool::new();
        let mut cols = fgaz.columns();
        cols.iter_mut().for_each(|c| c.1 = Some(0));
        fgaz.set_columns(cols);
        match FreqThrottleTool::compute(fgaz.options(), st.inputs(&[0])) {
            Ok(out) => check_freq_throttle(&mut rep, &refs, i, &out, &t),
            Err(e) => rep.fail(i, format!("fréquence × gaz : erreur {e}")),
        }
        match FreqTimeTool::compute(FreqTimeTool::new().options(), st.inputs(&[0])) {
            Ok(out) => check_freq_time(&mut rep, &refs, i, &out, &t),
            Err(e) => rep.fail(i, format!("fréquence × temps : erreur {e}")),
        }
        match StepTool::compute(
            StepOptions {
                calc: StepOpts::default(),
            },
            st.inputs(&[0]),
        ) {
            Ok(out) => check_step(&mut rep, &refs, i, &out, &t),
            Err(e) => rep.fail(i, format!("réponse indicielle : erreur {e}")),
        }
        match PidSliderTool::compute(Default::default(), st.inputs(&[0])) {
            Ok(out) => check_sliders(&mut rep, &refs, i, &out, &t),
            Err(e) => rep.fail(i, format!("curseurs PID : erreur {e}")),
        }
        match FilterSimTool::compute(
            FilterSimOptions {
                applied: None,
                with_current: true,
            },
            st.inputs(&[0]),
        ) {
            Ok(out) => check_filter_sim(&mut rep, &refs, i, &st.inputs(&[0])[0], &out, &t),
            Err(e) => rep.fail(i, format!("simulateur de filtres : erreur {e}")),
        }
        match AdvisorTool::compute(AdvisorOptions { profile: None }, st.inputs(&[0])) {
            Ok(out) => check_advisor(&mut rep, &refs, i, &st.inputs(&[0])[0], &out, &t),
            Err(e) => rep.fail(i, format!("assistant : erreur {e}")),
        }
        // toute clé de Python d'un des sept outils doit avoir été rejouée
        for k in refs.keys() {
            let ported = [
                "spectral.",
                "freq_throttle.",
                "freq_time.",
                "step[",
                "sliders[",
                "filter_sim.",
                "advisor[",
            ]
            .iter()
            .any(|p| k.starts_with(p));
            if !ported {
                ignored += 1;
            } else if k.contains(".entry.") || k.starts_with("sliders[0].settings.") || k.starts_with("advisor[0].log.")
            {
                ignored += 1; // champs du journal et réglages : couverts par les couches `flightlog` et `settings` (680 sessions)
            } else if rep.used.contains(k) {
                compared += 1;
            } else {
                rep.fail(i, format!("clé de Python jamais rejouée : {k}"));
            }
        }
        rep.used.clear();
        println!("{i:03} {rel} : {:.1} s", t0.elapsed().as_secs_f64());
    }
    println!(
        "\n{} journaux, {compared} clés rejouées, {ignored} clés ignorées (autres outils, champs du journal)",
        list.len()
    );
    for (family, (worst, n)) in &rep.dev {
        println!("  {family:<18} {n:>9} valeurs, écart maximal {worst:.2e}");
    }
    if rep.fails.is_empty() {
        println!("0 écart");
    } else {
        println!("\n{} ÉCARTS (40 premiers) :", rep.fails.len());
        for f in &rep.fails {
            println!("  {f}");
        }
        std::process::exit(1);
    }
}
