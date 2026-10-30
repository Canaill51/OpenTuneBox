//! Couches d'étalon propres à un fichier du corpus (hors des appels de fonctions de `value.rs`) : en-têtes, et plus tard
//! réglages et journaux de vol. Chaque couche relit un `.jsonl` écrit par `scripts/golden.py record-…`.

use otb_decoder::header::{find_log_starts, parse_header};
use serde_json::Value;
use std::path::Path;

pub struct LayerReport {
    pub checked: usize,
    pub skipped: usize,
    pub failures: Vec<String>,
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

fn compare_header(rec: &Value, data: &[u8], starts: &[usize]) -> Result<(), String> {
    let k = rec["session"].as_u64().unwrap() as usize - 1;
    let start = *starts.get(k).ok_or("session absente du fichier")?;
    let end = starts.get(k + 1).copied().unwrap_or(data.len());
    if (rec["start"].as_u64(), rec["end"].as_u64()) != (Some(start as u64), Some(end as u64)) {
        return Err(format!(
            "bornes de session : Python {}-{}, Rust {start}-{end}",
            rec["start"], rec["end"]
        ));
    }
    let h = parse_header(data, start, end);
    let raw = rec["raw"].as_object().ok_or("raw")?;
    if raw.len() != h.raw.len() {
        return Err(format!("{} clés d'en-tête (Python {})", h.raw.len(), raw.len()));
    }
    for (key, v) in raw {
        if h.raw.get(key).map(String::as_str) != v.as_str() {
            return Err(format!("clé {key:?} : Python {v}, Rust {:?}", h.raw.get(key)));
        }
    }
    let frames = rec["frames"].as_object().ok_or("frames")?;
    if frames.len() != h.frames.len() {
        return Err(format!("{} types de trames (Python {})", h.frames.len(), frames.len()));
    }
    for (t, f) in frames {
        let d = h
            .frames
            .get(&t.chars().next().unwrap())
            .ok_or(format!("trame {t} absente"))?;
        let names: Vec<&str> = f["names"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if d.names != names
            || d.signed != ints(&f["signed"])
            || d.predictor != ints(&f["predictor"])
            || d.encoding != ints(&f["encoding"])
        {
            return Err(format!("définition de la trame {t} différente"));
        }
    }
    if (rec["data_start"].as_u64(), rec["data_end"].as_u64()) != (Some(h.data_start as u64), Some(h.data_end as u64)) {
        return Err(format!(
            "data_start/end : Python {}/{}, Rust {}/{}",
            rec["data_start"], rec["data_end"], h.data_start, h.data_end
        ));
    }
    let p = &rec["props"];
    let got = [
        h.data_version(),
        h.i_interval(),
        h.p_interval().0,
        h.p_interval().1,
        h.minthrottle(),
        h.motor_output().0,
        h.motor_output().1,
        h.vbatref(),
    ];
    let want = [
        p["data_version"].as_i64(),
        p["i_interval"].as_i64(),
        ints(&p["p_interval"]).first().copied(),
        ints(&p["p_interval"]).get(1).copied(),
        p["minthrottle"].as_i64(),
        ints(&p["motor_output"]).first().copied(),
        ints(&p["motor_output"]).get(1).copied(),
        p["vbatref"].as_i64(),
    ];
    if want.iter().map(|x| x.unwrap_or(i64::MIN)).collect::<Vec<_>>() != got {
        return Err(format!("propriétés : Python {p}, Rust {got:?}"));
    }
    Ok(())
}

/// Compare l'analyse Rust des en-têtes à `dossier/header.jsonl` ; les fichiers sont cherchés sous `logs_root`.
pub fn check_header(dossier: &Path, logs_root: &Path) -> Result<LayerReport, String> {
    let text = std::fs::read_to_string(dossier.join("header.jsonl")).map_err(|e| format!("header.jsonl : {e}"))?;
    let mut rep = LayerReport {
        checked: 0,
        skipped: 0,
        failures: vec![],
    };
    let mut cache: Option<(String, Vec<u8>, Vec<usize>)> = None;
    for line in text.lines() {
        let rec: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if rec.get("error").is_some() {
            rep.skipped += 1; // Python plante sur ce fichier : non comparé
            continue;
        }
        let file = rec["file"].as_str().unwrap().to_string();
        if cache.as_ref().map(|c| &c.0) != Some(&file) {
            let data = std::fs::read(logs_root.join(&file)).map_err(|e| format!("{file} : {e}"))?;
            let starts = find_log_starts(&data);
            cache = Some((file.clone(), data, starts));
        }
        let (_, data, starts) = cache.as_ref().unwrap();
        rep.checked += 1;
        if let Err(e) = compare_header(&rec, data, starts) {
            rep.failures.push(format!("{file} session {} : {e}", rec["session"]));
        }
    }
    Ok(rep)
}

/// Compare le décodeur Rust (`decode_file`) aux empreintes de `dossier/decoder.json` (sans la clé `header`, voir
/// `decoded.rs`) ; les fichiers sont cherchés sous `logs_root`, décodés par `threads` fils.
pub fn check_decoder(dossier: &Path, logs_root: &Path, threads: usize) -> Result<LayerReport, String> {
    let text = std::fs::read_to_string(dossier.join("decoder.json")).map_err(|e| format!("decoder.json : {e}"))?;
    let reference: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let files: Vec<(&String, &Value)> = reference.as_object().ok_or("decoder.json")?.iter().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let report = std::sync::Mutex::new(LayerReport {
        checked: 0,
        skipped: 0,
        failures: vec![],
    });
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((rel, want)) = files.get(i) else { break };
                    let want = want.as_array().unwrap();
                    let result = match std::fs::read(logs_root.join(rel)) {
                        Err(e) => Err(format!("lecture : {e}")),
                        Ok(data) => {
                            let got = otb_decoder::file::decode_bytes(&data, None);
                            if got.len() != want.len() {
                                Err(format!("{} sessions décodées (Python {})", got.len(), want.len()))
                            } else {
                                got.iter().zip(want).try_for_each(|(g, w)| {
                                    let d = crate::decoded::diff(w, g);
                                    if d.is_empty() {
                                        Ok(())
                                    } else {
                                        Err(format!("session {} : {}", g.index, d.join(" ; ")))
                                    }
                                })
                            }
                        }
                    };
                    let mut r = report.lock().unwrap();
                    r.checked += want.len();
                    if let Err(e) = result {
                        r.failures.push(format!("{rel} : {e}"));
                    }
                }
            });
        }
    });
    Ok(report.into_inner().unwrap())
}

/// Compare `parse_settings` (Rust, à partir de l'en-tête lu par le Rust) à `dossier/settings.jsonl`.
pub fn check_settings(dossier: &Path, logs_root: &Path) -> Result<LayerReport, String> {
    let text = std::fs::read_to_string(dossier.join("settings.jsonl")).map_err(|e| format!("settings.jsonl : {e}"))?;
    let mut rep = LayerReport {
        checked: 0,
        skipped: 0,
        failures: vec![],
    };
    let mut cache: Option<(String, Vec<u8>, Vec<usize>)> = None;
    for line in text.lines() {
        let rec: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if rec.get("error").is_some() {
            rep.skipped += 1;
            continue;
        }
        let file = rec["file"].as_str().unwrap().to_string();
        if cache.as_ref().map(|c| &c.0) != Some(&file) {
            let data = std::fs::read(logs_root.join(&file)).map_err(|e| format!("{file} : {e}"))?;
            let starts = find_log_starts(&data);
            cache = Some((file.clone(), data, starts));
        }
        let (_, data, starts) = cache.as_ref().unwrap();
        let k = rec["session"].as_u64().unwrap() as usize - 1;
        let end = starts.get(k + 1).copied().unwrap_or(data.len());
        let header = parse_header(data, starts[k], end);
        let got = crate::settings_json::settings_json(&otb_log::settings::parse_settings(&header.raw));
        rep.checked += 1;
        let d = crate::settings_json::diff(&rec["settings"], &got);
        if !d.is_empty() {
            rep.failures
                .push(format!("{file} session {} : {}", rec["session"], d.join(" ; ")));
        }
    }
    Ok(rep)
}

/// Tolérance sur `d_term_prefilter` (écart maximal rapporté au plus grand échantillon de l'axe) : ≈ 2 ulp de `f32`. Mesuré
/// sur le corpus : 0 (identique au bit près) ; BLAS peut différer d'une machine à l'autre. Voir
/// `docs/migration-rust.md` (phase 4a-5).
pub const PREFILTER_TOLERANCE: f64 = 1e-7;

pub struct FlightlogReport {
    pub sessions: usize,
    pub failures: Vec<String>,
    /// Plus grand écart relatif de `d_term_prefilter` vu sur le corpus.
    pub prefilter_worst: f64,
}

/// Compare `FlightLog` (Rust) à `dossier/flightlog.json` : bits identiques partout, `d_term_prefilter` à tolérance.
pub fn check_flightlog(dossier: &Path, logs_root: &Path, threads: usize) -> Result<FlightlogReport, String> {
    let text = std::fs::read_to_string(dossier.join("flightlog.json")).map_err(|e| format!("flightlog.json : {e}"))?;
    let reference: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let files: Vec<(&String, &Value)> = reference.as_object().ok_or("flightlog.json")?.iter().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let report = std::sync::Mutex::new(FlightlogReport {
        sessions: 0,
        failures: vec![],
        prefilter_worst: 0.0,
    });
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((rel, want)) = files.get(i) else { break };
                    let want = want.as_array().unwrap();
                    let mut problems: Vec<String> = Vec::new();
                    let mut worst = 0.0f64;
                    match otb_log::flightlog::load(&logs_root.join(rel)) {
                        Err(e) => problems.push(format!("lecture : {e}")),
                        Ok(logs) if logs.len() != want.len() => {
                            problems.push(format!("{} sessions (Python {})", logs.len(), want.len()))
                        }
                        Ok(logs) => {
                            for (lg, w) in logs.iter().zip(want) {
                                let d =
                                    crate::flightlog_digest::diff(w, &crate::flightlog_digest::flightlog_digest(lg));
                                if !d.is_empty() {
                                    problems.push(format!("session {} : {}", lg.session, d.join(" ; ")));
                                }
                                match crate::flightlog_digest::prefilter_deviation(&w["d_term_prefilter"], lg) {
                                    Ok(x) if x <= PREFILTER_TOLERANCE => worst = worst.max(x),
                                    Ok(x) => {
                                        problems.push(format!("session {} : d_term_prefilter écart {x:e}", lg.session))
                                    }
                                    Err(e) => problems.push(format!("session {} : d_term_prefilter {e}", lg.session)),
                                }
                            }
                        }
                    }
                    let mut r = report.lock().unwrap();
                    r.sessions += want.len();
                    r.prefilter_worst = r.prefilter_worst.max(worst);
                    for p in problems {
                        r.failures.push(format!("{rel} : {p}"));
                    }
                }
            });
        }
    });
    Ok(report.into_inner().unwrap())
}
