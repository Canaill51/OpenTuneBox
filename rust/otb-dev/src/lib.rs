//! Outils de développement d'OpenTuneBox, jamais livrés (phase 7, D-035).
//!
//! `icons` : icônes d'application (PNG Linux, `.ico` Windows) refaites depuis le SVG (remplace `packaging/make_icons.py`).
//!
//! `release` : préparation d'une version et notes de la page Releases (remplace `scripts/release.py`).
//!
//! `docs` : cohérence de la documentation avec le code. Remplace `tests/test_docs.py` (Python) : les mêmes contrôles, un par fonction.
//! Chaque contrôle reçoit les textes ou les listes à comparer et rend la liste des problèmes (vide = tout va bien) ; les listes réelles
//! (outils, profils, commandes…) sont rassemblées par `tests/docs.rs`, les cas fabriqués qui doivent échouer sont dans `tests/checks.rs`.

pub mod corpus;
pub mod docs;
pub mod icons;
pub mod release;
