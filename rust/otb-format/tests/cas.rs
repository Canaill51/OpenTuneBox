//! Chaque cas vient de Python (`scripts/format_cases.py`) : le texte rendu doit être identique au caractère près.
//! `cas.tsv` (versionné, ≈ 4 000 valeurs) tourne en CI ; `OTB_FORMAT_CASES=fichier.tsv` lance un fichier plus gros (10⁶ cas).

use otb_format::{fixed, general, percent, repr, round_digits, round_half_even};

fn rendu(spec: &str, x: f64) -> String {
    match spec {
        ".0f" => fixed(x, 0, false),
        ".1f" => fixed(x, 1, false),
        ".2f" => fixed(x, 2, false),
        ".3f" => fixed(x, 3, false),
        "+.0f" => fixed(x, 0, true),
        "+.3f" => fixed(x, 3, true),
        ".0%" => percent(x, 0),
        "g" => general(x, 6),
        ".3g" => general(x, 3),
        "repr" => repr(x),
        "round0" => round_half_even(x).map_or("ERR".into(), |n| n.to_string()),
        "round2" => repr(round_digits(x, 2)),
        autre => panic!("spécification inconnue : {autre}"),
    }
}

#[test]
fn identique_a_python() {
    let path =
        std::env::var("OTB_FORMAT_CASES").unwrap_or_else(|_| format!("{}/tests/cas.tsv", env!("CARGO_MANIFEST_DIR")));
    let texte = std::fs::read_to_string(&path).unwrap();
    let (mut total, mut ecarts) = (0usize, Vec::new());
    for ligne in texte.lines() {
        let mut champs = ligne.split('\t');
        let (spec, bits, attendu) = (champs.next().unwrap(), champs.next().unwrap(), champs.next().unwrap());
        let x = f64::from_bits(u64::from_str_radix(bits, 16).unwrap());
        if spec == "round0" && x.is_finite() && x.abs() >= 9.2e18 {
            continue; // entier Python sans limite : hors de `i64`, assumé (`round_half_even`)
        }
        let obtenu = rendu(spec, x);
        total += 1;
        if obtenu != attendu && ecarts.len() < 15 {
            ecarts.push(format!(
                "{spec} {x:e} ({bits}) : Python « {attendu} », Rust « {obtenu} »"
            ));
        }
    }
    assert!(total > 1000, "trop peu de cas : {total}");
    assert!(
        ecarts.is_empty(),
        "{total} cas, écarts (15 premiers) :\n{}",
        ecarts.join("\n")
    );
}
