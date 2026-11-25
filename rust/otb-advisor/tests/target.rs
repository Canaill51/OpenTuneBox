//! Fiche cible (lot 31b, D-072) : les valeurs proposées sont les seuils de l'assistant pour la taille, et chaque case
//! vise une grandeur que la bibliothèque suit.

use otb_advisor::profiles::PROFILES;
use otb_advisor::rules::{PHASE_MARGIN_LOW_DEG, STEP_PEAK_HIGH};
use otb_advisor::target::{Bound, ROWS, band, charted, parse, proposed_values, shown};
use otb_advisor::tracked::{AXIS_KEYS, DTERM_NOISE_DB, LATENCY_MS, PEAK, PHASE_MARGIN_DEG, RISE_MS, WHOLE_KEYS};

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
        ["22", "", "1,15", "", "35", "", "-28,3", "-13,1", "1,90", "2,70"]
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

#[test]
fn un_graphique_par_grandeur_le_pic_une_fois() {
    let g = charted();
    assert_eq!(g.len(), 9, "{g:?}");
    assert_eq!(g.iter().filter(|k| **k == PEAK).count(), 1);
    assert_eq!(g[0], LATENCY_MS, "ordre des cases");
    for r in &ROWS {
        assert!(g.contains(&r.tracked), "{} sans graphique", r.key);
    }
}

#[test]
fn bande_lue_dans_les_cases() {
    let mut t = [None; 10];
    t[index("latency_ms")] = Some(14.0);
    t[index("peak_low")] = Some(1.05);
    t[index("peak_high")] = Some(1.2);
    t[index("phase_margin_deg")] = Some(35.0);
    t[index("dterm_noise_db")] = Some(-10.0);
    assert_eq!(band(LATENCY_MS, &t), (None, Some(14.0)), "« ≤ » donne le haut");
    assert_eq!(band(PHASE_MARGIN_DEG, &t), (Some(35.0), None), "« ≥ » donne le bas");
    assert_eq!(band(PEAK, &t), (Some(1.05), Some(1.2)), "le pic a ses deux bords");
    assert_eq!(band(DTERM_NOISE_DB, &t), (None, Some(-10.0)));
    assert_eq!(band(RISE_MS, &t), (None, None), "case vide : pas de bande");
    // un bord vide laisse l'autre
    t[index("peak_low")] = None;
    assert_eq!(band(PEAK, &t), (None, Some(1.2)));
}
