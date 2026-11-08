//! `otb-dev corpus calibrate` : mesures brutes du corpus qui servent à calibrer les seuils de l'assistant (remplace
//! `scripts/calibrate_advisor.py`, docs/assistant.md, « Calibration »).
//!
//! ```text
//! otb-dev corpus calibrate NB_FICHIERS SORTIE.json [-j FILS]   mesures de chaque session (dossier : OTB_LOGS)
//! otb-dev corpus calibrate --resume SORTIE.json                p10 / p50 / p90 par taille de drone (sans recalcul)
//! ```
//!
//! Mêmes sessions et mêmes clés que `calibrate_advisor.py` : les deux outils se lisent l'un l'autre (JSON avec `NaN` et
//! `Infinity`, voir `json.rs`). Différence assumée : Python tire ses `NB_FICHIERS` au hasard (`random.seed(0)`), ce que Rust ne
//! reproduit pas ; Rust prend les premiers dans l'ordre de la liste. Avec tout le corpus, les deux traitent les mêmes fichiers.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::Path;

use otb_advisor::profiles::{PROFILES, detect_profile, profile_key};
use otb_advisor::rules::{band_db, band_peak_ptb_db, frame_resonance};
use otb_analysis::delay::{MOVING_SETPOINT_DPS, compute_delays, stick_p95_dps, xcorr_delay_ms};
use otb_analysis::motors::{harmonic_prominence, hover_motor_hz, motor_noise};
use otb_analysis::npstat::{mean_f32, median_f64};
use otb_analysis::open_loop::{MEASURED, Terms};
use otb_analysis::stats::flight_stats;
use otb_analysis::step::{LoopInput, StepOptions, step_response};
use otb_analysis::step_series::flight_conditions;
use otb_log::flightlog::{FlightLog, Rows, clamp_range, load};
use serde_json::{Map, Value, json};

use super::json::{as_f64, dump, num, parse};
use super::{Args, Output, analysed, drone_id, log_files, logs_root, map_files, rel};

fn row_of<'a>(r: &'a Rows, ax: usize, sel: &Range<usize>) -> &'a [f32] {
    let row = r.row(ax);
    &row[clamp_range(sel.start, sel.end, row.len())]
}

/// `log.motor_hz[:, sel]`.
fn cols_of(r: &Rows, sel: &Range<usize>) -> Rows {
    let rg = clamp_range(sel.start, sel.end, r.n);
    let mut data = Vec::with_capacity(r.k * rg.len());
    for k in 0..r.k {
        data.extend_from_slice(&r.row(k)[rg.clone()]);
    }
    Rows {
        k: r.k,
        n: rg.len(),
        data,
    }
}

fn wide(v: &[f32]) -> Vec<f64> {
    v.iter().map(|&x| x as f64).collect()
}

/// `np.any(row)` d'une ligne `float32` : un NaN compte comme non nul.
fn any_nonzero(row: &[f32]) -> bool {
    row.iter().any(|&v| v != 0.0)
}

fn nums(v: &[f64]) -> Value {
    Value::Array(v.iter().map(|&x| num(x)).collect())
}

/// Les mesures de la session `lg` sur la période `sel` (les clés de `measure_file` de Python, dans le même ordre).
fn measure_session(lg: &FlightLog, sel: &Range<usize>, f: &Path, root: &Path) -> Result<Value, String> {
    let (prof, _) = detect_profile(lg);
    let s = &lg.settings;
    let mut r = Map::new();
    r.insert("file".into(), json!(rel(f, root)));
    r.insert("sess".into(), json!(lg.session));
    r.insert("profile".into(), json!(prof.key));
    r.insert("fs".into(), num(lg.fs));
    r.insert("craft".into(), json!(s.craft_name));
    r.insert("hover_hz".into(), hover_motor_hz(lg).map_or(Value::Null, num));
    r.insert("cells".into(), lg.cells().map_or(Value::Null, |c| json!(c)));
    r.insert("corrupt".into(), json!(lg.stats.corrupt_frames));
    let sliders: Map<String, Value> = s
        .sliders()
        .into_iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect();
    r.insert("sliders".into(), Value::Object(sliders));

    let cond = flight_conditions(lg, sel.clone())?;
    // aucune fenêtre sur un trou du log (D-020)
    let br: Vec<i64> = lg.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let fs = lg.fs;
    let band = |x: &[f32], lo: f64, hi: f64| band_db(x, fs, lo, hi, Some(&br));
    r.insert("rpmq".into(), json!(s.rpm_q));
    r.insert("rpmh".into(), json!(s.rpm_harmonics));
    r.insert("rpmw".into(), json!(s.rpm_weights));
    let mhz = lg.motor_hz.as_ref().map(|m| cols_of(m, sel));
    for a in 0..2 {
        let gyro = row_of(&lg.gyro, a, sel);
        r.insert(format!("gyro_{a}"), num(band(gyro, 80.0, 500.0)?));
        r.insert(
            format!("gptb_{a}"),
            num(band_peak_ptb_db(gyro, fs, 80.0, 500.0, Some(&br))?),
        );
        if any_nonzero(lg.d_term.row(a)) {
            let d = row_of(&lg.d_term, a, sel);
            r.insert(
                format!("dptb_{a}"),
                num(band_peak_ptb_db(d, fs, 80.0, 500.0, Some(&br))?),
            );
        }
        if let Some(unf) = &lg.gyro_unfilt {
            let unf = row_of(unf, a, sel);
            r.insert(
                format!("gdelayx_{a}"),
                num(xcorr_delay_ms(&wide(unf), &wide(gyro), fs, 30.0)?),
            );
            if let Some(mhz) = &mhz {
                // hauteur des harmoniques au-dessus du bruit voisin, avant / après filtres
                let raw = harmonic_prominence(&wide(unf), mhz, fs, 3, 300.0, 20.0, Some(&br))?;
                let left = harmonic_prominence(&wide(gyro), mhz, fs, 3, 300.0, 20.0, Some(&br))?;
                r.insert(format!("hprom_{a}"), nums(&raw));
                r.insert(format!("hleft_{a}"), nums(&left));
            }
            r.insert(format!("gyrou_{a}"), num(band(unf, 80.0, 500.0)?));
        }
        if any_nonzero(lg.d_term.row(a)) {
            r.insert(
                format!("dterm_{a}"),
                num(band(row_of(&lg.d_term, a, sel), 80.0, 500.0)?),
            );
        }
        r.insert(format!("lowband_{a}"), num(band(gyro, 20.0, 80.0)?));

        let (sp, gy) = (wide(row_of(&lg.setpoint, a, sel)), wide(gyro));
        let terms = lg.pid_terms(a, sel.clone());
        let (tp, ti, td, tf) = (wide(&terms.p), wide(&terms.i), wide(&terms.d), wide(&terms.f));
        let inst = terms.instrument.as_ref().map(|v| wide(v));
        let input = if terms.note.is_empty() {
            LoopInput::Terms(Terms {
                p: &tp,
                i: &ti,
                d: &td,
                f: &tf,
                instrument: inst.as_deref(),
            })
        } else {
            LoopInput::Unmeasurable(&terms.note)
        };
        let st = step_response(&sp, &gy, fs, &StepOptions::default(), &input, Some(&br))?;
        if st.n >= 20 {
            let (lp, m) = (&st.loop_, &st.metrics);
            r.insert(format!("peak_{a}"), num(m.peak));
            r.insert(format!("lat_{a}"), num(m.latency_ms));
            r.insert(format!("nwin_{a}"), json!(st.n));
            r.insert(format!("ripple_{a}"), num(m.ripple)); // vagues : mesure à vide du Diagnostic du réglage
            r.insert(format!("waves_{a}"), num(m.waves));
            r.insert(format!("wavesr_{a}"), num(m.waves_ratio));
            let pid = &s.pids[a];
            // pic selon le feedforward en service : 0 en mode Angle (L-005)
            r.insert(format!("ff_{a}"), json!(cond.ff_gain(a)));
            r.insert(format!("dgain_{a}"), json!(pid.d));
            r.insert(format!("pgain_{a}"), json!(pid.p));
            let measured = lp.method == MEASURED;
            r.insert(format!("smax_{a}"), num(lp.sensitivity_peak_db));
            r.insert(format!("pm_{a}"), num(lp.phase_margin_deg));
            r.insert(format!("measured_{a}"), json!(measured));
            r.insert(format!("smaxedge_{a}"), json!(lp.sensitivity_peak_at_edge));
            r.insert(format!("fc_{a}"), num(lp.crossover_hz));
            r.insert(format!("gm_{a}"), num(lp.gain_margin_db));
            r.insert(format!("dratio_{a}"), num(lp.d_ratio));
            r.insert(format!("bandtop_{a}"), num(lp.band_hz.1));
            r.insert(format!("track_{a}"), num(lp.tracking_peak_db));
            if measured {
                // mêmes mesures par l'ancienne formule T/(1 − T), pour comparaison
                let old = step_response(&sp, &gy, fs, &StepOptions::default(), &LoopInput::Absent, Some(&br))?.loop_;
                r.insert(format!("smaxold_{a}"), num(old.sensitivity_peak_db));
                r.insert(format!("pmold_{a}"), num(old.phase_margin_deg));
                r.insert(format!("fcold_{a}"), num(old.crossover_hz));
            }
        }
        if let Some(mhz) = &mhz {
            let mn = motor_noise(&wide(gyro), mhz, fs, 3, 300.0, 20.0, Some(&br))?;
            r.insert(format!("mnoise_{a}"), nums(&mn.mean_db));
        }
    }
    let d = compute_delays(lg, sel.clone())?;
    r.insert("gdelay".into(), num(d.gyro_ms));
    r.insert("ddelay".into(), num(d.dterm_ms));
    r.insert("sp95".into(), num(stick_p95_dps(lg, sel.clone())?));
    let fst = flight_stats(lg, sel.clone());
    r.insert("sat".into(), num(fst.motor_saturation_pct));
    r.insert("imb".into(), num(fst.motor_imbalance_pct));
    for a in 0..2 {
        r.insert(format!("imean_{a}"), num(mean_f32(row_of(&lg.i_term, a, sel)) as f64));
    }
    if let Some(res) = frame_resonance(lg, sel, 20.0)? {
        r.insert("res_hz".into(), num(res.freq_hz));
        r.insert("res_db".into(), num(res.excess_db));
        r.insert("res_near".into(), json!(res.near_motor));
    }
    Ok(Value::Object(r))
}

/// Mesures de chaque session analysable d'un log (au moins 8 s, période analysée d'au moins 5 s).
pub fn measure_file(f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        let Some(sel) = analysed(&lg) else { continue };
        out.push(measure_session(&lg, &sel, f, root)?);
    }
    Ok(out)
}

/// Clés qui ne sont pas des mesures à résumer (textes, listes, réglages) : écartées du tableau comme en Python.
const NOT_SUMMARISED: [&str; 16] = [
    "file", "sess", "profile", "craft", "drone", "res_near", "mnoise", "hprom", "hleft", "rpmw", "imean", "cells",
    "sliders", "ff", "dgain", "pgain",
];

/// `np.percentile(v, q)` (interpolation linéaire de numpy, `v` non vide).
fn percentile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let virt = (s.len() - 1) as f64 * (q / 100.0);
    let prev = virt.floor();
    let (a, b) = (s[prev as usize], s[(prev as usize + 1).min(s.len() - 1)]);
    let (diff, t) = (b - a, virt - prev);
    if t >= 0.5 { b - diff * (1.0 - t) } else { a + diff * t }
}

/// Les nombres finis (ni booléens, ni listes) des champs `k`, `k_0` et `k_1` des sessions.
fn vals(sub: &[&Map<String, Value>], k: &str) -> Vec<f64> {
    let names = [k.to_string(), format!("{k}_0"), format!("{k}_1")];
    sub.iter()
        .flat_map(|r| names.iter().filter_map(|n| r.get(n)))
        .filter_map(|x| if let Value::Number(n) = x { n.as_f64() } else { None })
        .filter(|x| x.is_finite())
        .collect()
}

fn field(r: &Map<String, Value>, k: &str) -> f64 {
    r.get(k).and_then(as_f64).unwrap_or(f64::NAN)
}

/// Mesures dérivées, telles que les règles les utilisent (reclassement par taille avec les limites actuelles).
fn derive(r: &mut Map<String, Value>) {
    if let Some(hz) = r.get("hover_hz").and_then(as_f64).filter(|h| *h != 0.0) {
        let cells = r.get("cells").and_then(Value::as_i64);
        r.insert("profile".into(), json!(profile_key(hz, cells)));
    }
    if r.contains_key("peak_0") && r.contains_key("peak_1") {
        r.insert("peakdiff".into(), num((field(r, "peak_1") - field(r, "peak_0")).abs()));
    }
    if r.contains_key("imean_0") {
        let (a, b) = (field(r, "imean_0").abs(), field(r, "imean_1").abs());
        r.insert("imean_abs".into(), num(if b > a { b } else { a })); // `max` de Python : le premier gagne sauf si le second est plus grand
    }
    for a in 0..2 {
        if let Some(first) = r
            .get(&format!("mnoise_{a}"))
            .and_then(Value::as_array)
            .and_then(|l| l.first())
            .and_then(as_f64)
        {
            r.insert(format!("mleft_{a}"), num(first));
        }
        for (k, n) in [("hprom", "prom"), ("hleft", "left")] {
            let list = r
                .get(&format!("{k}_{a}"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for (h, v) in list.iter().enumerate() {
                r.insert(format!("{n}{}_{a}", h + 1), v.clone());
            }
        }
    }
    let res_db = r.get("res_db").filter(|v| !v.is_null()).cloned();
    if let Some(db) = res_db
        && !r.get("res_near").and_then(Value::as_bool).unwrap_or(false)
    {
        r.insert("res_free_db".into(), db);
    }
    // vol sans mouvements : retards des filtres non représentatifs
    if r.get("sp95").and_then(as_f64).unwrap_or(1e9) < MOVING_SETPOINT_DPS {
        r.retain(|k, _| !k.starts_with("gdelay") && !k.starts_with("ddelay"));
    }
}

/// Le tableau de `--resume` : par taille puis pour tout le corpus (TOUS), p10 / p50 / p90 par session (roll et pitch regroupés) et
/// p10 / p90 avec une voix par drone (médiane de ses sessions ; drone = nom, sinon dossier).
pub fn resume_text(rows: &[Value]) -> Result<String, String> {
    let mut rows: Vec<Map<String, Value>> = rows
        .iter()
        .map(|r| r.as_object().cloned().ok_or("une ligne du JSON n'est pas un objet"))
        .collect::<Result<_, _>>()?;
    let drones: Vec<String> = rows
        .iter()
        .map(|r| {
            let (craft, file) = (
                r.get("craft").and_then(Value::as_str),
                r.get("file").and_then(Value::as_str),
            );
            drone_id(craft.unwrap_or(""), file.unwrap_or(""))
        })
        .collect();
    rows.iter_mut().for_each(derive);
    let keys: BTreeSet<String> = rows
        .iter()
        .flat_map(|r| r.keys())
        .map(|k| {
            if k.ends_with("_0") || k.ends_with("_1") {
                k[..k.len() - 2].to_string()
            } else {
                k.clone()
            }
        })
        .filter(|k| !NOT_SUMMARISED.contains(&k.as_str()))
        .collect();
    let mut text = String::new();
    let axes: Vec<(&Map<String, Value>, usize)> = rows
        .iter()
        .flat_map(|r| (0..2).map(move |a| (r, a)))
        .filter(|(r, a)| r.contains_key(&format!("measured_{a}")))
        .collect();
    if !axes.is_empty() {
        // boucle ouverte (analysis/loop.py) : part des axes mesurés, croisements trouvés, pics au bord
        let meas: Vec<_> = axes
            .iter()
            .filter(|(r, a)| r[&format!("measured_{a}")].as_bool().unwrap_or(false))
            .collect();
        let fc = meas
            .iter()
            .filter(|(r, a)| field(r, &format!("fc_{a}")).is_finite())
            .count();
        let edge = meas
            .iter()
            .filter(|(r, a)| r[&format!("smaxedge_{a}")].as_bool().unwrap_or(false))
            .count();
        let m = meas.len().max(1) as f64;
        text += &format!(
            "Boucle ouverte : {}/{} axes mesurés ({:.0} %), croisement trouvé {:.0} %, pic de sensibilité au bord {:.0} %\n",
            meas.len(),
            axes.len(),
            100.0 * meas.len() as f64 / axes.len() as f64,
            100.0 * fc as f64 / m,
            100.0 * edge as f64 / m,
        );
    }
    let names: Vec<&str> = PROFILES.iter().map(|p| p.key).chain(["TOUS"]).collect();
    for key in names {
        let idx: Vec<usize> = (0..rows.len())
            .filter(|&i| key == "TOUS" || rows[i].get("profile").and_then(Value::as_str) == Some(key))
            .collect();
        let sub: Vec<&Map<String, Value>> = idx.iter().map(|&i| &rows[i]).collect();
        let n_drones = idx.iter().map(|&i| &drones[i]).collect::<BTreeSet<_>>().len();
        text += &format!(
            "\n== {key} : {} sessions, {n_drones} drones   (par session p10 / p50 / p90 | une voix par drone p10 / p90)\n",
            sub.len()
        );
        for k in &keys {
            let v = vals(&sub, k);
            if v.is_empty() {
                continue;
            }
            let mut by: BTreeMap<&String, Vec<f64>> = BTreeMap::new();
            for &i in &idx {
                let x = vals(&[&rows[i]], k);
                if !x.is_empty() {
                    by.entry(&drones[i]).or_default().extend(x);
                }
            }
            let d: Vec<f64> = by.values().map(|x| median_f64(x)).collect();
            let fmt = |qs: &[f64], v: &[f64]| {
                qs.iter()
                    .map(|&q| format!("{:.2}", percentile(v, q)))
                    .collect::<Vec<_>>()
                    .join(" / ")
            };
            text += &format!(
                "{k:<12} n={:4}  {}  | {:3} drones  {}\n",
                v.len(),
                fmt(&[10.0, 50.0, 90.0], &v),
                d.len(),
                fmt(&[10.0, 90.0], &d)
            );
        }
    }
    text.truncate(text.trim_end_matches('\n').len()); // le programme ajoute le retour à la ligne final
    Ok(text)
}

fn read_rows(path: &str) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
    match parse(&text).map_err(|e| format!("{path} : {e}"))? {
        Value::Array(rows) => Ok(rows),
        _ => Err(format!("{path} : un tableau JSON était attendu")),
    }
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["-j"], &["--resume"])?;
    match (a.has("--resume"), a.positional.as_slice()) {
        (true, [path]) => Ok(Output::ok(resume_text(&read_rows(path)?)?)),
        (false, [n, path]) => {
            let n: usize = n
                .parse()
                .map_err(|_| format!("NB_FICHIERS attend un entier, reçu « {n} »"))?;
            let jobs = a.number("-j")?;
            let root = logs_root();
            let mut files = log_files(&root).map_err(|e| format!("{} : {e}", root.display()))?;
            files.truncate(n);
            let res = map_files(|f| measure_file(f, &root), &files, jobs, &root);
            let rows: Vec<Value> = res.into_iter().flat_map(|(_, rows)| rows).collect();
            std::fs::write(path, dump(&Value::Array(rows.clone()))?).map_err(|e| format!("{path} : {e}"))?;
            Ok(Output::ok(format!("{} sessions", rows.len())))
        }
        _ => Err(super::USAGE.into()),
    }
}
