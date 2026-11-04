//! Socle des outils du corpus (`otb_dev::corpus`), sans vrais logs. Portage de `tests/test_scripts.py` (map_files) et de
//! `tests/test_corpus_labels.py` (drone_id, log_files).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use otb_dev::corpus::{Args, drone_id, log_files, map_files, rel};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-dev-corpus-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn drone_id_est_celui_de_la_calibration() {
    assert_eq!(drone_id(" JeNo Ced ", "Cedric/JeNo 5/FINAL.bbl"), "JeNo Ced");
    assert_eq!(drone_id("", "Cedric/JeNo 5/FINAL.bbl"), "?Cedric/JeNo 5");
    assert_eq!(drone_id("", "a.bbl"), "?");
    // nom porté par plusieurs machines : nom + dossier
    assert_ne!(
        drone_id("Jax", "PiloteA/HORUS/a.bbl"),
        drone_id("Jax", "PiloteA/Whisper25/b.bbl")
    );
    assert_eq!(drone_id("Jax", "PiloteA/HORUS/a.bbl"), "Jax · PiloteA/HORUS");
}

#[test]
fn log_files_ecarte_les_doublons_et_macosx() {
    let d = tmp("liste");
    write(&d.join("A/x.bbl"), b"log 1");
    write(&d.join("B/copie.BBL"), b"log 1"); // même contenu, autre nom
    write(&d.join("B/y.bfl"), b"log 2");
    write(&d.join("B/__MACOSX/._y.bfl"), b"resource fork");
    write(&d.join("B/notes.txt"), b"texte");
    let files: Vec<String> = log_files(&d).unwrap().iter().map(|p| rel(p, &d)).collect();
    assert_eq!(files, ["A/x.bbl", "B/y.bfl"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn log_files_range_les_fichiers_d_un_dossier_avant_ses_sous_dossiers() {
    // même ordre que `os.walk` descendant, dossiers et fichiers triés
    let d = tmp("ordre");
    write(&d.join("b.bbl"), b"1");
    write(&d.join("a/z.bbl"), b"2");
    write(&d.join("a/b/y.bbl"), b"3");
    write(&d.join("c.bfl"), b"4");
    let files: Vec<String> = log_files(&d).unwrap().iter().map(|p| rel(p, &d)).collect();
    assert_eq!(files, ["b.bbl", "c.bfl", "a/z.bbl", "a/b/y.bbl"]);
    let _ = std::fs::remove_dir_all(&d);
}

fn fichiers(d: &Path) -> Vec<PathBuf> {
    // tailles différentes : les gros partent en premier, le résultat reste dans l'ordre de la liste
    let out: Vec<PathBuf> = ["a.bbl", "bad.bbl", "c.bbl", "d.bbl"]
        .iter()
        .map(|n| d.join(n))
        .collect();
    for (i, f) in out.iter().enumerate() {
        write(f, &vec![b'x'; 100 * (i + 1) * (if i % 2 == 0 { 1 } else { 3 })]);
    }
    out
}

fn taille(f: &Path) -> Result<u64, String> {
    std::fs::metadata(f).map(|m| m.len()).map_err(|e| e.to_string())
}

#[test]
fn map_files_garde_l_ordre_des_fichiers() {
    let d = tmp("ordre-map");
    let files = fichiers(&d);
    for jobs in [Some(1), Some(2), None] {
        let res = map_files(taille, &files, jobs, &d);
        let attendu: Vec<(PathBuf, u64)> = files.iter().map(|f| (f.clone(), taille(f).unwrap())).collect();
        assert_eq!(res, attendu, "jobs {jobs:?}");
    }
    assert!(map_files(taille, &[], Some(2), &d).is_empty());
    assert_eq!(map_files(taille, &files[..1], Some(2), &d).len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn un_fichier_en_erreur_est_refait_seul_puis_ignore_s_il_echoue_encore() {
    let d = tmp("reprise");
    let files = fichiers(&d);
    // `a.bbl` échoue au premier essai puis réussit (erreur rare vue en parallèle) ; `bad.bbl` échoue toujours ; `c.bbl` panique une fois
    let essais: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
    let work = |f: &Path| -> Result<u64, String> {
        let nom = f.file_name().unwrap().to_string_lossy().into_owned();
        let n = {
            let mut m = essais.lock().unwrap();
            let n = m.entry(nom.clone()).or_insert(0);
            *n += 1;
            *n
        };
        match nom.as_str() {
            "a.bbl" if n == 1 => Err("IndexError".into()),
            "bad.bbl" => Err("ValueError".into()),
            "c.bbl" if n == 1 => panic!("indice énorme"),
            _ => taille(f),
        }
    };
    for jobs in [Some(1), Some(3)] {
        essais.lock().unwrap().clear();
        let res = map_files(work, &files, jobs, &d);
        let noms: Vec<String> = res.iter().map(|(f, _)| rel(f, &d)).collect();
        assert_eq!(noms, ["a.bbl", "c.bbl", "d.bbl"], "jobs {jobs:?}");
        assert_eq!(res[0].1, 100);
        assert_eq!(essais.lock().unwrap()["a.bbl"], 2, "refait une fois, seul");
        assert_eq!(essais.lock().unwrap()["bad.bbl"], 2, "refait une fois puis ignoré");
        assert_eq!(essais.lock().unwrap()["d.bbl"], 1, "un fichier réussi n'est pas refait");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn les_options_se_lisent_n_importe_ou() {
    let a = Args::parse(
        &["sortie.json", "--limit", "40", "-j", "2"],
        &["--limit", "-j"],
        &["--ecrire"],
    )
    .unwrap();
    assert_eq!(a.positional, ["sortie.json"]);
    assert_eq!(a.number("--limit").unwrap(), Some(40));
    assert_eq!(a.number("-j").unwrap(), Some(2));
    assert!(!a.has("--ecrire"));
    let b = Args::parse(&["-j", "1", "x.csv", "--ecrire"], &["--limit", "-j"], &["--ecrire"]).unwrap();
    assert!(b.has("--ecrire") && b.number("--limit").unwrap().is_none() && b.positional == ["x.csv"]);
    assert!(
        Args::parse(&["--limite", "3"], &["--limit"], &[]).is_err(),
        "option inconnue refusée"
    );
    assert!(
        Args::parse(&["--limit"], &["--limit"], &[]).is_err(),
        "valeur manquante"
    );
    let c = Args::parse(&["--limit", "abc"], &["--limit"], &[]).unwrap();
    assert!(c.number("--limit").is_err());
}
