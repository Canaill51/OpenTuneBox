//! `advisor/drone_card.py` porté en Rust : cas écrits par `scripts/drone_card_cases.py` (régénérer `tests/data/drone_card.json` avec ce
//! script si `drone_card.py` change). Tout est exigé **identique** : valeurs conseillées au bit près, textes, séries, JSON de la fiche.

use otb_advisor::drone_card::*;
use serde_json::{Value, json};

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

fn hexes_of(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(f1).collect()
}

fn assert_same(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : {got:?} / {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(same(*g, *w), "{what} : Rust {got:?} / Python {want:?}");
    }
}

#[test]
fn les_tables_sont_celles_de_python() {
    let c = &cas("drone_card")["constants"];
    let pairs = |v: &Value| -> Vec<(String, String)> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_str().unwrap().into(), p[1].as_str().unwrap().into()))
            .collect()
    };
    let own = |a: &[(&str, &str)]| -> Vec<(String, String)> {
        a.iter().map(|(x, y)| (x.to_string(), y.to_string())).collect()
    };
    assert_eq!(own(&BF_FAMILIES), pairs(&c["bf"]));
    assert_eq!(own(&ESCS), pairs(&c["escs"]));
    let rates: Vec<(f64, String)> = c["pid_rates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (f1(&p[0]), p[1].as_str().unwrap().into()))
        .collect();
    assert_eq!(rates.len(), PID_RATES_KHZ.len());
    for (r, (a, b)) in rates.iter().zip(PID_RATES_KHZ) {
        assert!(same(r.0, a) && r.1 == b);
    }
    let sorted = |a: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = a.iter().map(|s| s.to_string()).collect();
        v.sort();
        v
    };
    let strs = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().into())
            .collect()
    };
    assert_eq!(sorted(&BIDIR_ESCS), strs(&c["bidir"]));
    assert_eq!(sorted(&LARGE_SIZES), strs(&c["large"]));
    assert_same(
        &DAMPING_SERIES_SMALL,
        &hexes_of(&c["damping"]["small"]),
        "amortissement small",
    );
    assert_same(
        &DAMPING_SERIES_LARGE,
        &hexes_of(&c["damping"]["large"]),
        "amortissement large",
    );
    assert_same(&MASTER_SERIES, &hexes_of(&c["master"]), "master");
    assert_same(&FF_SERIES, &hexes_of(&c["ff"]), "ff");
    assert_eq!(PITCH_SERIES_LEN as i64, c["pitch_len"].as_i64().unwrap());
    let d = &c["defaults"];
    assert_eq!(
        (
            RPM_DEFAULTS.harmonics,
            RPM_DEFAULTS.q,
            RPM_DEFAULTS.min_hz,
            RPM_DEFAULTS.fade_hz
        ),
        (
            d["rpm"]["harmonics"].as_i64().unwrap(),
            d["rpm"]["q"].as_i64().unwrap(),
            d["rpm"]["min_hz"].as_i64().unwrap(),
            d["rpm"]["fade_hz"].as_i64().unwrap()
        )
    );
    assert_eq!(
        (
            DYN_NOTCH_DEFAULTS.count,
            DYN_NOTCH_DEFAULTS.q,
            DYN_NOTCH_DEFAULTS.min_hz,
            DYN_NOTCH_DEFAULTS.max_hz
        ),
        (
            d["dyn_notch"]["count"].as_i64().unwrap(),
            d["dyn_notch"]["q"].as_i64().unwrap(),
            d["dyn_notch"]["min_hz"].as_i64().unwrap(),
            d["dyn_notch"]["max_hz"].as_i64().unwrap()
        )
    );
    let ir = &c["i_range"];
    assert_same(&[I_RANGE_SMALL.0, I_RANGE_SMALL.1], &hexes_of(&ir["small"]), "I small");
    assert_same(&[I_RANGE_LARGE.0, I_RANGE_LARGE.1], &hexes_of(&ir["large"]), "I large");
}

#[test]
fn les_fiches_donnent_les_memes_valeurs_conseillees() {
    let c = cas("drone_card");
    let cards = c["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 1170);
    for w in cards {
        let mut d = w["card"].clone();
        let khz = f1(&d["pid_khz"]);
        d["pid_khz"] = json!(khz);
        let card = DroneCard::from_value(&d).unwrap();
        let what = format!("{d}");
        assert_eq!(card.to_value(), d, "to_dict : {what}");
        assert_eq!(card.bidir(), w["bidir"].as_bool().unwrap(), "{what}");
        let (hz, cli) = card.log_rate();
        assert!(
            same(hz, f1(&w["log_rate"][0])) && cli == w["log_rate"][1].as_str().unwrap(),
            "log_rate {what} : {hz} {cli}"
        );
        let (hz, cli) = card.pid_log_rate();
        assert!(
            same(hz, f1(&w["pid_log_rate"][0])) && cli == w["pid_log_rate"][1].as_str().unwrap(),
            "pid_log_rate {what}"
        );
        assert_eq!(card.size_group(), w["group"].as_str().unwrap(), "{what}");
        assert_same(
            card.damping_series(),
            &hexes_of(&w["damping"]),
            &format!("damping {what}"),
        );
        let (a, b) = card.i_range();
        assert_same(&[a, b], &hexes_of(&w["i_range"]), &format!("i_range {what}"));
        assert_eq!(card.debug_mode(), w["debug"].as_str().unwrap(), "{what}");
        assert_eq!(card.angle_limit_setting(), w["angle"].as_str().unwrap(), "{what}");
    }
}

/// Python ne vérifie pas les types de `from_dict` (`name` = 3 passe). Rust : un `name` qui n'est pas un texte reste refusé ; les
/// autres types faux reprennent le défaut de la clé (tolérance, W-029 point 1). Motifs écrits dans l'ordre du fichier
/// (`preserve_order`).
const REFUSEES_PAR_RUST: [&str; 1] = [r#"{"name":3}"#];
const TOLEREES_PAR_RUST: [&str; 3] = [
    r#"{"name":"A","pid_khz":"4"}"#,
    r#"{"name":"A","done":[1]}"#,
    r#"{"name":"A","bf":null}"#,
];

#[test]
fn from_dict_lit_comme_python() {
    let c = cas("drone_card");
    let list = c["from_dict"].as_array().unwrap();
    assert_eq!(list.len(), 17);
    let (mut ok, mut refusees, mut tolerees) = (0, 0, 0);
    for w in list {
        let input = &w["in"];
        let got = DroneCard::from_value(input);
        if w["error"].as_bool().unwrap() {
            assert!(got.is_err(), "Python lève, Rust accepte : {input}");
            continue;
        }
        let txt = input.to_string();
        if REFUSEES_PAR_RUST.contains(&txt.as_str()) {
            assert!(got.is_err(), "type refusé attendu : {input}");
            refusees += 1;
            continue;
        }
        if TOLEREES_PAR_RUST.contains(&txt.as_str()) {
            assert_eq!(
                got.unwrap().to_value(),
                DroneCard::new("A").to_value(),
                "défaut attendu : {input}"
            );
            tolerees += 1;
            continue;
        }
        let mut want = w["out"].clone();
        want["pid_khz"] = json!(f1(&want["pid_khz"]));
        assert_eq!(got.unwrap().to_value(), want, "{input}");
        ok += 1;
    }
    assert_eq!((ok, refusees, tolerees), (11, 1, 3));
}

#[test]
fn chaque_type_faux_reprend_le_defaut_de_sa_cle() {
    let defaut = DroneCard::new("A");
    for (cle, faux) in [
        ("bf", json!(4.5)),
        ("size", json!(["five"])),
        ("esc", json!(null)),
        ("pid_khz", json!("3.2")),
        ("pid_khz", json!(null)),
        ("pid_khz", json!([4])),
        ("done", json!(true)),
        ("done", json!("oui")),
        ("done", json!(null)),
    ] {
        let mut d = json!({"name": "A"});
        d[cle] = faux.clone();
        let card = DroneCard::from_value(&d).unwrap_or_else(|e| panic!("{cle}={faux} refusé : {e}"));
        assert_eq!(card, defaut, "{cle}={faux}");
    }
    // la fiche qui n'est pas un objet, et le nom absent ou faux, restent des erreurs
    for d in [
        json!([]),
        json!("A"),
        json!(null),
        json!({}),
        json!({"name": null}),
        json!({"name": ["A"]}),
    ] {
        assert!(DroneCard::from_value(&d).is_err(), "{d}");
    }
}

#[test]
fn une_case_vaut_sa_valeur_de_verite_comme_en_python() {
    let d = json!({"name": "A", "done": {"a": 1, "b": 0, "c": "", "d": "x", "e": null, "f": [], "g": [0], "h": {}, "i": true}});
    let card = DroneCard::from_value(&d).unwrap();
    let got: Vec<(&str, bool)> = card.done.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        got,
        [
            ("a", true),
            ("b", false),
            ("c", false),
            ("d", true),
            ("e", false),
            ("f", false),
            ("g", true),
            ("h", false),
            ("i", true)
        ]
    );
}

#[test]
fn pid_khz_entier_est_reecrit_entier_et_le_flottant_reste_flottant() {
    let texte = |d: &Value| DroneCard::from_value(d).unwrap().to_value()["pid_khz"].to_string();
    assert_eq!(texte(&json!({"name": "A", "pid_khz": 4})), "4");
    assert_eq!(texte(&json!({"name": "A", "pid_khz": 8})), "8");
    assert_eq!(texte(&json!({"name": "A", "pid_khz": 4.0})), "4.0");
    assert_eq!(texte(&json!({"name": "A", "pid_khz": 3.2})), "3.2");
    assert_eq!(texte(&json!({"name": "A"})), "4.0");
    // une fiche faite à la main (fenêtre de saisie) reste en flottant
    let mut c = DroneCard::new("A");
    c.pid_khz = 2.0;
    assert_eq!(c.to_value()["pid_khz"].to_string(), "2.0");
}

#[test]
fn les_cases_cochees_gardent_l_ordre_du_fichier() {
    let d = json!({"name": "A", "done": {"b": true, "a": false, "c": true}});
    let card = DroneCard::from_value(&d).unwrap();
    let keys: Vec<&str> = card.done.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["b", "a", "c"]);
    // et la réécriture garde cet ordre (le dictionnaire de Python)
    let back: Vec<String> = card.to_value()["done"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(back, ["b", "a", "c"]);
}

#[test]
fn card_for_log_ignore_casse_et_blancs() {
    let c = &cas("drone_card")["card_for_log"];
    let cards: Vec<DroneCard> = c["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| DroneCard::new(n.as_str().unwrap()))
        .collect();
    let mut trouvees = 0;
    for q in c["queries"].as_array().unwrap() {
        let name = q["craft"].as_str().unwrap();
        let got = card_for_log(&cards, name).map(|r| cards.iter().position(|x| std::ptr::eq(x, r)).unwrap());
        let want = q["found"].as_i64().map(|i| i as usize);
        assert_eq!(got, want, "nom {name:?}");
        trouvees += want.is_some() as usize;
    }
    assert!(trouvees >= 10, "trop peu de noms trouvés : {trouvees}");
}

#[test]
fn pitch_series_donne_les_memes_valeurs() {
    let c = cas("drone_card");
    let list = c["pitch"].as_array().unwrap();
    assert_eq!(list.len(), 700);
    let mut non_vides = 0;
    for w in list {
        let (cur, step) = (f1(&w["cur"]), f1(&w["step"]));
        let dir = w["dir"].as_i64().unwrap();
        let got = pitch_series(cur, dir, step);
        let want = hexes_of(&w["out"]);
        assert_same(&got, &want, &format!("pitch_series({cur}, {dir}, {step})"));
        non_vides += !want.is_empty() as usize;
    }
    assert!(non_vides > 300, "trop peu de séries non vides : {non_vides}");
}

#[test]
fn la_fiche_se_preremplit_d_apres_le_log() {
    // (nom, famille, boucle PID Hz, ESC 32 ?, taille) → (nom, bf, kHz, ESC)
    let cas = [
        ("JeNo", "4.4", 8000.0, true, "five", "JeNo", "4.4", 8.0, "BLHeli_32"),
        ("", "4.5", 3200.0, false, "whoop", "Mon drone", "4.5", 3.2, "BLHeli_S"),
        ("X", "2026.6", 4000.0, true, "seven", "X", "2025.12", 4.0, "BLHeli_32"),
        ("X", "4.3", 2000.0, false, "five", "X", "2025.12", 2.0, "BLHeli_S"),
        // ex æquo entre 4 kHz et 3,2 kHz : 3 600 Hz est à 400 et 400 → la première entrée de la table (4 kHz)
        ("X", "4.5", 3600.0, true, "five", "X", "4.5", 4.0, "BLHeli_32"),
        // hors table : la plus proche
        ("X", "4.5", 12000.0, true, "five", "X", "4.5", 8.0, "BLHeli_32"),
        ("X", "4.5", 500.0, true, "five", "X", "4.5", 1.6, "BLHeli_32"),
    ];
    for (name, fam, hz, esc32, size, want_name, want_bf, want_khz, want_esc) in cas {
        let c = card_from_parts(name, fam, hz, esc32, size);
        assert_eq!(
            (
                c.name.as_str(),
                c.bf.as_str(),
                c.pid_khz,
                c.esc.as_str(),
                c.size.as_str()
            ),
            (want_name, want_bf, want_khz, want_esc, size),
            "{name} {fam} {hz}"
        );
        assert!(c.done.is_empty() && !c.pid_khz_int);
    }
}

/// Une fiche construite à la main peut porter une taille que `from_value` n'aurait jamais laissée passer : les méthodes
/// qui lisent le profil doivent alors se replier sur `five`, **jamais** faire mourir l'application (W-029 point 3).
#[test]
fn une_taille_inconnue_se_replie_sur_le_defaut_sans_paniquer() {
    let mut card = DroneCard::new("A");
    card.size = "n-importe-quoi".to_string();
    let attendu = DroneCard::new("A");
    assert_eq!(card.log_rate(), attendu.log_rate(), "log_rate");
    assert_eq!(card.pid_log_rate(), attendu.pid_log_rate(), "pid_log_rate");
    assert_eq!(card.i_range(), attendu.i_range(), "i_range");
    assert_eq!(card.damping_series(), attendu.damping_series(), "damping_series");
    // `from_value` continue, lui, à ramener la taille elle-même à `five`
    let lue = DroneCard::from_value(&json!({"name": "A", "size": "n-importe-quoi"})).unwrap();
    assert_eq!(lue.size, DEFAULT_SIZE);
}
