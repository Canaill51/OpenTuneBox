//! Récupération des logs par USB, partie protocole (lot 32a, D-063) : parler MSP à une carte Betaflight, copier sa
//! mémoire de logs, vérifier la copie, et seulement alors l'effacer. Sauvegarde de la configuration (`diff all`).
//!
//! - [`frame`] : trames MSP v1 (avec « jumbo ») et v2 ;
//! - [`link`] : port série réel (`serial2`), erreurs d'ouverture expliquées ;
//! - [`client`] : identité, état armé, résumé et lecture de la mémoire — **aucune écriture de réglage** ;
//! - [`erase`] : contrôles avant effacement et seul chemin qui efface ;
//! - [`cli`] : `diff all` ;
//! - [`sim`] : carte simulée pour les tests (aucun matériel en CI).
//!
//! Ce que fait le firmware, vérifié dans ses sources : `docs/spec-usb.md`. L'écran viendra au lot 32b.

pub mod cli;
pub mod client;
pub mod erase;
pub mod frame;
pub mod link;
pub mod sim;

pub use client::{FlashSummary, Identity, Msp, MspError, Uid};
pub use erase::{CopyReport, ErasePermit, VerifiedCopy, erase_flash, verify_and_save};
pub use link::{Link, SerialLink};
