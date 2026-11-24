//! Briques de Betaflight contre le firmware compilé sur PC, **au bit près** (lot 24b du plan, D-062).
//!
//! Les vecteurs de `tests/data/firmware/betaflight/` sont écrits par le banc `otb-dev firmware-bench`, qui télécharge chaque
//! version du firmware, la compile hors du dépôt et lui fait jouer les cas. Ce test rejoue les mêmes cas avec les briques Rust
//! (`otb_dsp::firmware::betaflight::cases::run`) et exige les mêmes bits : la CI le lance sans le corpus ni le firmware (L-023).
//! Les briques ne passent que par `+ − × ÷`, `sqrt`, `fmod` et `round` : exactes sur toute machine (L-048).

use otb_dsp::firmware::betaflight::{Arith, cases};
use serde_json::Value;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/firmware/betaflight")
}

fn read_f32(name: &str) -> Vec<f32> {
    let bytes = std::fs::read(dir().join(name)).unwrap_or_else(|e| panic!("{name} : {e}"));
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

fn manifest() -> Value {
    let text = std::fs::read_to_string(dir().join("manifest.json")).expect("manifest.json");
    serde_json::from_str(&text).expect("manifest.json : JSON invalide")
}

fn as_usize(v: &Value) -> usize {
    v.as_u64().expect("entier attendu") as usize
}

#[test]
fn les_briques_rendent_les_bits_du_firmware() {
    let m = manifest();
    let blob = read_f32(m["inputs_file"].as_str().expect("inputs_file"));
    let input = |name: &str| -> &[f32] {
        let e = &m["inputs"][name];
        let (o, l) = (as_usize(&e["offset"]), as_usize(&e["len"]));
        &blob[o..o + l]
    };
    let cases_by_id: Vec<&Value> = m["cases"].as_array().expect("cases").iter().collect();
    let versions = m["versions"].as_array().expect("versions");
    assert!(versions.len() >= 4, "une version au moins par arithmétique");

    let mut failures = Vec::new();
    let mut seen_arith = Vec::new();
    for v in versions {
        let name = v["version"].as_str().expect("version");
        let hv = v["header_version"].as_array().expect("header_version");
        let arith = Arith::for_version(hv[0].as_u64().unwrap() as u32, hv[1].as_u64().unwrap() as u32)
            .unwrap_or_else(|| panic!("{name} : aucune arithmétique"));
        // La version lue dans l'en-tête d'un log mène à l'arithmétique avec laquelle le banc a joué cette version.
        assert_eq!(
            format!("{arith:?}"),
            v["arith"].as_str().unwrap(),
            "{name} : arithmétique"
        );
        seen_arith.push(arith);
        let out = read_f32(v["outputs"].as_str().expect("outputs"));
        let played = v["cases"].as_array().expect("cases");
        // Chaque type de cas de la famille est joué au moins une fois.
        for kind in cases::kinds(arith) {
            assert!(
                played.iter().any(|p| {
                    let id = p["case"].as_str().unwrap();
                    cases_by_id.iter().any(|c| c["id"] == id && c["kind"] == *kind)
                }),
                "{name} : aucun cas « {kind} »"
            );
        }
        for p in played {
            let id = p["case"].as_str().unwrap();
            let c = cases_by_id
                .iter()
                .find(|c| c["id"] == id)
                .unwrap_or_else(|| panic!("cas {id} absent"));
            let ins: Vec<&[f32]> = c["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| input(i.as_str().unwrap()))
                .collect();
            let params = |n: &str| c["params"][n].as_f64();
            let got = cases::run(arith, c["kind"].as_str().unwrap(), &params, &ins).unwrap();
            let (o, l) = (as_usize(&p["offset"]), as_usize(&p["len"]));
            let want = &out[o..o + l];
            if got.len() != l {
                failures.push(format!("{name} / {id} : {} valeurs au lieu de {l}", got.len()));
            } else if let Some(i) = (0..l).find(|&i| got[i].to_bits() != want[i].to_bits()) {
                let n = (0..l).filter(|&i| got[i].to_bits() != want[i].to_bits()).count();
                failures.push(format!(
                    "{name} / {id} : {n} valeurs sur {l} diffèrent, la première à l'indice {i} ({:e} au lieu de {:e})",
                    got[i], want[i]
                ));
            }
        }
    }
    for a in Arith::ALL {
        assert!(seen_arith.contains(&a), "{a:?} : aucune version jouée par le banc");
    }
    assert!(
        failures.is_empty(),
        "différences avec le firmware :\n{}",
        failures.join("\n")
    );
}

#[test]
fn version_de_l_en_tete_vers_arithmetique() {
    assert_eq!(Arith::for_version(4, 2), None);
    assert_eq!(Arith::for_version(4, 3), Some(Arith::V4_3));
    assert_eq!(Arith::for_version(4, 4), Some(Arith::V4_3));
    assert_eq!(Arith::for_version(4, 5), Some(Arith::V4_5));
    assert_eq!(Arith::for_version(2025, 12), Some(Arith::V2025_12));
    assert_eq!(Arith::for_version(2026, 6), Some(Arith::V2026_6));
    assert_eq!(Arith::for_version(2026, 12), Some(Arith::V2026_6));
}
