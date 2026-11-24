//! Biquad RBJ de Betaflight, 4.3 à 2025.12 (`biquadFilter_t`, `common/filter.c`) ; remplacé par les SVF en 2026.6.
//!
//! Trois façons de l'appliquer, chacune à sa place dans le firmware : forme directe II ([`Biquad::apply_df2`],
//! `biquadFilterApply`) pour un filtre aux coefficients fixes (passe-bas et notchs statiques), forme directe I
//! ([`Biquad::apply_df1`]) pour un filtre recalculé en vol (LPF dynamique, notch dynamique), forme directe I pondérée
//! ([`Biquad::apply_df1_weighted`]) pour les notchs RPM.

use super::{Arith, PI};

/// `biquadFilterType_e`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiquadKind {
    Lowpass,
    Notch,
    Bandpass,
}

/// `biquadFilter_t` : coefficients divisés par a0, deux entrées et deux sorties passées, poids du mélange.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    arith: Arith,
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    weight: f32,
}

/// `BIQUAD_Q = 1.0f / sqrtf(2.0f)`.
pub fn butterworth_q() -> f32 {
    1.0 / 2.0_f32.sqrt()
}

impl Biquad {
    /// `biquadFilterInit` : coefficients, puis entrées et sorties passées à zéro. `refresh_rate_us` est l'entier de microsecondes
    /// du firmware (`uint32_t`).
    pub fn new(arith: Arith, freq: f32, refresh_rate_us: u32, q: f32, kind: BiquadKind, weight: f32) -> Self {
        let mut b = Biquad {
            arith,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            weight,
        };
        b.update(freq, refresh_rate_us, q, kind, weight);
        b
    }

    /// `biquadFilterInitLPF` : passe-bas de Butterworth.
    pub fn lowpass(arith: Arith, freq: f32, refresh_rate_us: u32) -> Self {
        Self::new(arith, freq, refresh_rate_us, butterworth_q(), BiquadKind::Lowpass, 1.0)
    }

    /// `biquadFilterUpdate` : nouveaux coefficients, état gardé.
    pub fn update(&mut self, freq: f32, refresh_rate_us: u32, q: f32, kind: BiquadKind, weight: f32) {
        let omega = 2.0 * PI * freq * refresh_rate_us as f32 * 0.000_001;
        let sn = self.arith.sin(omega);
        let cs = self.arith.cos(omega);
        let alpha = sn / (2.0 * q);
        match kind {
            BiquadKind::Lowpass => {
                self.b1 = 1.0 - cs;
                self.b0 = self.b1 * 0.5;
                self.b2 = self.b0;
                self.a1 = -2.0 * cs;
                self.a2 = 1.0 - alpha;
            }
            BiquadKind::Notch => {
                self.b0 = 1.0;
                self.b1 = -2.0 * cs;
                self.b2 = 1.0;
                self.a1 = self.b1;
                self.a2 = 1.0 - alpha;
            }
            BiquadKind::Bandpass => {
                self.b0 = alpha;
                self.b1 = 0.0;
                self.b2 = -alpha;
                self.a1 = -2.0 * cs;
                self.a2 = 1.0 - alpha;
            }
        }
        let a0 = 1.0 + alpha;
        self.b0 /= a0;
        self.b1 /= a0;
        self.b2 /= a0;
        self.a1 /= a0;
        self.a2 /= a0;
        self.weight = weight;
    }

    /// `biquadFilterUpdateLPF`.
    pub fn update_lowpass(&mut self, freq: f32, refresh_rate_us: u32) {
        self.update(freq, refresh_rate_us, butterworth_q(), BiquadKind::Lowpass, 1.0);
    }

    /// `biquadFilterApplyDF1` : forme directe I.
    pub fn apply_df1(&mut self, input: f32) -> f32 {
        let result = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = result;
        result
    }

    /// `biquadFilterApplyDF1Weighted` : sortie mélangée à l'entrée selon le poids (notchs RPM).
    pub fn apply_df1_weighted(&mut self, input: f32) -> f32 {
        let result = self.apply_df1(input);
        self.weight * result + (1.0 - self.weight) * input
    }

    /// `biquadFilterApply` : forme directe II transposée (deux états `x1`, `x2`).
    pub fn apply_df2(&mut self, input: f32) -> f32 {
        let result = self.b0 * input + self.x1;
        self.x1 = self.b1 * input - self.a1 * result + self.x2;
        self.x2 = self.b2 * input - self.a2 * result;
        result
    }
}
