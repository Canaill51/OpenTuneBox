//! Découpe d'une période du vol (`FlightLog::slice_time`).
//!
//! ⚠️ La fonction rendait `index_of(t0)..index_of(t1) + 1` sans remettre les bornes dans l'ordre : pour une période
//! donnée à l'envers, la plage partait à l'envers et **tout découpage de ligne qui s'en servait paniquait**
//! (`stats`, `spectral`, les pages d'outils…). L'invariant était tenu par les appelants (`state::set_period`, le
//! glissement du viewer), jamais par la fonction elle-même (W-054).

use otb_log::flightlog::{self, FlightLog};

fn journal() -> FlightLog {
    let p = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    flightlog::load(std::path::Path::new(&p)).unwrap().remove(0)
}

#[test]
fn slice_time_remet_les_bornes_dans_l_ordre() {
    let log = journal();
    let (t0, t1) = (log.t[1], log.t[log.t.len() - 2]);

    let endroit = log.slice_time(t0, t1);
    let envers = log.slice_time(t1, t0);
    assert_eq!(endroit, envers, "une période donnée à l'envers couvre les mêmes lignes");
    assert!(endroit.start < endroit.end, "plage vide ou inversée : {endroit:?}");

    // Le découpage d'une vraie ligne avec la plage inversée ne doit pas paniquer.
    let ligne = log.gyro.row(0);
    assert_eq!(ligne[envers.clone()].len(), envers.end - envers.start);
}

#[test]
fn slice_time_garde_les_bornes_comprises_et_reste_dans_le_journal() {
    let log = journal();
    let n = log.t.len();

    // Période entière : de la première à la dernière ligne, bornes comprises.
    let tout = log.slice_time(log.t[0], log.t[n - 1]);
    assert_eq!(tout, 0..n, "la période entière doit couvrir tout le journal");

    // Instant unique : une seule ligne, jamais une plage vide.
    let un = log.slice_time(log.t[5], log.t[5]);
    assert_eq!(un.end - un.start, 1, "un instant unique couvre une ligne");

    // Bornes hors du journal : ramenées dedans, sans déborder.
    let large = log.slice_time(-1e9, 1e9);
    assert!(large.end <= n, "la plage déborde du journal : {large:?}");
    assert_eq!(large, 0..n);
}
