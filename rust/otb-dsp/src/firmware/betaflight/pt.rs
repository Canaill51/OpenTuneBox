//! Passe-bas PT1, PT2, PT3 de Betaflight (`common/filter.c`) : gains et filtres.
//!
//! Constantes écrites comme dans le firmware ; Rust les arrondit au float32 le plus proche, comme le compilateur C.
#![allow(clippy::excessive_precision)]

use super::PI;

/// `CUTOFF_CORRECTION_PT2` (4.5 et après) : `1/sqrt(2^(1/2) − 1)` écrit par le firmware avec 10 chiffres.
pub const CUTOFF_CORRECTION_PT2: f32 = 1.553773974;
/// `CUTOFF_CORRECTION_PT3` (4.5 et après).
pub const CUTOFF_CORRECTION_PT3: f32 = 1.961459177;

/// `powf(2, 1.0f / order)` des 4.3 et 4.4. GCC l'évalue **à la compilation**, arrondi au plus près : pour l'ordre 2, le float32
/// le plus proche de √2 ; pour l'ordre 3, celui de 2^0,333333343 (l'exposant `1.0f / 3` est lui-même arrondi en float32).
/// Écrits ici plutôt que calculés par `f32::powf`, qui appelle la bibliothèque système, pas toujours arrondie au plus près (L-048).
const POW2_HALF: f32 = std::f32::consts::SQRT_2;
const POW2_THIRD: f32 = 1.259_921_1;

/// Gain 4.5 et après : `omega = 2.0f * M_PIf * f_cut * dT; return omega / (omega + 1.0f);`.
pub(super) fn gain_omega(f_cut: f32, dt: f32) -> f32 {
    let omega = 2.0 * PI * f_cut * dt;
    omega / (omega + 1.0)
}

/// Ordre d'un PTn des 4.3 et 4.4.
#[derive(Clone, Copy)]
pub(super) enum Order {
    One,
    Two,
    Three,
}

/// Gain 4.3, 4.4 par la constante de temps. PT1 : `RC = 1 / (2 * M_PIf * f_cut)` ; PT2, PT3 :
/// `RC = 1 / (2 * orderCutoffCorrection * M_PIf * f_cut)` (autre ordre des produits) ; puis `dT / (RC + dT)`.
pub(super) fn gain_rc(order: Order, f_cut: f32, dt: f32) -> f32 {
    let rc = match order {
        Order::One => 1.0 / (2.0 * PI * f_cut),
        Order::Two | Order::Three => {
            let p = if matches!(order, Order::Two) {
                POW2_HALF
            } else {
                POW2_THIRD
            };
            let corr = 1.0 / (p - 1.0).sqrt();
            1.0 / (2.0 * corr * PI * f_cut)
        }
    };
    dt / (rc + dt)
}

/// PT1 (`pt1Filter_t`) : `état = état + k·(entrée − état)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pt1 {
    state: f32,
    k: f32,
}

impl Pt1 {
    /// `pt1FilterInit` : état à zéro.
    pub fn new(k: f32) -> Self {
        Pt1 { state: 0.0, k }
    }
    /// `pt1FilterUpdateCutoff` : nouveau gain, état gardé.
    pub fn set_gain(&mut self, k: f32) {
        self.k = k;
    }
    /// `pt1FilterApply`.
    pub fn apply(&mut self, input: f32) -> f32 {
        self.state += self.k * (input - self.state);
        self.state
    }
}

/// PT2 (`pt2Filter_t`) : deux PT1 de même gain en cascade.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pt2 {
    state: f32,
    state1: f32,
    k: f32,
}

impl Pt2 {
    /// `pt2FilterInit`.
    pub fn new(k: f32) -> Self {
        Pt2 {
            state: 0.0,
            state1: 0.0,
            k,
        }
    }
    /// `pt2FilterUpdateCutoff`.
    pub fn set_gain(&mut self, k: f32) {
        self.k = k;
    }
    /// `pt2FilterApply`.
    pub fn apply(&mut self, input: f32) -> f32 {
        self.state1 += self.k * (input - self.state1);
        self.state += self.k * (self.state1 - self.state);
        self.state
    }
}

/// PT3 (`pt3Filter_t`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Pt3 {
    state: f32,
    state1: f32,
    state2: f32,
    k: f32,
}

impl Pt3 {
    /// `pt3FilterInit`.
    pub fn new(k: f32) -> Self {
        Pt3 {
            state: 0.0,
            state1: 0.0,
            state2: 0.0,
            k,
        }
    }
    /// `pt3FilterUpdateCutoff`.
    pub fn set_gain(&mut self, k: f32) {
        self.k = k;
    }
    /// `pt3FilterApply`.
    pub fn apply(&mut self, input: f32) -> f32 {
        self.state1 += self.k * (input - self.state1);
        self.state2 += self.k * (self.state1 - self.state2);
        self.state += self.k * (self.state2 - self.state);
        self.state
    }
}
