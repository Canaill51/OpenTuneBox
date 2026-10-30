//! Implémentations Rust rejouables, par nom de fonction Python (`MODULE.FONCTION`).
//!
//! Chaque entrée reçoit l'appel enregistré et rend le résultat que Python aurait rendu. Une fonction absente d'ici
//! est comptée « non portée » (ce n'est pas un échec) : les phases 4 et 5 la remplissent module par module.

use crate::value::{Array, Case, Value};
use otb_dsp::numeric;
use otb_log::flightlog::{FlightLog, PidTerms};
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
        &numeric::gaussian_filter(&a, &shape, &sigma),
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

/// Tolérance relative (au plus grand élément) de chaque fonction portée ; 0 = identique au bit près. Fixée en 4b-0 puis
/// précisée en 4b-1 (docs/migration-rust.md) : ce qui passe par `cos`, `exp` ou une FFT s'écarte de numpy au dernier bit.
pub fn tolerance(function: &str, c: &Case) -> f64 {
    let out_f32 = matches!(&c.result, Ok(Value::Array(a)) if a.kind == 'f' && a.size == 4);
    match function {
        "_numeric.hann" => 1e-14,
        "_numeric.gaussian_filter" if out_f32 => 2e-7,
        "_numeric.gaussian_filter" => 1e-14,
        "_numeric.xcorr_full" => 1e-12,
        _ => 0.0,
    }
}

pub fn lookup(function: &str) -> Option<Port> {
    Some(match function {
        "_numeric.hann" => hann,
        "_numeric.uniform_filter1d" => uniform_filter1d,
        "_numeric.gaussian_filter" => gaussian_filter,
        "_numeric.xcorr_full" => xcorr_full,
        "bf_filters.pt1_gain" => pt1_gain,
        "bf_filters.notch_q" => notch_q,
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
