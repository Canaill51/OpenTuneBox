//! Jeux de données analysables (port de `analysis/datasets.py`, phase 4c-1).

use otb_log::flightlog::{FlightLog, Rows};
use std::borrow::Cow;

/// Clé → libellé affiché, dans l'ordre de `datasets.DATASETS`.
pub const DATASETS: [(&str, &str); 11] = [
    ("gyro", "Gyro"),
    ("gyro_prefilt", "Gyro préfiltre"),
    ("dterm", "Terme D"),
    ("dterm_prefilt", "Terme D préfiltre"),
    ("pterm", "Terme P"),
    ("pid_error", "Erreur PID"),
    ("setpoint", "Consigne"),
    ("fterm", "Terme F"),
    ("pid_sum", "Somme PID"),
    ("motors", "Moteurs"),
    ("debug", "Debug"),
];

/// Tableau `(k, n)` du jeu de données demandé ; `Ok(None)` s'il n'existe pas dans ce log, `Err` si la clé est inconnue
/// (le `KeyError` de Python).
pub fn get<'a>(log: &'a FlightLog, key: &str) -> Result<Option<Cow<'a, Rows>>, String> {
    Ok(match key {
        "gyro" => Some(Cow::Borrowed(&log.gyro)),
        "gyro_prefilt" => log.gyro_unfilt.as_ref().map(Cow::Borrowed),
        "dterm" => Some(Cow::Borrowed(&log.d_term)),
        "dterm_prefilt" => Some(Cow::Owned(log.d_term_prefilter()?)),
        "pterm" => Some(Cow::Borrowed(&log.p_term)),
        "pid_error" => Some(Cow::Owned(log.pid_error())),
        "setpoint" => Some(Cow::Borrowed(&log.setpoint)),
        "fterm" => Some(Cow::Borrowed(&log.f_term)),
        "pid_sum" => Some(Cow::Owned(log.pid_sum())),
        "motors" => (log.n_motors() > 0).then_some(Cow::Borrowed(&log.motors)),
        "debug" => (log.debug.k > 0).then_some(Cow::Borrowed(&log.debug)),
        _ => return Err(format!("KeyError: {key}")),
    })
}

/// Clés des jeux de données présents, dans l'ordre de `DATASETS`. `Err` si l'un d'eux ne peut pas être calculé (terme D préfiltre d'un
/// journal d'un point : Python lève aussi).
pub fn available(log: &FlightLog) -> Result<Vec<&'static str>, String> {
    let mut out = Vec::new();
    for (k, _) in DATASETS.iter() {
        if get(log, k)?.is_some() {
            out.push(*k);
        }
    }
    Ok(out)
}

/// Le jeu existe-t-il sur cet axe avec au moins une valeur non nulle (un NaN compte : `NaN != 0`) ? Le terme D du yaw (0 par
/// défaut) n'est pas affiché.
pub fn has_axis(log: &FlightLog, key: &str, axis: usize) -> Result<bool, String> {
    Ok(match get(log, key)? {
        Some(x) if axis < x.k => x.row(axis).iter().any(|&v| v != 0.0),
        _ => false,
    })
}
