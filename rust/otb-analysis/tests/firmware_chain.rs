//! Chaîne de filtres assemblée à l'arithmétique du firmware (`otb_analysis::firmware_chain`, lot 24c du plan, D-062), sur
//! des logs fabriqués et sur `tests/data/jeno5_5s.bbl` : la CI n'a ni le corpus ni le firmware (L-023).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::path::PathBuf;

use otb_analysis::bf_filters::{GyroStage, gyro_stage, gyro_stage_own_rate};
use otb_analysis::chain::{self, Signal};
use otb_analysis::filter_replay::{before_after, before_after_with, chain_gap, replay, replay_with};
use otb_analysis::filter_stages::{check_stages, check_stages_with};
use otb_analysis::firmware_chain::{Engine, Release, every, gyro_steps};
use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows, load};
use otb_log::settings::{Lowpass, Settings};
use serde_json::Value;

fn rows(k: usize, n: usize) -> Rows {
    Rows {
        k,
        n,
        data: vec![0.0; k * n],
    }
}

fn lpf(kind: &'static str, hz: i64) -> Lowpass {
    Lowpass {
        kind,
        static_hz: hz,
        ..Lowpass::default()
    }
}

/// Log fabriqué de `n` échantillons, réglages `s`, gaz à 40 %, eRPM de `motor_hz` (Hz) sur 4 moteurs s'il est donné.
fn log_with(s: Settings, n: usize, motor_hz: Option<f64>) -> FlightLog {
    let fs = s.log_rate_hz();
    let motor = motor_hz.map(|hz| {
        let mut r = rows(4, n);
        // la fréquence enregistrée vient d'un eRPM entier (centaines de tours électriques par minute)
        let back = s.motor_poles as f64 / 2.0 * 60.0 / 100.0;
        let v = ((hz * back).round() / back) as f32;
        r.data.iter_mut().for_each(|x| *x = v);
        r
    });
    FlightLog {
        path: PathBuf::from("fabrique.bbl"),
        session: 1,
        settings: s,
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
        motor_hz: motor,
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
    }
}

/// Réglages sans aucun filtre : chaque test n'allume que ce qu'il contrôle.
fn bare(version: [i64; 2], looptime_us: i64, denom: i64) -> Settings {
    Settings {
        version: vec![version[0], version[1], 0],
        looptime_us,
        pid_denom: denom,
        p_interval: (1, 1),
        debug_mode: "GYRO_SCALED".into(),
        gyro_lpf1: lpf("PT1", 0),
        gyro_lpf2: lpf("PT1", 0),
        gyro_notches: vec![(0, 0), (0, 0)],
        dterm_lpf1: lpf("PT1", 0),
        dterm_lpf2: lpf("PT1", 0),
        dterm_notch: (0, 0),
        rpm_harmonics: 0,
        dyn_notch_count: 0,
        motor_poles: 14,
        ..Settings::default()
    }
}

fn bench_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../otb-dsp/tests/data/firmware/betaflight")
}

fn read_f32(name: &str) -> Vec<f32> {
    let bytes = std::fs::read(bench_dir().join(name)).unwrap_or_else(|e| panic!("{name} : {e}"));
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

fn as_usize(v: &Value) -> usize {
    v.as_u64().expect("entier") as usize
}

/// Bloc d'avant le notch dynamique (LPF2, RPM, notchs statiques, LPF1).
fn block() -> std::ops::Range<usize> {
    chain::betaflight()
        .gyro_between(Signal::GyroUnfilt, Signal::FftFreqBeforeDynNotch)
        .unwrap()
}

/// Les cas « deux cadences » du banc (lot 24b) que le firmware joue tels quels dans sa boucle : la chaîne assemblée doit
/// en rendre les bits, entrée à la cadence gyro, sortie à la cadence PID. Les autres cas du banc ne sont pas la boucle du
/// firmware (biquad du LPF1 en forme directe II au lieu de I) : ils ne servent pas ici.
#[test]
fn la_chaine_assemblee_rend_les_bits_du_banc() {
    let text = std::fs::read_to_string(bench_dir().join("manifest.json")).unwrap();
    let m: Value = serde_json::from_str(&text).unwrap();
    let blob = read_f32(m["inputs_file"].as_str().unwrap());
    let case = |id: &str| -> &Value { m["cases"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap() };
    let signal = {
        let e = &m["inputs"]["signal"];
        let (o, l) = (as_usize(&e["offset"]), as_usize(&e["len"]));
        &blob[o..o + l]
    };
    let mut checked = 0;
    for v in m["versions"].as_array().unwrap() {
        let hv = v["header_version"].as_array().unwrap();
        let version = [hv[0].as_i64().unwrap(), hv[1].as_i64().unwrap()];
        let out = read_f32(v["outputs"].as_str().unwrap());
        for (id, lpf1) in [
            ("two_rate_average_denom4", lpf("PT1", 150)),
            ("two_rate_average_denom3", lpf("PT3", 90)),
        ] {
            let c = case(id);
            let denom = c["params"]["denom"].as_f64().unwrap() as i64;
            let mut s = bare(version, 125, denom);
            s.gyro_lpf1 = lpf1;
            let center = c["params"]["notch_center_hz"].as_f64().unwrap() as i64;
            let cutoff = c["params"]["notch_cutoff_hz"].as_f64().unwrap() as i64;
            s.gyro_notches = vec![(center, cutoff), (0, 0)];
            let n_log = signal.len() / denom as usize;
            let log = log_with(s.clone(), n_log, None);
            let x: Vec<f64> = signal[..n_log * denom as usize].iter().map(|&v| v as f64).collect();
            let got = gyro_steps(&log, &s, block(), &[x], denom as usize, None).unwrap();
            let p = v["cases"].as_array().unwrap().iter().find(|p| p["case"] == id).unwrap();
            let want = &out[as_usize(&p["offset"])..as_usize(&p["offset"]) + as_usize(&p["len"])];
            let same = got[0].len() == want.len() && got[0].iter().zip(want).all(|(a, b)| a.to_bits() == b.to_bits());
            // ⚠️ jusqu'à la 4.5, le firmware initialise un PT3 gyro avec le gain d'un PT1 ; le banc, avec celui d'un PT3
            let pt3_quirk = id.ends_with("denom3") && Release::of(&s).unwrap() <= Release::V4_5;
            assert_eq!(same, !pt3_quirk, "{} / {id}", v["version"]);
            checked += 1;
        }
    }
    assert!(checked >= 10, "{checked} cas joués");
}

/// Chaque axe garde l'état de ses filtres : trois axes rejoués ensemble donnent ce que donne chacun rejoué seul (le LPF2
/// à la cadence gyro a un jour partagé l'état du roulis entre les trois axes, écart de 60 %).
#[test]
fn les_axes_ont_chacun_leurs_filtres() {
    let mut s = bare([2025, 12], 125, 2);
    s.gyro_lpf2 = lpf("PT1", 500);
    s.gyro_lpf1 = lpf("PT2", 200);
    s.gyro_notches = vec![(300, 200), (0, 0)];
    s.rpm_harmonics = 2;
    s.rpm_q = 500;
    s.rpm_min_hz = 100;
    s.rpm_fade_hz = 50;
    s.rpm_lpf_hz = 150;
    s.rpm_weights = vec![100, 80];
    let n = 4000;
    let log = log_with(s.clone(), n, Some(230.0));
    let x: Vec<Vec<f64>> = (0..3)
        .map(|a| {
            (0..n)
                .map(|i| 30.0 * (2.0 * PI * (40.0 + 90.0 * a as f64) * i as f64 / 4000.0).sin() + 5.0 * a as f64)
                .collect()
        })
        .collect();
    let together = gyro_steps(&log, &s, block(), &x, 1, None).unwrap();
    for a in 0..3 {
        let alone = gyro_steps(&log, &s, block(), &x[a..a + 1], 1, None).unwrap();
        assert_eq!(together[a], alone[0], "axe {a}");
    }
}

/// Notchs RPM : un sinus à la fréquence des moteurs est éteint, quelle que soit la version (eRPM retrouvé du log,
/// fréquence lissée, recalcul à tour de rôle, biquad pondéré ou SVF).
#[test]
fn les_notchs_rpm_eteignent_la_frequence_des_moteurs() {
    for version in [[4, 3], [4, 4], [4, 5], [2025, 12], [2026, 6]] {
        // une seule harmonique : avec deux, un nombre de pôles faux (fréquence moitié) placerait encore la deuxième sur
        // la fréquence des moteurs
        let mut s = bare(version, 125, 2);
        s.rpm_harmonics = 1;
        s.rpm_q = 500;
        s.rpm_min_hz = 100;
        s.rpm_fade_hz = 50;
        s.rpm_lpf_hz = 150;
        s.rpm_weights = vec![100];
        let n = 8000;
        let log = log_with(s.clone(), n, Some(220.0));
        let f = log.motor_hz.as_ref().unwrap().row(0)[0] as f64;
        let x: Vec<f64> = (0..n)
            .map(|i| 50.0 * (2.0 * PI * f * i as f64 / 4000.0).sin())
            .collect();
        let rpm = chain::betaflight()
            .gyro_between(Signal::GyroSample(1), Signal::GyroSample(2))
            .unwrap();
        let y = every(
            &gyro_steps(&log, &s, rpm, std::slice::from_ref(&x), 1, None).unwrap()[0],
            1,
        );
        let rms = |v: &[f64]| (v.iter().map(|a| a * a).sum::<f64>() / v.len() as f64).sqrt();
        let late = n / 2..n;
        let db = 20.0 * (rms(&y[late.clone()]) / rms(&x[late])).log10();
        assert!(db < -30.0, "{version:?} : {db:.1} dB");
        // sans eRPM dans le log, rien n'est filtré
        let mut no_erpm = log.clone();
        no_erpm.motor_hz = None;
        let rpm = chain::betaflight()
            .gyro_between(Signal::GyroSample(1), Signal::GyroSample(2))
            .unwrap();
        let same = gyro_steps(&no_erpm, &s, rpm, std::slice::from_ref(&x), 1, None).unwrap();
        assert_eq!(
            every(&same[0], 1),
            x.iter().map(|&v| v as f32 as f64).collect::<Vec<_>>()
        );
    }
}

/// Un notch RPM dont la fréquence dépasse 0,48 × la cadence PID est plaqué à ce plafond et reste actif, comme dans le
/// firmware (`constrainf`) : dans la chaîne du firmware **et** dans le rejeu de la chaîne actuelle (page Filtres,
/// assistant). Seule la simulation de l'étalon Python (`gyro_stage`, références figées) l'ignore encore (W-082).
#[test]
fn le_notch_rpm_au_plafond_reste_actif() {
    // 4.5, gyro, PID et log à 4 kHz ; moteurs à 650 Hz : la 3e harmonique (1 950 Hz) dépasse 1 920 Hz
    let mut s = bare([4, 5], 250, 1);
    s.rpm_harmonics = 3;
    s.rpm_q = 500;
    s.rpm_min_hz = 100;
    s.rpm_fade_hz = 50;
    s.rpm_lpf_hz = 150;
    s.rpm_weights = vec![100, 100, 100];
    let n = 8000;
    let log = log_with(s.clone(), n, Some(650.0));
    let ceiling = 0.48 * 4000.0;
    let x: Vec<f64> = (0..n)
        .map(|i| 50.0 * (2.0 * PI * ceiling * i as f64 / 4000.0).sin())
        .collect();
    let rms = |v: &[f64]| (v[n / 2..].iter().map(|a| a * a).sum::<f64>() / (n / 2) as f64).sqrt();
    let db = |y: &[f64]| 20.0 * (rms(y) / rms(&x)).log10();
    let replayed = gyro_stage_own_rate(GyroStage::Rpm, &log, &s, x.clone(), s.log_rate_hz()).unwrap();
    assert!(db(&replayed) < -20.0, "rejeu : {:.1} dB", db(&replayed));
    let rpm = GyroStage::Rpm.steps().unwrap();
    let fw = every(
        &gyro_steps(&log, &s, rpm, std::slice::from_ref(&x), 1, None).unwrap()[0],
        1,
    );
    assert!(db(&fw) < -20.0, "chaîne du firmware : {:.1} dB", db(&fw));
    let python = gyro_stage(GyroStage::Rpm, &log, &s, x.clone(), 1).unwrap();
    assert!(db(&python) > -1.0, "étalon : {:.1} dB", db(&python));
}

/// Avant la 4.3, la chaîne refuse le log : aucune arithmétique vérifiée.
#[test]
fn avant_la_4_3_rien_n_est_rejoue() {
    let s = bare([4, 2], 125, 2);
    let log = log_with(s.clone(), 100, None);
    assert!(gyro_steps(&log, &s, block(), &[vec![0.0; 100]], 1, None).is_err());
}

/// Bascule du lot 24d (D-062) : dès la 4.3, le rejeu par défaut — celui de la page Filtres (`before_after`), de l'assistant
/// (`replay`) et du contrôle par étage (`check_stages`) — est celui de la chaîne du firmware ; avant la 4.3, l'ancienne chaîne
/// reste (arithmétique non vérifiée).
#[test]
fn la_chaine_en_service_est_celle_du_firmware_des_la_4_3() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    let mut lg = load(&path).unwrap().swap_remove(0);
    let s = lg.settings.clone();
    assert_eq!(Engine::in_service(&s), Engine::Firmware);
    assert_eq!(replay(&lg, &s).unwrap().unwrap().engine, Engine::Firmware);
    let mut modf = s.clone();
    modf.gyro_lpf2 = lpf("PT1", 250);
    let sel = 0..lg.n();
    let ba = before_after(&lg, sel.clone(), &s, &modf, 300.0).unwrap().unwrap();
    let fw = before_after_with(&lg, sel.clone(), &s, &modf, 300.0, Engine::Firmware)
        .unwrap()
        .unwrap();
    let old = before_after_with(&lg, sel.clone(), &s, &modf, 300.0, Engine::Model)
        .unwrap()
        .unwrap();
    let cur = |b: &otb_analysis::filter_replay::BeforeAfter| b.gyro[0].current_db.clone();
    let modified = |b: &otb_analysis::filter_replay::BeforeAfter| b.gyro[0].modified_db.clone();
    assert_eq!(cur(&ba), cur(&fw));
    assert_eq!(modified(&ba), modified(&fw));
    assert_ne!(cur(&ba), cur(&old), "les deux chaînes ne rendent pas les mêmes bits");
    let gaps = |c: &otb_analysis::filter_stages::StageChecks| -> Vec<u64> {
        c.checks
            .iter()
            .flat_map(|k| k.bands.iter().map(|b| b.error_db.to_bits()))
            .collect()
    };
    let by_default = check_stages(&lg, sel.clone()).unwrap();
    assert_eq!(
        gaps(&by_default),
        gaps(&check_stages_with(&lg, sel.clone(), Engine::Firmware).unwrap())
    );
    // avant la 4.3 : l'ancienne chaîne, sans erreur
    lg.settings.version = vec![4, 2, 11];
    let s42 = lg.settings.clone();
    assert_eq!(Engine::in_service(&s42), Engine::Model);
    assert_eq!(replay(&lg, &s42).unwrap().unwrap().engine, Engine::Model);
    assert!(before_after(&lg, sel, &s42, &s42, 300.0).unwrap().is_some());
}

/// Sur un vrai log (JeNo 5, 2026.6, gyro et PID à 3,2 kHz, log une boucle sur deux), la chaîne du firmware et l'actuelle
/// rejouent presque le même gyro, et le contrôle par étage ne se dégrade pas. Mesuré le 2026-11-24 (lot 24c) : écart
/// moyen +0,01 / −0,02 dB sous 100 Hz, +0,24 / +0,05 dB de 100 à 250 Hz, écart efficace 7,7 % / 0,5 % (roulis, tangage) ;
/// bloc gyro 50–100 Hz −0,01 → −0,11 dB.
#[test]
fn jeno5_les_deux_chaines_se_rejoignent() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    let logs = load(&path).unwrap();
    let lg = &logs[0];
    let s = &lg.settings;
    assert!(Release::of(s).is_some());
    let gap = chain_gap(lg, 0..lg.n(), 300.0).unwrap().expect("gyro non filtré");
    for (a, (old, new)) in gap.gyro_db.iter().enumerate().take(2) {
        for (lo, hi, tol) in [(5.0, 100.0, 0.05), (100.0, 250.0, 0.4)] {
            let v: Vec<f64> = gap
                .freqs
                .iter()
                .zip(old.iter().zip(new))
                .filter(|(f, _)| **f >= lo && **f < hi)
                .map(|(_, (o, n))| n - o)
                .collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            assert!(mean.abs() < tol, "axe {a}, {lo}–{hi} Hz : {mean:+.3} dB");
        }
        assert!(gap.gyro_rel_rms[a] < 0.12, "axe {a} : {:.3}", gap.gyro_rel_rms[a]);
    }
    // le rejeu choisit bien sa chaîne, et la reprend pour des réglages modifiés
    let fw = replay_with(lg, s, Engine::Firmware).unwrap().unwrap();
    assert_eq!(fw.engine, Engine::Firmware);
    assert!(!fw.gyro_pid.is_empty());
    let old = check_stages_with(lg, 0..lg.n(), Engine::Model).unwrap();
    let new = check_stages_with(lg, 0..lg.n(), Engine::Firmware).unwrap();
    assert_eq!(old.checks.len(), new.checks.len());
    for (o, n) in old.checks.iter().zip(&new.checks) {
        for (bo, bn) in o.bands.iter().zip(&n.bands) {
            if bo.conclusive && bn.conclusive && bo.hi_hz <= 250.0 {
                assert!(
                    bn.error_db.abs() - bo.error_db.abs() < 0.25,
                    "{:?} {}–{} Hz : {:.2} → {:.2} dB",
                    o.stage,
                    bo.lo_hz,
                    bo.hi_hz,
                    bo.error_db,
                    bn.error_db
                );
            }
        }
    }
}

/// Le rejeu de la chaîne actuelle applique les notchs RPM dans l'ordre du firmware (harmonique puis moteur depuis la 4.4,
/// `firmware_chain::rpm_apply_order`) : quand la fréquence des moteurs bouge, il rejoint la chaîne du firmware à 2,1 %
/// (valeur efficace de l'écart, mesuré le 2026-11-24) ; dans l'ordre de l'étalon (moteur d'abord), à 5,3 % (L-120). Le reste
/// vient du calendrier des recalculs, que seule la chaîne du firmware recopie.
#[test]
fn le_rejeu_applique_les_notchs_rpm_dans_l_ordre_du_firmware() {
    for version in [[4, 5], [2025, 12]] {
        let mut s = bare(version, 125, 2);
        s.rpm_harmonics = 3;
        s.rpm_q = 500;
        s.rpm_min_hz = 100;
        s.rpm_fade_hz = 50;
        s.rpm_lpf_hz = 150;
        s.rpm_weights = vec![100, 100, 100];
        let n = 16000;
        let mut log = log_with(s.clone(), n, Some(200.0));
        let back = 7.0 * 60.0 / 100.0;
        let mh = log.motor_hz.as_mut().unwrap();
        for m in 0..4 {
            for i in 0..n {
                let t = i as f64 / 4000.0;
                let hz = 220.0 + 15.0 * m as f64 + 80.0 * (2.0 * PI * (3.0 + 1.0 * m as f64) * t).sin();
                mh.data[m * n + i] = ((hz * back).round() / back) as f32;
            }
        }
        let mut st = 5u64;
        let x: Vec<f64> = (0..n)
            .map(|_| {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((st >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 100.0
            })
            .collect();
        let rep = gyro_stage_own_rate(GyroStage::Rpm, &log, &s, x.clone(), s.log_rate_hz()).unwrap();
        let rpm = GyroStage::Rpm.steps().unwrap();
        let fw = every(
            &gyro_steps(&log, &s, rpm, std::slice::from_ref(&x), 1, None).unwrap()[0],
            1,
        );
        let d: f64 = rep
            .iter()
            .zip(&fw)
            .skip(n / 4)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>();
        let r: f64 = fw.iter().skip(n / 4).map(|a| a * a).sum::<f64>();
        let gap = (d / r).sqrt();
        assert!(gap < 0.035, "{version:?} : écart {gap:.4}");
    }
}
