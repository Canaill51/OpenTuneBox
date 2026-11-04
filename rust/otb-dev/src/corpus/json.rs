//! JSON des outils du corpus avec `NaN`, `Infinity` et `-Infinity`, comme `json.dump` / `json.load` de Python.
//!
//! `serde_json::Value` ne peut pas contenir de nombre non fini (il écrit `null`), alors que les mesures du corpus en ont
//! (croisement introuvable : NaN ; raie collée au bord : −∞). Une valeur non finie est donc gardée sous la forme d'une chaîne
//! sentinelle (qui commence par un caractère NUL, absent des vrais textes) ; `dump` l'écrit en jeton nu, `parse` fait l'inverse.
//! Les deux outils (Python et Rust) peuvent ainsi lire le fichier l'un de l'autre.

use serde_json::Value;

const NAN: &str = "\u{0}NaN";
const INF: &str = "\u{0}Infinity";
const NEG_INF: &str = "\u{0}-Infinity";

/// Un nombre JSON ; les valeurs non finies passent par la sentinelle.
pub fn num(x: f64) -> Value {
    if x.is_nan() {
        Value::String(NAN.into())
    } else if x == f64::INFINITY {
        Value::String(INF.into())
    } else if x == f64::NEG_INFINITY {
        Value::String(NEG_INF.into())
    } else {
        serde_json::json!(x)
    }
}

/// Valeur numérique d'un champ (nombre ou sentinelle), `None` pour tout autre type (texte, booléen, absent, `null`).
pub fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) if s == NAN => Some(f64::NAN),
        Value::String(s) if s == INF => Some(f64::INFINITY),
        Value::String(s) if s == NEG_INF => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

/// Le texte JSON, avec `NaN`, `Infinity` et `-Infinity` en jetons nus (ce que Python écrit et relit).
pub fn dump(v: &Value) -> Result<String, String> {
    let text = serde_json::to_string(v).map_err(|e| e.to_string())?;
    Ok(text
        .replace("\"\\u0000NaN\"", "NaN")
        .replace("\"\\u0000Infinity\"", "Infinity")
        .replace("\"\\u0000-Infinity\"", "-Infinity"))
}

/// Lit un JSON qui peut contenir `NaN`, `Infinity` et `-Infinity` hors des textes (un nom de drone « NaN » reste un texte).
pub fn parse(text: &str) -> Result<Value, String> {
    let mut out = String::with_capacity(text.len() + 16);
    let (mut in_str, mut escaped) = (false, false);
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let token = [
            ("NaN", "\"\\u0000NaN\""),
            ("-Infinity", "\"\\u0000-Infinity\""),
            ("Infinity", "\"\\u0000Infinity\""),
        ]
        .into_iter()
        .find(|(t, _)| rest.starts_with(t));
        if let Some((t, replacement)) = token {
            out.push_str(replacement);
            rest = &rest[t.len()..];
            continue;
        }
        if c == '"' {
            in_str = true;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    serde_json::from_str(&out).map_err(|e| e.to_string())
}
