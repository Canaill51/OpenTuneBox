//! Sinus et cosinus approchés de Betaflight (`common/maths.c`, `FAST_MATH` toujours défini) : des polynômes en float32, donc
//! de l'arithmétique pure, identique au bit près sur toute machine (contrairement au `sin` de la bibliothèque système, L-048).
//!
//! Les constantes sont écrites comme dans le firmware ; Rust les arrondit au float32 le plus proche, comme le compilateur C.
#![allow(clippy::excessive_precision)]

use super::PI;

// Ordre 7 (`VERY_FAST_MATH`, 4.3 à 2025.12) ; le coefficient d'ordre 9 vaut 0 et reste dans le calcul, comme dans le firmware.
const SIN_COEF3: f32 = -1.666568107e-1;
const SIN_COEF5: f32 = 8.312366210e-3;
const SIN_COEF7: f32 = -1.849218155e-4;
const SIN_COEF9: f32 = 0.0;

/// Polynôme d'ordre 7 sur [−π/2, π/2].
fn poly7(x: f32) -> f32 {
    let x2 = x * x;
    x + x * x2 * (SIN_COEF3 + x2 * (SIN_COEF5 + x2 * (SIN_COEF7 + x2 * SIN_COEF9)))
}

/// `sin_approx` de la 4.3 à la 4.5 : 0 hors de ±32 rad (entier tronqué), repli par boucles, réflexion autour de ±π/2.
pub fn sin_order7_while(x: f32) -> f32 {
    let mut x = x;
    // `int32_t xint = x;` : troncature vers zéro.
    let xint = x as i32;
    if !(-32..=32).contains(&xint) {
        return 0.0;
    }
    while x > PI {
        x -= 2.0 * PI;
    }
    while x < -PI {
        x += 2.0 * PI;
    }
    if x > 0.5 * PI {
        x = 0.5 * PI - (x - 0.5 * PI);
    } else if x < -(0.5 * PI) {
        x = -(0.5 * PI) - (0.5 * PI + x);
    }
    poly7(x)
}

/// `sin_approx` de la 2025.12 : repli par `fmodf` (exact), puis boucles, puis réflexion `π − x`.
pub fn sin_order7_fmod(x: f32) -> f32 {
    // Le `%` de Rust sur un flottant est `fmodf` : résultat exact, sans arrondi.
    let mut x = x % (2.0 * PI);
    while x > PI {
        x -= 2.0 * PI;
    }
    while x < -PI {
        x += 2.0 * PI;
    }
    if x > PI / 2.0 {
        x = PI - x;
    } else if x < -PI / 2.0 {
        x = -PI - x;
    }
    poly7(x)
}

// 2026.6 et après : polynômes en r = x·2/π − round(x·2/π) ∈ [−0,5 ; 0,5], valeurs exactes des constantes hexadécimales.
const C0: f32 = 1.5707871913909912109375;
const C1: f32 = -0.6456851959228515625;
const C2: f32 = 7.756288349628448486328125e-2;
const D1: f32 = -1.2336976528167724609375;
const D2: f32 = 0.25360107421875;
const D3: f32 = -2.04083733260631561279296875e-2;
/// `INV_PIO2 = 2.0f / M_PIf`, évalué par le compilateur en float32.
const INV_PIO2: f32 = 2.0 / PI;

fn sin_poly5(r: f32, s: f32) -> f32 {
    r * ((C2 * s + C1) * s + C0)
}

fn cos_poly6(s: f32) -> f32 {
    ((D3 * s + D2) * s + D1) * s + 1.0
}

/// Quadrant `q` (entier) et reste `r`.
fn range_reduce(x: f32) -> (f32, i32) {
    let t = x * INV_PIO2;
    // `roundf` : demi-entier arrondi loin de zéro, comme `f32::round`.
    let qf = t.round();
    (t - qf, qf as i32)
}

/// `sin_approx` de la 2026.6.
pub fn sin_quadrant(x: f32) -> f32 {
    let (r, q) = range_reduce(x);
    let s = r * r;
    let q = q & 3;
    let v = if q & 1 != 0 { cos_poly6(s) } else { sin_poly5(r, s) };
    if q & 2 != 0 { -v } else { v }
}

/// `cos_approx` de la 2026.6.
pub fn cos_quadrant(x: f32) -> f32 {
    let (r, q) = range_reduce(x);
    let s = r * r;
    let q = q & 3;
    let v = if q & 1 != 0 { -sin_poly5(r, s) } else { cos_poly6(s) };
    if q & 2 != 0 { -v } else { v }
}

/// `sincosf_approx` de la 2026.6 (coefficients des SVF) : (sinus, cosinus).
pub fn sincos_quadrant(x: f32) -> (f32, f32) {
    let (r, q) = range_reduce(x);
    let s = r * r;
    let q = q & 3;
    let sb = sin_poly5(r, s);
    let cb = cos_poly6(s);
    let (mut sv, mut cv) = if q & 1 != 0 { (cb, -sb) } else { (sb, cb) };
    if q & 2 != 0 {
        sv = -sv;
        cv = -cv;
    }
    (sv, cv)
}
