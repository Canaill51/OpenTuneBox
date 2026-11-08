//! `otb-dev verify` : options, étapes enchaînées, décompte des tests sautés et code de sortie.

use otb_dev::verify::*;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn les_options_se_lisent_et_une_option_inconnue_est_refusee() {
    assert_eq!(
        parse_options(&args(&[])).unwrap(),
        Options {
            quick: false,
            strict: false
        }
    );
    assert_eq!(
        parse_options(&args(&["--quick", "--strict"])).unwrap(),
        Options {
            quick: true,
            strict: true
        }
    );
    assert!(parse_options(&args(&["--vite"])).is_err());
}

#[test]
fn les_etapes_sont_fmt_clippy_test_licenses_puis_deny_sauf_en_quick() {
    let noms = |o: Options| steps(o).iter().map(|s| s.name).collect::<Vec<_>>();
    assert_eq!(
        noms(Options {
            quick: false,
            strict: false
        }),
        ["fmt", "clippy", "test", "licenses", "deny"]
    );
    assert_eq!(
        noms(Options {
            quick: true,
            strict: false
        }),
        ["fmt", "clippy", "test", "licenses"],
        "--quick ne retire que cargo deny : le contrôle des licences ne lance aucune commande"
    );
    // l'étape des tests affiche la sortie des tests réussis, sinon les sauts restent invisibles
    let etapes = steps(Options::default());
    let test = etapes.iter().find(|s| s.name == "test").unwrap();
    let args = test.cargo_args().expect("étape cargo");
    assert!(args.contains(&"--nocapture"));
    assert!(args.contains(&"--no-fail-fast"));
    assert!(
        !args.contains(&"-p"),
        "pas de liste de -p : cargo test seul est équivalent"
    );
    // le contrôle des licences est fait dans le processus : pas de commande cargo à lancer
    let licences = etapes.iter().find(|s| s.name == "licenses").unwrap();
    assert!(licences.cargo_args().is_none());
}

#[test]
fn les_sauts_se_comptent_dans_la_sortie_des_tests() {
    let sortie = format!("ok\n{SKIP_MARKER} OTB_GOLDEN absent\nok\n{SKIP_MARKER} OTB_BITEXACT absent\n");
    assert_eq!(count_skips(&sortie), 2);
    assert_eq!(count_skips("rien\n"), 0);
}

#[test]
fn le_code_de_sortie_vaut_1_si_une_etape_echoue_ou_si_un_saut_est_refuse() {
    assert_eq!(exit_code(true, 0, false), 0);
    assert_eq!(exit_code(true, 3, false), 0, "un saut seul n'est qu'un avertissement");
    assert_eq!(exit_code(true, 3, true), 1, "--strict : un saut vaut échec");
    assert_eq!(exit_code(false, 0, false), 1);
}

/// Tous les fichiers de tests de l'espace de travail.
fn fichiers_de_tests() -> Vec<std::path::PathBuf> {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut v = Vec::new();
    for crate_dir in std::fs::read_dir(&racine).unwrap().filter_map(Result::ok) {
        let tests = crate_dir.path().join("tests");
        let Ok(entries) = std::fs::read_dir(&tests) else {
            continue;
        };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "rs") {
                v.push(p);
            }
        }
    }
    assert!(v.len() > 20, "trop peu de fichiers de tests trouvés : {}", v.len());
    v
}

/// ⚠️ Un test qui se saute sans le dire se fait passer pour un test qui passe. Ce garde-fou **cherche** les sauts
/// au lieu d'en tenir une liste : une liste écrite à la main laisse passer le prochain (W-054)
#[test]
fn les_tests_qui_se_sautent_ecrivent_le_marqueur_sur_la_sortie_standard() {
    let mut sans_marqueur = Vec::new();
    for p in fichiers_de_tests() {
        let texte = std::fs::read_to_string(&p).unwrap();
        // Un saut conditionné par l'environnement : la variable est lue, puis le test rend la main.
        let saute = texte.contains("env::var(\"OTB_") && (texte.contains("return;") || texte.contains("else {"));
        if saute && !texte.contains(&format!("\"{SKIP_MARKER} ")) {
            sans_marqueur.push(p.display().to_string());
        }
    }
    assert!(
        sans_marqueur.is_empty(),
        "sauts sans marqueur sur stdout : {sans_marqueur:?}"
    );
}

/// Un fichier entier réservé à un système (`#![cfg(unix)]`) disparaît **sans rien afficher** ailleurs. Il doit alors
/// être annoncé par `sauts_par_systeme.rs`, qui, lui, est compilé partout.
#[test]
fn les_fichiers_reserves_a_un_systeme_sont_annonces() {
    let annonce =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sauts_par_systeme.rs"))
            .unwrap();
    let mut oublies = Vec::new();
    for p in fichiers_de_tests() {
        // le fichier d'annonce cite ces attributs dans son propre commentaire
        if p.ends_with("sauts_par_systeme.rs") {
            continue;
        }
        let texte = std::fs::read_to_string(&p).unwrap();
        if texte.contains("#![cfg(unix)]") || texte.contains("#![cfg(windows)]") {
            let nom = p.file_name().unwrap().to_string_lossy().to_string();
            if !annonce.contains(&nom) {
                oublies.push(nom);
            }
        }
    }
    assert!(
        oublies.is_empty(),
        "fichiers réservés à un système, non annoncés dans sauts_par_systeme.rs : {oublies:?}"
    );
}
