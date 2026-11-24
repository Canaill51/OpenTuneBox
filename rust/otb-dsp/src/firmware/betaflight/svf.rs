//! Filtres à variables d'état (SVF, forme TPT) de Betaflight 2026.6 et après (`common/filter.c`), à la place des biquads.
//!
//! Même prototype analogique que le biquad RBJ, mais la coupure passe par `tan(π·f·dt)` (sinus et cosinus approchés, puis
//! division) : transformation bilinéaire avec précompensation à la fréquence du filtre. L'état n'est pas celui d'un biquad,
//! et le notch RPM se **retranche** de l'entrée (`entrée − poids·bande`) au lieu d'être mélangé.

use super::PI;
use super::approx::sincos_quadrant;

/// `f = tan(π·f·dt)` par `sincosf_approx(M_PIf * filterFreq * dt)` puis `sn / cs`.
fn prewarp(freq: f32, dt: f32) -> f32 {
    let (sn, cs) = sincos_quadrant(PI * freq * dt);
    sn / cs
}

/// `svfLowpassFilter_t` : passe-bas de Butterworth (Q = 1/√2, `1/Q` écrit `1.41421356237f`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SvfLowpass {
    a1: f32,
    a2: f32,
    f: f32,
    ic1: f32,
    ic2: f32,
}

/// `inv_butterworth_q = 1.41421356237f` : arrondi en float32, c'est exactement `SQRT_2`.
const INV_BUTTERWORTH_Q: f32 = std::f32::consts::SQRT_2;

impl SvfLowpass {
    /// `svfLowpassFilterInit` : coefficients, état à zéro.
    pub fn new(freq: f32, dt: f32) -> Self {
        let mut s = SvfLowpass::default();
        s.update(freq, dt);
        s
    }
    /// `svfLowpassFilterUpdate`.
    pub fn update(&mut self, freq: f32, dt: f32) {
        let f = prewarp(freq, dt);
        let inv_denom = 1.0 / (1.0 + f * (f + INV_BUTTERWORTH_Q));
        self.a1 = inv_denom;
        self.a2 = f * inv_denom;
        self.f = f;
    }
    /// `svfLowpassFilterApply`.
    pub fn apply(&mut self, input: f32) -> f32 {
        let v3 = input - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.f * v1;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v2
    }
}

/// `svfNotchFilter_t` (notchs statiques et notch dynamique).
#[derive(Clone, Copy, Debug, Default)]
pub struct SvfNotch {
    a1: f32,
    a2q: f32,
    fq: f32,
    ic1q: f32,
    ic2: f32,
}

impl SvfNotch {
    /// `svfNotchInit`.
    pub fn new(freq: f32, dt: f32, q: f32) -> Self {
        let mut s = SvfNotch::default();
        s.update(freq, dt, q);
        s
    }
    /// `svfNotchUpdate`.
    pub fn update(&mut self, freq: f32, dt: f32, q_factor: f32) {
        let f = prewarp(freq, dt);
        let q = 1.0 / q_factor;
        let inv_denom = 1.0 / (1.0 + f * (f + q));
        self.a1 = inv_denom;
        self.a2q = (f * inv_denom) * q;
        self.fq = f * q_factor;
    }
    /// `svfNotchApply` : entrée moins la sortie passe-bande.
    pub fn apply(&mut self, input: f32) -> f32 {
        let v3 = input - self.ic2;
        let v1q = self.a1 * self.ic1q + self.a2q * v3;
        let v2 = self.ic2 + self.fq * v1q;
        self.ic1q = 2.0 * v1q - self.ic1q;
        self.ic2 = 2.0 * v2 - self.ic2;
        input - v1q
    }
}

/// `rpmNotch_t` : un notch RPM pondéré, mêmes coefficients pour les trois axes, un état par axe.
#[derive(Clone, Copy, Debug, Default)]
pub struct RpmNotch {
    a1: f32,
    a2: f32,
    f: f32,
    wq: f32,
    ic1: [f32; 3],
    ic2: [f32; 3],
}

impl RpmNotch {
    /// `rpmNotchInit`.
    pub fn new(freq: f32, dt: f32, q: f32, weight: f32) -> Self {
        let mut s = RpmNotch::default();
        s.update(freq, dt, q, weight);
        s
    }
    /// `rpmNotchUpdate`.
    pub fn update(&mut self, freq: f32, dt: f32, q_factor: f32, weight: f32) {
        let f = prewarp(freq, dt);
        let q = 1.0 / q_factor;
        let a1 = 1.0 / (1.0 + f * (f + q));
        self.f = f;
        self.a1 = a1;
        self.a2 = f * a1;
        self.wq = q * weight;
    }
    /// `rpmNotchApply` : filtre les trois axes en place.
    pub fn apply(&mut self, input: &mut [f32; 3]) {
        for (i, x) in input.iter_mut().enumerate() {
            let v3 = *x - self.ic2[i];
            let v1 = self.a1 * self.ic1[i] + self.a2 * v3;
            let v2 = self.ic2[i] + self.f * v1;
            self.ic1[i] = 2.0 * v1 - self.ic1[i];
            self.ic2[i] = 2.0 * v2 - self.ic2[i];
            *x -= self.wq * v1;
        }
    }
}
