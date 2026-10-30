//! Comparateur de parité (D-035, étape E2).
//!
//! Lit les références écrites par `scripts/golden_modules.py` (format décrit dans l'en-tête de ce script), rejoue chaque
//! appel avec l'implémentation Rust enregistrée dans [`registry`] et compare le résultat à celui de Python.

pub mod compare;
pub mod layers;
pub mod registry;
pub mod value;
