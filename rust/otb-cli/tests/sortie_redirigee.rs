//! `opentunebox report` avec la **sortie redirigée** vers un fichier (W-018).
//!
//! Le défaut d'origine était celui de la version Python : sous Windows, une sortie redirigée prenait l'encodage de la
//! console (cp1252) et `UnicodeEncodeError` arrêtait le rapport, qui contient « → », « − », « ≈ » et « ≤ ». Le binaire
//! Rust écrit de l'UTF-8 quoi qu'il arrive ; ce test le prouve au lieu de le supposer, et il tourne aussi sous Windows
//! (`tests.yml`).

use std::path::PathBuf;
use std::process::Command;

/// Caractères du rapport qui n'existent pas en cp1252 : ce sont eux qui faisaient échouer la version Python.
const HORS_CP1252: [&str; 4] = ["→", "−", "≈", "≤"];

fn log_essai() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/jeno5_5s.bbl")
        .to_string_lossy()
        .to_string()
}

#[test]
fn le_rapport_redirige_reste_en_utf8() {
    let dir = std::env::temp_dir().join(format!("otb-cli-redir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sortie = dir.join("rapport.txt");
    let fichier = std::fs::File::create(&sortie).unwrap();

    let code = Command::new(env!("CARGO_BIN_EXE_opentunebox"))
        .args(["report", &log_essai()])
        .stdout(fichier)
        .status()
        .unwrap();
    assert!(code.success(), "`report` a rendu {code:?} avec la sortie redirigée");

    // Lu en octets : une erreur d'encodage se verrait ici, pas dans un `String` déjà réparé.
    let octets = std::fs::read(&sortie).unwrap();
    let texte = String::from_utf8(octets).expect("la sortie redirigée n'est pas de l'UTF-8");

    // Le même rapport, calculé dans ce processus : la redirection ne doit rien changer, pas un octet.
    let (attendu, _) = otb_cli::report(std::slice::from_ref(&log_essai()), None);
    assert_eq!(texte, attendu, "le rapport redirigé diffère du rapport calculé");

    for c in HORS_CP1252 {
        assert!(
            texte.contains(c),
            "« {c} » absent : le test ne prouve plus rien sous Windows"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
