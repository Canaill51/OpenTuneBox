//! Assistant de réglage d'OpenTuneBox en Rust (phase 5a de la migration, D-035) : porte `advisor/` de Python.
//!
//! Chaque module reproduit le sien : textes caractère par caractère, décisions identiques. Les valeurs numériques viennent des
//! modules déjà prouvés de `otb-analysis` et `otb-log`.

pub mod drone_card;
pub mod log_check;
pub mod noise;
pub mod profiles;
mod pyclasses;
pub mod rules;
pub mod step_tuning;
pub mod tuning_path;
