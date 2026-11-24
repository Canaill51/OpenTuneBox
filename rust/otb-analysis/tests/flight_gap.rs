//! Écart du rejeu au vol (`otb_analysis::flight_gap`, fin du lot 24) : spectres fabriqués dont l'écart est connu d'avance,
//! causes tirées des réglages, et le vrai log de 5 s.

use std::path::PathBuf;

use otb_analysis::filter_replay::{BeforeAfter, BeforeAfterSpectra, before_after};
use otb_analysis::flight_gap::{BANDS, GapCause, band_gaps, causes, flight_gap};
use otb_log::settings::Settings;

/// Spectres d'un axe : log à 0 dB, rejeu à `offset(f)` dB.
fn spectra(freqs: &[f64], offset: impl Fn(f64) -> f64) -> BeforeAfterSpectra {
    BeforeAfterSpectra {
        logged_db: vec![0.0; freqs.len()],
        current_db: freqs.iter().map(|&f| offset(f)).collect(),
        modified_db: vec![0.0; freqs.len()],
    }
}

fn freqs() -> Vec<f64> {
    (0..=200).map(|i| i as f64 * 5.0).collect()
}

/// Roulis +1 dB, tangage +3 dB, lacet +100 dB (laissé de côté) ; terme D sur le roulis seul, −0,5 dB.
fn fabricated() -> BeforeAfter {
    let f = freqs();
    BeforeAfter {
        gyro: vec![spectra(&f, |_| 1.0), spectra(&f, |_| 3.0), spectra(&f, |_| 100.0)],
        dterm: vec![Some(spectra(&f, |_| -0.5)), None, Some(spectra(&f, |_| 100.0))],
        freqs: f,
    }
}

#[test]
fn ecart_moyen_du_roulis_et_du_tangage_sans_le_lacet() {
    let g = band_gaps(&fabricated(), 4000.0);
    assert_eq!(g.len(), BANDS.len());
    for (b, (lo, hi)) in g.iter().zip(BANDS) {
        assert_eq!((b.lo_hz, b.nominal_hi_hz, b.hi_hz), (lo, hi, hi));
        assert!((b.gyro_db - 2.0).abs() < 1e-12, "{b:?}");
        // un seul axe a un terme D parmi roulis et tangage : sa valeur, sans NaN
        assert!((b.d_db + 0.5).abs() < 1e-12, "{b:?}");
    }
}

#[test]
fn la_derniere_bande_s_arrete_sous_la_demi_cadence_du_log() {
    // log à 400 Hz : bord à 0,45 × 400 = 180 Hz ; 100–250 coupée à 180, 250–500 omise
    let g = band_gaps(&fabricated(), 400.0);
    assert_eq!(g.len(), 2);
    assert_eq!((g[1].lo_hz, g[1].nominal_hi_hz, g[1].hi_hz), (100.0, 250.0, 180.0));
}

#[test]
fn la_pire_bande_garde_l_ecart_le_plus_grand_quel_que_soit_son_signe() {
    let f = freqs();
    let off = |x: f64| {
        if x < 100.0 {
            0.5
        } else if x < 250.0 {
            -2.0
        } else {
            1.0
        }
    };
    let ba = BeforeAfter {
        gyro: vec![spectra(&f, off), spectra(&f, off), spectra(&f, off)],
        dterm: vec![None, None, None],
        freqs: f,
    };
    let g = flight_gap(&ba, 4000.0, &Settings::default());
    let (v, b) = g.worst_gyro().unwrap();
    assert!((v - 2.0).abs() < 1e-12 && b.lo_hz == 100.0, "{v} {b:?}");
    // sans terme D : aucune pire bande, et chaque bande le dit par NaN
    assert!(g.worst_d().is_none());
    assert!(g.bands.iter().all(|b| b.d_db.is_nan()));
}

fn timing(version: Vec<i64>, p_interval: (i64, i64)) -> Settings {
    Settings {
        version,
        looptime_us: 125,
        pid_denom: 2,
        p_interval,
        ..Settings::default()
    }
}

#[test]
fn causes_log_ralenti_et_firmware_d_avant_la_4_3() {
    // 8 kHz / 2 = PID à 4 kHz ; chaque boucle enregistrée, firmware récent : aucune cause
    assert!(causes(&timing(vec![2026, 6, 0], (1, 1))).is_empty());
    // une boucle sur 2 : log à 2 kHz
    assert_eq!(
        causes(&timing(vec![4, 5, 1], (1, 2))),
        vec![GapCause::Slowed {
            loops_per_sample: 2,
            log_hz: 2000.0
        }]
    );
    assert_eq!(
        causes(&timing(vec![4, 2, 11], (1, 4))),
        vec![
            GapCause::Slowed {
                loops_per_sample: 4,
                log_hz: 1000.0
            },
            GapCause::OldFirmware
        ]
    );
}

#[test]
fn vrai_log_ecart_mesure_et_cause_du_log_ralenti() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    let log = otb_log::flightlog::load(&p).unwrap().remove(0);
    let s = &log.settings;
    let ba = before_after(&log, 0..log.n(), s, s, 300.0)
        .unwrap()
        .expect("gyro non filtré");
    let g = flight_gap(&ba, log.fs, s);
    let (gyro, _) = g.worst_gyro().unwrap();
    let (d, _) = g.worst_d().unwrap();
    assert!(gyro.is_finite() && d.is_finite(), "{g:?}");
    // le JeNo 5 enregistre une boucle PID sur 2 (PID à 3,2 kHz, log à 1,6 kHz) : la cause est dite, seule (2026.6)
    let n = (s.pid_rate_hz() / s.log_rate_hz()).round() as usize;
    assert!(n > 1, "le log de test n'est plus ralenti : revoir ce test");
    assert_eq!(
        g.causes,
        vec![GapCause::Slowed {
            loops_per_sample: n,
            log_hz: s.log_rate_hz()
        }]
    );
    eprintln!("jeno5 : {g:?}");
}
