//! Cas fabriqués de la ligne de commande : les textes attendus sont ceux que `opentunebox` de Python écrit pour les mêmes
//! fichiers (relevés le 2026-11-01). Les fichiers du corpus sont comparés par `otb-parity check-cli`.

use otb_cli::{Parsed, info, parse, report, run};
use std::path::PathBuf;

fn data(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data")
        .join(name)
        .to_string_lossy()
        .to_string()
}

fn temp(name: &str, content: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("otb-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p.to_string_lossy().to_string()
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn empty_garbage_and_truncated_files_have_no_session() {
    let real = std::fs::read(data("jeno5_5s.bbl")).unwrap();
    for (name, content) in [
        ("vide.bbl", Vec::new()),
        ("texte.bbl", b"du texte qui n est pas un log\n".to_vec()),
        ("coupe.bbl", real[..3000].to_vec()),
    ] {
        let f = temp(name, &content);
        let expected = format!("{f} : aucune session exploitable\n");
        assert_eq!(info(std::slice::from_ref(&f)), expected, "info {name}");
        assert_eq!(
            report(std::slice::from_ref(&f), None),
            (expected, None),
            "report {name}"
        );
    }
}

#[test]
fn missing_file_is_reported_on_one_line() {
    let f = std::env::temp_dir()
        .join("otb-absent-xyz.bbl")
        .to_string_lossy()
        .to_string();
    for text in [info(std::slice::from_ref(&f)), report(std::slice::from_ref(&f), None).0] {
        assert!(text.starts_with(&format!("{f} : illisible (")), "{text}");
        assert_eq!(text.lines().count(), 1);
        assert!(text.ends_with(")\n"));
    }
}

fn reference(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(format!("jeno5_5s.{name}.txt"));
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn texts_of_a_real_log_are_python_s_texts_character_for_character() {
    // `scripts/cli_cases.py` écrit les trois textes avec `opentunebox` de Python
    let f = data("jeno5_5s.bbl");
    assert_eq!(info(std::slice::from_ref(&f)), reference("info"));
    assert_eq!(report(std::slice::from_ref(&f), None), (reference("report"), None));
    assert_eq!(
        report(std::slice::from_ref(&f), Some("whoop")),
        (reference("report_whoop"), None)
    );
}

#[test]
fn several_files_keep_their_order() {
    let missing = std::env::temp_dir()
        .join("otb-absent-abc.bbl")
        .to_string_lossy()
        .to_string();
    let text = info(&[data("jeno5_5s.bbl"), missing.clone(), data("jeno5_5s.bbl")]);
    let first = text.find("jeno5_5s —").unwrap();
    let gap = text.find(&format!("{missing} : illisible")).unwrap();
    let second = text.rfind("jeno5_5s —").unwrap();
    assert!(first < gap && gap < second, "{text}");
}

#[test]
fn forced_profile_is_announced_as_chosen_by_the_user() {
    let f = data("jeno5_5s.bbl");
    let auto = report(std::slice::from_ref(&f), None).0;
    let whoop = report(std::slice::from_ref(&f), Some("whoop")).0;
    assert!(auto.lines().nth(1).unwrap().starts_with("    4 à 5 pouces ("), "{auto}");
    assert!(
        whoop
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("    Tiny whoop / micro (≤ 75 mm, 1–2S) (choisi par l'utilisateur) ; période 0.0–5.1 s"),
        "{whoop}"
    );
}

#[test]
fn severity_is_padded_to_nine_characters() {
    let text = report(&[data("jeno5_5s.bbl")], None).0;
    assert!(text.contains("  [ATTENTION] Réponse — "), "{text}");
    for l in text.lines().filter(|l| l.starts_with("  [")) {
        assert_eq!(l.find(']'), Some(12), "{l}");
    }
}

fn collect(a: &[&str]) -> Result<String, (i32, String)> {
    let mut text = String::new();
    run(&args(a), &mut |s| text.push_str(s))?;
    Ok(text)
}

#[test]
fn usage_errors_have_code_2() {
    for bad in [
        vec![],
        vec!["info"],
        vec!["autre", "x.bbl"],
        vec!["-x", "info", "x.bbl"],
        vec!["report", "x.bbl", "--profil", "zz"],
        vec!["report", "x.bbl", "--profil=zz"],
        vec!["report", "x.bbl", "--profil="],
        vec!["report", "x.bbl", "--profil"],
        vec!["report", "x.bbl", "--profil", "-5"],
        vec!["info", "x.bbl", "--profil", "whoop"],
        vec!["report", "x.bbl", "--inconnue"],
        vec!["report", "x.bbl", "-x"],
        vec!["report", "x.bbl", "--profil", "five", "y.bbl"],
        vec!["report", "--profil", "--", "five"],
        vec!["report", "x.bbl", "--profil", "--", "five"],
    ] {
        let err = collect(&bad).expect_err(&format!("{bad:?}"));
        assert_eq!(err.0, 2, "{bad:?}");
    }
}

#[test]
fn argparse_forms_are_accepted() {
    let p = |a: &[&str]| parse(&args(a)).unwrap();
    let files = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // abréviations, forme avec « = », option avant les fichiers
    for opt in [
        vec!["--profil", "five"],
        vec!["--prof", "five"],
        vec!["--pro=five"],
        vec!["--profil=five"],
    ] {
        let mut a = vec!["report", "a.bbl"];
        a.extend(&opt);
        assert_eq!(p(&a), Parsed::Report(files(&["a.bbl"]), Some("five".into())), "{opt:?}");
        let mut b = vec!["report"];
        b.extend(&opt);
        b.push("a.bbl");
        assert_eq!(p(&b), Parsed::Report(files(&["a.bbl"]), Some("five".into())), "{opt:?}");
    }
    // `--` : le reste est fait de fichiers, même s'ils commencent par un tiret ; le dernier --profil gagne
    assert_eq!(p(&["info", "--", "-x.bbl"]), Parsed::Info(files(&["-x.bbl"])));
    assert_eq!(
        p(&["info", "a.bbl", "--", "b.bbl"]),
        Parsed::Info(files(&["a.bbl", "b.bbl"]))
    );
    assert_eq!(
        p(&["report", "a.bbl", "--profil", "five", "--profil", "large"]),
        Parsed::Report(files(&["a.bbl"]), Some("large".into()))
    );
    // un texte à espace est un fichier, même après un tiret ; `--profil --` n'a pas de valeur
    assert_eq!(p(&["info", "-x y"]), Parsed::Info(files(&["-x y"])));
    assert_eq!(p(&["info", "--zz a"]), Parsed::Info(files(&["--zz a"])));
    assert_eq!(
        p(&["report", "--profil", "five", "--", "a"]),
        Parsed::Report(files(&["a"]), Some("five".into()))
    );
    // « - » seul est un fichier (l'entrée standard pour argparse)
    assert_eq!(p(&["info", "-"]), Parsed::Info(files(&["-"])));
}

#[test]
fn help_wins_and_writes_the_usage() {
    for a in [
        vec!["-h"],
        vec!["info", "-h"],
        vec!["report", "a.bbl", "--help"],
        vec!["--he"],
    ] {
        let text = collect(&a).unwrap();
        assert!(text.starts_with("usage : opentunebox"), "{a:?}");
    }
}

#[test]
fn text_is_written_session_by_session_before_an_error() {
    // la sortie est remise au puits à chaque session : un échec plus loin ne perd pas ce qui est déjà écrit
    let mut chunks = 0;
    let missing = std::env::temp_dir()
        .join("otb-absent-def.bbl")
        .to_string_lossy()
        .to_string();
    run(&args(&["info", &data("jeno5_5s.bbl"), &missing]), &mut |_| chunks += 1).unwrap();
    assert_eq!(chunks, 2, "un résumé, puis la ligne « illisible »");
}
