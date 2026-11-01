//! Parité des calculs des outils spectraux avec Python (phase 6-2) : rejoue, avec les réglages par défaut, `compute` des
//! trois outils sur les 20 journaux de `golden.py record` (`DOSSIER/tools/NNN.npz`) et compare **chaque clé** de Python.
//!
//! `cargo run --release -p otb-gui --example check_tools -- $OTB_LOGS/golden [--tol X]`
//!
//! Décisions discrètes (clés présentes, formes, comptes, textes, booléens, emplacement des NaN) : identiques, sans tolérance.
//! Valeurs continues : écart rapporté au plus grand élément fini de la référence ≤ tolérance de la famille (mesurées avant
//! d'être posées : voir `docs/migration-rust.md`, 6-2). Toute clé de Python d'un outil porté doit être rejouée.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use otb_gui::state::AppState;
use otb_gui::tool::Tool;
use otb_gui::tools::freq_throttle::{FreqThrottleOutput, FreqThrottleTool};
use otb_gui::tools::freq_time::{FreqTimeOutput, FreqTimeTool};
use otb_gui::tools::spectral::{SpectralOutput, SpectralTool};
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
    for (n, c) in [("motor_hz", out.motor_hz.len())] {
        let have = (0..64)
            .filter(|k| refs.contains_key(&format!("freq_time.{n}[{k}]")))
            .count();
        if have != c {
            rep.fail(i, format!("freq_time.{n} : {have} listes ≠ {c}"));
        }
    }
    let have = (0..64)
        .filter(|k| refs.contains_key(&format!("freq_time.notches[{k}][0]")))
        .count();
    if have != out.notches.len() {
        rep.fail(i, format!("freq_time.notches : {have} listes ≠ {}", out.notches.len()));
    }
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
    };
    if let Some(x) = tol_override {
        t = Tols {
            grid: x,
            db: x,
            delay: x,
            trace: x,
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
        // toute clé de Python d'un des trois outils doit avoir été rejouée
        for k in refs.keys() {
            let ported = ["spectral.", "freq_throttle.", "freq_time."]
                .iter()
                .any(|p| k.starts_with(p));
            if !ported {
                ignored += 1;
            } else if k.contains(".entry.") {
                ignored += 1; // champs du journal lui-même, couverts par la couche `flightlog`
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
