//! Comparateur de parité (D-035, étape E2).
//!
//! Lit les références écrites par `scripts/golden_modules.py` (format décrit dans l'en-tête de ce script), rejoue chaque
//! appel avec l'implémentation Rust enregistrée dans [`registry`] et compare le résultat à celui de Python.

#![recursion_limit = "512"] // le grand `json!` des réglages

pub mod advisor_layer;
pub mod analysis_layer;
pub mod cli_layer;
pub mod compare;
pub mod decoded;
pub mod flightlog_digest;
pub mod guide_layer;
pub mod layers;
pub mod registry;
pub mod settings_json;
pub mod tuning_layer;
pub mod value;
