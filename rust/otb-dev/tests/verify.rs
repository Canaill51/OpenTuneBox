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

#[test]
fn les_tests_qui_se_sautent_ecrivent_le_marqueur_sur_la_sortie_standard() {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for f in ["otb-parity/tests/golden.rs", "otb-advisor/tests/smooth.rs"] {
        let texte = std::fs::read_to_string(racine.join(f)).unwrap();
        assert!(
            // `cargo fmt` peut couper l'appel après la parenthèse : on ne fige pas la mise en forme
            texte.contains("println!(") && texte.contains(&format!("\"{SKIP_MARKER} ")),
            "{f} : saut sans marqueur sur stdout"
        );
    }
}
