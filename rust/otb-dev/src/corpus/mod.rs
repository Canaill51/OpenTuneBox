//! Outils du corpus de vrais logs (remplacent `scripts/advisor_rates.py`, `calibrate_advisor.py`, `corpus_labels.py`
//! et `validate_corpus.py`). Ce module est le socle commun : liste des logs, passage parallèle, identité d'un drone,
//! lecture des options.
//!
//! Un seul processus : les fichiers se répartissent sur un pool rayon, et le noyau de `step_response` partage ce même pool,
//! donc pas de « cœurs × cœurs » fils (le `RAYON_NUM_THREADS=1` par processus de `map_files` en Python n'a plus d'objet).

use std::collections::HashSet;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use otb_log::flightlog::FlightLog;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

/// Noms donnés à plusieurs drones (confirmé par Cédric) : le drone est alors identifié par nom + dossier.
pub const SHARED_NAMES: [&str; 2] = ["Jax", "Blueberry"];

/// Durée minimale (s) d'une session prise en compte par tous les outils.
pub const MIN_SESSION_S: f64 = 8.0;

/// Dossier des logs : `OTB_LOGS`, sinon `~/Documents/Drones/Logs`.
pub fn logs_root() -> PathBuf {
    match std::env::var_os("OTB_LOGS") {
        Some(d) => PathBuf::from(d),
        None => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .unwrap_or_default();
            PathBuf::from(home).join("Documents").join("Drones").join("Logs")
        }
    }
}

/// Identité d'un drone : nom du drone, sinon « ? » + dossier du log ; nom + dossier pour un nom porté par plusieurs drones.
/// `relfile` est le chemin du log relatif au corpus, séparateurs `/`.
pub fn drone_id(craft: &str, relfile: &str) -> String {
    let name = craft.trim();
    let folder = relfile.rsplit_once('/').map_or("", |(d, _)| d);
    if name.is_empty() {
        format!("?{folder}")
    } else if SHARED_NAMES.contains(&name) {
        format!("{name} · {folder}")
    } else {
        name.to_string()
    }
}

/// Chemin relatif à `root`, avec des `/` partout (clé stable d'un fichier dans les JSON et les tableaux).
pub fn rel(path: &Path, root: &Path) -> String {
    let r = path.strip_prefix(root).unwrap_or(path);
    r.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn digest(path: &Path) -> std::io::Result<[u8; 16]> {
    let mut h = Sha256::new();
    h.update(std::fs::read(path)?);
    let full = h.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&full[..16]);
    Ok(out)
}

fn walk(dir: &Path, seen: &mut HashSet<[u8; 16]>, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        // `metadata` suit les liens symboliques : un lien vers un dossier est un dossier (comme `os.walk`)
        if std::fs::metadata(e.path()).map(|m| m.is_dir()).unwrap_or(false) {
            if name != "__MACOSX" {
                dirs.push(name);
            }
        } else {
            files.push(name);
        }
    }
    files.sort();
    dirs.sort();
    for f in files {
        let low = f.to_lowercase();
        if !(low.ends_with(".bbl") || low.ends_with(".bfl")) {
            continue;
        }
        let path = dir.join(&f);
        if seen.insert(digest(&path)?) {
            out.push(path);
        }
    }
    for d in dirs {
        walk(&dir.join(d), seen, out)?;
    }
    Ok(())
}

/// Logs du corpus (chemins triés : les fichiers d'un dossier, puis ses sous-dossiers), sans les dossiers `__MACOSX` et sans
/// doublons : un fichier au contenu identique à un autre n'est compté qu'une fois (le premier rencontré).
pub fn log_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let (mut seen, mut out) = (HashSet::new(), Vec::new());
    walk(root, &mut seen, &mut out)?;
    Ok(out)
}

fn message_of(p: Box<dyn std::any::Any + Send>) -> String {
    let m = p
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panique sans message".into());
    format!("panique : {m}")
}

fn guarded<R>(work: &(impl Fn(&Path) -> Result<R, String> + Sync), f: &Path) -> Result<R, String> {
    catch_unwind(AssertUnwindSafe(|| work(f))).unwrap_or_else(|p| Err(message_of(p)))
}

/// `[(chemin, work(chemin))]` dans l'ordre de `files`, calculés sur `jobs` fils (défaut : tous les cœurs ; 1 = un fichier à la fois).
/// Les plus gros fichiers partent en premier. Un fichier en erreur (ou qui panique) est refait seul à la fin : une erreur d'indice
/// rare a été vue en parallèle sur le poste de Cédric (inversions de bits, hors logiciel). S'il échoue encore, il est signalé
/// (`ERR` sur la sortie d'erreur) et ignoré.
pub fn map_files<R: Send>(
    work: impl Fn(&Path) -> Result<R, String> + Sync,
    files: &[PathBuf],
    jobs: Option<usize>,
    root: &Path,
) -> Vec<(PathBuf, R)> {
    let jobs = jobs
        .filter(|&j| j > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("pool rayon");
    let parallel = jobs > 1 && files.len() > 1;
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(std::fs::metadata(&files[i]).map_or(0, |m| m.len())));
    let counter = AtomicUsize::new(0);
    let first: Vec<(usize, Result<R, String>)> = pool.install(|| {
        order
            .par_iter()
            .map(|&i| {
                let res = guarded(&work, &files[i]);
                if parallel && res.is_ok() {
                    let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
                    eprintln!("[{n}/{}] {}", files.len(), rel(&files[i], root));
                }
                (i, res)
            })
            .collect()
    });
    let mut done: Vec<Option<R>> = (0..files.len()).map(|_| None).collect();
    let mut failed = Vec::new();
    for (i, res) in first {
        match res {
            Ok(r) => done[i] = Some(r),
            Err(_) => failed.push(i),
        }
    }
    failed.sort_unstable();
    for i in failed {
        match pool.install(|| guarded(&work, &files[i])) {
            Ok(r) => done[i] = Some(r),
            Err(e) => eprintln!(
                "ERR {} {}",
                rel(&files[i], root),
                e.chars().take(150).collect::<String>()
            ),
        }
    }
    files
        .iter()
        .cloned()
        .zip(done)
        .filter_map(|(f, r)| r.map(|r| (f, r)))
        .collect()
}

/// Période que l'assistant analyse dans cette session, ou `None` si elle est écartée : moins de 8 s, ou période d'au moins 5 s
/// introuvable (mêmes critères que `advisor_rates.py`, `calibrate_advisor.py` et `otb-parity`).
pub fn analysed(lg: &FlightLog) -> Option<Range<usize>> {
    if lg.duration() < MIN_SESSION_S {
        return None;
    }
    let (t0, t1) = otb_analysis::period::auto_period(lg, 8.0, 0.5).ok()?;
    let sel = lg.slice_time(t0, t1);
    (sel.end.saturating_sub(sel.start) as f64 >= lg.fs * 5.0).then_some(sel)
}

/// Options d'une ligne de commande : les mots sans tiret (positionnels) et la valeur de chaque option à valeur.
pub struct Args<'a> {
    pub positional: Vec<&'a str>,
    values: Vec<(&'a str, &'a str)>,
    flags: Vec<&'a str>,
}

impl<'a> Args<'a> {
    /// `valued` : options suivies d'une valeur (`--limit`, `-j`) ; `switches` : options seules (`--ecrire`).
    pub fn parse(args: &[&'a str], valued: &[&str], switches: &[&str]) -> Result<Self, String> {
        let (mut positional, mut values, mut flags) = (Vec::new(), Vec::new(), Vec::new());
        let mut it = args.iter().copied();
        while let Some(a) = it.next() {
            if let Some(&name) = valued.iter().find(|&&v| v == a) {
                let v = it.next().ok_or_else(|| format!("{name} attend une valeur"))?;
                values.push((a, v));
            } else if switches.contains(&a) {
                flags.push(a);
            } else if a.starts_with('-') && a.len() > 1 {
                return Err(format!("option inconnue : {a}"));
            } else {
                positional.push(a);
            }
        }
        Ok(Args {
            positional,
            values,
            flags,
        })
    }

    /// Valeur entière d'une option (la dernière si elle est répétée).
    pub fn number(&self, name: &str) -> Result<Option<usize>, String> {
        match self.values.iter().rev().find(|(k, _)| *k == name) {
            None => Ok(None),
            Some((_, v)) => v
                .parse()
                .map(Some)
                .map_err(|_| format!("{name} attend un entier, reçu « {v} »")),
        }
    }

    pub fn text(&self, name: &str) -> Option<&'a str> {
        self.values.iter().rev().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }

    pub fn has(&self, name: &str) -> bool {
        self.flags.contains(&name)
    }
}

pub const USAGE: &str = "Usage :\n  otb-dev corpus <outil> …\nOutils : (à venir) rates, calibrate, labels, validate";

/// Point d'entrée de `otb-dev corpus …`.
pub fn run(args: &[&str]) -> Result<String, String> {
    match args {
        [outil, ..] => Err(format!("outil inconnu : {outil}\n{USAGE}")),
        [] => Err(USAGE.into()),
    }
}
