//! Les fichiers de tests réservés à un système disparaissent ailleurs **sans rien afficher** : `#![cfg(unix)]` empêche
//! toute compilation sous Windows, donc aucun message ne peut en sortir. Ce fichier-ci est compilé partout et
//! l'annonce à leur place, pour que `otb-dev verify` les compte comme des sauts (W-054).
//!
//! `verify.rs::les_fichiers_reserves_a_un_systeme_sont_annonces` vérifie que tout fichier `#![cfg(…)]` est bien
//! nommé ici : en ajouter un sans l'annoncer fait échouer ce contrôle.

/// Les fichiers réservés à Unix, avec ce qu'ils vérifient.
const RESERVES_A_UNIX: [(&str, &str); 1] = [(
    "install_linux.rs",
    "mise à jour d'un programme en cours d'exécution par packaging/linux/install_linux.sh",
)];

#[test]
#[cfg(not(unix))]
fn les_tests_reserves_a_unix_ne_tournent_pas_ici() {
    for (fichier, quoi) in RESERVES_A_UNIX {
        println!("[OTB-SAUT] {fichier} : réservé à Unix — {quoi}");
    }
}

/// Sous Unix, ces fichiers tournent : rien à annoncer, mais la liste doit rester juste.
#[test]
#[cfg(unix)]
fn les_fichiers_annonces_existent_bien() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    for (fichier, _) in RESERVES_A_UNIX {
        assert!(
            tests.join(fichier).exists(),
            "{fichier} est annoncé comme réservé à Unix mais n'existe pas"
        );
    }
}
