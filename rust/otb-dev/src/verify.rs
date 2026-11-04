//! `otb-dev verify` : la vérification complète en une commande, code de sortie 0 seulement si tout passe (lot 1 de `docs/plan-chantiers.md`).
//!
//! Étapes : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --no-fail-fast` (tout l'espace de travail : mêmes
//! exécutables qu'avec la liste des `-p`, mesuré le 2026-11-04), puis `cargo deny check` si `cargo-deny` est installé (sauf `--quick`).
//! Un test qui se saute faute de corpus écrit [`SKIP_MARKER`] sur la sortie standard ; `verify` les compte, et `--strict` en fait un échec.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// Début de la ligne qu'un test qui se saute écrit avec `println!` (littéral recopié : les crates testés ne dépendent pas d'`otb-dev`).
pub const SKIP_MARKER: &str = "[OTB-SAUT]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    /// Sans `cargo deny` (plus rapide).
    pub quick: bool,
    /// Un test sauté vaut échec (poste qui a le corpus).
    pub strict: bool,
}

pub fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    for a in args {
        match a.as_str() {
            "--quick" => o.quick = true,
            "--strict" => o.strict = true,
            autre => {
                return Err(format!(
                    "option inconnue : {autre}\nUsage : otb-dev verify [--quick] [--strict]"
                ));
            }
        }
    }
    Ok(o)
}

pub struct Step {
    pub name: &'static str,
    /// Arguments de `cargo`.
    pub args: Vec<&'static str>,
}

pub fn steps(o: Options) -> Vec<Step> {
    let mut v = vec![
        Step {
            name: "fmt",
            args: vec!["fmt", "--check"],
        },
        Step {
            name: "clippy",
            args: vec!["clippy", "--all-targets", "--", "-D", "warnings"],
        },
        Step {
            name: "test",
            args: vec!["test", "--no-fail-fast", "--", "--nocapture"],
        },
    ];
    if !o.quick {
        v.push(Step {
            name: "deny",
            args: vec!["deny", "check"],
        });
    }
    v
}

pub fn count_skips(output: &str) -> usize {
    output
        .lines()
        .filter(|l| l.trim_start().starts_with(SKIP_MARKER))
        .count()
}

pub fn exit_code(all_steps_ok: bool, skips: usize, strict: bool) -> u8 {
    u8::from(!all_steps_ok || (strict && skips > 0))
}

fn deny_installed() -> bool {
    Command::new("cargo")
        .args(["deny", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Lance les étapes depuis `root/rust` ; rend le rapport et le code de sortie.
pub fn run(root: &Path, o: Options) -> (String, u8) {
    let mut report = String::new();
    let (mut ok, mut skips) = (true, 0);
    let total = Instant::now();
    for step in steps(o) {
        if step.name == "deny" && !deny_installed() {
            report.push_str(
                "deny : AVERTISSEMENT, cargo-deny n'est pas installé (cargo install cargo-deny) ; étape non faite\n",
            );
            continue;
        }
        let t = Instant::now();
        let out = Command::new("cargo")
            .args(&step.args)
            .current_dir(root.join("rust"))
            .output();
        let secs = t.elapsed().as_secs_f32();
        match out {
            Ok(out) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                let n = count_skips(&text);
                skips += n;
                let etat = if out.status.success() { "ok" } else { "ÉCHEC" };
                report.push_str(&format!("{} : {etat} ({secs:.0} s)\n", step.name));
                if n > 0 {
                    report.push_str(&format!("  {n} test(s) sauté(s) :\n"));
                    for l in text.lines().filter(|l| l.trim_start().starts_with(SKIP_MARKER)) {
                        report.push_str(&format!("    {}\n", l.trim()));
                    }
                }
                if !out.status.success() {
                    ok = false;
                    let lines: Vec<&str> = text.lines().collect();
                    let from = lines.len().saturating_sub(40);
                    report.push_str(&lines[from..].join("\n"));
                    report.push('\n');
                }
            }
            Err(e) => {
                ok = false;
                report.push_str(&format!("{} : ÉCHEC, cargo introuvable ({e})\n", step.name));
            }
        }
    }
    let code = exit_code(ok, skips, o.strict);
    report.push_str(&format!(
        "Total : {:.0} s ; {skips} saut(s) ; ",
        total.elapsed().as_secs_f32()
    ));
    report.push_str(match (ok, code) {
        (true, 0) => "TOUT PASSE",
        (true, _) => "ÉCHEC (--strict : des tests ont été sautés)",
        _ => "ÉCHEC",
    });
    (report, code)
}
