//! Simulateur de filtres de la page Filtres (`otb_analysis::filter_replay`, lot 22e, D-058) sur des logs fabriqués : le
//! « firmware » y est la chaîne de filtres connue, appliquée à la cadence gyro (LPF2) et PID (le reste).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::PathBuf;

use otb_analysis::bf_filters::{
    GyroStage, Target, WhatIfCache, gyro_stage, gyro_stage_at, simulate_dterm, whatif_spectrum,
};
use otb_analysis::filter_replay::{BeforeAfter, before_after, replay, replay_from};
use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::{AxisPid, Lowpass, Settings};

const FS: f64 = 4000.0;
const SECONDS: usize = 10;

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

/// Whoop en 2026.12 : gyro 8 kHz, PID 4 kHz, log à chaque boucle PID, sans notch dynamique (ajouté par les tests qui le
/// demandent).
fn settings() -> Settings {
    let mut s = Settings {
        version: vec![2026, 12, 0],
        looptime_us: 125,
        pid_denom: 2,
        p_interval: (1, 1),
        debug_mode: "GYRO_SCALED".into(),
        gyro_lpf2: lpf("PT1", 500),
        gyro_lpf1: lpf("PT2", 200),
        gyro_notches: vec![(300, 200)],
        dterm_lpf1: lpf("PT1", 120),
        dterm_lpf2: lpf("BIQUAD", 180),
        rpm_harmonics: 2,
        rpm_weights: vec![100, 80],
        dyn_notch_count: 0,
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

/// Somme de sinus de 5 à 900 Hz (phases au hasard, fixées) à 8 kHz, plus un pic `peak` (Hz, amplitude ×8) s'il est donné.
fn gyro_8k(count: usize, peak: Option<f64>) -> Vec<f64> {
    let mut state = 777u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let tones: Vec<(f64, f64)> = (0..180)
        .map(|i| (5.0 + 5.0 * i as f64 + next(), 2.0 * PI * next()))
        .collect();
    (0..count)
        .map(|k| {
            let t = k as f64 / 8000.0;
            let base: f64 = tones.iter().map(|(f, p)| (2.0 * PI * f * t + p).sin()).sum();
            let p = peak.map_or(0.0, |f| 8.0 * (2.0 * PI * f * t).sin());
            (base + p) * 40.0
        })
        .collect()
}

/// Log fabriqué : `firmware` = réglages qui ont réellement filtré (ceux de l'en-tête par défaut). Gyro filtré par la chaîne
/// LPF2 (8 kHz) → RPM → notch statique + LPF1 (4 kHz), sans notch dynamique ; terme D = chaîne D × 0,0173.
fn fabricated(firmware: Option<&Settings>, peak: Option<f64>) -> FlightLog {
    let n = (FS as usize) * SECONDS;
    let s = settings();
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
    let fw = firmware.unwrap_or(&s).clone();
    let g8 = gyro_8k(2 * n, peak);
    let unfilt: Vec<f64> = (0..n).map(|i| g8[2 * i + 1]).collect();
    for ax in 0..3 {
        set_row(log.gyro_unfilt.as_mut().unwrap(), ax, &unfilt);
    }
    let a8 = gyro_stage_at(GyroStage::Lpf2, &log, &fw, g8, 1, 2.0 * FS).unwrap();
    let mut g: Vec<f64> = (0..n).map(|i| a8[2 * i]).collect();
    for st in [GyroStage::Rpm, GyroStage::StaticLpf1] {
        g = gyro_stage(st, &log, &fw, g, 1).unwrap();
    }
    for ax in 0..3 {
        set_row(&mut log.gyro, ax, &g);
    }
    let d = simulate_dterm(&log, Some(&fw), None).unwrap();
    for ax in 0..3 {
        let v: Vec<f64> = d.row(ax).iter().map(|x| x * 0.0173).collect();
        set_row(&mut log.d_term, ax, &v);
    }
    log
}

/// Moyenne de `a − b` (dB) sur les fréquences de [`lo`, `hi`[.
fn band(f: &[f64], a: &[f64], b: &[f64], lo: f64, hi: f64) -> f64 {
    let v: Vec<f64> = f
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(x, _)| **x >= lo && **x < hi)
        .map(|(_, (p, q))| p - q)
        .collect();
    v.iter().sum::<f64>() / v.len() as f64
}

const BANDS: [(f64, f64); 3] = [(20.0, 100.0), (100.0, 250.0), (250.0, 500.0)];

fn run(log: &FlightLog, modf: &Settings) -> BeforeAfter {
    before_after(log, 0..log.n(), &log.settings, modf, 300.0)
        .unwrap()
        .expect("gyro non filtré présent")
}

#[test]
fn reglages_inchanges_rendent_exactement_le_log() {
    let log = fabricated(None, None);
    let r = run(&log, &log.settings.clone());
    for ax in 0..3 {
        assert_eq!(r.gyro[ax].modified_db, r.gyro[ax].logged_db, "gyro axe {ax}");
        let d = r.dterm[ax].as_ref().expect("terme D enregistré");
        assert_eq!(d.modified_db, d.logged_db, "D axe {ax}");
    }
    // sans gyro non filtré, rien à rejouer
    let mut no_unfilt = log.clone();
    no_unfilt.gyro_unfilt = None;
    assert!(
        before_after(&no_unfilt, 0..log.n(), &log.settings, &log.settings, 300.0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn rejeu_actuel_juge_le_modele_au_lieu_de_recopier_le_log() {
    // modèle juste : le rejeu « actuel » retombe sur le log (petit écart d'interpolation du LPF2 au-dessus de 250 Hz)
    let log = fabricated(None, None);
    let r = run(&log, &log.settings.clone());
    for (lo, hi) in BANDS {
        let g = band(&r.freqs, &r.gyro[0].current_db, &r.gyro[0].logged_db, lo, hi);
        assert!(g.abs() < 0.3, "gyro {lo}–{hi} Hz : {g:+.3} dB");
        let d = r.dterm[0].as_ref().unwrap();
        let e = band(&r.freqs, &d.current_db, &d.logged_db, lo, hi);
        assert!(e.abs() < 0.3, "D {lo}–{hi} Hz : {e:+.3} dB");
    }
    // contre-preuve : le « firmware » filtre avec un LPF1 à 150 Hz, l'en-tête annonce 200 Hz ; la courbe « actuel » le
    // montre (l'ancienne recopiait le log)
    let mut fw = settings();
    fw.gyro_lpf1 = lpf("PT2", 150);
    let log = fabricated(Some(&fw), None);
    let r = run(&log, &log.settings.clone());
    let g = band(&r.freqs, &r.gyro[0].current_db, &r.gyro[0].logged_db, 100.0, 250.0);
    assert!(g > 1.0, "LPF1 faussé non vu par le rejeu actuel : {g:+.3} dB");
}

#[test]
fn filtrer_plus_ne_prevoit_jamais_plus_de_bruit() {
    // ⚠️ gyro enregistré arrondi au °/s : si le rapport modifié / actuel venait de rejeux arrondis, leur bruit d'arrondi le
    // gonflerait, et un LPF1 plus bas « ajouterait » du bruit dans toutes les bandes (L-105)
    let mut log = fabricated(None, None);
    for v in log.gyro.data.iter_mut() {
        *v = (*v / 40.0).round();
    }
    for v in log.gyro_unfilt.as_mut().unwrap().data.iter_mut() {
        *v = (*v / 40.0).round();
    }
    let mut m = log.settings.clone();
    m.gyro_lpf1 = lpf("PT2", 120);
    let r = run(&log, &m);
    let g = &r.gyro[0];
    for (lo, hi) in BANDS {
        let e = band(&r.freqs, &g.modified_db, &g.logged_db, lo, hi);
        assert!(e < 0.01, "LPF1 plus bas, {lo}–{hi} Hz : {e:+.3} dB");
    }
    assert!(band(&r.freqs, &g.modified_db, &g.logged_db, 100.0, 250.0) < -2.0);
    // et le D suit le gyro modifié, en plus de ses propres filtres
    let mut m = log.settings.clone();
    m.dterm_lpf1 = lpf("PT1", 80);
    let r = run(&log, &m);
    let d = r.dterm[0].as_ref().unwrap();
    let e = band(&r.freqs, &d.modified_db, &d.logged_db, 100.0, 250.0);
    assert!(e < -2.0, "LPF1 D plus bas : {e:+.3} dB");
}

#[test]
fn notchs_dynamiques_ajoutes_suivent_les_pics_et_non_les_centres_initiaux() {
    // un seul pic, à 230 Hz ; le log n'a pas de debug FFT_FREQ. Trois notchs au lieu d'un : l'ancien rapport |H|² posait
    // les notchs ajoutés à leurs centres initiaux (350 et 517 Hz) et y creusait un trou sans pic (W-057) ; le rejeu les
    // fait suivre par le suivi du firmware.
    let mut s = settings();
    s.dyn_notch_count = 1;
    let mut log = fabricated(None, Some(230.0));
    log.settings = s.clone();
    let base = replay(&log, &s).unwrap().unwrap();
    for ax in 0..3 {
        set_row(&mut log.gyro, ax, &base.gyro[ax]);
    }
    let mut m = s.clone();
    m.dyn_notch_count = 3;
    let r = run(&log, &m);
    let g = &r.gyro[0];
    let dip = band(&r.freqs, &g.modified_db, &g.logged_db, 505.0, 530.0);
    // contre-preuve : l'ancienne méthode creuse à 517 Hz
    let x: Vec<f64> = log.gyro.row(0).iter().map(|&v| v as f64).collect();
    let u: Vec<f64> = log
        .gyro_unfilt
        .as_ref()
        .unwrap()
        .row(0)
        .iter()
        .map(|&v| v as f64)
        .collect();
    let w = whatif_spectrum(
        &x,
        Some(&u),
        &log,
        0..log.n(),
        &s,
        &m,
        Target::Gyro,
        300.0,
        &mut WhatIfCache::default(),
    )
    .unwrap();
    let old = band(&w.freqs, &w.modified_db, &w.current_db, 505.0, 530.0);
    assert!(old < -6.0, "l'ancienne méthode ne creuse plus à 517 Hz : {old:+.2} dB");
    assert!(dip > -3.0, "creux sans pic à 517 Hz : {dip:+.2} dB");
}

/// `replay_from` reprend le rejeu actuel (l'assistant essaie plusieurs réglages sur un même vol) : il doit rendre au bit près le
/// rejeu complet, qu'on ne change que les filtres du D (gyro repris) ou aussi ceux du gyro (tout est refait).
#[test]
fn replay_from_rend_le_rejeu_complet() {
    let log = fabricated(None, Some(180.0));
    let base = &log.settings;
    let cur = replay(&log, base).unwrap().unwrap();
    let mut d_only = base.clone();
    d_only.dterm_lpf1 = lpf("PT1", 140);
    d_only.dterm_notch = (220, 160);
    let mut gyro_too = d_only.clone();
    gyro_too.gyro_lpf1 = lpf("PT1", 150);
    for (what, s) in [
        ("filtres du D seuls", &d_only),
        ("filtres du gyro aussi", &gyro_too),
        ("rien", base),
    ] {
        let want = replay(&log, s).unwrap().unwrap();
        let got = replay_from(&log, s, base, &cur).unwrap().unwrap();
        assert_eq!(got.gyro, want.gyro, "{what} : gyro");
        assert_eq!(got.dterm.data, want.dterm.data, "{what} : terme D");
    }
    assert_ne!(
        replay(&log, &gyro_too).unwrap().unwrap().gyro,
        cur.gyro,
        "le LPF1 du gyro doit changer le gyro rejoué, sinon ce test ne prouve rien"
    );
}
