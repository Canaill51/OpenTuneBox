//! Couche « ligne de commande » (D-035, phase 5c) : `golden.py record-cli` écrit `DOSSIER/cli.json` (le texte exact de
//! `opentunebox info` et `report` de Python pour chaque fichier du corpus, le dossier du corpus remplacé par `@`) ; ici
//! `otb_cli::info` et `otb_cli::report` sont rejoués et comparés **caractère par caractère**.
//!
//! Seul le message d'exception entre parenthèses d'une ligne « illisible (…) » n'est pas comparé : c'est le texte d'une
//! exception Python (voir `same_text`). Une commande où Python plante (`<texte déjà écrit>@@ERREUR <type>`) doit écrire le même
//! texte puis échouer aussi, avec une erreur du même type (`ValueError…`) ; le message de l'exception n'est pas comparé.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub struct CliReport {
    /// Textes comparés (2 par fichier : `info` et `report`).
    pub texts: usize,
    /// Dont les lignes « illisible » dont seul le préfixe a pu être comparé.
    pub unreadable_lines: usize,
    pub failures: Vec<String>,
}

/// Même texte, à une exception près : après `illisible (`, tout jusqu'à la fin de la ligne vient de l'exception de Python.
/// Rend `None` si les textes diffèrent, sinon le nombre de lignes « illisible » comparées par leur préfixe.
pub fn same_text(expected: &str, actual: &str) -> Option<usize> {
    let (e, a): (Vec<&str>, Vec<&str>) = (expected.split('\n').collect(), actual.split('\n').collect());
    if e.len() != a.len() {
        return None;
    }
    let mut partial = 0;
    for (le, la) in e.iter().zip(&a) {
        if le == la {
            continue;
        }
        const MARK: &str = " : illisible (";
        match (le.find(MARK), la.find(MARK)) {
            (Some(i), Some(j)) if le[..i + MARK.len()] == la[..j + MARK.len()] => partial += 1,
            _ => return None,
        }
    }
    Some(partial)
}

fn first_difference(expected: &str, actual: &str) -> String {
    let (e, a) = (expected.lines(), actual.lines());
    for (i, (le, la)) in e.zip(a).enumerate() {
        if le != la {
            return format!("ligne {} : Python {le:?}, Rust {la:?}", i + 1);
        }
    }
    format!(
        "{} lignes pour Python, {} pour Rust",
        expected.lines().count(),
        actual.lines().count()
    )
}

const CRASH: &str = "@@ERREUR ";

/// `actual` = (texte écrit, erreur éventuelle). Python a planté si `expected` finit par `@@ERREUR <type>`.
fn check_text(rel: &str, cmd: &str, expected: &str, actual: (String, Option<String>), local: &mut CliReport) {
    local.texts += 1;
    let (text, err) = actual;
    let (expected_text, crash) = match expected.rfind(CRASH) {
        Some(i) => (&expected[..i], Some(&expected[i + CRASH.len()..])),
        None => (expected, None),
    };
    match (crash, err) {
        (None, Some(e)) => local
            .failures
            .push(format!("{rel} {cmd} : Rust échoue ({e}), Python rend un texte")),
        (Some(t), None) => local
            .failures
            .push(format!("{rel} {cmd} : Python plante ({t}), Rust rend un texte")),
        (Some(t), Some(e)) if !e.starts_with(t) => {
            local
                .failures
                .push(format!("{rel} {cmd} : Python plante avec {t}, Rust avec {e:?}"));
        }
        _ => match same_text(expected_text, &text) {
            Some(n) => local.unreadable_lines += n,
            None => local
                .failures
                .push(format!("{rel} {cmd} : {}", first_difference(expected_text, &text))),
        },
    }
}

/// Compare les sorties aux références `dossier/cli.json`.
pub fn check_cli(dossier: &Path, logs_root: &Path, threads: usize) -> Result<CliReport, String> {
    let text = std::fs::read_to_string(dossier.join("cli.json")).map_err(|e| format!("cli.json : {e}"))?;
    let reference: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let files: Vec<(&String, &serde_json::Value)> = reference.as_object().ok_or("cli.json")?.iter().collect();
    let prefix = format!("{}/", logs_root.to_string_lossy().trim_end_matches('/'));
    let next = AtomicUsize::new(0);
    let report = Mutex::new(CliReport::default());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((rel, refs)) = files.get(i) else { break };
                    let path = format!("{prefix}{rel}");
                    let mut local = CliReport::default();
                    for cmd in ["info", "report", "report_whoop"] {
                        let expected = refs[cmd].as_str().unwrap_or("").replace("@/", &prefix);
                        let one = std::slice::from_ref(&path);
                        let actual = match cmd {
                            "info" => (otb_cli::info(one), None),
                            "report" => otb_cli::report(one, None),
                            _ => otb_cli::report(one, Some("whoop")),
                        };
                        check_text(rel, cmd, &expected, actual, &mut local);
                    }
                    let mut r = report.lock().unwrap();
                    r.texts += local.texts;
                    r.unreadable_lines += local.unreadable_lines;
                    r.failures.append(&mut local.failures);
                }
            });
        }
    });
    Ok(report.into_inner().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_has_no_partial_line() {
        assert_eq!(same_text("a\nb\n", "a\nb\n"), Some(0));
    }

    #[test]
    fn differing_text_is_refused() {
        assert_eq!(same_text("a\nb\n", "a\nc\n"), None);
        assert_eq!(same_text("a\nb\n", "a\nb\n\n"), None);
        assert_eq!(same_text("a", ""), None);
    }

    #[test]
    fn only_the_exception_text_of_an_unreadable_line_is_free() {
        let py = "@/x.bbl : illisible (invalid header)\n";
        assert_eq!(same_text(py, "@/x.bbl : illisible (autre message)\n"), Some(1));
        assert_eq!(
            same_text(py, "@/y.bbl : illisible (invalid header)\n"),
            None,
            "le chemin se compare"
        );
        assert_eq!(same_text(py, "@/x.bbl : aucune session exploitable\n"), None);
    }

    fn ok(text: &str) -> (String, Option<String>) {
        (text.to_string(), None)
    }

    #[test]
    fn python_crash_requires_the_same_text_then_a_failure_of_the_same_type() {
        let crash = "ligne 1\nligne 2\n@@ERREUR ValueError";
        let mut r = CliReport::default();
        check_text(
            "f",
            "report",
            crash,
            ("ligne 1\nligne 2\n".into(), Some("ValueError : autre".into())),
            &mut r,
        );
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        // Rust rend un texte alors que Python plante
        check_text("f", "report", crash, ok("ligne 1\nligne 2\n"), &mut r);
        assert_eq!(r.failures.len(), 1);
        // le texte écrit avant l'échec diffère
        check_text(
            "f",
            "report",
            crash,
            ("ligne 1\n".into(), Some("ValueError".into())),
            &mut r,
        );
        assert_eq!(r.failures.len(), 2);
        // autre type d'exception
        check_text(
            "f",
            "report",
            crash,
            ("ligne 1\nligne 2\n".into(), Some("IndexError".into())),
            &mut r,
        );
        assert_eq!(r.failures.len(), 3);
        // Rust échoue alors que Python rend un texte
        check_text("f", "info", "texte\n", ("texte\n".into(), Some("boum".into())), &mut r);
        assert_eq!(r.failures.len(), 4);
        check_text("f", "info", "texte\n", ok("texte\n"), &mut r);
        assert_eq!(r.failures.len(), 4);
    }
}
