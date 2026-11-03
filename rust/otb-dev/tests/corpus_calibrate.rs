//! `otb-dev corpus calibrate` : le JSON avec `NaN` et `Infinity`, lu et écrit comme Python.

use otb_dev::corpus::json::{as_f64, dump, num, parse};
use serde_json::{Value, json};

#[test]
fn les_valeurs_non_finies_s_ecrivent_en_jetons_nus() {
    let v = json!([num(f64::NAN), num(f64::INFINITY), num(f64::NEG_INFINITY), num(1.5)]);
    assert_eq!(dump(&v).unwrap(), "[NaN,Infinity,-Infinity,1.5]");
}

#[test]
fn un_texte_python_se_relit_valeur_par_valeur() {
    let v = parse(r#"[{"a": NaN, "b": -Infinity, "c": Infinity, "d": 2, "e": null, "f": true}]"#).unwrap();
    let r = &v[0];
    assert!(as_f64(&r["a"]).unwrap().is_nan());
    assert_eq!(as_f64(&r["b"]), Some(f64::NEG_INFINITY));
    assert_eq!(as_f64(&r["c"]), Some(f64::INFINITY));
    assert_eq!(as_f64(&r["d"]), Some(2.0));
    assert_eq!(as_f64(&r["e"]), None);
    assert_eq!(as_f64(&r["f"]), None);
}

#[test]
fn un_texte_qui_contient_nan_reste_un_texte() {
    let text = r#"{"craft": "NaN \"Infinity\" -Infinity", "x": NaN, "y": "a\\"}"#;
    let v = parse(text).unwrap();
    assert_eq!(v["craft"], Value::String("NaN \"Infinity\" -Infinity".into()));
    assert_eq!(v["y"], Value::String("a\\".into()));
    assert!(as_f64(&v["x"]).unwrap().is_nan());
    // et à l'écriture : le nom n'est pas pris pour une valeur non finie
    let out = dump(&v).unwrap();
    assert!(out.contains("\"craft\":\"NaN \\\"Infinity\\\" -Infinity\""), "{out}");
    assert!(out.contains("\"x\":NaN"), "{out}");
}

#[test]
fn ecrire_puis_lire_redonne_les_memes_valeurs() {
    let v = json!({"v": [num(f64::NAN), num(-0.25), num(f64::NEG_INFINITY)], "n": 3, "ok": false, "none": null});
    let back = parse(&dump(&v).unwrap()).unwrap();
    assert_eq!(back, v);
}

#[test]
fn un_json_invalide_est_refuse() {
    assert!(parse("[NaN,").is_err());
    assert!(parse("[Nan]").is_err());
}

// --- `--resume` : le tableau des percentiles, sur des sessions fabriquées -------------------------------------------------------

use otb_dev::corpus::calibrate::resume_text;

fn session(file: &str, craft: &str, extra: Value) -> Value {
    let mut r = json!({"file": file, "sess": 1, "profile": "large", "craft": craft, "hover_hz": 400.0, "cells": 2});
    r.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    r
}

/// Deux sessions du drone X (peak 1) et une du drone Y (peak 2) : 400 Hz sur 2 cellules = whoop, quel que soit le profil enregistré.
fn rows() -> Vec<Value> {
    let flat = |p: f64| json!({"peak_0": p, "peak_1": p});
    vec![
        session("d/a.bbl", "X", flat(1.0)),
        session("d/b.bbl", "X", flat(1.0)),
        session("d/c.bbl", "Y", flat(2.0)),
    ]
}

#[test]
fn le_tableau_donne_les_percentiles_par_session_et_par_drone() {
    let t = resume_text(&rows()).unwrap();
    assert!(t.contains("\n== whoop : 3 sessions, 2 drones   (par session"), "{t}");
    assert!(t.contains("\n== TOUS : 3 sessions, 2 drones"), "{t}");
    assert!(t.contains("\n== five : 0 sessions, 0 drones"), "{t}");
    // 6 valeurs (roll et pitch regroupés) : 1,1,1,1,2,2 ; une voix par drone : médianes 1 et 2
    assert!(
        t.contains("peak         n=   6  1.00 / 1.00 / 2.00  |   2 drones  1.10 / 1.90\n"),
        "{t}"
    );
    // mesure dérivée : écart pitch − roll, une valeur par session
    assert!(
        t.contains("peakdiff     n=   3  0.00 / 0.00 / 0.00  |   2 drones  0.00 / 0.00\n"),
        "{t}"
    );
    assert!(!t.ends_with('\n'), "le programme ajoute le dernier retour à la ligne");
}

#[test]
fn la_taille_est_reclassee_avec_les_limites_actuelles() {
    let mut r = rows();
    r[2]["hover_hz"] = json!(170.0);
    r[2]["cells"] = json!(4);
    let t = resume_text(&r).unwrap();
    assert!(t.contains("\n== whoop : 2 sessions, 1 drones"), "{t}");
    assert!(t.contains("\n== five : 1 sessions, 1 drones"), "{t}");
}

#[test]
fn les_retards_des_vols_sans_mouvement_sont_ecartes() {
    let mut r = rows();
    for (i, x) in r.iter_mut().enumerate() {
        x["gdelay"] = json!(2.0);
        x["gdelayx_0"] = json!(2.0);
        x["sp95"] = json!(if i == 0 { 10.0 } else { 100.0 });
    }
    let t = resume_text(&r).unwrap();
    assert!(
        t.contains("gdelay       n=   2 "),
        "2 sessions avec mouvements sur 3 : {t}"
    );
    assert!(t.contains("gdelayx      n=   2 "), "{t}");
}

#[test]
fn les_textes_les_listes_et_les_reglages_ne_sont_pas_resumes() {
    let mut r = rows();
    r[0]["rpmw"] = json!([100, 100]);
    r[0]["sliders"] = json!({"master": 100});
    r[0]["mnoise_0"] = json!([-30.0, -35.0, -40.0]);
    r[0]["ff_0"] = json!(120);
    let t = resume_text(&r).unwrap();
    for absent in ["rpmw", "sliders", "craft", "mnoise ", "ff ", "cells", "profile "] {
        assert!(
            !t.contains(&format!("\n{absent}")),
            "{absent} ne doit pas être résumé : {t}"
        );
    }
    assert!(
        t.contains("\nmleft        n=   1  -30.00"),
        "première harmonique du bruit moteur : {t}"
    );
}

#[test]
fn la_boucle_ouverte_est_resumee_en_tete() {
    let mut r = rows();
    r[0]["measured_0"] = json!(true);
    r[0]["fc_0"] = num(f64::NAN);
    r[0]["smaxedge_0"] = json!(true);
    r[1]["measured_0"] = json!(true);
    r[1]["fc_0"] = json!(30.0);
    r[1]["smaxedge_0"] = json!(false);
    r[2]["measured_1"] = json!(false);
    let t = resume_text(&r).unwrap();
    assert!(
        t.starts_with(
            "Boucle ouverte : 2/3 axes mesurés (67 %), croisement trouvé 50 %, pic de sensibilité au bord 50 %\n"
        ),
        "{t}"
    );
}

#[test]
fn une_ligne_qui_n_est_pas_un_objet_est_refusee() {
    assert!(resume_text(&[json!(3)]).is_err());
}
