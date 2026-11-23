//! Récupération des logs par USB, partie protocole (lot 32a, D-063) : parler MSP à une carte Betaflight, copier sa
//! mémoire de logs, vérifier la copie, et seulement alors l'effacer. Sauvegarde de la configuration (`diff all`).
//!
//! - [`frame`] : trames MSP v1 (avec « jumbo ») et v2 ;
//! - [`link`] : port série réel (`serial2`), erreurs d'ouverture expliquées ;
//! - [`client`] : identité, état armé, résumé et lecture de la mémoire — **aucune écriture de réglage** ;
//! - [`erase`] : contrôles avant effacement et seul chemin qui efface ;
//! - [`cli`] : `diff all` ;
//! - [`msc`] : redémarrage en mode stockage de masse, disque de la carte, copie de la mémoire par ce disque (lot 32c) ;
//! - [`sd`] : logs d'une carte SD, copiés, vérifiés, et seul chemin qui les supprime (lot 32c) ;
//! - [`sim`] : carte simulée pour les tests (aucun matériel en CI).
//!
//! Ce que fait le firmware, vérifié dans ses sources : `docs/spec-usb.md`. L'écran : `otb-gui` (`usb.rs`, `usb_page.rs`).

pub mod cli;
pub mod client;
pub mod erase;
pub mod frame;
pub mod link;
pub mod msc;
pub mod sd;
pub mod sim;

pub use client::{ChipId, FlashSummary, Identity, Msp, MspError};
pub use erase::{CopyReport, ErasePermit, VerifiedCopy, erase_flash, verify_and_save};
pub use link::{Link, SerialLink};
pub use msc::{BeforeDisk, Disk, DiskKind, Storage, reboot_to_disk};
pub use sd::{SdCopy, SdErasePermit, SdFile, confirm_sd, copy_sd_log, delete_sd_logs};
