//! Journaux de vol fabriqués pour les tests (sans corpus : la CI ne l'a pas).
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::Settings;

fn rows(k: usize, n: usize) -> Rows {
    Rows {
        k,
        n,
        data: vec![0.0; k * n],
    }
}

/// Journal de `n` points à `fs` Hz, appelé `fichier` (sans les gaz : `auto_period` rend tout le log).
pub fn synthetic_log(fichier: &str, n: usize, fs: f64) -> FlightLog {
    FlightLog {
        path: PathBuf::from(format!("{fichier}.bbl")),
        session: 1,
        settings: Settings::default(),
        t: (0..n).map(|i| i as f64 / fs).collect(),
        fs,
        gyro: rows(3, n),
        gyro_unfilt: None,
        setpoint: rows(4, n),
        rc_command: rows(4, n),
        p_term: rows(3, n),
        i_term: rows(3, n),
        d_term: rows(3, n),
        f_term: rows(3, n),
        throttle: vec![0.0; n],
        motors: rows(4, n),
        motor_hz: None,
        debug: rows(8, n),
        vbat: None,
        current: None,
        acc: None,
        mode_flags: None,
        events: Matrix::default(),
        stats: Stats::default(),
        implausible_frames: 0,
        pid_columns: BTreeSet::new(),
    }
}
