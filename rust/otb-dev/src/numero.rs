//! `numero D|L|W [NOTE…]` : réserve le prochain numéro libre d'un registre (décisions, leçons, travaux reportés) quand
//! plusieurs conversations travaillent en même temps, chacune dans son worktree (compétence `chantiers`).
//!
//! ⚠️ Le registre d'un seul dossier ne suffit pas : un autre chantier peut avoir cité un numéro dans son code sans l'avoir
//! encore écrit dans le registre (une décision de la fiche drone le 2026-11-25, L-125). Le numéro rendu dépasse donc tout ce qui est vu dans :
//! le registre de chaque worktree, celui d'`origin/main`, les changements de chaque worktree par rapport à `origin/main`
//! (validés ou non), et le fichier des réservations. Ce fichier est dans le dossier `.git` commun à tous les worktrees,
//! jamais dans le dépôt ; un verrou (création exclusive d'un fichier) évite que deux réservations simultanées
//! tombent sur le même numéro.
//!
//! `numero` sans argument liste les réservations.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use regex::Regex;

use crate::docs::REGISTERS;

/// Nom du fichier des réservations, dans le dossier `.git` commun.
pub const RESERVATIONS: &str = "otb-numeros.txt";

/// Plus grand numéro `PREFIXE-nnn` cité dans `text` (0 s'il n'y en a aucun).
pub fn highest(prefix: &str, text: &str) -> u32 {
    let re =
        Regex::new(&format!(r"\b{prefix}-(\d{{3}})\b")).unwrap_or_else(|e| panic!("préfixe invalide {prefix} : {e}"));
    re.captures_iter(text)
        .filter_map(|c| c[1].parse().ok())
        .max()
        .unwrap_or(0)
}

/// Prochain numéro libre : un de plus que le plus grand vu dans toutes les sources.
pub fn next<'a>(prefix: &str, sources: impl IntoIterator<Item = &'a str>) -> u32 {
    sources.into_iter().map(|t| highest(prefix, t)).max().unwrap_or(0) + 1
}

/// Ligne ajoutée au fichier des réservations : numéro, date, dossier, note.
pub fn reservation_line(id: &str, date: &str, folder: &str, note: &str) -> String {
    let note = note.replace(['\t', '\n', '\r'], " ");
    format!("{id}\t{date}\t{folder}\t{note}\n")
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git introuvable : {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} : {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Dossier `.git` commun à tous les worktrees du dépôt.
fn common_dir(root: &Path) -> Result<PathBuf, String> {
    let d = PathBuf::from(git(root, &["rev-parse", "--git-common-dir"])?.trim());
    Ok(if d.is_absolute() { d } else { root.join(d) })
}

/// Dossiers de tous les worktrees du dépôt (le principal compris).
fn worktrees(root: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(git(root, &["worktree", "list", "--porcelain"])?
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect())
}

/// Tout ce qui peut porter un numéro déjà pris : voir l'en-tête du module.
fn sources(root: &Path, register: &str) -> Result<Vec<String>, String> {
    let rel = format!("docs/{register}");
    let mut texts = Vec::new();
    // origin/main peut manquer (dépôt sans remote, essai) : ce n'est pas une erreur, seulement une source de moins
    texts.extend(git(root, &["show", &format!("origin/main:{rel}")]).ok());
    for wt in worktrees(root)? {
        texts.extend(std::fs::read_to_string(wt.join(&rel)).ok());
        texts.extend(git(&wt, &["diff", "origin/main"]).ok());
    }
    texts.extend(std::fs::read_to_string(common_dir(root)?.join(RESERVATIONS)).ok());
    Ok(texts)
}

/// Verrou par création exclusive : rendu à la sortie, même en cas d'erreur.
struct Lock(PathBuf);

impl Lock {
    fn take(path: PathBuf) -> Result<Self, String> {
        let start = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Lock(path)),
                Err(_) if start.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => {
                    return Err(format!(
                        "verrou {} toujours pris après 10 s ({e}) : une autre réservation a planté ? Supprimer ce fichier",
                        path.display()
                    ));
                }
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Réserve le prochain numéro de `prefix` (D, L ou W) et l'inscrit dans le fichier des réservations.
pub fn reserve(root: &Path, prefix: &str, note: &str, date: &str) -> Result<String, String> {
    let register = REGISTERS
        .iter()
        .find(|r| r.0 == prefix)
        .map(|r| r.1)
        .ok_or_else(|| format!("registre inconnu : {prefix} (D, L ou W)"))?;
    let common = common_dir(root)?;
    let _lock = Lock::take(common.join(format!("{RESERVATIONS}.verrou")))?;
    let texts = sources(root, register)?;
    let id = format!("{prefix}-{:03}", next(prefix, texts.iter().map(String::as_str)));
    let line = reservation_line(&id, date, &root.display().to_string(), note);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(common.join(RESERVATIONS))
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| format!("réservation impossible à écrire : {e}"))?;
    Ok(format!("{id} réservé pour {} ({register})\n", root.display()))
}

/// Les réservations faites jusqu'ici.
pub fn list(root: &Path) -> Result<String, String> {
    let path = common_dir(root)?.join(RESERVATIONS);
    Ok(std::fs::read_to_string(&path).unwrap_or_else(|_| format!("aucune réservation ({})\n", path.display())))
}

/// Date du jour (UTC) au format AAAA-MM-JJ, sans dépendance : jours depuis 1970 convertis en date civile.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date(secs / 86_400)
}

/// Jours depuis le 1970-01-01 → « AAAA-MM-JJ » (calendrier grégorien, méthode des ères de 400 ans).
pub fn civil_date(days: u64) -> String {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
