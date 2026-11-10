//! Ouvrir plusieurs logs en parallèle rend **exactement** les mêmes `FlightLog` que les ouvrir les uns après les autres
//! (D-051, lot 11).
//!
//! La preuve est l'empreinte complète du `FlightLog` (`flightlog_digest`, la même qui compare le Rust aux références
//! figées) : tout écart d'un bit sur une série la change. Le test tourne **sans corpus** (CI, L-023) en ouvrant
//! plusieurs fois le log de `tests/data` ; la même égalité a été vérifiée à la main sur six vrais logs
//! (`docs/validation.md`, « Ouverture de plusieurs logs »).

use std::path::PathBuf;

use otb_log::parallel::{MAX_PARALLEL_FILES, load_many_jobs};
use otb_parity::flightlog_digest::flightlog_digest;
use serde_json::Value;

fn log_essai() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

/// Empreintes de tous les `FlightLog` rendus, fichier par fichier.
fn empreintes(paths: &[PathBuf], jobs: usize) -> Vec<Vec<Value>> {
    load_many_jobs(paths, jobs)
        .into_iter()
        .map(|(p, r)| {
            r.unwrap_or_else(|e| panic!("{} : {e}", p.display()))
                .iter()
                .map(flightlog_digest)
                .collect()
        })
        .collect()
}

#[test]
fn le_parallele_rend_les_memes_flightlog_que_le_sequentiel() {
    let paths = vec![log_essai(); 6];
    let sequentiel = empreintes(&paths, 1);
    let parallele = empreintes(&paths, MAX_PARALLEL_FILES);
    assert_eq!(sequentiel.len(), 6);
    assert!(!sequentiel[0].is_empty(), "le log d'essai a au moins une session");
    assert_eq!(parallele, sequentiel, "une série diffère entre parallèle et séquentiel");
}
