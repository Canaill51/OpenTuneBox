//! `advisor/profiles.py` porté en Rust : cas écrits par `scripts/profiles_cases.py` (régénérer `tests/data/profiles.json` avec ce
//! script si `profiles.py` change). Tout est exigé **identique** : seuils, clés de profil, textes d'explication, erreurs.

use otb_advisor::profiles::{self, detect_profile_from, filter_delay_target_ms, profile, profile_key};
use serde_json::Value;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

fn pair(v: &Value) -> (f64, f64) {
    let a = hexes(v);
    (a[0], a[1])
}

#[test]
fn les_cinq_profils_ont_les_seuils_de_python() {
    let c = cas("profiles");
    let want = c["profiles"].as_object().unwrap();
    assert_eq!(want.len(), profiles::PROFILES.len());
    for (key, w) in want {
        let p = profile(key).unwrap_or_else(|| panic!("profil {key} absent"));
        assert_eq!(p.key, w["key"].as_str().unwrap());
        assert_eq!(p.label, w["label"].as_str().unwrap(), "{key} : libellé");
        for (name, got) in [
            ("hover_hz", p.hover_hz),
            ("resonance_hz", p.resonance_hz),
            ("latency_ms", p.latency_ms),
        ] {
            let (a, b) = pair(&w[name]);
            assert!(
                same(got.0, a) && same(got.1, b),
                "{key}.{name} : Rust {got:?} / Python ({a}, {b})"
            );
        }
        for (name, got) in [
            ("gyro_delay_max_ms", p.gyro_delay_max_ms),
            ("dterm_delay_max_ms", p.dterm_delay_max_ms),
            ("gyro_noise_high_db", p.gyro_noise_high_db),
            ("dterm_noise_high_db", p.dterm_noise_high_db),
            ("low_band_high_db", p.low_band_high_db),
            ("motor_left_high_db", p.motor_left_high_db),
            ("imbalance_high_pct", p.imbalance_high_pct),
            ("iterm_offset_high", p.iterm_offset_high),
            ("saturation_high_pct", p.saturation_high_pct),
            ("roll_pitch_gap_high", p.roll_pitch_gap_high),
            ("resonance_high_db", p.resonance_high_db),
        ] {
            assert!(
                same(got, f1(&w[name])),
                "{key}.{name} : Rust {got} / Python {}",
                f1(&w[name])
            );
        }
        assert_eq!(
            p.provisional,
            w["provisional"].as_bool().unwrap(),
            "{key} : provisional"
        );
        match p.nominal_inch {
            None => assert!(w["nominal_inch"].is_null(), "{key} : nominal_inch"),
            Some(x) => assert!(same(x, f1(&w["nominal_inch"])), "{key} : nominal_inch"),
        }
    }
}

#[test]
fn objectifs_de_retard() {
    let c = cas("profiles");
    for k in c["delay_target"].as_array().unwrap() {
        let inch = (!k["inch"].is_null()).then(|| f1(&k["inch"]));
        let got = filter_delay_target_ms(inch);
        if is_error(k) {
            panic!("Python lève pour {inch:?} : cas non prévu");
        }
        match (&k["out"], got) {
            (Value::Null, None) => {}
            (w, Some((g, d))) => {
                let (wg, wd) = pair(w);
                assert!(
                    same(g, wg) && same(d, wd),
                    "{inch:?} : Rust ({g}, {d}) / Python ({wg}, {wd})"
                );
            }
            (w, g) => panic!("{inch:?} : Rust {g:?} / Python {w}"),
        }
    }
}

#[test]
fn cle_de_profil_a_toutes_les_frontieres() {
    let c = cas("profiles");
    for k in c["profile_key"].as_array().unwrap() {
        let hz = f1(&k["hz"]);
        let cells = k["cells"].as_i64();
        assert!(!is_error(k));
        assert_eq!(
            profile_key(hz, cells),
            k["out"].as_str().unwrap(),
            "{hz} Hz, cellules {cells:?}"
        );
    }
}

#[test]
fn detection_du_profil_et_texte_d_explication() {
    let c = cas("profiles");
    let mut n = 0;
    for k in c["detect"].as_array().unwrap() {
        let hz = (!k["hz"].is_null()).then(|| f1(&k["hz"]));
        let cells = k["cells"].as_i64();
        let name = k["name"].as_str().unwrap();
        assert!(
            !is_error(k),
            "Python lève pour {hz:?} {cells:?} {name:?} : cas non prévu"
        );
        let (p, why) = detect_profile_from(hz, cells, name);
        let want = &k["out"];
        assert_eq!(p.key, want["key"].as_str().unwrap(), "{hz:?} {cells:?} {name:?}");
        assert_eq!(why, want["reason"].as_str().unwrap(), "{hz:?} {cells:?} {name:?}");
        n += 1;
    }
    println!("{n} détections identiques");
}

/// Les suites de Python sont triées : recherche par dichotomie (un balayage linéaire de 1,1 M de points prendrait 30 s en debug).
fn in_ranges(r: &[Value], c: u32) -> bool {
    let i = r.partition_point(|x| (x[1].as_u64().unwrap() as u32) < c);
    r.get(i).is_some_and(|x| x[0].as_u64().unwrap() as u32 <= c)
}

/// Les classes `\d`, `\s`, `\w` de Python : exactes sur les 1 114 112 points de code.
#[test]
fn classes_de_caracteres_de_python() {
    let c = cas("profiles");
    let digits = c["digits"].as_array().unwrap();
    let spaces: Vec<u32> = c["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as u32)
        .collect();
    let words = c["words"].as_array().unwrap();
    for code in 0..0x110000u32 {
        let Some(ch) = char::from_u32(code) else { continue };
        let is_digit = in_ranges(digits, code);
        assert_eq!(profiles::digit_value(ch).is_some(), is_digit, "\\d : U+{code:04X}");
        if is_digit {
            // la valeur est la même que celle de `float()` : chiffre à la position (c − zéro) de sa suite de dix
            let v = profiles::digit_value(ch).unwrap();
            assert!(v < 10);
        }
        assert_eq!(
            profiles::is_space(ch),
            spaces.binary_search(&code).is_ok(),
            "\\s : U+{code:04X}"
        );
        assert_eq!(profiles::is_word(ch), in_ranges(words, code), "\\w : U+{code:04X}");
    }
}

#[test]
fn valeur_des_chiffres_unicode() {
    for (zero, n) in [
        (0x30u32, 10),
        (0x660, 10),
        (0x1D7CE, 50),
        (0x116D0, 20),
        (0xFF10, 10),
        (0x1E5F1, 10),
    ] {
        for i in 0..n {
            let ch = char::from_u32(zero + i).unwrap();
            assert_eq!(profiles::digit_value(ch), Some(i % 10), "U+{:04X}", zero + i);
        }
    }
    assert_eq!(profiles::digit_value('a'), None);
    assert_eq!(profiles::digit_value('²'), None);
}

/// `str.lower()` de Python : mêmes minuscules que `to_lowercase` de Rust pour tout caractère non ASCII qui change.
#[test]
fn minuscules_comme_python() {
    let c = cas("profiles");
    let mut diff = vec![];
    for (code, lower) in c["lower"].as_object().unwrap() {
        let ch = char::from_u32(code.parse().unwrap()).unwrap();
        if ch.to_lowercase().collect::<String>() != lower.as_str().unwrap() {
            diff.push(format!("U+{:04X}", ch as u32));
        }
    }
    println!("{} minuscules différentes : {diff:?}", diff.len());
    assert!(diff.is_empty(), "minuscules différentes de Python : {diff:?}");
}
