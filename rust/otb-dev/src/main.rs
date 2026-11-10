//! `otb-dev` : outils de développement d'OpenTuneBox, jamais livrés (phase 7, D-035).
//!
//! ```text
//! otb-dev release prepare X.Y.Z [--date AAAA-MM-JJ]   numéro de version, CHANGELOG
//! otb-dev release notes vX.Y.Z                        texte de la page Releases
//! otb-dev icons [--svg CHEMIN]                        PNG Linux et .ico Windows depuis le SVG
//! otb-dev licenses [--check]                           THIRD-PARTY-LICENSES.txt (mentions de licences des composants livrés) ; --check : compare sans écrire
//! otb-dev corpus <outil> …                            outils du corpus de vrais logs (`OTB_LOGS`)
//! otb-dev verify [--quick] [--strict]                 fmt, clippy, tests, cargo deny : code 0 seulement si tout passe
//! otb-dev bench-open LOG… [--jobs N] [--trials N] [--drop]   durée et pic de mémoire de l'ouverture de logs (--jobs 1 : les uns après les autres)
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "Usage :\n  otb-dev release prepare X.Y.Z [--date AAAA-MM-JJ]\n  otb-dev release notes vX.Y.Z\n  otb-dev icons [--svg CHEMIN]\n  otb-dev licenses [--check]\n  otb-dev corpus <outil> …\n  otb-dev verify [--quick] [--strict]\n  otb-dev bench-open LOG… [--jobs N] [--trials N] [--drop]";

/// Racine du dépôt : `OTB_DEV_ROOT`, sinon le premier dossier parent qui contient `CHANGELOG.md` et `rust/Cargo.toml`.
fn repo_root() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("OTB_DEV_ROOT") {
        return Ok(PathBuf::from(dir));
    }
    let start = std::env::current_dir().map_err(|e| format!("dossier courant illisible : {e}"))?;
    start
        .ancestors()
        .find(|d| d.join("CHANGELOG.md").exists() && d.join("rust").join("Cargo.toml").exists())
        .map(PathBuf::from)
        .ok_or_else(|| "racine du dépôt introuvable (lancer depuis le dépôt ou définir OTB_DEV_ROOT)".into())
}

/// `bench-open LOG… [--jobs N] [--trials N]` : les options peuvent être n'importe où, le reste est la liste des logs.
fn bench_open(args: &[&str]) -> Result<String, String> {
    let mut logs: Vec<PathBuf> = Vec::new();
    let mut jobs = otb_log::parallel::MAX_PARALLEL_FILES;
    let mut trials = 1usize;
    let mut drop_logs = false;
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--jobs" | "--trials" => {
                let v: usize = args
                    .get(i + 1)
                    .and_then(|v| v.parse().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| format!("{} attend un nombre ≥ 1", args[i]))?;
                if args[i] == "--jobs" {
                    jobs = v
                } else {
                    trials = v
                }
                i += 2;
            }
            "--drop" => {
                drop_logs = true;
                i += 1;
            }
            other if other.starts_with("--") => return Err(USAGE.into()),
            other => {
                logs.push(PathBuf::from(other));
                i += 1;
            }
        }
    }
    otb_dev::bench_open::run(&logs, jobs, trials, drop_logs)
}

fn run(args: &[String]) -> Result<String, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["release", "prepare", version] => otb_dev::release::prepare(&repo_root()?, version, None),
        ["release", "prepare", version, "--date", date] => {
            otb_dev::release::prepare(&repo_root()?, version, Some(date))
        }
        ["release", "notes", tag] => otb_dev::release::notes(&repo_root()?, tag),
        ["licenses"] => otb_dev::licenses::generate(&repo_root()?),
        ["licenses", "--check"] => otb_dev::licenses::check(&repo_root()?),
        ["bench-open", rest @ ..] if !rest.is_empty() => bench_open(rest),
        ["icons"] => otb_dev::icons::generate(&repo_root()?, None),
        ["icons", "--svg", path] => otb_dev::icons::generate(&repo_root()?, Some(Path::new(path))),
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("corpus") {
        // les outils du corpus rendent aussi un code de sortie (1 : écart trouvé)
        let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
        return match otb_dev::corpus::run(&rest) {
            Ok(out) => {
                println!("{}", out.text);
                ExitCode::from(out.code)
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("verify") {
        return match repo_root().and_then(|root| Ok((root, otb_dev::verify::parse_options(&args[1..])?))) {
            Ok((root, options)) => {
                let (report, code) = otb_dev::verify::run(&root, options);
                println!("{report}");
                ExitCode::from(code)
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        };
    }
    match run(&args) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
