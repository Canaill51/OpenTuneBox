//! Briques de filtrage **à l'arithmétique des firmwares** (lot 24 du plan, D-062) : float32, mêmes approximations, même ordre
//! des opérations, mêmes initialisations que le firmware compilé. Rangées par famille de firmware ; seule celle de Betaflight est
//! écrite (D-062 : Betaflight seul tant qu'il n'y a pas de vrais logs d'un autre firmware).
//!
//! Trois niveaux de fidélité, jamais confondus (D-062) : *même algorithme* (l'engagement), *bit à bit contre le firmware compilé
//! sur PC* (le critère de test : `tests/firmware_vectors.rs` rejoue les vecteurs du banc `otb-dev firmware-bench`), *identité au
//! vol réel* (mesurée, jamais promise).
//!
//! Rien n'est encore branché : la chaîne en service reste celle de `otb-analysis::bf_filters` (float64) jusqu'à la bascule (lot 24d).

pub mod betaflight;
