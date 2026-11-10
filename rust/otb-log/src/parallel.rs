//! Ouverture de plusieurs fichiers à la fois, avec un plafond de fichiers décodés en même temps (D-051, lot 11).
//!
//! Le plafond est posé en **nombre de fichiers** et non en mémoire : chaque fichier décodé coûte 168 à 181 Mo de pic, et
//! un compteur de fichiers se vérifie dans un test, une mesure de mémoire non (D-051). Il est tenu par un pool `rayon`
//! **propre à l'appel** (`num_threads`), jamais par le pool global : ainsi le plafond ne dépend pas du nombre de cœurs de
//! la machine.
//!
//! L'ordre des résultats est celui des chemins donnés, quelle que soit la durée de chaque fichier
//! (`par_iter().collect()` le garantit) : l'interface affiche les logs dans l'ordre où l'utilisateur les a choisis.

use std::path::{Path, PathBuf};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

/// Nombre maximum de fichiers décodés en même temps (D-051). ⚠️ L'augmenter multiplie le pic de mémoire : environ
/// 180 Mo par fichier (L-081).
pub const MAX_PARALLEL_FILES: usize = 3;

/// Applique `f` à chaque élément, au plus `jobs` en même temps, en gardant l'ordre des éléments.
///
/// Séparée de [`load_many`] pour que les tests puissent compter les appels en cours sans décoder de vrais fichiers
/// (témoin exigé par D-051).
pub fn map_bounded<T, R, F>(items: &[T], jobs: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Send + Sync,
{
    let jobs = jobs.max(1).min(items.len());
    if jobs <= 1 {
        // un seul fichier (ou plafond de 1) : aucun fil lancé, donc aucun surcoût par rapport à l'ouverture d'avant
        return items.iter().map(&f).collect();
    }
    let pool = rayon::ThreadPoolBuilder::new().num_threads(jobs).build();
    match pool {
        Ok(pool) => pool.install(|| items.par_iter().map(&f).collect()),
        // plus de fil disponible : on ouvre les fichiers les uns après les autres plutôt que de ne rien ouvrir
        Err(_) => items.iter().map(f).collect(),
    }
}

/// Charge plusieurs fichiers `.bbl`/`.bfl`, au plus [`MAX_PARALLEL_FILES`] en même temps.
///
/// Rend un résultat **par fichier**, dans l'ordre des chemins : un fichier illisible n'empêche pas les autres, et
/// l'appelant remonte chaque erreur avec le nom de son fichier.
pub fn load_many(paths: &[PathBuf]) -> Vec<(PathBuf, std::io::Result<Vec<super::flightlog::FlightLog>>)> {
    load_many_jobs(paths, MAX_PARALLEL_FILES)
}

/// Comme [`load_many`], avec un plafond choisi : sert au banc (`otb-dev bench-open --jobs N`) pour comparer le
/// séquentiel (`1`) au parallèle.
pub fn load_many_jobs(
    paths: &[PathBuf],
    jobs: usize,
) -> Vec<(PathBuf, std::io::Result<Vec<super::flightlog::FlightLog>>)> {
    map_bounded(paths, jobs, |p: &PathBuf| {
        (p.clone(), super::flightlog::load(Path::new(p)))
    })
}
