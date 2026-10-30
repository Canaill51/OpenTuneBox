//! Implémentations Rust rejouables, par nom de fonction Python (`MODULE.FONCTION`).
//!
//! Chaque entrée reçoit l'appel enregistré et rend le résultat que Python aurait rendu. Une fonction absente d'ici
//! est comptée « non portée » (ce n'est pas un échec) : les phases 4 et 5 la remplissent module par module.

use crate::value::{Array, Case, Value};
use otb_analysis::bf_filters::{self, Chain, ChainItem, FilterSpec, Kind, Matrix64, Target, WhatIfCache};
use otb_analysis::sliders;
use otb_analysis::spectral::{self, Mode};
use otb_dsp::numeric;
use otb_log::flightlog::{FlightLog, PidTerms, Rows};
use otb_log::settings::{AxisPid, Lowpass, Settings};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

pub type Port = fn(&Case) -> Result<Value, String>;

fn f(case: &Case, pos: usize, name: &str) -> Result<f64, String> {
    match case.arg(pos, name) {
        Some(Value::F64(x)) => Ok(*x),
        Some(Value::Int(n)) => Ok(*n as f64),
        other => Err(format!("argument {name} : flottant attendu, reçu {other:?}")),
    }
}

fn pt1_gain(c: &Case) -> Result<Value, String> {
    Ok(Value::F64(otb_dsp::pt1_gain(f(c, 0, "f_cut")?, f(c, 1, "fs")?)))
}

fn notch_q(c: &Case) -> Result<Value, String> {
    Ok(Value::F64(otb_dsp::notch_q(
        f(c, 0, "center_hz")?,
        f(c, 1, "cutoff_hz")?,
    )))
}

fn version(c: &Case, pos: usize) -> Result<Vec<i64>, String> {
    match c.arg(pos, "version") {
        Some(Value::Tuple(v)) => v
            .iter()
            .map(|x| {
                if let Value::Int(n) = x {
                    Ok(*n)
                } else {
                    Err(format!("version : {x:?}"))
                }
            })
            .collect(),
        other => Err(format!("version : tuple attendu, reçu {other:?}")),
    }
}

fn family(c: &Case) -> Result<Value, String> {
    Ok(Value::Str(otb_log::debug_tables::family(&version(c, 0)?).to_string()))
}

fn debug_mode_name(c: &Case) -> Result<Value, String> {
    let index = match c.arg(0, "index") {
        Some(Value::Int(n)) => Some(*n),
        Some(Value::Null) => None,
        other => return Err(format!("index : {other:?}")),
    };
    let explicit = match c.arg(2, "explicit") {
        Some(Value::Str(s)) => Some(s.as_str()),
        Some(Value::Null) | None => None,
        other => return Err(format!("explicit : {other:?}")),
    };
    Ok(Value::Str(otb_log::debug_tables::debug_mode_name(
        index,
        &version(c, 1)?,
        explicit,
    )))
}

fn mode_names(c: &Case) -> Result<Value, String> {
    let Some(Value::Int(mask)) = c.arg(0, "mask") else {
        return Err("mask : entier attendu".into());
    };
    let shown = match c.arg(2, "shown_only") {
        Some(Value::Bool(b)) => *b,
        None => true,
        other => return Err(format!("shown_only : {other:?}")),
    };
    let names = otb_log::modes::mode_names(*mask, &version(c, 1)?, shown);
    Ok(Value::List(
        names.into_iter().map(|n| Value::Str(n.to_string())).collect(),
    ))
}

fn mode_changes(c: &Case) -> Result<Value, String> {
    let flags: Option<Vec<i32>> = match c.arg(0, "flags") {
        Some(Value::Array(a)) if a.kind == 'i' && a.size == 4 => {
            Some((0..a.len()).map(|i| a.get_f64(i) as i32).collect())
        }
        Some(Value::Null) => None,
        other => return Err(format!("flags : {other:?}")),
    };
    let Some(Value::Array(t)) = c.arg(1, "t") else {
        return Err("t : tableau attendu".into());
    };
    let t: Vec<f64> = (0..t.len()).map(|i| t.get_f64(i)).collect();
    let out = otb_log::modes::mode_changes(flags.as_deref(), &t, &version(c, 2)?);
    Ok(Value::List(
        out.into_iter()
            .map(|(x, s)| Value::Tuple(vec![Value::F64(x), Value::Str(s)]))
            .collect(),
    ))
}

// ---------------------------------------------------------------- journaux de vol

type LogCache = Mutex<HashMap<(String, i64), Arc<FlightLog>>>;

/// Journal rouvert par le décodeur Rust (chemin relatif au corpus `OTB_LOGS`, session) ; mis en cache : un fichier est
/// décodé une fois pour toutes ses sessions.
fn flightlog(v: Option<&Value>) -> Result<Arc<FlightLog>, String> {
    static CACHE: OnceLock<LogCache> = OnceLock::new();
    let Some(Value::FlightLog(rel, session)) = v else {
        return Err(format!("journal attendu, reçu {v:?}"));
    };
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some(lg) = cache.get(&(rel.clone(), *session)) {
        return Ok(lg.clone());
    }
    let root = std::env::var("OTB_LOGS")
        .unwrap_or_else(|_| format!("{}/Documents/Drones/Logs", std::env::var("HOME").unwrap()));
    let logs = otb_log::flightlog::load(&std::path::Path::new(&root).join(rel)).map_err(|e| format!("{rel} : {e}"))?;
    for lg in logs {
        cache.insert((rel.clone(), lg.session as i64), Arc::new(lg));
    }
    cache
        .get(&(rel.clone(), *session))
        .cloned()
        .ok_or(format!("{rel} session {session} : non chargée"))
}

/// `slice(start, stop)` de Python (pas 1 ou absent) rapporté à une longueur `n`.
fn slice_range(v: Option<&Value>, n: usize) -> Result<Range<usize>, String> {
    let Some(Value::Slice(b)) = v else {
        return Err(format!("tranche attendue, reçu {v:?}"));
    };
    let at = |x: &Value, default: i64| -> Result<i64, String> {
        match x {
            Value::Null => Ok(default),
            Value::Int(i) => Ok(if *i < 0 { (*i + n as i64).max(0) } else { *i }),
            other => Err(format!("borne de tranche : {other:?}")),
        }
    };
    if !matches!(b[2], Value::Null | Value::Int(1)) {
        return Err("pas de tranche non géré".into());
    }
    let (start, stop) = (
        at(&b[0], 0)?.min(n as i64) as usize,
        at(&b[1], n as i64)?.min(n as i64) as usize,
    );
    Ok(start.min(stop)..stop)
}

fn gap_factor(c: &Case, pos: usize) -> Result<f64, String> {
    match c.arg(pos, "max_gap_factor") {
        None => Ok(4.0),
        Some(Value::F64(x)) => Ok(*x),
        Some(Value::Int(n)) => Ok(*n as f64),
        other => Err(format!("max_gap_factor : {other:?}")),
    }
}

fn array_f32(v: &[f32]) -> Value {
    Value::Array(Array {
        shape: vec![v.len()],
        kind: 'f',
        size: 4,
        bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    })
}

fn lg_breaks(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    let b = lg.breaks(slice_range(c.arg(1, "sel"), lg.n())?, gap_factor(c, 2)?);
    Ok(Value::Array(Array {
        shape: vec![b.len()],
        kind: 'i',
        size: 8,
        bytes: b.iter().flat_map(|&x| (x as i64).to_le_bytes()).collect(),
    }))
}

fn lg_recording_gaps(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    let (n, lost) = lg.recording_gaps(slice_range(c.arg(1, "sel"), lg.n())?, gap_factor(c, 2)?);
    Ok(Value::Tuple(vec![Value::Int(n as i64), Value::F64(lost)]))
}

fn lg_index_of(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    Ok(Value::Int(lg.index_of(f(c, 1, "time_s")?) as i64))
}

fn lg_slice_time(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    let r = lg.slice_time(f(c, 1, "t0")?, f(c, 2, "t1")?);
    Ok(Value::Slice(Box::new([
        Value::Int(r.start as i64),
        Value::Int(r.end as i64),
        Value::Null,
    ])))
}

fn lg_term_present(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    let (Some(Value::Str(t)), Some(Value::Int(ax))) = (c.arg(1, "term"), c.arg(2, "ax")) else {
        return Err("term, ax".into());
    };
    Ok(Value::Bool(
        lg.term_present(t.chars().next().unwrap_or('?'), *ax as usize),
    ))
}

fn lg_debug_name(c: &Case) -> Result<Value, String> {
    Ok(Value::Str(flightlog(c.arg(0, "self"))?.debug_name().to_string()))
}

fn lg_chirp(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    let Some(Value::Int(ax)) = c.arg(1, "ax") else {
        return Err("ax".into());
    };
    let sel = slice_range(c.arg(2, "sel"), lg.n())?;
    Ok(lg
        .chirp_excitation(*ax as usize, sel)
        .map_or(Value::Null, |v| array_f32(&v)))
}

fn pid_terms_value(t: &PidTerms) -> Value {
    Value::Obj(
        "PidTerms".into(),
        vec![
            ("p".into(), array_f32(&t.p)),
            ("i".into(), array_f32(&t.i)),
            ("d".into(), array_f32(&t.d)),
            ("f".into(), array_f32(&t.f)),
            ("note".into(), Value::Str(t.note.clone())),
            (
                "instrument".into(),
                t.instrument.as_ref().map_or(Value::Null, |v| array_f32(v)),
            ),
        ],
    )
}

fn lg_pid_terms(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "self"))?;
    let Some(Value::Int(ax)) = c.arg(1, "ax") else {
        return Err("ax".into());
    };
    let sel = slice_range(c.arg(2, "sel"), lg.n())?;
    Ok(pid_terms_value(&lg.pid_terms(*ax as usize, sel)))
}

/// `flightlog.cell_count(settings, vbat)` : seul `raw` des réglages compte (`vbatref`, `vbatcellvoltage`).
fn cell_count(c: &Case) -> Result<Value, String> {
    let Some(Value::Obj(_, fields)) = c.arg(0, "settings") else {
        return Err("settings : objet attendu".into());
    };
    let Some((_, Value::Dict(items))) = fields.iter().find(|(k, _)| k == "raw") else {
        return Err("settings.raw".into());
    };
    let raw: BTreeMap<String, String> = items
        .iter()
        .filter_map(|(k, v)| {
            if let (Value::Str(k), Value::Str(v)) = (k, v) {
                Some((k.clone(), v.clone()))
            } else {
                None
            }
        })
        .collect();
    let vbat: Option<Vec<f32>> = match c.arg(1, "vbat") {
        Some(Value::Array(a)) if a.kind == 'f' && a.size == 4 => {
            Some((0..a.len()).map(|i| a.get_f64(i) as f32).collect())
        }
        Some(Value::Null) | None => None,
        other => return Err(format!("vbat : {other:?}")),
    };
    let st = otb_log::settings::parse_settings(&raw);
    Ok(otb_log::flightlog::cell_count(&st, vbat.as_deref()).map_or(Value::Null, Value::Int))
}

// ---------------------------------------------------------------- _numeric (phase 4b-1)

/// Tableau numérique d'un argument : forme, valeurs en f64 et `true` s'il était en float32 (le résultat l'est alors aussi).
fn floats(v: Option<&Value>, name: &str) -> Result<(Vec<usize>, Vec<f64>, bool), String> {
    match v {
        Some(Value::Array(a)) if matches!(a.kind, 'f' | 'i' | 'u' | 'b') => Ok((
            a.shape.clone(),
            (0..a.len()).map(|i| a.get_f64(i)).collect(),
            a.kind == 'f' && a.size == 4,
        )),
        other => Err(format!("{name} : tableau attendu, reçu {other:?}")),
    }
}

fn int(v: Option<&Value>, name: &str) -> Result<i64, String> {
    match v {
        Some(Value::Int(n)) => Ok(*n),
        Some(Value::NpScalar(_, inner)) => int(Some(inner), name),
        other => Err(format!("{name} : entier attendu, reçu {other:?}")),
    }
}

fn float(v: &Value) -> Result<f64, String> {
    match v {
        Value::F64(x) => Ok(*x),
        Value::Int(n) => Ok(*n as f64),
        Value::NpScalar(_, inner) => float(inner),
        other => Err(format!("flottant attendu, reçu {other:?}")),
    }
}

/// Tableau float64 (ou float32 arrondi) de forme `shape`.
fn array_nd(shape: Vec<usize>, data: &[f64], as_f32: bool) -> Value {
    let (size, bytes): (usize, Vec<u8>) = if as_f32 {
        (4, data.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect())
    } else {
        (8, data.iter().flat_map(|x| x.to_le_bytes()).collect())
    };
    Value::Array(Array {
        shape,
        kind: 'f',
        size,
        bytes,
    })
}

fn hann(c: &Case) -> Result<Value, String> {
    let sym = match c.arg(1, "sym") {
        None => true,
        Some(Value::Bool(b)) => *b,
        other => return Err(format!("sym : {other:?}")),
    };
    let w = numeric::hann(int(c.arg(0, "length"), "length")?, sym);
    Ok(array_nd(vec![w.len()], &w, false))
}

fn uniform_filter1d(c: &Case) -> Result<Value, String> {
    let (shape, x, as_f32) = floats(c.arg(0, "x"), "x")?;
    let axis = if c.arg(2, "axis").is_some() {
        int(c.arg(2, "axis"), "axis")?
    } else {
        -1
    };
    let out = numeric::uniform_filter1d(&x, &shape, int(c.arg(1, "size"), "size")?, axis)?;
    Ok(array_nd(shape, &out, as_f32))
}

fn gaussian_filter(c: &Case) -> Result<Value, String> {
    let (shape, a, as_f32) = floats(c.arg(0, "a"), "a")?;
    let sigma = match c.arg(1, "sigma") {
        Some(Value::Tuple(v) | Value::List(v)) => v.iter().map(float).collect::<Result<Vec<_>, _>>()?,
        Some(v) => vec![float(v)?; shape.len()],
        None => return Err("sigma".into()),
    };
    if sigma.len() != shape.len() {
        return Err(format!("{} sigma pour {} axes", sigma.len(), shape.len()));
    }
    Ok(array_nd(
        shape.clone(),
        &numeric::gaussian_filter(&a, &shape, &sigma)?,
        as_f32,
    ))
}

fn xcorr_full(c: &Case) -> Result<Value, String> {
    let (_, y, _) = floats(c.arg(0, "y"), "y")?;
    let (_, x, _) = floats(c.arg(1, "x"), "x")?;
    let (corr, lags) = numeric::xcorr_full(&y, &x)?;
    Ok(Value::Tuple(vec![
        array_nd(vec![corr.len()], &corr, false),
        Value::Array(Array {
            shape: vec![lags.len()],
            kind: 'i',
            size: 8,
            bytes: lags.iter().flat_map(|x| x.to_le_bytes()).collect(),
        }),
    ]))
}

// ---------------------------------------------------------------- spectral (phase 4b-2)

fn array_i64(v: &[i64]) -> Value {
    Value::Array(Array {
        shape: vec![v.len()],
        kind: 'i',
        size: 8,
        bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect(),
    })
}

fn array_f64(v: &[f64]) -> Value {
    array_nd(vec![v.len()], v, false)
}

/// Raccords du log : `None` ou tableau d'entiers.
fn breaks_arg(v: Option<&Value>) -> Result<Option<Vec<i64>>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if matches!(a.kind, 'i' | 'u') => {
            Ok(Some((0..a.len()).map(|i| a.get_f64(i) as i64).collect()))
        }
        other => Err(format!("breaks : {other:?}")),
    }
}

fn mode_arg(v: Option<&Value>, default: &str) -> Result<String, String> {
    match v {
        None => Ok(default.to_string()),
        Some(Value::Str(s)) => Ok(s.clone()),
        other => Err(format!("mode : {other:?}")),
    }
}

fn float_or(c: &Case, pos: usize, name: &str, default: f64) -> Result<f64, String> {
    c.arg(pos, name).map_or(Ok(default), float)
}

fn int_or(c: &Case, pos: usize, name: &str, default: i64) -> Result<i64, String> {
    if c.arg(pos, name).is_some() {
        int(c.arg(pos, name), name)
    } else {
        Ok(default)
    }
}

fn clear_of_breaks(c: &Case) -> Result<Value, String> {
    let (_, starts, _) = floats(c.arg(0, "starts"), "starts")?;
    let starts: Vec<i64> = starts.iter().map(|&v| v as i64).collect();
    let out = spectral::clear_of_breaks(
        &starts,
        int(c.arg(1, "length"), "length")?,
        breaks_arg(c.arg(2, "breaks"))?.as_deref(),
    );
    Ok(Value::Array(Array {
        shape: vec![out.len()],
        kind: 'b',
        size: 1,
        bytes: out.iter().map(|&b| b as u8).collect(),
    }))
}

fn ptb_subsample(c: &Case) -> Result<Value, String> {
    let d = float(c.arg(0, "duration_s").ok_or("duration_s")?)?;
    Ok(Value::Int(spectral::ptb_subsample(
        d,
        int(c.arg(1, "short"), "short")?,
        int(c.arg(2, "mid"), "mid")?,
        int(c.arg(3, "long"), "long")?,
    )))
}

fn to_db(c: &Case) -> Result<Value, String> {
    let (shape, p, _) = floats(c.arg(0, "p"), "p")?;
    Ok(array_nd(shape, &spectral::to_db(&p), false))
}

fn to_ptb_db(c: &Case) -> Result<Value, String> {
    let (shape, db, _) = floats(c.arg(0, "db"), "db")?;
    Ok(array_nd(shape, &spectral::to_ptb_db(&db), false))
}

fn smooth_freq(c: &Case) -> Result<Value, String> {
    let (shape, y, _) = floats(c.arg(0, "y"), "y")?;
    let out = spectral::smooth_freq(&y, &shape, int(c.arg(1, "level"), "level")?)?;
    Ok(array_nd(shape, &out, false))
}

fn spectrum(c: &Case) -> Result<Value, String> {
    let (_, x, _) = floats(c.arg(0, "x"), "x")?;
    let fs = float(c.arg(1, "fs").ok_or("fs")?)?;
    let mode = mode_arg(c.arg(2, "mode"), "psd")?;
    let res = float_or(c, 3, "resolution_hz", 0.5)?;
    let sp = spectral::spectrum(
        &x,
        fs,
        Mode::parse(&mode),
        res,
        int_or(c, 4, "smoothing", 0)?,
        breaks_arg(c.arg(5, "breaks"))?.as_deref(),
    )?;
    Ok(Value::Obj(
        "Spectrum".into(),
        vec![
            ("freqs".into(), array_f64(&sp.freqs)),
            ("values".into(), array_f64(&sp.values)),
            ("mode".into(), Value::Str(mode)),
            ("resolution_hz".into(), Value::F64(sp.resolution_hz)),
        ],
    ))
}

fn freq_throttle(c: &Case) -> Result<Value, String> {
    let (_, x, _) = floats(c.arg(0, "x"), "x")?;
    let (_, xaxis, _) = floats(c.arg(1, "xaxis"), "xaxis")?;
    let fs = float(c.arg(2, "fs").ok_or("fs")?)?;
    let mode = mode_arg(c.arg(3, "mode"), "amp")?;
    let range = match c.arg(6, "x_range") {
        None => (0.0, 100.0),
        Some(Value::Tuple(v) | Value::List(v)) if v.len() == 2 => (float(&v[0])?, float(&v[1])?),
        other => return Err(format!("x_range : {other:?}")),
    };
    let nbins = int_or(c, 5, "nbins", 100)?;
    if nbins < 1 {
        return Err("nbins < 1".into());
    }
    let ft = spectral::freq_throttle(
        &x,
        &xaxis,
        fs,
        Mode::parse(&mode),
        float_or(c, 4, "seg_ms", 300.0)?,
        nbins as usize,
        range,
        breaks_arg(c.arg(7, "breaks"))?.as_deref(),
    )?;
    Ok(Value::Obj(
        "FreqThrottle".into(),
        vec![
            ("freqs".into(), array_f64(&ft.freqs)),
            ("bins".into(), array_f64(&ft.bins)),
            ("z".into(), array_nd(vec![ft.freqs.len(), ft.bins.len()], &ft.z, false)),
            ("counts".into(), array_i64(&ft.counts)),
            ("mode".into(), Value::Str(mode)),
        ],
    ))
}

fn smooth_map(c: &Case) -> Result<Value, String> {
    let (shape, z, _) = floats(c.arg(0, "z"), "z")?;
    let out = spectral::smooth_map(&z, &shape, int(c.arg(1, "level"), "level")?)?;
    Ok(array_nd(shape, &out, false))
}

fn freq_time(c: &Case) -> Result<Value, String> {
    let (_, x, _) = floats(c.arg(0, "x"), "x")?;
    let (_, t, _) = floats(c.arg(1, "t"), "t")?;
    let fs = float(c.arg(2, "fs").ok_or("fs")?)?;
    let sg = spectral::freq_time(
        &x,
        &t,
        fs,
        float_or(c, 3, "win_ms", 300.0)?,
        int_or(c, 4, "hop_div", 2)?,
        int_or(c, 5, "freq_smoothing", 0)?,
        breaks_arg(c.arg(6, "breaks"))?.as_deref(),
    )?;
    Ok(Value::Obj(
        "Spectrogram".into(),
        vec![
            ("times".into(), array_f64(&sg.times)),
            ("freqs".into(), array_f64(&sg.freqs)),
            ("z".into(), array_nd(vec![sg.nf, sg.nt], &sg.z, false)),
        ],
    ))
}

// ---------------------------------------------------------------- bf_filters : chaînes et réponses (phase 4b-3)

fn obj_fields<'a>(v: Option<&'a Value>, name: &str) -> Result<&'a [(String, Value)], String> {
    match v {
        Some(Value::Obj(n, f)) if n == name => Ok(f),
        other => Err(format!("{name} attendu, reçu {other:?}")),
    }
}

fn field<'a>(fields: &'a [(String, Value)], key: &str) -> Result<&'a Value, String> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("champ {key} absent"))
}

fn int_field(fields: &[(String, Value)], key: &str) -> Result<i64, String> {
    int(Some(field(fields, key)?), key)
}

fn pair_of(v: &Value) -> Result<(i64, i64), String> {
    match v {
        Value::Tuple(p) | Value::List(p) if p.len() == 2 => {
            Ok((int(Some(&p[0]), "paire")?, int(Some(&p[1]), "paire")?))
        }
        other => Err(format!("paire attendue, reçu {other:?}")),
    }
}

fn lowpass_from(v: Option<&Value>) -> Result<Lowpass, String> {
    let f = obj_fields(v, "LowpassSetting")?;
    let kind = match field(f, "type")? {
        Value::Str(s) if s == "PT1" => "PT1",
        Value::Str(s) if s == "BIQUAD" => "BIQUAD",
        Value::Str(s) if s == "PT2" => "PT2",
        Value::Str(s) if s == "PT3" => "PT3",
        other => return Err(format!("type de passe-bas : {other:?}")),
    };
    Ok(Lowpass {
        kind,
        static_hz: int_field(f, "static_hz")?,
        dyn_min_hz: int_field(f, "dyn_min_hz")?,
        dyn_max_hz: int_field(f, "dyn_max_hz")?,
        expo: int_field(f, "expo")?,
    })
}

fn lowpass_value(lp: &Lowpass) -> Value {
    Value::Obj(
        "LowpassSetting".into(),
        vec![
            ("type".into(), Value::Str(lp.kind.to_string())),
            ("static_hz".into(), Value::Int(lp.static_hz)),
            ("dyn_min_hz".into(), Value::Int(lp.dyn_min_hz)),
            ("dyn_max_hz".into(), Value::Int(lp.dyn_max_hz)),
            ("expo".into(), Value::Int(lp.expo)),
        ],
    )
}

/// `Settings` enregistré → réglages Rust, **champ par champ** (pas par `raw`) : les réglages « modifiés » du simulateur sont
/// faits par `dataclasses.replace` et `raw` ne les reflète pas. Seuls les champs que lisent les chaînes de filtres sont repris.
fn settings_for_filters(v: Option<&Value>) -> Result<Settings, String> {
    let f = obj_fields(v, "Settings")?;
    let notches = match field(f, "gyro_notches")? {
        Value::List(l) | Value::Tuple(l) => l.iter().map(pair_of).collect::<Result<Vec<_>, _>>()?,
        other => return Err(format!("gyro_notches : {other:?}")),
    };
    let weights = match field(f, "rpm_weights")? {
        Value::List(l) | Value::Tuple(l) => l
            .iter()
            .map(|x| int(Some(x), "rpm_weights"))
            .collect::<Result<Vec<_>, _>>()?,
        other => return Err(format!("rpm_weights : {other:?}")),
    };
    Ok(Settings {
        looptime_us: int_field(f, "looptime_us")?,
        pid_denom: int_field(f, "pid_denom")?,
        gyro_lpf1: lowpass_from(Some(field(f, "gyro_lpf1")?))?,
        gyro_lpf2: lowpass_from(Some(field(f, "gyro_lpf2")?))?,
        dterm_lpf1: lowpass_from(Some(field(f, "dterm_lpf1")?))?,
        dterm_lpf2: lowpass_from(Some(field(f, "dterm_lpf2")?))?,
        gyro_notches: notches,
        dterm_notch: pair_of(field(f, "dterm_notch")?)?,
        dyn_notch_count: int_field(f, "dyn_notch_count")?,
        dyn_notch_q: int_field(f, "dyn_notch_q")?,
        dyn_notch_min_hz: int_field(f, "dyn_notch_min_hz")?,
        dyn_notch_max_hz: int_field(f, "dyn_notch_max_hz")?,
        rpm_harmonics: int_field(f, "rpm_harmonics")?,
        rpm_weights: weights,
        rpm_q: int_field(f, "rpm_q")?,
        rpm_min_hz: int_field(f, "rpm_min_hz")?,
        rpm_fade_hz: int_field(f, "rpm_fade_hz")?,
        rpm_lpf_hz: int_field(f, "rpm_lpf_hz")?,
        debug_mode: match field(f, "debug_mode")? {
            Value::Str(m) => m.clone(),
            other => return Err(format!("debug_mode : {other:?}")),
        },
        version: match field(f, "version")? {
            Value::Tuple(v) | Value::List(v) => v
                .iter()
                .map(|x| int(Some(x), "version"))
                .collect::<Result<Vec<_>, _>>()?,
            other => return Err(format!("version : {other:?}")),
        },
        ..Settings::default()
    })
}

fn spec_from(v: &Value) -> Result<FilterSpec, String> {
    let f = obj_fields(Some(v), "FilterSpec")?;
    let kind = match field(f, "kind")? {
        Value::Str(s) => Kind::parse(s)?,
        other => return Err(format!("kind : {other:?}")),
    };
    Ok(FilterSpec::with(
        kind,
        float(field(f, "f_hz")?)?,
        float(field(f, "q")?)?,
        float(field(f, "weight")?)?,
    ))
}

fn spec_value(sp: &FilterSpec) -> Value {
    Value::Obj(
        "FilterSpec".into(),
        vec![
            ("kind".into(), Value::Str(sp.kind.name().to_string())),
            ("f_hz".into(), Value::F64(sp.f_hz)),
            ("q".into(), Value::F64(sp.q)),
            ("weight".into(), Value::F64(sp.weight)),
        ],
    )
}

/// Éléments `FilterSpec` ou `(FilterSpec, cadence)` d'une liste.
fn items_from(v: Option<&Value>) -> Result<Vec<ChainItem>, String> {
    let Some(Value::List(items) | Value::Tuple(items)) = v else {
        return Err(format!("liste de filtres attendue, reçu {v:?}"));
    };
    items
        .iter()
        .map(|it| match it {
            Value::Tuple(p) | Value::List(p) if p.len() == 2 => {
                Ok(ChainItem::WithRate(spec_from(&p[0])?, float(&p[1])?))
            }
            other => Ok(ChainItem::Spec(spec_from(other)?)),
        })
        .collect()
}

fn chain_value(chain: &Chain) -> Value {
    Value::List(
        chain
            .iter()
            .map(|(sp, rate)| Value::Tuple(vec![spec_value(sp), Value::F64(*rate)]))
            .collect(),
    )
}

/// Tableau complexe (parties réelle et imaginaire entrelacées) de forme `shape`.
fn array_c128(shape: Vec<usize>, re: &[f64], im: &[f64]) -> Value {
    Value::Array(Array {
        shape,
        kind: 'c',
        size: 16,
        bytes: re
            .iter()
            .zip(im)
            .flat_map(|(r, i)| r.to_le_bytes().into_iter().chain(i.to_le_bytes()))
            .collect(),
    })
}

fn bf_coefficients(c: &Case) -> Result<Value, String> {
    let (b, a) = bf_filters::coefficients(
        &spec_from(c.arg(0, "spec").ok_or("spec")?)?,
        float(c.arg(1, "fs").ok_or("fs")?)?,
    );
    Ok(Value::Tuple(vec![array_f64(&b), array_f64(&a)]))
}

fn bf_freq_response(c: &Case) -> Result<Value, String> {
    let specs = items_from(c.arg(0, "specs"))?
        .into_iter()
        .map(|it| match it {
            ChainItem::Spec(s) => Ok(s),
            ChainItem::WithRate(..) => Err("FilterSpec attendu".to_string()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (shape, freqs, _) = floats(c.arg(1, "freqs"), "freqs")?;
    let (re, im) = bf_filters::freq_response(&specs, &freqs, float(c.arg(2, "fs").ok_or("fs")?)?)?;
    Ok(array_c128(shape, &re, &im))
}

fn bf_chain_response(c: &Case) -> Result<Value, String> {
    let chain = bf_filters::as_chain(items_from(c.arg(0, "chain"))?, None)?;
    let (shape, freqs, _) = floats(c.arg(1, "freqs"), "freqs")?;
    let (re, im) = bf_filters::chain_response(&chain, &freqs)?;
    Ok(array_c128(shape, &re, &im))
}

fn bf_chain_delay_ms(c: &Case) -> Result<Value, String> {
    let fs = match c.arg(1, "fs") {
        None | Some(Value::Null) => None,
        Some(v) => Some(float(v)?),
    };
    let chain = bf_filters::as_chain(items_from(c.arg(0, "specs"))?, fs)?;
    let d = bf_filters::chain_delay_ms(&chain, float_or(c, 2, "f_hz", 10.0)?)?;
    Ok(Value::Obj(
        "ChainDelay".into(),
        vec![
            ("group_ms".into(), Value::F64(d.group_ms)),
            ("phase_ms".into(), Value::F64(d.phase_ms)),
        ],
    ))
}

fn bf_dyn_lpf_cutoff(c: &Case) -> Result<Value, String> {
    let g = |pos: usize, name: &str| float(c.arg(pos, name).ok_or(name.to_string())?);
    let (mn, mx, ex) = (g(0, "min_hz")?, g(1, "max_hz")?, g(2, "expo")?);
    match c.arg(3, "throttle_frac") {
        Some(Value::Array(_)) => {
            let (shape, t, _) = floats(c.arg(3, "throttle_frac"), "throttle_frac")?;
            let out: Vec<f64> = t.iter().map(|&x| bf_filters::dyn_lpf_cutoff(mn, mx, ex, x)).collect();
            Ok(array_nd(shape, &out, false))
        }
        Some(v) => Ok(Value::F64(bf_filters::dyn_lpf_cutoff(mn, mx, ex, float(v)?))),
        None => Err("throttle_frac".into()),
    }
}

fn bf_scale_lowpass(c: &Case) -> Result<Value, String> {
    let lp = lowpass_from(c.arg(0, "lp"))?;
    Ok(lowpass_value(&bf_filters::scale_lowpass(
        &lp,
        float(c.arg(1, "factor").ok_or("factor")?)?,
    )?))
}

fn bf_lowpass_off(c: &Case) -> Result<Value, String> {
    Ok(lowpass_value(&bf_filters::lowpass_off(&lowpass_from(c.arg(0, "lp"))?)))
}

fn bf_rpm_notch_freq_weight(c: &Case) -> Result<Value, String> {
    let (shape, motor, _) = floats(c.arg(0, "motor_hz"), "motor_hz")?;
    let s = settings_for_filters(c.arg(2, "settings"))?;
    let (f, w) = bf_filters::rpm_notch_freq_weight(
        &motor,
        int(c.arg(1, "harmonic"), "harmonic")?,
        &s,
        float(c.arg(3, "rate_hz").ok_or("rate_hz")?)?,
    )?;
    Ok(Value::Tuple(vec![
        array_nd(shape.clone(), &f, false),
        array_nd(shape, &w, false),
    ]))
}

fn bf_dyn_notch_enabled(c: &Case) -> Result<Value, String> {
    Ok(Value::Bool(bf_filters::dyn_notch_enabled(&settings_for_filters(
        c.arg(0, "settings"),
    )?)))
}

fn bf_dyn_notch_initial_centres(c: &Case) -> Result<Value, String> {
    let centres = bf_filters::dyn_notch_initial_centres(&settings_for_filters(c.arg(0, "settings"))?);
    Ok(Value::List(centres.into_iter().map(Value::F64).collect()))
}

/// Liste de nombres d'un argument facultatif : tuple, liste, tableau 0-d ou 1-d, ou nombre seul (`np.atleast_1d`).
/// Un tableau à plus d'une dimension est refusé (Python ne l'accepte que s'il a une seule colonne).
fn numbers_arg(v: Option<&Value>, name: &str) -> Result<Option<Vec<f64>>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Tuple(l) | Value::List(l)) => Ok(Some(l.iter().map(float).collect::<Result<Vec<_>, _>>()?)),
        Some(Value::Array(a)) if a.shape.len() <= 1 => Ok(Some(floats(v, name)?.1)),
        Some(Value::Array(a)) => Err(format!("{name} : tableau à {} dimensions", a.shape.len())),
        Some(x) => Ok(Some(vec![float(x)?])),
    }
}

fn bf_gyro_chain(c: &Case) -> Result<Value, String> {
    let s = settings_for_filters(c.arg(0, "settings"))?;
    let motor = numbers_arg(c.arg(2, "motor_hz"), "motor_hz")?;
    let dynh = numbers_arg(c.arg(3, "dyn_notch_hz"), "dyn_notch_hz")?;
    let chain = bf_filters::gyro_chain(
        &s,
        float_or(c, 1, "throttle_frac", 0.5)?,
        motor.as_deref(),
        dynh.as_deref(),
    )?;
    Ok(chain_value(&chain))
}

fn bf_dterm_chain(c: &Case) -> Result<Value, String> {
    let s = settings_for_filters(c.arg(0, "settings"))?;
    Ok(chain_value(&bf_filters::dterm_chain(
        &s,
        float_or(c, 1, "throttle_frac", 0.5)?,
    )))
}

// ---------------------------------------------------------------- bf_filters : simulation (phase 4b-4)

/// Journal gardé jusqu'à la colonne `m` (`v[..., :m]` de chaque courbe lue par la simulation), comme `golden_modules.py` :
/// les simulations y sont enregistrées sur un journal tronqué (`dataclasses.replace(lg, **cut)`) dont la référence ne garde
/// que le chemin du fichier. La largeur à garder est celle du résultat enregistré.
fn truncated(lg: &FlightLog, m: usize) -> FlightLog {
    let mut l = lg.clone();
    let m = m.min(l.n());
    l.t.truncate(m);
    l.throttle.truncate(m);
    let cut = |r: &mut Rows| {
        let w = m.min(r.n);
        r.data = (0..r.k).flat_map(|i| r.data[i * r.n..i * r.n + w].to_vec()).collect();
        r.n = w;
    };
    cut(&mut l.gyro);
    cut(&mut l.debug);
    if let Some(g) = l.gyro_unfilt.as_mut() {
        cut(g);
    }
    if let Some(mh) = l.motor_hz.as_mut() {
        cut(mh);
    }
    l
}

/// Nombre de colonnes du résultat enregistré (tableau 2D) : la coupe de la simulation.
fn recorded_width(c: &Case) -> Option<usize> {
    match &c.result {
        Ok(Value::Array(a)) if a.shape.len() == 2 => Some(a.shape[1]),
        _ => None,
    }
}

fn matrix_value(m: &Matrix64) -> Value {
    array_nd(vec![m.k, m.n], &m.data, false)
}

fn optional_settings(v: Option<&Value>) -> Result<Option<Settings>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        other => settings_for_filters(other).map(Some),
    }
}

fn bf_fft_freq_centres(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "log"))?;
    let s = optional_settings(c.arg(1, "settings"))?;
    Ok(bf_filters::fft_freq_centres(&lg, s.as_ref()).map_or(Value::Null, |m| matrix_value(&m)))
}

fn bf_simulate_gyro(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "log"))?;
    let s = optional_settings(c.arg(1, "settings"))?;
    let lg = match recorded_width(c) {
        Some(w) => std::sync::Arc::new(truncated(&lg, w)),
        None => lg,
    };
    Ok(matrix_value(&bf_filters::simulate_gyro(&lg, s.as_ref())?))
}

fn bf_simulate_dterm(c: &Case) -> Result<Value, String> {
    let lg = flightlog(c.arg(0, "log"))?;
    let s = optional_settings(c.arg(1, "settings"))?;
    let lg = match recorded_width(c) {
        Some(w) => std::sync::Arc::new(truncated(&lg, w)),
        None => lg,
    };
    let gyro = match c.arg(2, "gyro") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let (shape, data, _) = floats(Some(v), "gyro")?;
            if shape.len() != 2 {
                return Err("gyro : tableau 2D attendu".into());
            }
            Some(Matrix64 {
                k: shape[0],
                n: shape[1],
                data,
            })
        }
    };
    Ok(matrix_value(&bf_filters::simulate_dterm(
        &lg,
        s.as_ref(),
        gyro.as_ref(),
    )?))
}

/// Un élément facultatif de la clé du cache : `None` ou tuple de flottants, en bits.
fn key_bits(v: &Value) -> Result<Option<Vec<u64>>, String> {
    match v {
        Value::Null => Ok(None),
        Value::Tuple(l) | Value::List(l) => Ok(Some(
            l.iter().map(|x| float(x).map(f64::to_bits)).collect::<Result<_, _>>()?,
        )),
        other => Err(format!("clé du cache : {other:?}")),
    }
}

/// Le `dict` `cache` enregistré à l'entrée de l'appel : ses réponses sont réutilisées, comme par Python.
fn cache_arg(v: Option<&Value>) -> Result<WhatIfCache, String> {
    let mut cache = WhatIfCache::default();
    let items = match v {
        None | Some(Value::Null) => return Ok(cache),
        Some(Value::Dict(items)) => items,
        other => return Err(format!("cache : {other:?}")),
    };
    for (k, val) in items {
        let (Value::Tuple(key), Value::Tuple(pair)) = (k, val) else {
            return Err("cache : (clé, valeur) en tuples attendus".into());
        };
        if pair.len() != 2 {
            return Err("cache : paire de réponses attendue".into());
        }
        let arr = |x: &Value| floats(Some(x), "cache").map(|r| r.1);
        let resp = (arr(&pair[0])?, arr(&pair[1])?);
        match key.as_slice() {
            [Value::Str(kind), nf, t, motors, centres] if kind == "gyro" => {
                let (nf, t) = (int(Some(nf), "nf")? as usize, float(t)?);
                cache
                    .gyro
                    .insert((nf, t.to_bits(), key_bits(motors)?, key_bits(centres)?), resp);
            }
            [Value::Str(kind), nf, t] if kind == "dterm" => {
                cache
                    .dterm
                    .insert((int(Some(nf), "nf")? as usize, float(t)?.to_bits()), resp);
            }
            other => return Err(format!("clé du cache : {other:?}")),
        }
    }
    Ok(cache)
}

fn bf_whatif_spectrum(c: &Case) -> Result<Value, String> {
    let (_, x_logged, _) = floats(c.arg(0, "x_logged"), "x_logged")?;
    let x_prefilt = match c.arg(1, "x_prefilt") {
        None | Some(Value::Null) => None,
        Some(v) => Some(floats(Some(v), "x_prefilt")?.1),
    };
    let lg = flightlog(c.arg(2, "log"))?;
    let sel = slice_range(c.arg(3, "sel"), lg.n())?;
    let base = settings_for_filters(c.arg(4, "base"))?;
    let modf = settings_for_filters(c.arg(5, "mod"))?;
    let target = Target::parse(&mode_arg(c.arg(6, "target"), "gyro")?);
    let mut cache = cache_arg(c.arg(8, "cache"))?;
    let r = bf_filters::whatif_spectrum(
        &x_logged,
        x_prefilt.as_deref(),
        &lg,
        sel,
        &base,
        &modf,
        target,
        float_or(c, 7, "seg_ms", 300.0)?,
        &mut cache,
    )?;
    Ok(Value::Obj(
        "WhatIfSpectrum".into(),
        vec![
            ("freqs".into(), array_f64(&r.freqs)),
            ("current_db".into(), array_f64(&r.current_db)),
            ("modified_db".into(), array_f64(&r.modified_db)),
        ],
    ))
}

/// Le résultat enregistré est en dB : objet de mode `psd`, ou spectrogramme (toujours en dB).
fn result_is_db(c: &Case) -> bool {
    match &c.result {
        Ok(Value::Obj(name, fields)) => {
            name == "Spectrogram"
                || fields
                    .iter()
                    .any(|(k, v)| k == "mode" && matches!(v, Value::Str(m) if m == "psd"))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------- sliders (phase 4b-5)

fn axis_pid_value(p: &AxisPid) -> Value {
    Value::Obj(
        "AxisPID".into(),
        vec![
            ("P".into(), Value::Int(p.p)),
            ("I".into(), Value::Int(p.i)),
            ("D".into(), Value::Int(p.d)),
            ("Dmax".into(), Value::Int(p.dmax)),
            ("F".into(), Value::Int(p.f)),
        ],
    )
}

fn axis_pids_value(pids: &[AxisPid]) -> Value {
    Value::List(pids.iter().map(axis_pid_value).collect())
}

fn axis_pids_arg(v: &Value) -> Result<Vec<AxisPid>, String> {
    let Value::List(items) = v else {
        return Err(format!("liste de AxisPID attendue, reçu {v:?}"));
    };
    items
        .iter()
        .map(|item| {
            let f = obj_fields(Some(item), "AxisPID")?;
            let n = |k: &str| int(Some(field(f, k)?), k);
            Ok(AxisPid {
                p: n("P")?,
                i: n("I")?,
                d: n("D")?,
                dmax: n("Dmax")?,
                f: n("F")?,
            })
        })
        .collect()
}

/// Dictionnaire de curseurs : les valeurs `None` (absentes) sont retirées, comme le fait l'appelant Python de chaque fonction.
fn sliders_arg(v: Option<&Value>) -> Result<Vec<(String, f64)>, String> {
    let Some(Value::Dict(items)) = v else {
        return Err(format!("dict de curseurs attendu, reçu {v:?}"));
    };
    let mut out = Vec::new();
    for (k, val) in items {
        let Value::Str(k) = k else {
            return Err(format!("clé de curseur : {k:?}"));
        };
        if !matches!(val, Value::Null) {
            out.push((k.clone(), float(val)?));
        }
    }
    Ok(out)
}

fn family_arg(c: &Case, pos: usize) -> Result<String, String> {
    match c.arg(pos, "family") {
        Some(Value::Str(s)) => Ok(s.clone()),
        other => Err(format!("family : chaîne attendue, reçu {other:?}")),
    }
}

fn s_default_pids(c: &Case) -> Result<Value, String> {
    Ok(axis_pids_value(&sliders::default_pids(&family_arg(c, 0)?)))
}

fn s_pids_from_sliders(c: &Case) -> Result<Value, String> {
    let sl = sliders_arg(c.arg(0, "sliders"))?;
    let borrowed: Vec<(&str, f64)> = sl.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let base = match c.arg(2, "base") {
        None | Some(Value::Null) => None,
        Some(v) => Some(axis_pids_arg(v)?),
    };
    sliders::pids_from_sliders(&borrowed, &family_arg(c, 1)?, base.as_deref()).map(|p| axis_pids_value(&p))
}

fn s_filters_from_sliders(c: &Case) -> Result<Value, String> {
    let s = optional_settings(c.arg(3, "settings"))?;
    let out = sliders::filters_from_sliders(
        int(c.arg(0, "gyro_mult"), "gyro_mult")?,
        int(c.arg(1, "dterm_mult"), "dterm_mult")?,
        s.as_ref(),
    );
    Ok(Value::Dict(
        out.into_iter()
            .map(|(k, v)| (Value::Str(k.into()), Value::Int(v)))
            .collect(),
    ))
}

fn s_sliders_from_pids(c: &Case) -> Result<Value, String> {
    let pids = axis_pids_arg(c.arg(0, "pids").ok_or("pids")?)?;
    let out = sliders::sliders_from_pids(&pids, &family_arg(c, 1)?);
    Ok(Value::Dict(
        out.into_iter()
            .map(|(k, v)| (Value::Str(k.into()), Value::F64(v)))
            .collect(),
    ))
}

/// En-tête (`raw`) d'un `Settings` enregistré : le journal Rust relit tout depuis lui, comme `parse_settings`.
fn raw_of_settings(v: Option<&Value>) -> Result<BTreeMap<String, String>, String> {
    let fields = obj_fields(v, "Settings")?;
    let Value::Dict(items) = field(fields, "raw")? else {
        return Err("settings.raw : dict attendu".into());
    };
    Ok(items
        .iter()
        .filter_map(|(k, v)| match (k, v) {
            (Value::Str(k), Value::Str(v)) => Some((k.clone(), v.clone())),
            _ => None,
        })
        .collect())
}

fn s_slider_view(c: &Case) -> Result<Value, String> {
    let st = otb_log::settings::parse_settings(&raw_of_settings(c.arg(0, "settings"))?);
    let v = sliders::slider_view(&st)?;
    Ok(Value::Obj(
        "SliderView".into(),
        vec![
            (
                "values".into(),
                Value::Dict(
                    v.values
                        .into_iter()
                        .map(|(k, x)| (Value::Str(k.into()), Value::F64(x)))
                        .collect(),
                ),
            ),
            ("from_header".into(), Value::Bool(v.from_header)),
            ("consistent".into(), Value::Bool(v.consistent)),
        ],
    ))
}

fn s_cli_commands(c: &Case) -> Result<Value, String> {
    let sl = sliders_arg(c.arg(0, "sliders"))?;
    let borrowed: Vec<(&str, f64)> = sl.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    sliders::cli_commands(&borrowed, &family_arg(c, 1)?)
        .map(|lines| Value::List(lines.into_iter().map(Value::Str).collect()))
}

/// Tolérance relative (au plus grand élément) de chaque fonction portée ; 0 = identique au bit près. Fixée en 4b-0, précisée
/// en 4b-1 à 4b-3 (docs/migration-rust.md) : ce qui passe par `cos`, `exp` ou une FFT s'écarte de numpy au dernier bit.
pub fn tolerance(function: &str, c: &Case) -> f64 {
    let out_f32 = matches!(&c.result, Ok(Value::Array(a)) if a.kind == 'f' && a.size == 4);
    match function {
        "_numeric.hann" => 1e-14,
        "_numeric.gaussian_filter" if out_f32 => 2e-7,
        "_numeric.gaussian_filter" => 1e-14,
        "_numeric.xcorr_full" => 1e-12,
        // sin / cos (libm), np.convolve (BLAS), atan2 : mesures de 4b-3 dans docs/migration-rust.md
        "bf_filters.coefficients" => 1e-14,
        "bf_filters.freq_response" | "bf_filters.chain_response" => 1e-12,
        "bf_filters.chain_delay_ms" => 1e-10,
        // filtres récursifs (coefficients sin / cos), puis FFT et dB : voir tests/bf_sim.rs
        "bf_filters.simulate_gyro" | "bf_filters.simulate_dterm" => 1e-12,
        "bf_filters.whatif_spectrum" => 1e-11,
        // SVD de Jacobi au lieu de LAPACK, `ln` et `exp` (mesure de 4b-5 dans docs/migration-rust.md)
        "sliders.sliders_from_pids" => 1e-11,
        "sliders.slider_view" => 1e-12,
        "spectral.to_db" => 1e-14,
        "spectral.smooth_map" => 1e-14,
        // FFT : amplitude ≈ 4e-16 du plus grand élément (mesuré en 4b-2) ; en dB la dynamique du spectre amplifie l'écart
        "spectral.spectrum" | "spectral.freq_throttle" | "spectral.freq_time" => {
            if result_is_db(c) {
                1e-9
            } else {
                1e-14
            }
        }
        _ => 0.0,
    }
}

pub fn lookup(function: &str) -> Option<Port> {
    Some(match function {
        "_numeric.hann" => hann,
        "_numeric.uniform_filter1d" => uniform_filter1d,
        "_numeric.gaussian_filter" => gaussian_filter,
        "_numeric.xcorr_full" => xcorr_full,
        "spectral.clear_of_breaks" => clear_of_breaks,
        "spectral.ptb_subsample" => ptb_subsample,
        "spectral.to_db" => to_db,
        "spectral.to_ptb_db" => to_ptb_db,
        "spectral.smooth_freq" => smooth_freq,
        "spectral.spectrum" => spectrum,
        "spectral.freq_throttle" => freq_throttle,
        "spectral.smooth_map" => smooth_map,
        "spectral.freq_time" => freq_time,
        "bf_filters.coefficients" => bf_coefficients,
        "bf_filters.freq_response" => bf_freq_response,
        "bf_filters.chain_response" => bf_chain_response,
        "bf_filters.chain_delay_ms" => bf_chain_delay_ms,
        "bf_filters.dyn_lpf_cutoff" => bf_dyn_lpf_cutoff,
        "bf_filters.scale_lowpass" => bf_scale_lowpass,
        "bf_filters.lowpass_off" => bf_lowpass_off,
        "bf_filters.rpm_notch_freq_weight" => bf_rpm_notch_freq_weight,
        "bf_filters.dyn_notch_enabled" => bf_dyn_notch_enabled,
        "bf_filters.dyn_notch_initial_centres" => bf_dyn_notch_initial_centres,
        "bf_filters.gyro_chain" => bf_gyro_chain,
        "bf_filters.dterm_chain" => bf_dterm_chain,
        "bf_filters.fft_freq_centres" => bf_fft_freq_centres,
        "bf_filters.simulate_gyro" => bf_simulate_gyro,
        "bf_filters.simulate_dterm" => bf_simulate_dterm,
        "bf_filters.whatif_spectrum" => bf_whatif_spectrum,
        "bf_filters.pt1_gain" => pt1_gain,
        "bf_filters.notch_q" => notch_q,
        "sliders.default_pids" => s_default_pids,
        "sliders.pids_from_sliders" => s_pids_from_sliders,
        "sliders.filters_from_sliders" => s_filters_from_sliders,
        "sliders.sliders_from_pids" => s_sliders_from_pids,
        "sliders.slider_view" => s_slider_view,
        "sliders.cli_commands" => s_cli_commands,
        "debug_tables.family" => family,
        "debug_tables.debug_mode_name" => debug_mode_name,
        "modes.mode_names" => mode_names,
        "modes.mode_changes" => mode_changes,
        "FlightLog.breaks" => lg_breaks,
        "FlightLog.recording_gaps" => lg_recording_gaps,
        "FlightLog.index_of" => lg_index_of,
        "FlightLog.slice_time" => lg_slice_time,
        "FlightLog.term_present" => lg_term_present,
        "FlightLog.debug_name" => lg_debug_name,
        "FlightLog.chirp_excitation" => lg_chirp,
        "FlightLog.pid_terms" => lg_pid_terms,
        "flightlog.cell_count" => cell_count,
        _ => return None,
    })
}
