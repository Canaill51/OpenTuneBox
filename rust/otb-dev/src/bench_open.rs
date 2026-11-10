//! Banc d'ouverture de logs : durée et pic de mémoire, séquentiel ou en parallèle (lots 5 et 11 de `docs/plan.md`).
//!
//! ```text
//! otb-dev bench-open LOG… [--jobs N] [--trials N] [--drop]
//! ```
//!
//! `--jobs 1` ouvre les fichiers les uns après les autres (mesure « avant ») ; au-delà, au plus `N` fichiers sont
//! décodés en même temps (`--jobs 3` = le plafond livré, D-051).
//!
//! Par défaut les logs ouverts sont **gardés** jusqu'à la fin de l'essai, comme l'interface qui les affiche : le pic
//! mesuré est donc celui d'une vraie ouverture multiple. `--drop` relâche chaque log dès qu'il est décodé et donne le
//! pic du **seul décodage**, c'est-à-dire ce que coûtent les `--jobs` fichiers décodés en même temps (D-051).
//!
//! Le pic de mémoire est le compteur du système (`VmHWM` de `/proc/self/status`), pas une estimation. Il ne descend
//! jamais en cours de processus : c'est le pic de **tout le banc**, et la commande s'arrête après les logs demandés —
//! pour comparer deux plafonds, lancer **deux fois** la commande, jamais deux plafonds dans le même processus.
//! Seul Linux le fournit ; ailleurs, la durée seule est donnée.

use std::path::PathBuf;
use std::time::Instant;

use otb_log::flightlog::FlightLog;
use otb_log::parallel::load_many_jobs;

/// Nombre de sessions et total des trames d'un fichier ouvert.
fn forme(logs: &[FlightLog]) -> (usize, usize) {
    (logs.len(), logs.iter().map(FlightLog::n).sum())
}

/// Pic de mémoire résidente du processus en Mo (Linux), `None` si le système ne le dit pas.
pub fn peak_memory_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

/// Ouvre tous les logs `trials` fois (au moins une), au plus `jobs` en même temps, et rend le rapport :
/// durée totale de chaque essai, sessions et trames par fichier, pic de mémoire du banc.
pub fn run(paths: &[PathBuf], jobs: usize, trials: usize, drop_logs: bool) -> Result<String, String> {
    if paths.is_empty() {
        return Err("aucun log donné".into());
    }
    let mut totals = Vec::new();
    let mut lignes = Vec::new();
    for essai in 0..trials.max(1) {
        let t0 = Instant::now();
        // `forme` est calculée dans le fil qui décode : avec `--drop`, le log y meurt aussitôt et ne pèse plus sur le pic
        let out = if drop_logs {
            otb_log::parallel::map_bounded(paths, jobs, |p: &PathBuf| {
                (p.clone(), otb_log::flightlog::load(p).map(|v| forme(&v)))
            })
        } else {
            load_many_jobs(paths, jobs)
                .into_iter()
                .map(|(p, r)| (p, r.map(|v| forme(&v))))
                .collect()
        };
        totals.push(t0.elapsed().as_secs_f64());
        if essai == 0 {
            for (p, r) in &out {
                let (sessions, frames) = r.as_ref().map_err(|e| format!("{} : {e}", p.display()))?;
                lignes.push(format!("  {} : {sessions} session(s), {frames} trames", p.display()));
            }
        }
    }
    let garde = if drop_logs {
        "logs relâchés dès décodés (pic du seul décodage)"
    } else {
        "logs gardés en mémoire (comme l'interface)"
    };
    let totals_txt: Vec<String> = totals.iter().map(|t| format!("{t:.3} s")).collect();
    let memory = peak_memory_mb().map_or("non disponible sur ce système".to_string(), |m| format!("{m:.0} Mo"));
    Ok(format!(
        "{} fichier(s), au plus {} décodé(s) en même temps, {garde}\n{}\n  durées totales : {}\n  pic de mémoire (tout le banc) : {memory}",
        paths.len(),
        jobs.max(1),
        lignes.join("\n"),
        totals_txt.join(", ")
    ))
}
