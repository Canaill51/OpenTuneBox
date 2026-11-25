//! Outils de développement d'OpenTuneBox, jamais livrés (phase 7, D-035).
//!
//! `icons` : icônes d'application (PNG Linux, `.ico` Windows) refaites depuis le SVG (remplace `packaging/make_icons.py`).
//!
//! `release` : préparation d'une version et notes de la page Releases (remplace `scripts/release.py`).
//!
//! `usb` : essai à la main de la récupération par USB sur une vraie carte (lot 32a) ; mesures de `docs/spec-usb.md`.
//!
//! `docs` : cohérence de la documentation avec le code. Remplace `tests/test_docs.py` (Python, supprimé en 7-5) : les mêmes contrôles, un par fonction.
//! Chaque contrôle reçoit les textes ou les listes à comparer et rend la liste des problèmes (vide = tout va bien) ; les listes réelles
//! (outils, profils, commandes…) sont rassemblées par `tests/docs.rs`, les cas fabriqués qui doivent échouer sont dans `tests/checks.rs`.

pub mod bench_open;
pub mod corpus;
pub mod docs;
pub mod firmware_bench;
pub mod icons;
pub mod licenses;
pub mod numero;
pub mod release;
pub mod usb;
pub mod verify;
