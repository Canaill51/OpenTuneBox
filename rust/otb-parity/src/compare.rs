//! Comparaison d'un résultat Rust à celui de Python.

use crate::value::{Array, Value};

/// `rtol = 0` : identique au bit près (les NaN s'égalent). Sinon écart permis `rtol × max(|a|, |b|)` plus `rtol × maximum du tableau`,
/// comme `golden.py:same` (écart relatif au maximum près de zéro).
#[derive(Clone, Copy, Debug)]
pub struct Tol {
    pub rtol: f64,
}

fn floats_equal(a: f64, b: f64, tol: Tol, atol: f64) -> bool {
    if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) {
        return true;
    }
    tol.rtol > 0.0 && (a - b).abs() <= tol.rtol * a.abs().max(b.abs()) + atol
}

fn arrays_equal(a: &Array, b: &Array, tol: Tol) -> Result<(), String> {
    if a.shape != b.shape {
        return Err(format!("forme {:?} ≠ {:?}", a.shape, b.shape));
    }
    if (a.kind, a.size) != (b.kind, b.size) {
        return Err(format!("type {}{} ≠ {}{}", a.kind, a.size * 8, b.kind, b.size * 8));
    }
    if !matches!(a.kind, 'f' | 'c') {
        return if a.bytes == b.bytes {
            Ok(())
        } else {
            Err("entiers différents".into())
        };
    }
    let atol = tol.rtol
        * (0..a.n_floats())
            .map(|i| a.get_f64(i))
            .filter(|x| x.is_finite())
            .fold(0.0f64, |m, x| m.max(x.abs()));
    let mut bad = 0;
    let mut first = None;
    for i in 0..a.n_floats() {
        let (x, y) = (a.get_f64(i), b.get_f64(i));
        if !floats_equal(x, y, tol, atol) {
            bad += 1;
            first.get_or_insert((i, x, y));
        }
    }
    match first {
        None => Ok(()),
        Some((i, x, y)) => Err(format!(
            "{bad} valeurs sur {} différentes, première à l'indice {i} : {x:e} ≠ {y:e}",
            a.n_floats()
        )),
    }
}

/// `Ok(())` si identiques ; sinon le chemin et la première différence.
pub fn equal(expected: &Value, got: &Value, tol: Tol) -> Result<(), String> {
    use Value::*;
    let at =
        |path: &str, r: Result<(), String>| r.map_err(|e| if path.is_empty() { e } else { format!("{path} : {e}") });
    match (expected, got) {
        (Null, Null) => Ok(()),
        (Bool(a), Bool(b)) if a == b => Ok(()),
        (Int(a), Int(b)) if a == b => Ok(()),
        (Str(a), Str(b)) if a == b => Ok(()),
        (F64(a), F64(b)) if floats_equal(*a, *b, tol, 0.0) => Ok(()),
        (NpScalar(t, a), NpScalar(u, b)) if t == u => equal(a, b, tol),
        (Array(a), Array(b)) => arrays_equal(a, b, tol),
        (List(a), List(b)) | (Tuple(a), Tuple(b)) => {
            if a.len() != b.len() {
                return Err(format!("{} éléments ≠ {}", a.len(), b.len()));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                at(&format!("[{i}]"), equal(x, y, tol))?;
            }
            Ok(())
        }
        (Dict(a), Dict(b)) => {
            if a.len() != b.len() {
                return Err(format!("{} entrées ≠ {}", a.len(), b.len()));
            }
            for (i, ((ka, va), (kb, vb))) in a.iter().zip(b).enumerate() {
                at(&format!("clé {i}"), equal(ka, kb, tol))?;
                at(&format!("valeur {i}"), equal(va, vb, tol))?;
            }
            Ok(())
        }
        (Obj(n, fa), Obj(m, fb)) if n == m => {
            for (k, va) in fa {
                let vb = fb
                    .iter()
                    .find(|(l, _)| l == k)
                    .map(|(_, v)| v)
                    .ok_or(format!(".{k} absent"))?;
                at(&format!(".{k}"), equal(va, vb, tol))?;
            }
            Ok(())
        }
        (Slice(a), Slice(b)) => {
            for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
                at(&format!("tranche[{i}]"), equal(x, y, tol))?;
            }
            Ok(())
        }
        (FlightLog(p, s), FlightLog(q, t)) if p == q && s == t => Ok(()),
        (a, b) => Err(format!(
            "valeurs de natures ou de contenus différents : {} ≠ {}",
            brief(a),
            brief(b)
        )),
    }
}

fn brief(v: &Value) -> String {
    let s = format!("{v:?}");
    if s.len() > 80 {
        format!("{}…", s.chars().take(80).collect::<String>())
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arr(vals: &[f64]) -> Value {
        Value::Array(Array {
            shape: vec![vals.len()],
            kind: 'f',
            size: 8,
            bytes: vals.iter().flat_map(|v| v.to_le_bytes()).collect(),
        })
    }
    const EXACT: Tol = Tol { rtol: 0.0 };

    #[test]
    fn un_bit_de_difference_est_vu_en_mode_exact() {
        let a = 1.0f64;
        let b = f64::from_bits(a.to_bits() + 1);
        assert!(equal(&F(a), &F(a), EXACT).is_ok());
        assert!(equal(&F(a), &F(b), EXACT).is_err());
        assert!(equal(&arr(&[1.0, 2.0]), &arr(&[1.0, 2.0 + 4e-16]), EXACT).is_err());
    }

    #[allow(non_snake_case)]
    fn F(x: f64) -> Value {
        Value::F64(x)
    }

    #[test]
    fn la_tolerance_relative_accepte_le_petit_ecart_et_refuse_le_grand() {
        let tol = Tol { rtol: 1e-9 };
        assert!(equal(&arr(&[1.0, 100.0]), &arr(&[1.0 + 1e-12, 100.0]), tol).is_ok());
        assert!(equal(&arr(&[1.0, 100.0]), &arr(&[1.0 + 1e-6, 100.0]), tol).is_err());
    }

    #[test]
    fn les_nan_s_egalent_et_la_forme_compte() {
        assert!(equal(&F(f64::NAN), &F(f64::NAN), EXACT).is_ok());
        assert!(equal(&arr(&[f64::NAN]), &arr(&[f64::NAN]), EXACT).is_ok());
        assert!(equal(&arr(&[1.0]), &arr(&[1.0, 1.0]), EXACT).is_err());
        assert!(equal(&Value::Int(1), &F(1.0), EXACT).is_err());
    }

    #[test]
    fn les_tranches_se_comparent_borne_par_borne() {
        let sl = |a: i64, b: i64| Value::Slice(Box::new([Value::Int(a), Value::Int(b), Value::Null]));
        assert!(equal(&sl(1, 5), &sl(1, 5), EXACT).is_ok());
        assert!(
            equal(&sl(1, 5), &sl(1, 6), EXACT)
                .unwrap_err()
                .starts_with("tranche[1]")
        );
    }

    #[test]
    fn un_champ_different_d_une_dataclass_est_localise() {
        let a = Value::Obj("M".into(), vec![("x".into(), F(1.0)), ("y".into(), F(2.0))]);
        let b = Value::Obj("M".into(), vec![("x".into(), F(1.0)), ("y".into(), F(3.0))]);
        assert!(equal(&a, &b, EXACT).unwrap_err().starts_with(".y"));
    }
}
