//! Supprimer un log de la bibliothèque (D-089) : ses fragments partent avec tout ce qui les désigne, et son fichier
//! copié ne s'efface que sur demande, avec les garde-fous de la suppression d'un drone (W-075).
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté sous
//! le test concerné.

use otb_store::Library;
use otb_store::model::{Analysis, AnalysisItem, Drone, Fragment, LogEntry, Measure};
use std::path::PathBuf;

/// Un dossier d'essai propre, hors du dépôt. Le nom porte le processus : deux exécutions ne se gênent pas.
fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-store-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

fn log(id: &str, drone: &str, path: &str, copied: bool) -> LogEntry {
    LogEntry {
        id: id.to_string(),
        drone_id: drone.to_string(),
        name: id.to_string(),
        path: path.to_string(),
        copied,
        bytes: 3,
        added: "2026-11-27T10:00:00".to_string(),
    }
}

fn fragment(id: &str, log_id: &str, bornes: (u64, u64)) -> Fragment {
    Fragment {
        id: id.to_string(),
        log_id: log_id.to_string(),
        session: 1,
        start: bornes.0,
        end: bornes.1,
        name: id.to_string(),
        role: "filters".to_string(),
        date: "2026-11-13".to_string(),
        chosen: true,
    }
}

/// Supprimer un log retire **ses** fragments (et ce qui les désigne : mesures, liste d'analyse, fragment de travail,
/// choix sur les conseils), et garde le drone, ses configurations et ses autres logs.
///
/// Contre-preuve vue en échec (2026-11-27) : avec `self.fragments.retain(…)` à la place de la boucle sur
/// `remove_fragment`, `fr-1` restait dans la liste d'analyse et l'assertion qui suit `remove_log` rougissait.
#[test]
fn supprimer_un_log_retire_ses_fragments_et_rien_d_autre() {
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new("uid-0001", "JeNo Ced"));
    assert!(lib.add_log(log("emp-aaa", "uid-0001", "/logs/FINAL.bbl", false)));
    assert!(lib.add_log(log("emp-bbb", "uid-0001", "/logs/LAST.bbl", false)));
    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 5_000)));
    lib.put_fragment(fragment("fr-1b", "emp-aaa", (5_000, 9_000)));
    lib.put_fragment(fragment("fr-2", "emp-bbb", (0, 10)));
    lib.record_config("uid-0001", "2026-11-13T18:10:12", "set gyro_lpf1_static_hz = 165");
    lib.set_measure(Measure {
        fragment_id: "fr-1".into(),
        axis: 0,
        key: "latence_ms".into(),
        value: 12.5,
        start: 0,
        end: 5_000,
        settings: "reglages-a".into(),
        calc_version: 1,
    });
    lib.set_analysis(Analysis {
        items: ["fr-1", "fr-2"]
            .iter()
            .enumerate()
            .map(|(i, f)| AnalysisItem {
                fragment_id: f.to_string(),
                colour: i as u64,
                active: true,
            })
            .collect(),
        working: Some("fr-1".into()),
    });
    assert_eq!(lib.fragments_of_log("emp-aaa").len(), 2);

    let mut retires = lib.remove_log("emp-aaa");
    retires.sort();
    assert_eq!(retires, vec!["fr-1".to_string(), "fr-1b".to_string()]);
    assert!(lib.analysis().items.iter().all(|i| i.fragment_id != "fr-1"));
    assert!(lib.log("emp-aaa").is_none() && lib.fragment("fr-1").is_none() && lib.fragment("fr-1b").is_none());
    assert_eq!(lib.analysis().working, None, "le fragment de travail était fr-1");
    assert!(lib.measure("fr-1", 0, "latence_ms").is_none());
    // le drone, ses configurations et son autre log restent
    assert!(lib.drone("uid-0001").is_some());
    assert_eq!(lib.configs_of_drone("uid-0001").len(), 1);
    assert!(lib.log("emp-bbb").is_some() && lib.fragment("fr-2").is_some());
    assert!(lib.analysis().items.iter().any(|i| i.fragment_id == "fr-2"));
    assert!(lib.remove_log("inconnu").is_empty());
}

/// Le fichier copié d'un log s'efface seul, avec les garde-fous de la suppression d'un drone : jamais un log de
/// l'utilisateur, jamais un chemin hors du dossier des copies.
///
/// Contre-preuve vue en échec (2026-11-27) : sans le contrôle `l.copied` de `delete_copy_of_log`, le fichier de
/// l'utilisateur rangé **dans** le dossier des copies était effacé (`delete_copy_of_log("u1")` rendait `Ok(true)`).
#[test]
fn seul_le_fichier_copie_du_log_s_efface() {
    let dir = dossier("effacer-copie-log");
    let mut lib = Library::open(dir.clone());
    let copie = lib.copy_path("JeNo 5", "a").unwrap().unwrap();
    let voisine = lib.copy_path("JeNo 5", "b").unwrap().unwrap();
    // un fichier sans la marque « copié » n'est pas à la bibliothèque, même rangé dans son dossier
    let a_lui = lib.copy_path("JeNo 5", "c").unwrap().unwrap();
    let dehors = dir.join("dehors.bbl");
    for p in [&copie, &voisine, &a_lui, &dehors] {
        std::fs::write(p, b"vol").unwrap();
    }
    for (id, path, copied) in [
        ("c1", &copie, true),
        ("c2", &voisine, true),
        ("u1", &a_lui, false),
        ("x1", &dehors, true),
    ] {
        lib.add_log(log(id, "d1", &path.display().to_string(), copied));
    }
    assert_eq!(lib.delete_copy_of_log("c1"), Ok(true));
    assert!(!copie.exists() && voisine.exists(), "seul le fichier de ce log part");
    assert_eq!(lib.delete_copy_of_log("c1"), Ok(false), "déjà absent : rien à dire");
    assert_eq!(lib.delete_copy_of_log("u1"), Ok(false));
    assert!(a_lui.exists(), "le log de l'utilisateur reste");
    assert!(lib.delete_copy_of_log("x1").is_err());
    assert!(dehors.exists(), "un chemin hors du dossier des copies reste");
    assert_eq!(lib.delete_copy_of_log("inconnu"), Ok(false));
    assert_eq!(Library::in_memory().delete_copy_of_log("c1"), Ok(false));
    let _ = std::fs::remove_dir_all(&dir);
}
