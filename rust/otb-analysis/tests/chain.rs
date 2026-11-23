//! Description de la chaîne de filtres en données (`otb_analysis::chain`, lot 23 du plan, D-062) : le fichier
//! `chains/betaflight.json` se lit, suit l'ordre du firmware (`docs/spec-betaflight.md` §3), et la réponse en fréquence
//! comme la simulation dans le temps le suivent. Une étape déplacée ou une cadence changée dans le fichier fait échouer
//! ces tests (contre-preuve du lot 23).

use otb_analysis::bf_filters::{GyroStage, Kind, dterm_chain, gyro_chain, stages_before};
use otb_analysis::chain::{self, Brick, ChainDesc, Rate, Signal, Which};
use otb_log::settings::{Lowpass, Settings};

const JSON: &str = include_str!("../chains/betaflight.json");

fn lpf(kind: &'static str, hz: i64) -> Lowpass {
    Lowpass {
        kind,
        static_hz: hz,
        ..Lowpass::default()
    }
}

/// Tous les filtres en service, cadence gyro 8 kHz, PID 4 kHz.
fn settings() -> Settings {
    Settings {
        looptime_us: 125,
        pid_denom: 2,
        gyro_lpf2: lpf("PT1", 500),
        gyro_lpf1: lpf("PT1", 250),
        gyro_notches: vec![(300, 200), (400, 300)],
        dterm_notch: (200, 150),
        dterm_lpf1: lpf("PT1", 100),
        dterm_lpf2: lpf("BIQUAD", 150),
        rpm_harmonics: 1,
        rpm_weights: vec![100],
        dyn_notch_count: 2,
        dyn_notch_q: 300,
        dyn_notch_min_hz: 100,
        dyn_notch_max_hz: 600,
        ..Settings::default()
    }
}

#[test]
fn la_description_betaflight_se_lit() {
    let d = chain::betaflight();
    assert_eq!(d.firmware, "betaflight");
    let ids = |w: Which| d.steps(w).iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
    // ordre du firmware, spec-betaflight.md §3
    assert_eq!(
        ids(Which::Gyro),
        ["lpf2", "rpm", "notch1", "notch2", "lpf1", "dyn_notch"]
    );
    assert_eq!(ids(Which::Dterm), ["d_notch", "d_lpf1", "d_lpf2"]);
    let rates: Vec<Rate> = d.gyro.iter().map(|s| s.rate).collect();
    assert_eq!(
        rates,
        [Rate::Gyro, Rate::Pid, Rate::Pid, Rate::Pid, Rate::Pid, Rate::Pid]
    );
    assert!(d.dterm.iter().all(|s| s.rate == Rate::Pid));
    // le D filtre le gyro puis dérive : dérivée après les trois filtres, signe −1 (D sur la mesure)
    assert_eq!((d.derivative.at, d.derivative.sign), (3, -1.0));
    assert_eq!(
        d.tap(Signal::GyroUnfilt).map(|t| (t.chain, t.at)),
        Some((Which::Gyro, 0))
    );
    assert_eq!(d.tap(Signal::Gyro).map(|t| t.at), Some(6));
    assert_eq!(d.tap(Signal::AxisD).map(|t| (t.chain, t.at)), Some((Which::Dterm, 3)));
    // le fichier relu à part donne la même description
    assert_eq!(&ChainDesc::parse(JSON).unwrap(), d);
}

#[test]
fn les_etages_couvrent_la_chaine_gyro() {
    let d = chain::betaflight();
    let mut next = 0;
    for st in GyroStage::ALL {
        let r = st.steps().unwrap();
        assert_eq!(r.start, next, "{st:?} ne suit pas l'étage précédent");
        assert!(d.gyro_rate(&r).is_ok(), "{st:?} mêle deux cadences");
        next = r.end;
    }
    assert_eq!(next, d.gyro.len(), "les étages ne couvrent pas toute la chaîne gyro");
    let bricks = |st: GyroStage| d.gyro[st.steps().unwrap()].iter().map(|s| s.brick).collect::<Vec<_>>();
    assert!(matches!(bricks(GyroStage::Lpf2)[..], [Brick::Lowpass(_)]));
    assert_eq!(bricks(GyroStage::Rpm), [Brick::RpmNotch]);
    assert!(matches!(
        bricks(GyroStage::StaticLpf1)[..],
        [Brick::StaticNotch(..), Brick::StaticNotch(..), Brick::Lowpass(_)]
    ));
    assert_eq!(bricks(GyroStage::DynNotch), [Brick::DynNotch]);
    assert_eq!(
        stages_before(Signal::FftFreqBeforeDynNotch).unwrap(),
        [GyroStage::Lpf2, GyroStage::Rpm, GyroStage::StaticLpf1]
    );
}

#[test]
fn la_reponse_en_frequence_suit_la_description() {
    let s = settings();
    let got: Vec<(Kind, f64, f64)> = gyro_chain(&s, 0.5, Some(&[200.0]), None)
        .unwrap()
        .into_iter()
        .map(|(sp, rate)| (sp.kind, sp.f_hz, rate))
        .collect();
    let expected = [
        (Kind::Pt1, 500.0, 8000.0),   // LPF2, cadence gyro
        (Kind::Notch, 200.0, 4000.0), // RPM, fondamentale d'un moteur à 200 Hz
        (Kind::Notch, 300.0, 4000.0), // notch statique 1
        (Kind::Notch, 400.0, 4000.0), // notch statique 2
        (Kind::Pt1, 250.0, 4000.0),   // LPF1
        (Kind::Notch, 225.0, 4000.0), // notchs dynamiques, centres initiaux
        (Kind::Notch, 475.0, 4000.0),
    ];
    assert_eq!(got, expected);
    let d: Vec<(Kind, f64, f64)> = dterm_chain(&s, 0.5)
        .unwrap()
        .into_iter()
        .map(|(sp, rate)| (sp.kind, sp.f_hz, rate))
        .collect();
    assert_eq!(
        d,
        [
            (Kind::Notch, 200.0, 4000.0),
            (Kind::Pt1, 100.0, 4000.0),
            (Kind::BiquadLpf, 150.0, 4000.0)
        ]
    );
}

#[test]
fn le_notch_gyro_refuse_une_cadence_nulle_comme_l_etalon() {
    let s = Settings {
        pid_denom: 0,
        ..settings()
    };
    assert_eq!(gyro_chain(&s, 0.5, None, None).unwrap_err(), "float division by zero");
}

#[test]
fn une_description_fautive_est_refusee() {
    let cases = [
        (
            r#""brick": "lowpass", "rate": "gyro""#,
            r#""brick": "biquad", "rate": "gyro""#,
            "brique inconnue",
        ),
        (
            r#""setting": "gyro_lpf2""#,
            r#""setting": "gyro_lpf3""#,
            "passe-bas inconnu",
        ),
        (r#""rate": "gyro""#, r#""rate": "fast""#, "cadence inconnue"),
        (r#""rate": "gyro""#, r#""rate": "gyro", "q": 2"#, "champ inconnu « q »"),
        (r#"{"id": "rpm""#, r#"{"id": "lpf2""#, "décrite deux fois"),
        (r#""after:rpm""#, r#""after:rpm2""#, "étape inconnue « rpm2 »"),
        (
            r#""gyro": "end:gyro""#,
            r#""gyro_filtered": "end:gyro""#,
            "signal inconnu",
        ),
        (
            r#""after": "d_lpf2""#,
            r#""after": "lpf1""#,
            "pas une étape de la chaîne dterm",
        ),
        (r#""sign": -1"#, r#""sign": 2"#, "-1 ou 1"),
        (
            r#""nyquist": "pid""#,
            r#""nyquist": "gyro""#,
            "limite de notch inconnue",
        ),
    ];
    for (from, to, why) in cases {
        assert!(JSON.contains(from), "motif absent du fichier : {from}");
        let err = ChainDesc::parse(&JSON.replacen(from, to, 1)).unwrap_err();
        assert!(err.contains(why), "{to} : « {err} » ne dit pas « {why} »");
    }
}

#[test]
fn deux_prises_dans_le_mauvais_ordre_sont_refusees() {
    let d = chain::betaflight();
    assert_eq!(d.gyro_between(Signal::GyroSample(0), Signal::GyroSample(3)), Ok(0..5));
    assert!(d.gyro_between(Signal::GyroSample(3), Signal::GyroSample(0)).is_err());
    assert!(d.gyro_between(Signal::GyroSample(0), Signal::AxisD).is_err());
    // un étage qui mêlerait les deux cadences (LPF2 à la cadence gyro et notchs RPM à la cadence PID)
    assert!(d.gyro_rate(&(0..2)).is_err());
}
