//! Mentions de licences (`otb-dev licenses`, W-043) : quels composants sont livrés, quels fichiers de licence sont lus, comment le fichier
//! final est écrit, et que le fichier du dépôt est bien livré par les deux installateurs.

use otb_dev::licenses::*;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("otb-lic-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

fn package(id: &str, external: bool, kinds: &[&str]) -> Value {
    json!({
        "id": id, "name": id, "version": "1.0.0", "license": "MIT", "authors": ["Ana <a@x.y>", "Bob"],
        "source": if external { json!("registry+https://github.com/rust-lang/crates.io-index") } else { Value::Null },
        "manifest_path": format!("/reg/{id}/Cargo.toml"),
        "targets": [{ "kind": kinds }],
    })
}

fn dep(pkg: &str, kind: Value) -> Value {
    json!({ "pkg": pkg, "dep_kinds": [{ "kind": kind }] })
}

/// racine → normale `a` (→ normale `e`), `build` (construction), `test` (test), `macro` (procédurale), `membre` (de l'espace de travail).
fn metadata() -> Value {
    json!({
        "packages": [
            package("otb-gui", false, &["bin"]),
            package("a", true, &["lib"]),
            package("e", true, &["lib"]),
            package("build", true, &["lib"]),
            package("test", true, &["lib"]),
            package("macro", true, &["proc-macro"]),
            package("membre", false, &["lib"]),
        ],
        "resolve": { "nodes": [
            { "id": "otb-gui", "deps": [
                dep("a", Value::Null), dep("build", json!("build")), dep("test", json!("dev")),
                dep("macro", Value::Null), dep("membre", Value::Null),
            ]},
            { "id": "a", "deps": [dep("e", Value::Null)] },
            { "id": "e", "deps": [] },
            { "id": "build", "deps": [] },
            { "id": "test", "deps": [] },
            { "id": "macro", "deps": [] },
            { "id": "membre", "deps": [dep("a", Value::Null)] },
        ]},
    })
}

#[test]
fn seules_les_dependances_normales_tierces_sont_livrees() {
    let names: Vec<String> = shipped(&metadata(), "otb-gui")
        .expect("composants")
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        names,
        ["a", "e"],
        "ni construction, ni test, ni macro procédurale, ni membre du dépôt"
    );
    assert!(shipped(&metadata(), "absent").unwrap_err().contains("introuvable"));
}

#[test]
fn les_auteurs_et_la_licence_declares_sont_lus() {
    let c = &shipped(&metadata(), "otb-gui").expect("composants")[0];
    assert_eq!((c.license.as_str(), c.authors.as_str()), ("MIT", "Ana <a@x.y>, Bob"));
    assert_eq!(c.dir, Path::new("/reg/a"));
}

#[test]
fn les_fichiers_de_licence_sont_trouves_par_leur_nom() {
    let dir = sandbox("fichiers");
    fs::write(dir.join("LICENSE-MIT"), "texte MIT\r\n\r\n").expect("mit");
    fs::write(dir.join("license-apache-2.0"), "texte Apache").expect("apache");
    fs::write(dir.join("COPYING"), "copie").expect("copying");
    fs::write(dir.join("README.md"), "pas une licence").expect("readme");
    fs::write(dir.join("Cargo.toml"), "x").expect("cargo");
    let files = license_files(&dir);
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(names, ["COPYING", "LICENSE-MIT", "license-apache-2.0"]);
    assert_eq!(
        files[1].1, "texte MIT",
        "fins de ligne normalisées, espaces finaux retirés"
    );
    assert!(license_files(&dir.join("absent")).is_empty());
}

fn component(name: &str, dir: &Path) -> Component {
    Component {
        name: name.into(),
        version: "1.0.0".into(),
        license: "MIT".into(),
        authors: "Ana".into(),
        dir: dir.to_path_buf(),
    }
}

#[test]
fn le_fichier_regroupe_les_textes_identiques_et_liste_les_composants_sans_licence() {
    let d1 = sandbox("c1");
    let d2 = sandbox("c2");
    let d3 = sandbox("c3");
    let d4 = sandbox("c4");
    fs::write(d1.join("LICENSE"), "MIT identique").expect("1");
    fs::write(d2.join("LICENSE"), "MIT identique").expect("2");
    fs::write(d3.join("LICENSE"), "Un autre texte").expect("3");
    let text = render(
        &[("Police X".to_string(), "licence de la police X".to_string())],
        &[
            component("un", &d1),
            component("deux", &d2),
            component("trois", &d3),
            component("quatre", &d4),
        ],
    );
    assert!(
        text.contains("### Police X") && text.contains("licence de la police X"),
        "{text}"
    );
    assert_eq!(
        text.matches("MIT identique").count(),
        1,
        "texte identique écrit une fois"
    );
    assert!(text.contains("utilisé par : deux 1.0.0, un 1.0.0"), "{text}");
    assert!(text.contains("Un autre texte"));
    let sans = text.split("3. Composants sans fichier").nth(1).expect("section 3");
    assert!(
        sans.contains("- quatre 1.0.0 : MIT ; auteurs : Ana") && !sans.contains("- un 1.0.0"),
        "{sans}"
    );
}

fn repo(file: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file} : {e}"))
}

#[test]
fn le_fichier_du_depot_porte_les_polices_et_les_icones_livrees() {
    let t = repo("THIRD-PARTY-LICENSES.txt");
    for attendu in [
        "UBUNTU FONT LICENCE Version 1.0",
        "Copyright 2011 Canonical Ltd.",
        "Source Foundry Authors",
        "Bitstream",
        "John Slegers",
        "Lucide Icons and Contributors",
        "Emil Ernerfeldt",
        "Permission is hereby granted",
        "Apache License",
    ] {
        assert!(
            t.contains(attendu),
            "mention absente de THIRD-PARTY-LICENSES.txt : {attendu}"
        );
    }
    assert!(
        !t.contains("SIL OPEN FONT LICENSE"),
        "l'OFL (NotoEmoji) n'est pas compilée : sa présence signale un changement à examiner (W-043)"
    );
}

#[test]
fn les_deux_installateurs_livrent_le_fichier_de_mentions() {
    assert!(
        repo("packaging/windows/opentunebox.iss").contains("THIRD-PARTY-LICENSES.txt"),
        "installateur Windows"
    );
    assert!(
        repo("packaging/linux/build_appimage.sh").contains("THIRD-PARTY-LICENSES.txt"),
        "AppImage"
    );
}

// --- `licenses --check` : comparaison du fichier du dépôt au texte attendu (lot 7a) ---

#[test]
fn le_controle_accepte_un_fichier_identique_et_refuse_tout_ecart() {
    let attendu = "ligne 1\nligne 2\nligne 3\n";
    let ok = compare(attendu, Some(attendu)).expect("identique");
    assert!(ok.contains("à jour") && ok.contains(&attendu.len().to_string()), "{ok}");

    // une ligne retirée : l'écart est refusé et la réparation est nommée
    let ampute = "ligne 1\nligne 3\n";
    let e = compare(attendu, Some(ampute)).expect_err("écart");
    assert!(e.contains("ligne 2"), "la ligne en cause est repérée : {e}");
    assert!(e.contains(RERUN), "{e}");

    // longueurs identiques, contenu différent : l'écart est trouvé quand même
    let e = compare(attendu, Some("ligne 1\nligne X\nligne 3\n")).expect_err("écart");
    assert!(e.contains("ligne 2"), "{e}");

    // fichier plus long que le texte attendu (une dépendance retirée)
    let e = compare("ligne 1\n", Some(attendu)).expect_err("écart");
    assert!(e.contains(RERUN), "{e}");

    // fichier absent
    let e = compare(attendu, None).expect_err("absent");
    assert!(e.contains("absent") && e.contains(RERUN), "{e}");
}

#[test]
fn le_controle_ne_touche_jamais_au_fichier() {
    let dir = sandbox("check");
    let path = dir.join("THIRD-PARTY-LICENSES.txt");
    fs::write(&path, "contenu périmé\n").expect("fichier");
    assert_eq!(path_in(&dir), path, "chemin du fichier livré");
    // `compare` est pure : rien n'est écrit, même quand l'écart est refusé
    let _ = compare("autre contenu\n", Some("contenu périmé\n"));
    assert_eq!(fs::read_to_string(&path).expect("relecture"), "contenu périmé\n");
}

#[test]
fn le_fichier_du_depot_correspond_aux_dependances_livrees() {
    // le vrai contrôle : il lance `cargo metadata` sur les deux cibles (quelques secondes)
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    check(&root).expect("THIRD-PARTY-LICENSES.txt périmé");
}
