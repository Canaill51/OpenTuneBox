//! Retard des filtres gyro sans le décalage d'horloge de `gyroUnfilt` (L-100, lot 22d du plan) : sur un log fabriqué où le
//! gyro « filtré » n'est que l'échantillon gyro d'avant celui qu'écrit `gyroUnfilt`, sans aucun filtre, le retard des filtres
//! doit être nul. Contre-preuve : sans la correction, `compute_delays` y lit 0,125 ms (un tick à 8 kHz).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::PathBuf;

use otb_analysis::delay::compute_delays;
use otb_analysis::filter_stages::clock_offset_s;
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

fn set_row(r: &mut Rows, i: usize, v: &[f64]) {
    for (o, x) in r.data[i * r.n..(i + 1) * r.n].iter_mut().zip(v) {
        *o = *x as f32;
    }
}

/// Somme de sinus de 5 à 300 Hz (phases fixes), échantillonnée à la cadence gyro `gyro_hz`.
fn tones(count: usize, gyro_hz: f64) -> Vec<f64> {
    let at = |k: usize| {
        let t = k as f64 / gyro_hz;
        (0..60)
            .map(|i| {
                let f = 5.0 + 5.0 * i as f64;
                (2.0 * PI * f * t + 0.7 * i as f64).sin()
            })
            .sum::<f64>()
            * 200.0
    };
    (0..count).map(at).collect()
}

/// Gyro à 8 kHz, boucle PID et log une boucle gyro sur `denom` : le firmware filtre l'échantillon `denom·i` et le
/// Blackbox écrit `gyroUnfilt` = l'échantillon ⌊denom/2⌋ ticks plus tard (L-100). Aucun filtre entre les deux.
fn log_without_filters(denom: i64) -> FlightLog {
    let d = denom as usize;
    let fs = 8000.0 / denom as f64;
    let n = (fs as usize) * 8;
    let g8 = tones(d * n + d, 8000.0);
    let filtered: Vec<f64> = (0..n).map(|i| g8[d * i]).collect();
    let unfilt: Vec<f64> = (0..n).map(|i| g8[d * i + d / 2]).collect();
    let settings = Settings {
        version: vec![2026, 12, 0],
        looptime_us: 125,
        pid_denom: denom,
        p_interval: (1, 1),
        ..Settings::default()
    };
    let mut log = FlightLog {
        path: PathBuf::from("fabrique.bbl"),
        session: 1,
        settings,
        t: (0..n).map(|i| i as f64 / fs).collect(),
        fs,
        gyro: rows(3, n),
        gyro_unfilt: Some(rows(3, n)),
        setpoint: rows(4, n),
        rc_command: rows(4, n),
        p_term: rows(3, n),
        i_term: rows(3, n),
        d_term: rows(3, n),
        f_term: rows(3, n),
        throttle: vec![40.0; n],
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
    };
    for ax in 0..3 {
        set_row(&mut log.gyro, ax, &filtered);
        set_row(log.gyro_unfilt.as_mut().unwrap(), ax, &unfilt);
    }
    log
}

#[test]
fn retard_gyro_sans_le_decalage_d_horloge() {
    for (denom, offset_ms) in [(1, 0.0), (2, 0.125), (4, 0.25)] {
        let log = log_without_filters(denom);
        assert!((clock_offset_s(&log.settings) * 1000.0 - offset_ms).abs() < 1e-12);
        let d = compute_delays(&log, 0..log.n()).unwrap();
        // sans la correction, on lirait `offset_ms` : le seuil est bien en dessous
        assert!(
            d.gyro_ms.abs() < 0.01,
            "denom {denom} : retard gyro {} ms au lieu de 0 (décalage d'horloge {offset_ms} ms)",
            d.gyro_ms
        );
    }
}
