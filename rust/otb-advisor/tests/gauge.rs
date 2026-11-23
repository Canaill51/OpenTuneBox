//! Jauges « mesure contre seuil » des constats (D-070), sur `tests/data/jeno5_5s.bbl` : toute jauge posée se dessine, et sa mesure est
//! le nombre que le texte du constat affiche (même variable que la règle, jamais un second calcul).

use otb_advisor::rules::{self, Finding};
use otb_format::fixed;
use otb_log::flightlog::{self, FlightLog};

fn journal() -> FlightLog {
    let p = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    flightlog::load(std::path::Path::new(&p)).unwrap().remove(0)
}

fn constats() -> Vec<Finding> {
    rules::analyze(&journal(), None, None).expect("rapport").findings
}

#[test]
fn chaque_jauge_posee_a_des_reperes_a_dessiner() {
    let f = constats();
    let avec: Vec<&Finding> = f.iter().filter(|x| x.gauge.is_some()).collect();
    assert!(!avec.is_empty(), "aucun constat de ce log ne porte de jauge");
    for x in avec {
        let g = x.gauge.as_ref().unwrap();
        let m = g
            .marks()
            .unwrap_or_else(|| panic!("{} : jauge sans repères ({g:?})", x.key));
        for p in [m.value, m.threshold, m.band_start, m.band_width] {
            assert!((0.0..=1.0).contains(&p), "{} : repère hors de la jauge ({m:?})", x.key);
        }
    }
}

#[test]
fn les_constats_de_bruit_et_de_retard_ont_leur_jauge_au_nombre_du_texte() {
    let f = constats();
    // clés présentes dans le rapport de ce log et dont la règle compare une seule mesure à un seul repère
    for cle in ["gyro_noise_level", "motor_noise_left", "gyro_delay", "dterm_delay"] {
        let x = f
            .iter()
            .find(|x| x.key == cle)
            .unwrap_or_else(|| panic!("{cle} absent du rapport"));
        let g = x.gauge.as_ref().unwrap_or_else(|| panic!("{cle} : pas de jauge"));
        let nombre = fixed(g.value, g.decimals, false);
        assert!(
            x.detail.contains(&nombre),
            "{cle} : la jauge dit {nombre}, le texte dit « {} »",
            x.detail
        );
        assert!(g.marks().is_some(), "{cle} : jauge sans repères");
    }
}

#[test]
fn le_terme_d_bruite_a_sa_jauge_quand_un_seul_repere_est_depasse() {
    let f = constats();
    let x = f.iter().find(|x| x.key == "dterm_noisy").expect("dterm_noisy absent");
    let g = x
        .gauge
        .as_ref()
        .expect("un seul repère dépassé : la jauge doit exister");
    assert!(
        g.value > g.threshold,
        "le constat se déclenche : la mesure dépasse le repère"
    );
    assert!(x.detail.contains(&fixed(g.value, g.decimals, false)), "{}", x.detail);
}

#[test]
fn la_strategie_de_filtrage_n_a_pas_de_jauge() {
    let f = constats();
    let x = f
        .iter()
        .find(|x| x.key == "filter_strategy")
        .expect("filter_strategy absent");
    assert!(x.gauge.is_none());
}

#[test]
fn la_jauge_place_mesure_et_seuil_et_borne_ce_qui_depasse() {
    use otb_advisor::rules::Gauge;
    // prototype : 13,8 ms contre une cible de 10 ms, échelle 0 à 20 ms → 69 % et 50 %, bande entre les deux
    let m = Gauge::new(13.8, 10.0, "ms", 1).marks().unwrap();
    assert!(
        (m.value - 0.69).abs() < 1e-12 && (m.threshold - 0.5).abs() < 1e-12,
        "{m:?}"
    );
    assert!(
        (m.band_start - 0.5).abs() < 1e-12 && (m.band_width - 0.19).abs() < 1e-12,
        "{m:?}"
    );
    // mesure sous le seuil : la bande part de la mesure
    let m = Gauge::new(4.0, 10.0, "ms", 1).marks().unwrap();
    assert!(
        (m.band_start - 0.2).abs() < 1e-12 && (m.band_width - 0.3).abs() < 1e-12,
        "{m:?}"
    );
    // hors d'échelle : bornée au bord, jamais dessinée hors de la jauge
    let m = Gauge::new(55.0, 10.0, "ms", 1).marks().unwrap();
    assert_eq!(m.value, 1.0);
    assert_eq!(Gauge::new(-3.0, 10.0, "ms", 1).marks().unwrap().value, 0.0);
    // décibels : le seuil au milieu de son échelle, même négatif
    let m = Gauge::db(-10.2, -13.1, 1).marks().unwrap();
    assert!((m.threshold - 0.5).abs() < 1e-12, "{m:?}");
    assert!(m.value > 0.5);
    // rien à dessiner : valeur non finie ou échelle vide
    assert!(Gauge::new(f64::NAN, 10.0, "ms", 1).marks().is_none());
    assert!(Gauge::new(1.0, 10.0, "ms", 1).range(5.0, 5.0).marks().is_none());
    // textes à la française, signe moins typographique
    assert_eq!(Gauge::new(13.8, 10.0, "ms", 1).value_text(), "13,8 ms");
    assert_eq!(Gauge::db(-10.25, -13.0, 1).threshold_text(), "−13,0 dB");
    assert_eq!(Gauge::new(1.234, 1.0, "", 2).value_text(), "1,23");
}
