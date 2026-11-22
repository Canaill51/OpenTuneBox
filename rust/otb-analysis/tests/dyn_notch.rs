//! Suivi des pics du notch dynamique rejoué (`otb_analysis::dyn_notch`, lot 22c) : chaque axe suit son propre pic, les
//! réglages tirés de l'en-tête reproduisent les arrondis de chaque version du firmware, et le contrôle contre les centres
//! enregistrés rend un écart nul quand ces centres sortent du même suivi (contre-preuve : des centres faux).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::PathBuf;

use otb_analysis::dyn_notch::{
    DynNotchTracker, FloorRule, PREROLL_S, check_tracking, firmware_build_date, ticks_per_log_sample,
};
use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::Settings;

const FS: f64 = 4000.0;

/// Réglages d'un whoop en 2026.12 : gyro 8 kHz, PID 4 kHz, log à chaque boucle PID, un notch dynamique 100–600 Hz.
fn settings(version: &[i64], date: Option<&str>) -> Settings {
    let mut s = Settings {
        version: version.to_vec(),
        looptime_us: 125,
        pid_denom: 2,
        p_interval: (1, 1),
        debug_mode: "FFT_FREQ".into(),
        dyn_notch_count: 1,
        dyn_notch_q: 300,
        dyn_notch_min_hz: 100,
        dyn_notch_max_hz: 600,
        ..Settings::default()
    };
    if let Some(d) = date {
        s.raw.insert("Firmware date".into(), d.into());
    }
    s
}

/// Bruit uniforme reproductible dans [−1, 1].
fn noise(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

/// Sinus de fréquence `f(t)` (Hz, peut varier) d'amplitude 30 °/s, plus un bruit large bande, à la cadence PID.
fn tone(n: usize, f: impl Fn(f64) -> f64, seed: u64) -> Vec<f64> {
    let mut r = noise(seed);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            phase += 2.0 * PI * f(i as f64 / FS) / FS;
            30.0 * phase.sin() + 4.0 * r()
        })
        .collect()
}

#[test]
fn chaque_axe_suit_son_propre_pic() {
    let s = settings(&[2026, 12, 0], None);
    let t = DynNotchTracker::new(&s).expect("notch actif");
    let n = 2 * FS as usize;
    let freqs = [180.0, 260.0, 410.0];
    let x: Vec<Vec<f64>> = (0..3).map(|a| tone(n, |_| freqs[a], a as u64 + 1)).collect();
    let c = t.track([Some(&x[0]), Some(&x[1]), Some(&x[2])], 1, 0);
    let half_bin = t.resolution_hz as f64 / 2.0;
    for (a, f) in freqs.iter().enumerate() {
        // dernière demi-seconde : le suivi a eu le temps d'accrocher (SDFT pleine en 54 ms, lissage de 4 à 40 Hz)
        for &v in &c.row(a, 0)[n * 3 / 4..] {
            assert!(
                (v as f64 - f).abs() < half_bin,
                "axe {a} : centre {v} Hz, pic à {f} Hz (demi-case {half_bin:.1} Hz)"
            );
        }
    }
}

#[test]
fn le_centre_rejoint_un_pic_qui_saute() {
    let s = settings(&[2026, 12, 0], None);
    let t = DynNotchTracker::new(&s).expect("notch actif");
    let n = 3 * FS as usize;
    let x = tone(n, |t| if t < 1.5 { 200.0 } else { 350.0 }, 7);
    let c = t.track([Some(&x), None, None], 1, 0);
    let at = |s: f64| c.row(0, 0)[(s * FS) as usize] as f64;
    assert!((at(1.4) - 200.0).abs() < 9.0, "avant le saut : {}", at(1.4));
    assert!((at(2.9) - 350.0).abs() < 9.0, "après le saut : {}", at(2.9));
    // le lissage n'est pas instantané : 5 ms après le saut, le centre n'est pas encore arrivé
    assert!(at(1.505) < 300.0, "5 ms après le saut : {}", at(1.505));
}

#[test]
fn log_une_boucle_sur_deux_rejoue_les_boucles_manquantes() {
    // même signal enregistré une boucle sur deux : le suivi interpole et accroche le même pic
    let mut s = settings(&[2026, 12, 0], None);
    s.p_interval = (1, 2);
    assert_eq!(ticks_per_log_sample(&s), 2);
    let t = DynNotchTracker::new(&s).expect("notch actif");
    let full = tone(2 * FS as usize, |_| 240.0, 3);
    let half: Vec<f64> = full.iter().step_by(2).copied().collect();
    let c = t.track([Some(&half), None, None], 2, 0);
    let v = c.row(0, 0)[half.len() - 1] as f64;
    assert!((v - 240.0).abs() < 9.0, "centre {v} Hz");
}

#[test]
fn sample_count_suit_l_arrondi_de_chaque_version() {
    // ⚠️ Nyquist / max entier (2000 / 500 = 4) : la cadence en float du firmware tombe juste au-dessus (4.x, 2025.12)
    // ou juste en dessous (2026.6, puis 2026.12 avant le 2026-11-20) de 4000 Hz ; exacte ensuite. Contre-épreuve sur le
    // corpus : forcer la cadence exacte en 2026.6 dégrade l'accord avec les centres enregistrés (validation.md).
    let cas: [(&[i64], Option<&str>, usize); 6] = [
        (&[4, 5, 2], None, 4),
        (&[2025, 12, 1], None, 4),
        (&[2026, 6, 1], None, 3),
        (&[2026, 12, 0], Some("Nov 13 2026 15:43:59"), 3),
        (&[2026, 12, 0], Some("Nov 21 2026 09:00:00"), 4),
        (&[2026, 12, 0], None, 4),
    ];
    for (v, date, attendu) in cas {
        let mut s = settings(v, date);
        s.dyn_notch_min_hz = 150;
        s.dyn_notch_max_hz = 500;
        let t = DynNotchTracker::new(&s).expect("notch actif");
        assert_eq!(t.sample_count, attendu, "version {v:?}, date {date:?}");
    }
    // hors de ce cas, toutes les versions s'accordent : 2000 / 600 = 3,33 → 3, case de 18,5 Hz
    let t = DynNotchTracker::new(&settings(&[2026, 6, 1], None)).unwrap();
    assert_eq!(t.sample_count, 3);
    assert!((t.resolution_hz - 4000.0 / 3.0 / 72.0).abs() < 1e-3);
}

#[test]
fn variante_4_3() {
    // 4.3 : max ≥ 2 × min, sample_count = cadence / (2 × max), première case ≥ 2, plancher sur les cases intérieures
    let mut s = settings(&[4, 3, 2], None);
    s.dyn_notch_min_hz = 350;
    s.dyn_notch_max_hz = 600;
    let t = DynNotchTracker::new(&s).unwrap();
    assert_eq!(t.max_hz, 700.0);
    assert_eq!(t.sample_count, 2);
    assert_eq!(t.floor, FloorRule::Inner);
    s.dyn_notch_min_hz = 20;
    assert_eq!(DynNotchTracker::new(&s).unwrap().start_bin, 2);
    // avant la 4.3, le firmware suivait les pics par FFT : non rejoué
    assert!(DynNotchTracker::new(&settings(&[4, 2, 11], None)).is_none());
}

#[test]
fn plancher_au_dessus_de_60_hz_selon_la_date_de_compilation() {
    let floor = |v: &[i64], d: Option<&str>| {
        let mut s = settings(v, d);
        s.dyn_notch_min_hz = 20;
        let t = DynNotchTracker::new(&s).unwrap();
        (t.floor, t.floor_start_bin)
    };
    assert_eq!(
        floor(&[2026, 12, 0], Some("Sep  1 2026 10:00:00")),
        (FloorRule::Full, 1)
    );
    assert_eq!(
        floor(&[2026, 12, 0], Some("Sep 10 2026 10:00:00")),
        (FloorRule::Above60 { strict: false }, 3)
    );
    assert_eq!(
        floor(&[2026, 12, 0], Some("Oct  8 2026 10:00:00")),
        (FloorRule::Above60 { strict: true }, 3)
    );
    // la date ne compte que pour la branche de développement 2026.12
    assert_eq!(floor(&[2026, 6, 2], Some("Oct  8 2026 10:00:00")), (FloorRule::Full, 1));
}

#[test]
fn date_de_compilation_lue_dans_l_en_tete() {
    assert_eq!(
        firmware_build_date(&settings(&[2026, 12, 0], Some("Nov 13 2026 15:43:59"))),
        Some(20261113)
    );
    assert_eq!(
        firmware_build_date(&settings(&[2026, 12, 0], Some("Sep  5 2026 01:02:03"))),
        Some(20260905)
    );
    assert_eq!(firmware_build_date(&settings(&[2026, 12, 0], Some("hier"))), None);
    assert_eq!(firmware_build_date(&settings(&[2026, 12, 0], None)), None);
}

fn rows(k: usize, n: usize) -> Rows {
    Rows {
        k,
        n,
        data: vec![0.0; k * n],
    }
}

fn set_row(r: &mut Rows, i: usize, v: impl IntoIterator<Item = f64>) {
    for (o, x) in r.data[i * r.n..(i + 1) * r.n].iter_mut().zip(v) {
        *o = x as f32;
    }
}

/// Log `FFT_FREQ` fabriqué : gyro avant le notch dans `debug[0]` (pic qui glisse de 180 à 320 Hz), centres dans `debug[1]`.
fn fabricated(centres: impl Fn(&Settings, &[f64]) -> Vec<f64>) -> FlightLog {
    let s = settings(&[2026, 12, 0], None);
    let n = 10 * FS as usize;
    let x = tone(n, |t| 180.0 + 14.0 * t, 11);
    let mut debug = rows(8, n);
    set_row(&mut debug, 0, x.iter().copied());
    set_row(&mut debug, 1, centres(&s, &x));
    FlightLog {
        path: PathBuf::from("fabrique.bbl"),
        session: 1,
        settings: s,
        t: (0..n).map(|i| i as f64 / FS).collect(),
        fs: FS,
        gyro: rows(3, n),
        gyro_unfilt: None,
        setpoint: rows(4, n),
        rc_command: rows(4, n),
        p_term: rows(3, n),
        i_term: rows(3, n),
        d_term: rows(3, n),
        f_term: rows(3, n),
        throttle: vec![40.0; n],
        motors: rows(4, n),
        motor_hz: None,
        debug,
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

#[test]
fn controle_nul_si_les_centres_enregistres_sortent_du_meme_suivi() {
    let log = fabricated(|s, x| {
        let t = DynNotchTracker::new(s).unwrap();
        let preroll = (PREROLL_S * s.log_rate_hz()).round() as usize;
        t.track([Some(x), None, None], 1, preroll)
            .row(0, 0)
            .iter()
            .map(|&v| v as f64)
            .collect()
    });
    let c = check_tracking(&log, 4000..36000).unwrap().expect("log FFT_FREQ");
    assert_eq!(c.axis, 0);
    let n = &c.notches[0];
    assert_eq!(n.samples, 32000);
    assert_eq!(n.from_debug.median_abs_hz, 0.0);
    assert_eq!(n.from_debug.p90_abs_hz, 0.0);
    assert_eq!(n.from_debug.within_bin, 1.0);
    assert!(n.from_model.is_none(), "pas de gyro non filtré");
    // les centres initiaux, fixes à 350 Hz, sont loin du pic qui glisse
    assert!(n.initial.median_abs_hz > 50.0, "{:?}", n.initial);
}

#[test]
fn controle_voit_des_centres_enregistres_faux() {
    // contre-preuve : centres enregistrés décalés de 40 Hz
    let log = fabricated(|s, x| {
        let t = DynNotchTracker::new(s).unwrap();
        let preroll = (PREROLL_S * s.log_rate_hz()).round() as usize;
        t.track([Some(x), None, None], 1, preroll)
            .row(0, 0)
            .iter()
            .map(|&v| v as f64 + 40.0)
            .collect()
    });
    let n = &check_tracking(&log, 4000..36000).unwrap().unwrap().notches[0];
    assert!((n.from_debug.median_hz + 40.0).abs() < 1e-3, "{:?}", n.from_debug);
    assert_eq!(n.from_debug.within_bin, 0.0);
}
