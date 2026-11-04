//! Données du visualiseur : lissage comparé à `np.convolve(..., "same")`, traces présentes ou non.

mod support;

use otb_gui::viewer::{TRACES, has_trace, measure_text, readout_lines, smooth, stick_positions, trace_data};
use otb_log::flightlog::Rows;
use support::synthetic_log;

fn hex(s: &str) -> f64 {
    // « 0x1.999999999999ap-2 » → f64
    let (m, e) = s.trim_start_matches("0x").split_once('p').unwrap();
    let (ent, frac) = m.split_once('.').unwrap();
    let mant = u64::from_str_radix(&format!("{ent}{frac}"), 16).unwrap() as f64 / 16f64.powi(frac.len() as i32);
    mant * 2f64.powi(e.parse().unwrap())
}

const Y: [f32; 12] = [1.0, -2.0, 3.0, 0.5, 4.0, -1.0, 2.0, 7.0, -3.0, 0.25, 6.0, 1.5];

#[test]
fn le_lissage_suit_numpy_convolve_same() {
    // attendus : numpy 2, `np.convolve(y, np.ones(w)/w, mode="same")` sur Y en float32 (valeurs en hexadécimal)
    let w5 = [
        "0x1.999999999999ap-2",
        "0x1.0000000000000p-1",
        "0x1.4cccccccccccep+0",
        "0x1.ccccccccccccep-1",
        "0x1.b333333333334p+0",
        "0x1.4000000000000p+1",
        "0x1.ccccccccccccep+0",
        "0x1.0cccccccccccdp+0",
        "0x1.399999999999ap+1",
        "0x1.2cccccccccccep+1",
        "0x1.e666666666667p-1",
        "0x1.8cccccccccccdp+0",
    ];
    let w11 = [
        "0x1.0000000000000p-1",
        "0x1.5d1745d1745d2p-1",
        "0x1.51745d1745d18p+0",
        "0x1.0ba2e8ba2e8bbp+0",
        "0x1.11745d1745d18p+0",
        "0x1.9d1745d1745d0p+0",
        "0x1.a8ba2e8ba2e8ap+0",
        "0x1.d745d1745d175p+0",
        "0x1.91745d1745d18p+0",
        "0x1.85d1745d1745dp+0",
        "0x1.28ba2e8ba2e8cp+0",
        "0x1.4000000000001p+0",
    ];
    for (w, att) in [(5, w5), (11, w11)] {
        let got = smooth(&Y, w);
        assert_eq!(got.len(), Y.len());
        for (i, (g, a)) in got.iter().zip(att).enumerate() {
            let a = hex(a);
            assert!(
                (g - a).abs() <= 1e-14 * a.abs().max(1.0),
                "w={w} i={i} : {g} contre {a}"
            );
        }
    }
}

#[test]
fn le_lissage_sans_effet_ou_pair_ou_trop_large() {
    assert_eq!(smooth(&Y, 1), Y.iter().map(|&v| v as f64).collect::<Vec<_>>());
    assert_eq!(smooth(&Y, 0).len(), 12);
    assert!(smooth(&[], 5).is_empty());
    // largeur paire (valeurs de numpy 2 : np.convolve([1,2,3,4], ones(w)/w, "same"))
    assert_eq!(smooth(&[1.0, 2.0, 3.0, 4.0], 2), vec![0.5, 1.5, 2.5, 3.5]);
    assert_eq!(smooth(&[1.0, 2.0, 3.0, 4.0], 4), vec![0.75, 1.5, 2.5, 2.25]);
    // plus large que la courbe : même longueur, moyenne des points présents sur w
    assert_eq!(smooth(&[3.0, 3.0], 25).len(), 2);
}

#[test]
fn les_traces_nulles_sont_masquees_et_les_autres_gardees() {
    let mut log = synthetic_log("a", 50, 1000.0);
    for (k, _) in TRACES {
        assert!(!has_trace(&log, k), "{k} : tout est à zéro");
    }
    log.gyro.row_mut(1)[10] = 5.0;
    assert!(
        has_trace(&log, "gyro") && has_trace(&log, "pid_error"),
        "erreur PID = gyro − consigne"
    );
    assert!(!has_trace(&log, "pterm"));
    log.gyro_unfilt = Some(log.gyro.clone());
    assert!(has_trace(&log, "gyro_prefilt"));
    // debug : au plus 3 canaux ; absent s'il n'y en a aucun
    log.debug.data[0] = 1.0;
    assert_eq!(trace_data(&log, "debug").unwrap().k, 3);
    log.debug = otb_log::flightlog::Rows::zeros(0, 50);
    assert!(trace_data(&log, "debug").is_none());
    log.debug = otb_log::flightlog::Rows::zeros(2, 50);
    assert_eq!(trace_data(&log, "debug").unwrap().k, 2, "debug[:3] avec 2 canaux");
}

#[test]
#[should_panic(expected = "trace inconnue")]
fn une_trace_inconnue_est_une_faute_de_code() {
    trace_data(&synthetic_log("a", 5, 1000.0), "nimporte");
}

#[test]
fn la_mesure_entre_deux_points() {
    // chaînes produites par le Python de `measure_text`
    assert_eq!(measure_text(&[]), "");
    assert_eq!(measure_text(&[1.0]), "");
    assert_eq!(measure_text(&[10.1, 10.125]), "Δt 25,0 ms · 40,0 Hz");
    assert_eq!(measure_text(&[2.0, 2.0]), "Δt 0,0 ms · — Hz");
    assert_eq!(
        measure_text(&[3.0, 1.5]),
        "Δt 1500,0 ms · 0,7 Hz",
        "l'ordre des points n'a pas d'importance"
    );
}

#[test]
fn les_valeurs_au_curseur_ont_le_format_de_python() {
    let mut log = synthetic_log("a", 3, 1000.0);
    log.motors = Rows {
        k: 5,
        n: 3,
        data: [12.5, 0.5, 99.6, 100.0, 3.49].iter().flat_map(|&v| [v, v, v]).collect(),
    };
    log.gyro.row_mut(0)[1] = -123.5;
    log.gyro.row_mut(1)[1] = 7.25;
    log.gyro.row_mut(2)[1] = 0.4;
    log.throttle[1] = 37.5;
    log.vbat = Some(vec![15.995; 3]);
    // attendu : f-strings du Python (arrondi au pair : 12,5 → 12 ; −123,5 → −124 ; 37,5 → 38)
    let attendu = "M1 :  12 %   M2 :   0 %\nM3 : 100 %   M4 : 100 %\nM5 :   3 %\nGyro Roll  :   -124 °/s\nGyro Pitch :      7 °/s\nGyro Yaw   :      0 °/s\nGaz : 38 %\nBatterie : 15.99 V";
    assert_eq!(readout_lines(&log, 1).join("\n"), attendu);
    log.vbat = None;
    assert!(!readout_lines(&log, 1).last().unwrap().starts_with("Batterie"));
}

#[test]
fn les_manches_suivent_rc_command_et_restent_dans_le_carre() {
    let mut log = synthetic_log("a", 2, 1000.0);
    // canaux : roulis, tangage, lacet, gaz (rangés canal après canal), deux instants
    log.rc_command = Rows {
        k: 4,
        n: 2,
        data: vec![250.0, 0.0, 0.0, 0.0, -500.0, 600.0, 1500.0, 2500.0],
    };
    // gauche : (lacet / 500, (gaz − 1500) / 500) ; droite : (roulis / 500, tangage / 500)
    assert_eq!(stick_positions(&log, 0), ([-1.0, 0.0], [0.5, 0.0]));
    assert_eq!(stick_positions(&log, 1), ([1.0, 1.0], [0.0, 0.0]), "borné à ±1");
    // sans voies RC : gaz à 0 = (0 − 1500) / 500 = −3, borné à −1
    log.rc_command = Rows::zeros(0, 2);
    assert_eq!(stick_positions(&log, 0), ([0.0, -1.0], [0.0, 0.0]));
}
