//! Banc d'ouverture d'un log : durée et pic de mémoire (lot 5 de `docs/plan.md`).
//!
//! ```text
//! otb-dev bench-open LOG [ESSAIS]
//! ```
//!
//! Le pic de mémoire est le compteur du système (`VmHWM` de `/proc/self/status`), pas une estimation. Il ne descend jamais
//! en cours de processus : **un processus par log**, c'est donc la commande qui s'arrête après le premier log ouvert.
//! Seul Linux le fournit ; ailleurs, la durée seule est donnée.

use std::path::Path;
use std::time::Instant;

/// Pic de mémoire résidente du processus en Mo (Linux), `None` si le système ne le dit pas.
pub fn peak_memory_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

/// Ouvre le log `trials` fois (au moins une) et rend le rapport : durées, pic de mémoire, nombre de sessions et de trames.
pub fn run(path: &Path, trials: usize) -> Result<String, String> {
    let mut times = Vec::new();
    let mut shape = (0usize, 0usize);
    for _ in 0..trials.max(1) {
        let t0 = Instant::now();
        let logs = otb_log::flightlog::load(path).map_err(|e| format!("{} : {e}", path.display()))?;
        times.push(t0.elapsed().as_secs_f64());
        shape = (logs.len(), logs.iter().map(|l| l.n()).sum());
    }
    let times_txt: Vec<String> = times.iter().map(|t| format!("{t:.3} s")).collect();
    let memory = peak_memory_mb().map_or("non disponible sur ce système".to_string(), |m| format!("{m:.0} Mo"));
    Ok(format!(
        "{}\n  sessions : {}, trames : {}\n  durées : {}\n  pic de mémoire (tout le processus) : {memory}",
        path.display(),
        shape.0,
        shape.1,
        times_txt.join(", ")
    ))
}
