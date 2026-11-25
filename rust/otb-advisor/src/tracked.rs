//! Mesures suivies dans le temps (lot 31 de `docs/plan.md`) : les grandeurs d'un fragment que la bibliothèque garde
//! pour répondre à « est-ce que ce drone s'améliore ? », et que la fiche cible du drone fixe.
//!
//! ⚠️ **Jamais un second calcul** : chaque grandeur est **lue dans le rapport de l'assistant** (`Report`), là où ses
//! règles la lisent. Une latence suivie qui ne serait pas celle dont parle l'assistant ferait deux chiffres pour une
//! même chose, sur deux écrans.
//!
//! Une grandeur qui ne s'est pas mesurée (pas de réponse indicielle, pas de terme D, vol sans mouvements) vaut NaN, et
//! sa ligne est **gardée** : le fragment compte alors comme mesuré, sans se recalculer à chaque ouverture.

use crate::rules::{Report, analyze};
use otb_log::flightlog::FlightLog;
use std::ops::Range;

/// Version du calcul des mesures suivies. ⚠️ **À monter dès qu'une grandeur change de calcul** (règle, rapport,
/// réponse indicielle, bandes de bruit, retards) : la bibliothèque jette alors les mesures d'une version antérieure
/// et les recalcule. Sans cela, l'historique mêlerait deux calculs sans le dire.
/// 2 (2026-11-25, lot 35, D-082) : tri des fenêtres de la réponse indicielle (pic, latence, temps de montée, marges).
pub const CALC_VERSION: u64 = 2;

/// Axe d'une grandeur qui ne dépend pas d'un axe (bruit, retard des filtres : roll et pitch réunis comme dans la
/// règle qui les juge). Roll, pitch et lacet sont 0, 1 et 2.
pub const ALL_AXES: u64 = 3;

/// Grandeurs par axe (roll, pitch, lacet), lues dans la réponse indicielle du rapport.
pub const LATENCY_MS: &str = "latency_ms";
pub const PEAK: &str = "peak";
pub const RISE_MS: &str = "rise_ms";
pub const PHASE_MARGIN_DEG: &str = "phase_margin_deg";
pub const GAIN_MARGIN_DB: &str = "gain_margin_db";
/// Nombre de fenêtres de la réponse indicielle : dit si les quatre grandeurs précédentes sont sûres.
pub const STEP_WINDOWS: &str = "step_windows";

/// Grandeurs du fragment entier (`ALL_AXES`).
pub const GYRO_NOISE_DB: &str = "gyro_noise_db";
pub const DTERM_NOISE_DB: &str = "dterm_noise_db";
pub const GYRO_FILTER_DELAY_MS: &str = "gyro_filter_delay_ms";
pub const DTERM_FILTER_DELAY_MS: &str = "dterm_filter_delay_ms";

/// Les grandeurs par axe, dans l'ordre d'écriture.
pub const AXIS_KEYS: [&str; 6] = [
    LATENCY_MS,
    PEAK,
    RISE_MS,
    PHASE_MARGIN_DEG,
    GAIN_MARGIN_DB,
    STEP_WINDOWS,
];
/// Les grandeurs du fragment entier, dans l'ordre d'écriture.
pub const WHOLE_KEYS: [&str; 4] = [
    GYRO_NOISE_DB,
    DTERM_NOISE_DB,
    GYRO_FILTER_DELAY_MS,
    DTERM_FILTER_DELAY_MS,
];

/// Une grandeur mesurée sur un fragment.
#[derive(Clone, Debug, PartialEq)]
pub struct Tracked {
    pub axis: u64,
    pub key: &'static str,
    pub value: f64,
}

/// Le plus grand des nombres finis, NaN s'il n'y en a aucun (une liste vide n'est pas un bruit nul).
fn max_finite(v: &[f64]) -> f64 {
    v.iter().copied().filter(|x| x.is_finite()).fold(f64::NAN, f64::max)
}

/// Les grandeurs suivies d'un rapport : toujours les mêmes lignes (six par axe, quatre pour le fragment entier),
/// NaN pour ce qui ne s'est pas mesuré.
///
/// - Réponse indicielle (par axe) : `StepMetrics` et `LoopMetrics` de `Report::steps`, que lisent `step_findings`.
/// - Bruit (fragment entier) : le plus haut de roll et pitch, comme `noise_rules` le compare au seuil du profil.
/// - Retard des filtres (fragment entier) : `Delays`, **seulement si le vol a assez de mouvements** (`Delays::moving`) ;
///   sinon NaN, comme l'assistant, qui ne juge pas un retard « peu représentatif ».
pub fn from_report(report: &Report) -> Vec<Tracked> {
    let mut out = Vec::with_capacity(3 * AXIS_KEYS.len() + WHOLE_KEYS.len());
    for axis in 0..3u64 {
        let step = report.steps.get(axis as usize).map(|s| &s.result);
        let values = match step {
            Some(r) => [
                r.metrics.latency_ms,
                r.metrics.peak,
                r.metrics.rise_ms,
                r.loop_.phase_margin_deg,
                r.loop_.gain_margin_db,
                r.n as f64,
            ],
            None => [f64::NAN; 6],
        };
        out.extend(
            AXIS_KEYS
                .iter()
                .zip(values)
                .map(|(&key, value)| Tracked { axis, key, value }),
        );
    }
    let m = &report.measures;
    let noise = |v: &Option<Vec<f64>>| v.as_deref().map_or(f64::NAN, max_finite);
    let (gyro_delay, dterm_delay) = match &m.delays {
        Some(d) if d.moving() => (d.gyro_ms, d.dterm_ms),
        _ => (f64::NAN, f64::NAN),
    };
    let whole = [
        noise(&m.gyro_noise_db),
        noise(&m.dterm_noise_db),
        gyro_delay,
        dterm_delay,
    ];
    out.extend(WHOLE_KEYS.iter().zip(whole).map(|(&key, value)| Tracked {
        axis: ALL_AXES,
        key,
        value,
    }));
    out
}

/// Les réglages de calcul d'un rapport, gardés avec chaque mesure : le profil de taille (détecté, comme l'assistant
/// le fait par défaut) et la méthode de prévision des filtres.
pub fn settings_of(report: &Report) -> String {
    format!("profil={};filtres=rejeu", report.profile.key)
}

/// Mesure un fragment : l'analyse de l'assistant sur ses échantillons `sel`, profil détecté. Rend les réglages de
/// calcul et les grandeurs.
pub fn measure(log: &FlightLog, sel: Range<usize>) -> Result<(String, Vec<Tracked>), String> {
    let report = analyze(log, Some(sel), None)?;
    Ok((settings_of(&report), from_report(&report)))
}
