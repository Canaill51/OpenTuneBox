//! Contrôle du modèle de filtres étage par étage (`otb_analysis::filter_stages`, lot 22a) sur des logs fabriqués : les
//! canaux debug y sortent de filtres connus, donc l'écart doit être nul ; un filtre faussé doit faire rougir son seul
//! étage (contre-preuve, L-071).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::PathBuf;

use otb_analysis::bf_filters::{GyroStage, gyro_stage, gyro_stage_at, pre_boost_throttle, simulate_dterm_at};
use otb_analysis::filter_stages::{
    InputNoise, Stage, StageCheck, StageSignals, check_stages, clock_offset_s, compare, model_rate_hz, stage_signals,
    tpa_d_factor,
};
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

/// Somme de sinus de 5 à 900 Hz, phases fixes, échantillonnée à 8 kHz (cadence gyro) : les `2n` échantillons gyro. Assez
/// ample pour que l'arrondi au °/s supposé par l'outil (L-103) soit négligeable, même au fond des notchs : ces valeurs ne
/// sont pas arrondies, et à une amplitude 10 fois plus faible, l'outil y retire un bruit absent (0,0025 dB d'écart). Le
/// firmware filtre l'échantillon `2i` et le Blackbox écrit `gyroUnfilt` = l'échantillon `2i + 1`, un tick gyro plus tard
/// (L-100).
fn gyro_8k(n: usize) -> Vec<f64> {
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
        tones.iter().map(|(f, p)| (2.0 * PI * f * t + p).sin()).sum::<f64>() * 4000.0
    };
    (0..2 * n).map(at).collect()
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
    let g8 = gyro_8k(n);
    let filtered: Vec<f64> = (0..n).map(|i| g8[2 * i]).collect();
    let unfilt: Vec<f64> = (0..n).map(|i| g8[2 * i + 1]).collect();
    let stage = |log: &FlightLog, st, x: &[f64]| gyro_stage(st, log, &fw, x.to_vec(), 1).unwrap();
    for ax in 0..3 {
        set_row(log.gyro_unfilt.as_mut().unwrap(), ax, &unfilt);
    }
    // LPF2 comme le firmware : calculé pour 8 kHz, appliqué à chaque échantillon gyro (L-104)
    let a8 = gyro_stage_at(GyroStage::Lpf2, &log, &fw, g8, 1, 2.0 * FS).unwrap();
    let a: Vec<f64> = (0..n).map(|i| a8[2 * i]).collect();
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
    let d = simulate_dterm_at(&log, Some(&fw), None, FS).unwrap();
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
    for st in [Stage::Rpm, Stage::StaticLpf1] {
        let c = find(&r.checks, st);
        let (e, d) = worst(c, 500.0);
        assert!(e < 1e-3 && d < 1e-3, "{st:?} : écart {e} dB, retard {d} ms");
        assert!(c.residual < 1e-4, "{st:?} : résidu {}", c.residual);
    }
    // LPF2 : les échantillons gyro absents du log sont interpolés, d'où un petit écart qui croît avec la fréquence
    let c = find(&r.checks, Stage::Lpf2);
    let (e, d) = worst(c, 250.0);
    assert!(e < 0.05 && d < 0.002, "LPF2 sous 250 Hz : écart {e} dB, retard {d} ms");
    let (e, _) = worst(c, 500.0);
    assert!(e < 0.25, "LPF2 sous 500 Hz : écart {e} dB");
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
    let (e, _) = worst(find(&r.checks, Stage::Rpm), 500.0);
    assert!(e < 1e-3, "RPM accusé à tort : {e} dB");
    let (e, _) = worst(find(&r.checks, Stage::Lpf2), 250.0);
    assert!(e < 0.05, "LPF2 accusé à tort : {e} dB");
}

#[test]
fn lpf2_calcule_a_la_cadence_gyro() {
    // le firmware fabriqué filtre à 8 kHz (L-104) ; le modèle aussi, d'où un écart d'interpolation seulement
    let log = fabricated("GYRO_SAMPLE", None);
    let (sigs, _) = stage_signals(&log, 0..log.n()).unwrap();
    let mut lpf2 = sigs.into_iter().find(|s| s.stage == Stage::Lpf2).unwrap();
    let band = |c: &StageCheck| c.bands.iter().find(|b| b.lo_hz == 250.0).unwrap().error_db;
    let ok = band(&compare(&lpf2, FS, &[], true).unwrap());
    assert!(ok.abs() < 0.25, "LPF2 à la cadence gyro : {ok} dB entre 250 et 500 Hz");
    // contre-preuve : calculé à la cadence du log, le LPF2 filtre trop (PT1 à 500 Hz : ≈ +0,45 dB)
    lpf2.model = gyro_stage(GyroStage::Lpf2, &log, &log.settings, lpf2.input.clone(), 1).unwrap();
    let wrong = band(&compare(&lpf2, FS, &[], true).unwrap());
    assert!(wrong > 0.35, "cadence du log non distinguée : {wrong} dB");
}

/// Une ligne sur deux (la seconde de chaque paire) : `Rows` d'un log qui n'enregistre qu'une boucle PID sur deux.
fn every_other(r: &Rows) -> Rows {
    let n = r.n / 2;
    let mut out = rows(r.k, n);
    for i in 0..r.k {
        let v: Vec<f64> = (0..n).map(|j| r.row(i)[2 * j + 1] as f64).collect();
        set_row(&mut out, i, &v);
    }
    out
}

/// Le log fabriqué à 4 kHz (filtres « firmware » à la cadence PID), dont on ne garde qu'une boucle sur deux : log à 2 kHz.
fn reduced_rate(debug: &str) -> FlightLog {
    let full = fabricated(debug, None);
    let mut log = full.clone();
    log.settings.p_interval = (1, 2);
    log.fs = FS / 2.0;
    let n = full.n() / 2;
    log.t = (0..n).map(|i| i as f64 / log.fs).collect();
    log.throttle = (0..n).map(|j| full.throttle[2 * j + 1]).collect();
    for (dst, src) in [
        (&mut log.gyro, &full.gyro),
        (&mut log.setpoint, &full.setpoint),
        (&mut log.rc_command, &full.rc_command),
        (&mut log.p_term, &full.p_term),
        (&mut log.i_term, &full.i_term),
        (&mut log.d_term, &full.d_term),
        (&mut log.f_term, &full.f_term),
        (&mut log.motors, &full.motors),
        (&mut log.debug, &full.debug),
    ] {
        *dst = every_other(src);
    }
    log.gyro_unfilt = full.gyro_unfilt.as_ref().map(every_other);
    log.motor_hz = full.motor_hz.as_ref().map(every_other);
    log
}

#[test]
fn filtres_de_la_boucle_pid_rejoues_a_sa_cadence_quand_le_log_en_saute() {
    let log = reduced_rate("FFT_FREQ");
    assert_eq!(model_rate_hz(&log.settings), FS / 2.0);
    let r = check_stages(&log, 0..log.n()).unwrap();
    let (e, d) = worst(find(&r.checks, Stage::GyroBlock), 250.0);
    assert!(e < 0.01 && d < 0.005, "bloc : écart {e} dB, retard {d} ms");
    let (e, d) = worst(find(&r.checks, Stage::DynNotch), 250.0);
    assert!(e < 0.01 && d < 0.005, "notch dynamique : écart {e} dB, retard {d} ms");
    // contre-preuve : le bloc calculé pour la cadence du log (2 kHz) filtre trop (L-104)
    let (sigs, _) = stage_signals(&log, 0..log.n()).unwrap();
    let mut block = sigs.into_iter().find(|s| s.stage == Stage::GyroBlock).unwrap();
    let mut y = block.input.clone();
    for st in [GyroStage::Lpf2, GyroStage::Rpm, GyroStage::StaticLpf1] {
        y = gyro_stage(st, &log, &log.settings, y, 1).unwrap();
    }
    block.model = y;
    let (e, _) = worst(&compare(&block, FS / 2.0, &[], true).unwrap(), 250.0);
    assert!(e > 0.3, "cadence du log non distinguée : {e} dB");
}

/// Bruit blanc déterministe (variance 1).
fn white(n: usize, seed: u64) -> Vec<f64> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 12f64.sqrt()
        })
        .collect()
}

fn pt1(x: &[f64], hz: f64) -> Vec<f64> {
    let w = 2.0 * PI * hz / FS;
    let k = w / (w + 1.0);
    let mut y = 0.0;
    x.iter()
        .map(|&v| {
            y += k * (v - y);
            y
        })
        .collect()
}

/// Étage fabriqué : le « firmware » filtre l'entrée exacte, le log n'en garde que l'arrondi ; le modèle filtre l'arrondi.
fn rounded_stage(exact: Vec<f64>, logged: Vec<f64>, noise: InputNoise) -> StageSignals {
    StageSignals {
        stage: Stage::Lpf2,
        axis: 0,
        model: pt1(&logged, 400.0),
        output: pt1(&exact, 400.0),
        input: logged,
        clock_offset_s: 0.0,
        fit_gain: false,
        breaks: Vec::new(),
        input_noise: noise,
    }
}

fn worst_signed(c: &StageCheck, max_hz: f64) -> f64 {
    c.bands
        .iter()
        .filter(|b| b.hi_hz <= max_hz && b.conclusive)
        .map(|b| b.error_db)
        .fold(0.0, |a, e| if e.abs() > a.abs() { e } else { a })
}

#[test]
fn arrondi_de_l_entree_retire_de_la_mesure() {
    // gyro surtout lent (PT1 à 60 Hz d'un bruit blanc), arrondi au °/s comme les canaux debug : peu de signal en haut
    let n = FS as usize * SECONDS;
    let exact: Vec<f64> = pt1(&white(n, 7), 30.0).iter().map(|v| v * 12.0).collect();
    let logged: Vec<f64> = exact.iter().map(|v| v.round()).collect();
    let fixed = compare(
        &rounded_stage(exact.clone(), logged.clone(), InputNoise::Rounded(1.0)),
        FS,
        &[],
        false,
    )
    .unwrap();
    let e = worst_signed(&fixed, 500.0);
    assert!(e.abs() < 0.1, "arrondi retiré : écart {e} dB");
    // contre-preuve : sans la correction, le firmware paraît filtrer plus (L-103)
    let raw = compare(&rounded_stage(exact, logged, InputNoise::None), FS, &[], false).unwrap();
    let e = worst_signed(&raw, 500.0);
    assert!(e < -0.3, "biais de l'arrondi invisible : {e} dB");
}

#[test]
fn arrondi_du_gyro_retire_de_l_etage_d() {
    // entrée du D = −dg/dt d'un gyro arrondi : le bruit de l'arrondi croît avec la fréquence
    let n = FS as usize * SECONDS;
    let g: Vec<f64> = pt1(&white(n, 11), 30.0).iter().map(|v| v * 12.0).collect();
    let deriv = |g: &[f64]| -> Vec<f64> {
        let mut x: Vec<f64> = (0..g.len())
            .map(|i| if i > 0 { -(g[i] - g[i - 1]) * FS } else { 0.0 })
            .collect();
        x[0] = x[1];
        x
    };
    let rounded: Vec<f64> = g.iter().map(|v| v.round()).collect();
    let stage = |noise| {
        let mut s = rounded_stage(deriv(&g), deriv(&rounded), noise);
        s.fit_gain = true;
        s.output.iter_mut().for_each(|v| *v *= 0.0173);
        s
    };
    let fixed = compare(&stage(InputNoise::RoundedDerivative(1.0)), FS, &[], false).unwrap();
    assert!(
        (fixed.gain_factor - 0.0173).abs() < 1e-4,
        "gain D {}",
        fixed.gain_factor
    );
    let e = worst_signed(&fixed, 500.0);
    assert!(e.abs() < 0.1, "arrondi retiré : écart {e} dB");
    let raw = compare(&stage(InputNoise::None), FS, &[], false).unwrap();
    let e = worst_signed(&raw, 500.0);
    assert!(e < -0.3, "biais de l'arrondi invisible : {e} dB");
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

// ---- Lot 22a bis : cadence nominale, TPA, D forcé à zéro ----

fn with_raw(mut s: Settings, kv: &[(&str, &str)]) -> Settings {
    for (k, v) in kv {
        s.raw.insert(k.to_string(), v.to_string());
    }
    s
}

const TPA: [(&str, &str); 5] = [
    ("tpa_breakpoint", "1350"),
    ("tpa_rate", "65"),
    ("tpa_low_breakpoint", "1050"),
    ("tpa_low_rate", "20"),
    ("tpa_low_always", "0"),
];

#[test]
fn facteur_tpa_du_d_comme_le_firmware() {
    let s = with_raw(Settings::default(), &TPA);
    // seuil 0,35, pente 0,65 / 0,65 = 1 ; TPA bas : seuil 0,05, pente 0,2 / 0,05 = 4, oublié après le premier passage
    let got = tpa_d_factor(&s, &[0.0, 0.5, 0.0, 0.35, 1.0]).unwrap();
    let want = [0.8, 0.85, 1.0, 1.0, 0.35];
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-12, "{got:?}");
    }
    let always = with_raw(s.clone(), &[("tpa_low_always", "1")]);
    assert!((tpa_d_factor(&always, &[0.0, 0.5, 0.0]).unwrap()[2] - 0.8).abs() < 1e-12);
    assert!(
        tpa_d_factor(&Settings::default(), &[0.5]).is_none(),
        "pas de TPA dans l'en-tête"
    );
    assert!(tpa_d_factor(&with_raw(s, &[("tpa_curve_type", "1")]), &[0.5]).is_none());
}

/// Gaz d'avant le boost (fraction) → gaz enregistrés (%), comme `flight/mixer.c` : y = x + b·(x − PT1(x)).
fn boosted(x: &[f64], b: f64, cutoff: f64, fs: f64) -> Vec<f32> {
    let w = 2.0 * PI * cutoff / fs;
    let k = w / (w + 1.0);
    let mut l = x[0];
    x.iter()
        .map(|&v| {
            l += k * (v - l);
            ((v + b * (v - l)).clamp(0.0, 1.0) * 100.0) as f32
        })
        .collect()
}

/// Gaz d'avant le boost : palier puis sinus, entre 0,2 et 0,9.
fn throttle_before_boost(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let t = i as f64 / FS;
            0.3 + if t > 3.0 { 0.3 } else { 0.0 } + 0.2 * (2.0 * PI * 0.7 * t).sin().max(0.0)
        })
        .collect()
}

#[test]
fn gaz_d_avant_le_boost_retrouves() {
    let s = with_raw(
        Settings::default(),
        &[("throttle_boost", "5"), ("throttle_boost_cutoff", "15")],
    );
    let x = throttle_before_boost(8 * FS as usize);
    let y = boosted(&x, 0.5, 15.0, FS);
    let back = pre_boost_throttle(&s, &y, FS);
    let gap = x.iter().zip(&back).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    assert!(gap < 1e-5, "écart {gap}");
    // contre-preuve : le boost déforme bien les gaz enregistrés
    let raw = x
        .iter()
        .zip(&y)
        .map(|(a, b)| (a - *b as f64 / 100.0).abs())
        .fold(0.0, f64::max);
    assert!(raw > 0.05, "boost invisible : {raw}");
}

#[test]
fn filtres_calcules_a_la_cadence_nominale_meme_si_la_boucle_ralentit() {
    // contrôleur surchargé : les filtres sont calculés pour 4 kHz, mais la boucle ne tourne qu'à 2 kHz (L-101)
    let mut log = fabricated("FFT_FREQ", None);
    log.fs = FS / 2.0;
    log.t = (0..log.n()).map(|i| i as f64 / log.fs).collect();
    assert_eq!(model_rate_hz(&log.settings), FS);
    let r = check_stages(&log, 0..log.n()).unwrap();
    let (e, d) = worst(find(&r.checks, Stage::GyroBlock), 250.0);
    assert!(e < 0.05 && d < 0.005, "bloc : écart {e} dB, retard {d} ms");
    let (e, _) = worst(find(&r.checks, Stage::DynNotch), 500.0);
    assert!(e < 1e-3, "notch dynamique : écart {e} dB");
    let (e, _) = worst(find(&r.checks, Stage::Dterm), 500.0);
    assert!(e < 1e-3, "D : écart {e} dB");
    // contre-preuve : le même étage calculé à la cadence mesurée fait croire à un écart du firmware
    let (sigs, _) = stage_signals(&log, 0..log.n()).unwrap();
    let mut dyn_notch = sigs.into_iter().find(|s| s.stage == Stage::DynNotch).unwrap();
    dyn_notch.model = gyro_stage(GyroStage::DynNotch, &log, &log.settings, dyn_notch.input.clone(), 1).unwrap();
    let (e, _) = worst(&compare(&dyn_notch, FS, &[], true).unwrap(), 500.0);
    assert!(e > 1.0, "cadence mesurée non distinguée : {e} dB");
}

#[test]
fn le_d_suit_le_tpa_des_gaz_d_avant_le_boost() {
    let mut log = fabricated("FFT_FREQ", None);
    let n = log.n();
    let x = throttle_before_boost(n);
    log.throttle = boosted(&x, 0.5, 15.0, FS);
    log.settings = with_raw(
        log.settings.clone(),
        &[
            TPA.as_slice(),
            &[("throttle_boost", "5"), ("throttle_boost_cutoff", "15")],
        ]
        .concat(),
    );
    let tpa = tpa_d_factor(&log.settings, &x).unwrap();
    let d = simulate_dterm_at(&log, None, None, FS).unwrap();
    for ax in 0..3 {
        let v: Vec<f64> = d.row(ax).iter().zip(&tpa).map(|(v, f)| v * f * 0.0173).collect();
        set_row(&mut log.d_term, ax, &v);
    }
    let dt = |log: &FlightLog| find(&check_stages(log, 0..n).unwrap().checks, Stage::Dterm).clone();
    let c = dt(&log);
    assert!((c.gain_factor - 0.0173).abs() < 1e-4, "gain D {}", c.gain_factor);
    assert!(c.residual < 2e-3, "résidu {}", c.residual);
    // contre-preuve : sans TPA dans l'en-tête, le modèle garde un gain constant et ne suit plus
    log.settings.raw.remove("tpa_breakpoint");
    let c = dt(&log);
    assert!(c.residual > 0.05, "TPA invisible : résidu {}", c.residual);
}

#[test]
fn instants_ou_le_firmware_force_le_d_a_zero_ecartes() {
    let mut log = fabricated("FFT_FREQ", None);
    let n = log.n();
    for c in ["axisP[0]", "axisP[1]", "axisP[2]", "axisI[0]", "axisI[1]", "axisI[2]"] {
        log.pid_columns.insert(c.to_string());
    }
    for ax in 0..3 {
        set_row(&mut log.p_term, ax, &vec![5.0; n]);
        set_row(&mut log.i_term, ax, &vec![3.0; n]);
    }
    // récupération de vrille en lacet pendant 50 ms : roulis et tangage à zéro, I du lacet à zéro, P du lacet gardé
    let cut = 4 * FS as usize..4 * FS as usize + 200;
    for i in cut.clone() {
        for ax in 0..2 {
            for r in [&mut log.p_term, &mut log.i_term, &mut log.d_term] {
                r.data[ax * n + i] = 0.0;
            }
        }
        log.i_term.data[2 * n + i] = 0.0;
    }
    let (sigs, _) = stage_signals(&log, 0..n).unwrap();
    let d = sigs.iter().find(|s| s.stage == Stage::Dterm && s.axis == 0).unwrap();
    assert_eq!(d.breaks.len(), 201, "chaque instant coupé et le suivant");
    let kept = compare(d, FS, &[], true).unwrap();
    assert!(kept.residual < 1e-3, "résidu {}", kept.residual);
    // contre-preuve : sans ces raccords, les 50 ms à zéro faussent l'étage
    let mut raw = d.clone();
    raw.breaks.clear();
    let all = compare(&raw, FS, &[], true).unwrap();
    assert!(all.residual > 0.02, "coupure invisible : résidu {}", all.residual);
    // un seul axe à zéro (hasard d'arrondi en vol calme) n'est pas une coupure
    log.i_term.data[cut.start + 2 * n..cut.end + 2 * n].fill(3.0);
    let (sigs, _) = stage_signals(&log, 0..n).unwrap();
    assert!(
        sigs.iter()
            .filter(|s| s.stage == Stage::Dterm)
            .all(|s| s.breaks.is_empty())
    );
}
