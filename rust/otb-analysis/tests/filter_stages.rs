//! Contrôle du modèle de filtres étage par étage (`otb_analysis::filter_stages`, lot 22a) sur des logs fabriqués : les
//! canaux debug y sortent de filtres connus, donc l'écart doit être nul ; un filtre faussé doit faire rougir son seul
//! étage (contre-preuve, L-071).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::PathBuf;

use otb_analysis::bf_filters::{GyroStage, gyro_stage, simulate_dterm};
use otb_analysis::filter_stages::{Stage, StageCheck, check_stages, clock_offset_s, compare, stage_signals};
use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::{AxisPid, Lowpass, Settings};

const FS: f64 = 4000.0;
const SECONDS: usize = 12;

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

fn lpf(kind: &'static str, hz: i64) -> Lowpass {
    Lowpass {
        kind,
        static_hz: hz,
        ..Lowpass::default()
    }
}

/// Réglages d'un whoop en 2026.12 : gyro 8 kHz, PID 4 kHz (denom 2), log à chaque boucle PID.
fn settings(debug: &str) -> Settings {
    let mut s = Settings {
        version: vec![2026, 12, 0],
        looptime_us: 125,
        pid_denom: 2,
        p_interval: (1, 1),
        debug_mode: debug.into(),
        gyro_lpf2: lpf("PT1", 500),
        gyro_lpf1: lpf("PT2", 200),
        gyro_notches: vec![(300, 200)],
        dterm_lpf1: lpf("PT1", 120),
        dterm_lpf2: lpf("BIQUAD", 180),
        rpm_harmonics: 2,
        rpm_weights: vec![100, 80],
        dyn_notch_count: 1,
        dyn_notch_q: 300,
        dyn_notch_min_hz: 100,
        dyn_notch_max_hz: 600,
        ..Settings::default()
    };
    s.pids = [AxisPid {
        p: 40,
        i: 80,
        d: 30,
        dmax: 30,
        f: 100,
    }; 3];
    s
}

/// Somme de sinus de 5 à 900 Hz, phases fixes, échantillonnée à 8 kHz (cadence gyro) : `(pair, impair)`. Le firmware
/// filtre l'échantillon `2i` et le Blackbox écrit `gyroUnfilt` = l'échantillon `2i + 1`, un tick gyro plus tard (L-100).
fn gyro_8k(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut state = 12345u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let tones: Vec<(f64, f64)> = (0..180)
        .map(|i| (5.0 + 5.0 * i as f64 + next(), 2.0 * PI * next()))
        .collect();
    let at = |k: usize| {
        let t = k as f64 / 8000.0;
        tones.iter().map(|(f, p)| (2.0 * PI * f * t + p).sin()).sum::<f64>() * 4.0
    };
    (
        (0..n).map(|i| at(2 * i)).collect(),
        (0..n).map(|i| at(2 * i + 1)).collect(),
    )
}

/// Log fabriqué : `filtered_with` donne les réglages des filtres « du firmware » (ceux du log par défaut).
fn fabricated(debug: &str, filtered_with: Option<&Settings>) -> FlightLog {
    let n = (FS as usize) * SECONDS;
    let s = settings(debug);
    let mut motor_hz = rows(4, n);
    for m in 0..4 {
        set_row(&mut motor_hz, m, &vec![220.0 + 10.0 * m as f64; n]);
    }
    let mut log = FlightLog {
        path: PathBuf::from("fabrique.bbl"),
        session: 1,
        settings: s.clone(),
        t: (0..n).map(|i| i as f64 / FS).collect(),
        fs: FS,
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
        motor_hz: Some(motor_hz),
        debug: rows(8, n),
        vbat: None,
        current: None,
        acc: None,
        mode_flags: None,
        events: Matrix::default(),
        stats: Stats::default(),
        implausible_frames: 0,
        pid_columns: ["axisD[0]", "axisD[1]", "axisD[2]"]
            .iter()
            .map(|c| c.to_string())
            .collect::<BTreeSet<_>>(),
    };
    let fw = filtered_with.unwrap_or(&s).clone();
    let (filtered, unfilt) = gyro_8k(n);
    let stage = |log: &FlightLog, st, x: &[f64]| gyro_stage(st, log, &fw, x.to_vec(), 1).unwrap();
    for ax in 0..3 {
        set_row(log.gyro_unfilt.as_mut().unwrap(), ax, &unfilt);
    }
    let a = stage(&log, GyroStage::Lpf2, &filtered);
    let b = stage(&log, GyroStage::Rpm, &a);
    let c = stage(&log, GyroStage::StaticLpf1, &b);
    if debug == "GYRO_SAMPLE" {
        for (i, v) in [&filtered, &a, &b, &c].into_iter().enumerate() {
            set_row(&mut log.debug, i, v);
        }
    } else {
        // FFT_FREQ (2025.12+) : gyro avant le notch dynamique dans debug[0], centre du notch dans debug[1]
        set_row(&mut log.debug, 0, &c);
        set_row(&mut log.debug, 1, &vec![350.0; n]);
    }
    let g = stage(&log, GyroStage::DynNotch, &c);
    for ax in 0..3 {
        set_row(&mut log.gyro, ax, &g);
    }
    // terme D : chaîne D du firmware sur le gyro enregistré, gain Kd × TPA inconnu de l'outil
    let d = simulate_dterm(&log, Some(&fw), None).unwrap();
    for ax in 0..3 {
        let v: Vec<f64> = d.row(ax).iter().map(|x| x * 0.0173).collect();
        set_row(&mut log.d_term, ax, &v);
    }
    log
}

fn find(checks: &[StageCheck], st: Stage) -> &StageCheck {
    checks
        .iter()
        .find(|c| c.stage == st)
        .unwrap_or_else(|| panic!("étage {st:?} absent"))
}

/// Plus grand |écart| (dB) et |retard| (ms) des bandes sous `max_hz`.
fn worst(c: &StageCheck, max_hz: f64) -> (f64, f64) {
    c.bands
        .iter()
        .filter(|b| b.hi_hz <= max_hz)
        .fold((0.0, 0.0), |(e, d), b| {
            (e.max(b.error_db.abs()), d.max(b.delay_ms.abs()))
        })
}

#[test]
fn gyro_sample_identique_au_modele_donne_un_ecart_nul() {
    let log = fabricated("GYRO_SAMPLE", None);
    let r = check_stages(&log, 0..log.n()).unwrap();
    for st in [Stage::Lpf2, Stage::Rpm, Stage::StaticLpf1] {
        let c = find(&r.checks, st);
        let (e, d) = worst(c, 500.0);
        assert!(e < 1e-3 && d < 1e-3, "{st:?} : écart {e} dB, retard {d} ms");
        assert!(c.residual < 1e-4, "{st:?} : résidu {}", c.residual);
    }
    // filtres D : gain retrouvé (0,0173 × Kd inconnu), puis aucun écart
    let dt = find(&r.checks, Stage::Dterm);
    assert!((dt.gain_factor - 0.0173).abs() < 1e-5, "gain D {}", dt.gain_factor);
    let (e, d) = worst(dt, 500.0);
    assert!(e < 1e-3 && d < 1e-3, "D : écart {e} dB, retard {d} ms");
}

#[test]
fn un_filtre_fausse_ne_fait_rougir_que_son_etage() {
    // contre-preuve : le « firmware » a un LPF1 à 150 Hz, le log annonce 200 Hz
    let mut fw = settings("GYRO_SAMPLE");
    fw.gyro_lpf1 = lpf("PT2", 150);
    let log = fabricated("GYRO_SAMPLE", Some(&fw));
    let r = check_stages(&log, 0..log.n()).unwrap();
    let (e, _) = worst(find(&r.checks, Stage::StaticLpf1), 250.0);
    assert!(e > 1.0, "LPF1 faussé non vu : {e} dB");
    for st in [Stage::Lpf2, Stage::Rpm] {
        let (e, _) = worst(find(&r.checks, st), 500.0);
        assert!(e < 1e-3, "{st:?} accusé à tort : {e} dB");
    }
}

#[test]
fn decalage_d_horloge_retire_seulement_quand_demande() {
    let log = fabricated("GYRO_SAMPLE", None);
    assert!((clock_offset_s(&log.settings) - 125e-6).abs() < 1e-12);
    let (sigs, _) = stage_signals(&log, 0..log.n()).unwrap();
    let clock = sigs.iter().find(|s| s.stage == Stage::Clock).unwrap();
    let corrected = compare(clock, FS, &[], true).unwrap();
    let raw = compare(clock, FS, &[], false).unwrap();
    let (_, d) = worst(&corrected, 500.0);
    assert!(d < 0.005, "retard résiduel {d} ms avec la correction");
    for b in raw.bands.iter().filter(|b| b.hi_hz <= 500.0) {
        assert!(
            (b.delay_ms - 0.125).abs() < 0.005,
            "sans correction : {} ms à {} Hz",
            b.delay_ms,
            b.lo_hz
        );
    }
}

#[test]
fn fft_freq_bloc_et_notch_dynamique() {
    let log = fabricated("FFT_FREQ", None);
    let r = check_stages(&log, 0..log.n()).unwrap();
    // bloc LPF2 + RPM + statiques + LPF1 depuis gyroUnfilt : même chaîne, décalée d'un tick gyro (retiré)
    let (e, d) = worst(find(&r.checks, Stage::GyroBlock), 250.0);
    assert!(e < 0.05 && d < 0.005, "bloc : écart {e} dB, retard {d} ms");
    let (e, d) = worst(find(&r.checks, Stage::DynNotch), 500.0);
    assert!(e < 1e-3 && d < 1e-3, "notch dynamique : écart {e} dB, retard {d} ms");
    // contre-preuve : sans la correction d'horloge, le bloc paraît en retard d'un tick gyro
    let (sigs, _) = stage_signals(&log, 0..log.n()).unwrap();
    let block = sigs.iter().find(|s| s.stage == Stage::GyroBlock).unwrap();
    let (_, d) = worst(&compare(block, FS, &[], false).unwrap(), 250.0);
    assert!(d > 0.1, "décalage d'horloge invisible sans correction : {d} ms");
}

#[test]
fn logs_trop_lents_ou_d_variable_sautes_avec_la_raison() {
    let mut log = fabricated("GYRO_SAMPLE", None);
    log.settings.p_interval = (1, 4); // 1 kHz < 1,6 kHz (D-057)
    let r = check_stages(&log, 0..log.n()).unwrap();
    assert!(r.checks.is_empty());
    assert!(
        r.skipped.iter().all(|(_, why)| why.contains("D-057")),
        "{:?}",
        r.skipped
    );

    let mut log = fabricated("GYRO_SAMPLE", None);
    for p in &mut log.settings.pids {
        p.dmax = 45;
    }
    let r = check_stages(&log, 0..log.n()).unwrap();
    assert!(r.checks.iter().all(|c| c.stage != Stage::Dterm));
    assert!(
        r.skipped
            .iter()
            .any(|(st, why)| *st == Stage::Dterm && why.contains("D max"))
    );
}
