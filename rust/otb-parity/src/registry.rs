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

pub fn lookup(function: &str) -> Option<Port> {
    Some(match function {
        "bf_filters.pt1_gain" => pt1_gain,
        "bf_filters.notch_q" => notch_q,
        _ => return None,
    })
}
