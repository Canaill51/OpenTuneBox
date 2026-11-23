//! Fiche cible (lot 31b, D-072) : les valeurs proposées sont les seuils de l'assistant pour la taille, et chaque case
//! vise une grandeur que la bibliothèque suit.

use otb_advisor::profiles::PROFILES;
use otb_advisor::rules::{PHASE_MARGIN_LOW_DEG, STEP_PEAK_HIGH};
use otb_advisor::target::{Bound, ROWS, parse, proposed_values, shown};
use otb_advisor::tracked::{AXIS_KEYS, WHOLE_KEYS};

fn index(key: &str) -> usize {
    ROWS.iter().position(|r| r.key == key).unwrap()
}

#[test]
fn proposees_egales_aux_seuils_de_chaque_profil() {
    for p in &PROFILES {
        let v = proposed_values(p.key);
        let attendu = [
            ("latency_ms", Some(p.latency_ms.1)),
            ("peak_low", None),
            ("peak_high", Some(STEP_PEAK_HIGH.0)),
            ("rise_ms", None),
            ("phase_margin_deg", Some(PHASE_MARGIN_LOW_DEG)),
            ("gain_margin_db", None),
            ("gyro_noise_db", Some(p.gyro_noise_high_db)),
            ("dterm_noise_db", Some(p.dterm_noise_high_db)),
            ("gyro_filter_delay_ms", Some(p.gyro_delay_max_ms)),
            ("dterm_filter_delay_ms", Some(p.dterm_delay_max_ms)),
        ];
        for (key, x) in attendu {
            assert_eq!(v[index(key)], x, "profil {}, case {key}", p.key);
        }
    }
}

/// Les chiffres d'un 5″ écrits en clair : un seuil échangé avec son voisin (gyro et D, latence basse et haute) se
/// verrait ici même si la table ci-dessus avait été recopiée de la même erreur.
#[test]
fn un_cinq_pouces_en_clair() {
    let texte: Vec<String> = ROWS
        .iter()
        .zip(proposed_values("five"))
        .map(|(r, x)| x.map_or(String::new(), |x| shown(r, x)))
        .collect();
    assert_eq!(
        texte,
        ["22", "", "1,20", "", "35", "", "-28,3", "-13,1", "1,90", "2,70"]
    );
}

#[test]
fn profil_inconnu_rien_de_propose() {
    assert_eq!(proposed_values("inconnu"), [None; 10]);
}

#[test]
fn chaque_case_vise_une_grandeur_suivie() {
    for r in &ROWS {
        assert!(
            AXIS_KEYS.contains(&r.tracked) || WHOLE_KEYS.contains(&r.tracked),
            "{} → {}",
            r.key,
            r.tracked
        );
    }
    let mut cles: Vec<&str> = ROWS.iter().map(|r| r.key).collect();
    cles.sort_unstable();
    cles.dedup();
    assert_eq!(cles.len(), ROWS.len(), "clés en double");
    // les bords de la zone du pic se font face
    assert_eq!(ROWS[index("peak_low")].bound, Bound::AtLeast);
    assert_eq!(ROWS[index("peak_high")].bound, Bound::AtMost);
}

#[test]
fn lecture_des_cases() {
    assert_eq!(parse(""), Ok(None));
    assert_eq!(parse("  "), Ok(None));
    assert_eq!(parse("1,15"), Ok(Some(1.15)));
    assert_eq!(parse(" 1.15 "), Ok(Some(1.15)));
    assert_eq!(parse("-30"), Ok(Some(-30.0)));
    assert!(parse("abc").is_err());
    assert!(parse("1,2,3").is_err());
    assert!(parse("inf").is_err());
    assert!(parse("NaN").is_err());
}
