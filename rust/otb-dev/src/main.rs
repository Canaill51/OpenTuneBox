//! `otb-dev` : outils de développement d'OpenTuneBox, jamais livrés (phase 7, D-035).
//!
//! ```text
//! otb-dev release prepare X.Y.Z [--date AAAA-MM-JJ]   numéro de version, CHANGELOG
//! otb-dev release notes vX.Y.Z                        texte de la page Releases
//! otb-dev icons [--svg CHEMIN]                        PNG Linux et .ico Windows depuis le SVG
//! otb-dev corpus <outil> …                            outils du corpus de vrais logs (`OTB_LOGS`)
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "Usage :\n  otb-dev release prepare X.Y.Z [--date AAAA-MM-JJ]\n  otb-dev release notes vX.Y.Z\n  otb-dev icons [--svg CHEMIN]\n  otb-dev corpus <outil> …";

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

fn run(args: &[String]) -> Result<String, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["release", "prepare", version] => otb_dev::release::prepare(&repo_root()?, version, None),
        ["release", "prepare", version, "--date", date] => {
            otb_dev::release::prepare(&repo_root()?, version, Some(date))
        }
        ["release", "notes", tag] => otb_dev::release::notes(&repo_root()?, tag),
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
