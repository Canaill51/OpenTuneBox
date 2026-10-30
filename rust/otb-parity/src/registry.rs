//! Implémentations Rust rejouables, par nom de fonction Python (`MODULE.FONCTION`).
//!
//! Chaque entrée reçoit l'appel enregistré et rend le résultat que Python aurait rendu. Une fonction absente d'ici
//! est comptée « non portée » (ce n'est pas un échec) : les phases 4 et 5 la remplissent module par module.

use crate::value::{Case, Value};

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

pub fn lookup(function: &str) -> Option<Port> {
    Some(match function {
        "bf_filters.pt1_gain" => pt1_gain,
        "bf_filters.notch_q" => notch_q,
        "debug_tables.family" => family,
        "debug_tables.debug_mode_name" => debug_mode_name,
        "modes.mode_names" => mode_names,
        "modes.mode_changes" => mode_changes,
        _ => return None,
    })
}
