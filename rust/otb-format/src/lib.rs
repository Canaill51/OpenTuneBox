//! Formats de nombres de Python reproduits en Rust (D-035, étape E2).
//!
//! Les textes de l'assistant et de la ligne de commande sont comparés au caractère près à ceux de l'étalon Python :
//! `format!("{:.2}")` suffit pour les décimales fixes, mais ni `%g`, ni `repr`, ni `round` n'ont d'équivalent direct
//! (`f64::round` arrondit les moitiés vers l'extérieur, `round` de Python vers le pair).

/// Valeur non finie telle que Python l'écrit (`nan`, `inf`, `-inf`), avec le signe `+` demandé.
fn non_finite(x: f64, plus: bool) -> Option<String> {
    if x.is_finite() {
        return None;
    }
    let s = if x.is_nan() {
        "nan"
    } else if x > 0.0 {
        "inf"
    } else {
        "-inf"
    };
    Some(if plus && !s.starts_with('-') {
        format!("+{s}")
    } else {
        s.to_string()
    })
}

fn with_plus(s: String, plus: bool) -> String {
    if plus && !s.starts_with('-') {
        format!("+{s}")
    } else {
        s
    }
}

/// `f"{x:.{prec}f}"` (ou `{x:+.{prec}f}` si `plus`).
pub fn fixed(x: f64, prec: usize, plus: bool) -> String {
    non_finite(x, plus).unwrap_or_else(|| with_plus(format!("{x:.prec$}"), plus))
}

/// `f"{x:.{prec}%}"` : la valeur est multipliée par 100 (le produit est arrondi comme en Python : en flottant).
pub fn percent(x: f64, prec: usize) -> String {
    match non_finite(x, false) {
        Some(s) => format!("{s}%"),
        None => format!("{:.prec$}%", x * 100.0),
    }
}

fn strip_zeros(mut s: String) -> String {
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn exp_suffix(e: i32) -> String {
    format!("e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// `f"{x:.{prec}g}"` (`prec` = 6 pour `f"{x:g}"`).
pub fn general(x: f64, prec: usize) -> String {
    if let Some(s) = non_finite(x, false) {
        return s;
    }
    let p = prec.max(1);
    let sci = format!("{x:.*e}", p - 1);
    let (mant, exp) = sci.split_once('e').unwrap();
    let e: i32 = exp.parse().unwrap();
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    if (-4..p as i32).contains(&e) {
        strip_zeros(format!("{x:.*}", (p as i32 - 1 - e) as usize))
    } else {
        strip_zeros(mant.to_string()) + &exp_suffix(e)
    }
}

/// `repr(x)` d'un flottant Python : plus court texte qui se relit identique.
pub fn repr(x: f64) -> String {
    if let Some(s) = non_finite(x, false) {
        return s;
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    // Moins de chiffres possible dont l'arrondi exact se relit identique. `{:e}` (algorithme court de Rust) ne convient
    // pas : sur une égalité exacte à 16 chiffres (ex. 696994262879799.25), il prend le chiffre supérieur, Python le pair.
    let sci = (0..17)
        .map(|p| format!("{x:.p$e}"))
        .find(|s| s.parse::<f64>() == Ok(x))
        .unwrap();
    let (mant, exp) = sci.split_once('e').unwrap();
    let e: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let sign = if neg { "-" } else { "" };
    if (-4..16).contains(&e) {
        let n = digits.len() as i32;
        if e < 0 {
            format!("{sign}0.{}{digits}", "0".repeat((-e - 1) as usize))
        } else if e + 1 >= n {
            format!("{sign}{digits}{}.0", "0".repeat((e + 1 - n) as usize))
        } else {
            format!("{sign}{}.{}", &digits[..=e as usize], &digits[e as usize + 1..])
        }
    } else {
        let m = if digits.len() == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!("{sign}{m}{}", exp_suffix(e))
    }
}

/// `round(x)` de Python 3 : entier, moitiés vers le pair. `None` pour `nan` et `inf` (Python lève une exception) et
/// au-delà de `i64` (Python rend un entier sans limite : aucune valeur de l'application n'en approche).
pub fn round_half_even(x: f64) -> Option<i64> {
    if !x.is_finite() || x.abs() >= 9.2e18 {
        return None;
    }
    Some(x.round_ties_even() as i64)
}

/// `round(x, n)` de Python 3 : comme CPython, arrondi exact du texte décimal puis relecture.
pub fn round_digits(x: f64, n: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.n$}").parse().unwrap()
}
