//! Briques de Betaflight à l'arithmétique du firmware (`src/main/common/filter.c` et `maths.c`, lus version par version).
//!
//! L'arithmétique change d'une version à l'autre ([`Arith`]) :
//! - **4.3, 4.4** : sinus approché d'ordre 7 (repli par boucles `while`), gain des PT1/PT2/PT3 par la constante de temps
//!   `RC = 1/(2π·fc)`, filtres biquad ;
//! - **4.5** : gain des PTn par `ω/(ω+1)`, `ω = 2π·fc·dT` (avril 2023) ;
//! - **2025.12** : sinus approché replié par `fmodf` puis réflexion `π − x` ;
//! - **2026.6 et après** : sinus et cosinus par quadrant (mars 2026), filtres **SVF** à la place des biquads (mai 2026).
//!
//! ⚠️ Chaque opération suit l'ordre écrit dans le firmware : en float32, `état + k·(entrée − état)` n'a pas les bits de
//! `(1 − k)·état + k·entrée`, et `(a·b)·c` n'a pas ceux de `a·(b·c)`. Jamais de `mul_add` ici (ni `clippy::suboptimal_flops`,
//! L-051) : le firmware compilé sur PC ne fusionne pas la multiplication et l'addition.
//!
//! Le passage de la cadence gyro à la cadence PID ([`Decimator`]) est recopié de `sensors/gyro.c` et
//! `gyro_filter_impl.c` ; le banc ne compile pas ces fichiers (trop de dépendances) : son cas « deux cadences » vérifie
//! les briques du firmware enchaînées par cette recopie.

pub mod approx;
mod biquad;
pub mod cases;
mod decimate;
mod pt;
mod svf;

pub use biquad::{Biquad, BiquadKind};
pub use decimate::Decimator;
pub use pt::{Pt1, Pt2, Pt3};
pub use svf::{RpmNotch, SvfLowpass, SvfNotch};

/// `M_PIf` du firmware (`3.14159265358979323846f`) : c'est exactement `std::f32::consts::PI`.
pub const PI: f32 = std::f32::consts::PI;

/// Arithmétique d'une plage de versions de Betaflight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arith {
    /// 4.3 et 4.4.
    V4_3,
    /// 4.5.
    V4_5,
    /// 2025.12.
    V2025_12,
    /// 2026.6 et après (SVF).
    V2026_6,
}

impl Arith {
    /// Toutes les arithmétiques, de la plus ancienne à la plus récente.
    pub const ALL: [Arith; 4] = [Arith::V4_3, Arith::V4_5, Arith::V2025_12, Arith::V2026_6];

    /// Arithmétique d'une version (`major.minor` de l'en-tête « Firmware revision ») ; `None` avant la 4.3 (aucun log du
    /// corpus, rien de vérifié). Au-delà de la dernière version vérifiée, la plus récente s'applique (plages : lot 25).
    pub fn for_version(major: u32, minor: u32) -> Option<Arith> {
        match (major, minor) {
            (4, 3) | (4, 4) => Some(Arith::V4_3),
            (4, 5) => Some(Arith::V4_5),
            (2025, _) => Some(Arith::V2025_12),
            (y, _) if y >= 2026 => Some(Arith::V2026_6),
            _ => None,
        }
    }

    /// Filtres SVF (2026.6 et après) au lieu des biquads.
    pub fn uses_svf(self) -> bool {
        self == Arith::V2026_6
    }

    /// `sin_approx` de la version.
    pub fn sin(self, x: f32) -> f32 {
        match self {
            Arith::V4_3 | Arith::V4_5 => approx::sin_order7_while(x),
            Arith::V2025_12 => approx::sin_order7_fmod(x),
            Arith::V2026_6 => approx::sin_quadrant(x),
        }
    }

    /// `cos_approx` de la version.
    pub fn cos(self, x: f32) -> f32 {
        match self {
            Arith::V4_3 | Arith::V4_5 => approx::sin_order7_while(x + 0.5 * PI),
            Arith::V2025_12 => approx::sin_order7_fmod(x + 0.5 * PI),
            Arith::V2026_6 => approx::cos_quadrant(x),
        }
    }

    /// `pt1FilterGain(f_cut, dT)`.
    pub fn pt1_gain(self, f_cut: f32, dt: f32) -> f32 {
        match self {
            Arith::V4_3 => pt::gain_rc(pt::Order::One, f_cut, dt),
            _ => pt::gain_omega(f_cut, dt),
        }
    }

    /// `pt2FilterGain(f_cut, dT)`.
    pub fn pt2_gain(self, f_cut: f32, dt: f32) -> f32 {
        match self {
            Arith::V4_3 => pt::gain_rc(pt::Order::Two, f_cut, dt),
            _ => pt::gain_omega(f_cut * pt::CUTOFF_CORRECTION_PT2, dt),
        }
    }

    /// `pt3FilterGain(f_cut, dT)`.
    pub fn pt3_gain(self, f_cut: f32, dt: f32) -> f32 {
        match self {
            Arith::V4_3 => pt::gain_rc(pt::Order::Three, f_cut, dt),
            _ => pt::gain_omega(f_cut * pt::CUTOFF_CORRECTION_PT3, dt),
        }
    }
}

/// `filterGetNotchQ(centerFreq, cutoffFreq)` : identique dans toutes les versions.
pub fn notch_q(center_hz: f32, cutoff_hz: f32) -> f32 {
    center_hz * cutoff_hz / (center_hz * center_hz - cutoff_hz * cutoff_hz)
}
