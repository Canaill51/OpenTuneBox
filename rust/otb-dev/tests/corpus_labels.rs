//! `otb-dev corpus labels` sans vrais logs : portage de `tests/test_corpus_labels.py` (états, tailles, fusion, noms partagés) et
//! fidélité du fichier au `csv` de Python (BOM, `;`, `\r\n`, guillemets).

use otb_dev::corpus::labels::{
    COLUMNS, Labels, Row, guess_size, guess_state, labels_text, merge, parse_labels, rows_from_sessions,
    shared_name_candidates,
};

fn row(pairs: &[(&str, &str)]) -> Row {
    let mut r: Row = COLUMNS.iter().map(|c| (c.to_string(), String::new())).collect();
    r.extend(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    r
}

fn labels(rows: &[Row]) -> Labels {
    rows.iter().map(|r| (r["drone"].clone(), r.clone())).collect()
}

#[test]
fn l_etat_vient_du_dossier_puis_des_mots() {
    assert_eq!(guess_state(&["1. PTB"], &["tune final.bbl"]), "aide"); // demande d'aide avant tout
    assert_eq!(guess_state(&["3. BBLR FB"], &["x.bbl"]), "aide");
    assert_eq!(guess_state(&["Cedric"], &["FINAL.bbl"]), "regle");
    assert_eq!(guess_state(&["Cedric"], &["LAST.bbl", "Stock.bbl"]), "regle");
    assert_eq!(guess_state(&["Cedric"], &["Stock_Air65.bbl"]), "origine");
    assert_eq!(guess_state(&["Cedric"], &["blast.bbl", "LOG00001.BFL"]), "inconnu");
    // « last » est un mot : collé à une lettre, ce n'en est pas un ; collé à un chiffre ou un tiret, si
    assert_eq!(guess_state(&["Cedric"], &["lasting.bbl"]), "inconnu");
    assert_eq!(guess_state(&["Cedric"], &["tune_last2.bbl"]), "regle");
    assert_eq!(guess_state(&["Cedric"], &["Tuned.bbl"]), "regle");
}

#[test]
fn la_taille_est_lue_dans_les_noms() {
    assert_eq!(guess_size(&["Apex 3.5 inch"]), "3,5");
    assert_eq!(guess_size(&["Source One 5\""]), "5");
    assert_eq!(guess_size(&["mon 7in"]), "7");
    assert_eq!(guess_size(&["Cine 3 pouces"]), "3");
    assert_eq!(guess_size(&["Cinewhoop 3 pouces"]), "whoop"); // le mot whoop l'emporte
    assert_eq!(guess_size(&["F1_Air65_2026.6RC2.bbl"]), "whoop");
    assert_eq!(guess_size(&["LOG00065.BFL", "JeNo 5"]), "");
    // « in » doit finir le mot ; « inch » et « inches » comptent
    assert_eq!(guess_size(&["5inx"]), "");
    assert_eq!(guess_size(&["5 inches"]), "5");
}

#[test]
fn le_chiffre_precedent_decide_comme_le_lookbehind_de_python() {
    // `(?<!\d)(?:65|75) ?mm` : « 75 mm » est un whoop, « 1075mm » non
    assert_eq!(guess_size(&["frame 75 mm"]), "whoop");
    assert_eq!(guess_size(&["x1075mm"]), "");
    // `(?<![\d.,])` : un chiffre collé à un point ou une virgule n'ouvre pas une taille
    assert_eq!(guess_size(&["3.5.7 inch"]), "");
    assert_eq!(guess_size(&["v2.5 inch"]), "2,5");
    // deux tailles : la première dans l'ordre du texte
    assert_eq!(guess_size(&["a 7 inch", "b 5 inch"]), "7");
}

#[test]
fn les_noms_partages_ne_se_confondent_pas_avec_les_sous_dossiers() {
    let rows = labels(&[
        row(&[("drone", "A"), ("dossiers", "Cedric/Air65 | Cedric/Air65/16012025")]), // sous-dossiers : même drone
        row(&[("drone", "B"), ("dossiers", "PiloteA | PiloteA/HORUS | PiloteA/Whisper25")]),
        row(&[("drone", "?Leo"), ("dossiers", "Leo")]),
        row(&[
            ("drone", "AIR65 CED"),
            ("dossiers", "Cedric/Air65/16012025 | Cedric/Air65/ChirP"),
        ]), // confirmé : un drone
    ]);
    let found: Vec<String> = shared_name_candidates(&rows)
        .iter()
        .map(|x| x.split(" : ").next().unwrap().to_string())
        .collect();
    assert_eq!(found, ["B"]);
}

#[test]
fn la_fusion_garde_les_colonnes_de_cedric() {
    let old = labels(&[
        row(&[
            ("drone", "A"),
            ("sessions", "3"),
            ("etat", "origine"),
            ("taille_reelle", "3,5"),
            ("remarque", "note"),
        ]),
        row(&[("drone", "B"), ("sessions", "1"), ("etat", "inconnu")]),
    ]);
    let new = labels(&[
        row(&[
            ("drone", "A"),
            ("sessions", "5"),
            ("etat", "aide"),
            ("taille_reelle", "5"),
        ]),
        row(&[("drone", "C"), ("sessions", "2"), ("etat", "regle")]),
    ]);
    let by = |rows: Vec<Row>| -> std::collections::HashMap<String, Row> {
        rows.into_iter().map(|r| (r["drone"].clone(), r)).collect()
    };
    let rows = by(merge(&old, &new, false));
    assert_eq!(rows["A"]["sessions"], "5"); // colonne automatique mise à jour
    assert_eq!(
        (
            &rows["A"]["etat"][..],
            &rows["A"]["taille_reelle"][..],
            &rows["A"]["remarque"][..]
        ),
        ("origine", "3,5", "note")
    );
    assert_eq!(rows["B"]["etat"], "inconnu"); // drone absent du passage : ligne gardée
    assert_eq!(rows["B"]["sessions"], "1");
    assert_eq!(rows["C"]["etat"], "regle"); // nouveau drone : pré-rempli
    let full = by(merge(&old, &new, true));
    assert_eq!((&full["B"]["sessions"][..], &full["B"]["etat"][..]), ("0", "inconnu")); // passage complet : 0 session
}

#[test]
fn les_lignes_sont_triees_par_origine_puis_par_drone() {
    let new = labels(&[
        row(&[("drone", "zeta"), ("origine", "B")]),
        row(&[("drone", "Beta"), ("origine", "B")]),
        row(&[("drone", "alpha"), ("origine", "B")]),
        row(&[("drone", "x"), ("origine", "A")]),
    ]);
    let names: Vec<String> = merge(&Vec::new(), &new, true)
        .iter()
        .map(|r| r["drone"].clone())
        .collect();
    assert_eq!(names, ["x", "alpha", "Beta", "zeta"]); // sans tenir compte de la casse
}

#[test]
fn le_fichier_est_ecrit_comme_python() {
    let rows = vec![row(&[
        ("drone", "04Pro3\""),
        ("sessions", "2"),
        ("remarque", "a;b"),
        ("dossiers", "x | y"),
    ])];
    let t = labels_text(&rows);
    assert!(t.starts_with("\u{feff}drone;dossiers;sessions;"), "BOM puis en-tête");
    assert!(t.contains("\r\n\"04Pro3\"\"\";x | y;2;;;;;;;;\"a;b\"\r\n"), "{t:?}");
    assert!(t.ends_with("\r\n"));
    assert!(!t.replace("\r\n", "").contains('\n'), "que des fins de ligne \\r\\n");
    // et on le relit tel quel
    let back = parse_labels(&t);
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].0, "04Pro3\"");
    assert_eq!(back[0].1["remarque"], "a;b");
}

#[test]
fn un_tableur_peut_reenregistrer_avec_des_virgules() {
    let text = "drone,sessions,etat,taille_reelle,remarque\nA,3,origine,\"3,5\",note\n,4,aide,5,sans drone\nB,1,,,\n";
    let rows = parse_labels(text);
    assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["A", "B"]); // la ligne sans drone est ignorée
    assert_eq!(rows[0].1["taille_reelle"], "3,5");
    assert_eq!(rows[1].1["etat"], "");
    assert!(parse_labels("").is_empty());
}

#[test]
fn un_drone_en_double_garde_sa_place_et_la_derniere_valeur() {
    let rows = parse_labels("drone;etat\nA;aide\nB;regle\nA;origine\n");
    assert_eq!(
        rows.iter()
            .map(|r| (r.0.as_str(), r.1["etat"].as_str()))
            .collect::<Vec<_>>(),
        [("A", "origine"), ("B", "regle")]
    );
}

#[test]
fn les_colonnes_en_trop_ou_manquantes_sont_tolerees() {
    let rows = parse_labels("drone;etat\nA;aide;en trop\nB\n");
    assert_eq!(rows[0].1["etat"], "aide");
    assert!(!rows[1].1.contains_key("etat"));
}

#[test]
fn les_lignes_viennent_des_sessions_de_chaque_drone() {
    let s = |craft: &str, hz: Option<f64>, kv: i64, cells: Option<i64>, prof| (craft.to_string(), hz, kv, cells, prof);
    let files = vec![
        (
            "1. PTB/Ben/a.bbl".to_string(),
            vec![
                s("Alex", Some(160.0), 1960, Some(6), "five"),
                s("Alex", Some(170.0), 2400, Some(6), "five"),
            ],
        ),
        (
            "1. PTB/Ben/b FINAL.bbl".to_string(),
            vec![s("Alex", None, 0, None, "large")],
        ),
        ("Cedric/JeNo 5/x.bbl".to_string(), vec![s("", None, 0, None, "seven")]),
    ];
    let rows = rows_from_sessions(&files);
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        ["Alex", "?Cedric/JeNo 5"]
    ); // ordre de première rencontre
    let a = &rows[0].1;
    assert_eq!(a["dossiers"], "1. PTB/Ben");
    assert_eq!(a["sessions"], "3");
    assert_eq!(a["origine"], "1. PTB");
    assert_eq!(a["freq_moteur_hz"], "165"); // médiane de 160 et 170
    assert_eq!(a["taille_detectee"], "five"); // 165 Hz, 6 cellules
    assert_eq!(a["motor_kv"], "2400"); // 1960 (défaut du firmware) et 0 écartés
    assert_eq!(a["cellules"], "6");
    assert_eq!(a["etat"], "aide");
    assert_eq!(a["taille_reelle"], "");
    let j = &rows[1].1;
    assert_eq!(j["taille_detectee"], "seven"); // sans fréquence moteur : le profil le plus fréquent
    assert_eq!(
        (&j["freq_moteur_hz"][..], &j["motor_kv"][..], &j["cellules"][..]),
        ("", "", "")
    );
    assert_eq!(j["etat"], "inconnu");
}

#[test]
fn a_egalite_la_premiere_valeur_rencontree_gagne() {
    let s = |cells: i64| ("D".to_string(), Some(300.0), 0, Some(cells), "micro");
    let rows = rows_from_sessions(&[("A/x.bbl".to_string(), vec![s(4), s(6), s(6), s(4)])]);
    assert_eq!(rows[0].1["cellules"], "4"); // 4 et 6 deux fois chacun : le 4 est venu le premier
}
