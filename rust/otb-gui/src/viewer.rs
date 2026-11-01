//! Visualiseur de log (port de la partie « données » de `gui/main_window.py`, 6-1) : traces, présence d'une trace,
//! lissage. L'affichage vient dans `viewer::page` (6-1b).

use otb_log::flightlog::{FlightLog, Rows};

/// (clé, libellé) des traces tracées sur les trois axes.
pub const TRACES: [(&str, &str); 11] = [
    ("gyro_prefilt", "Gyro (préfiltre)"),
    ("gyro", "Gyro"),
    ("pterm", "Terme P"),
    ("iterm", "Terme I"),
    ("dterm_prefilt", "Terme D (préfiltre)"),
    ("dterm", "Terme D"),
    ("fterm", "Terme F"),
    ("setpoint", "Consigne"),
    ("pid_sum", "Somme PID"),
    ("pid_error", "Erreur PID"),
    ("debug", "Debug"),
];

/// Groupes d'affichage du panneau de gauche.
pub const TRACE_GROUPS: [(&str, &[&str]); 3] = [
    ("Gyro", &["gyro", "gyro_prefilt"]),
    (
        "Consigne et PID",
        &[
            "setpoint",
            "pterm",
            "iterm",
            "dterm",
            "dterm_prefilt",
            "fterm",
            "pid_sum",
            "pid_error",
        ],
    ),
    ("Autres", &["debug"]),
];

pub const DEFAULT_TRACES: [&str; 2] = ["gyro", "setpoint"];

/// (libellé, largeur de la moyenne glissante en échantillons).
pub const SMOOTHING: [(&str, usize); 4] = [("aucun", 1), ("léger", 5), ("moyen", 11), ("fort", 25)];

/// Libellé d'une trace.
pub fn trace_label(key: &str) -> &'static str {
    TRACES.iter().find(|(k, _)| *k == key).map_or("?", |(_, l)| l)
}

/// Canaux d'une trace (`_trace_data` de Python). `None` : absente de ce log.
/// Panique sur une clé inconnue (faute de frappe du code : `KeyError` en Python).
pub fn trace_data(log: &FlightLog, key: &str) -> Option<Rows> {
    match key {
        "gyro_prefilt" => log.gyro_unfilt.clone(),
        "gyro" => Some(log.gyro.clone()),
        "pterm" => Some(log.p_term.clone()),
        "iterm" => Some(log.i_term.clone()),
        "dterm_prefilt" => log.d_term_prefilter().ok(),
        "dterm" => Some(log.d_term.clone()),
        "fterm" => Some(log.f_term.clone()),
        "setpoint" => Some(log.setpoint.clone()),
        "pid_sum" => Some(log.pid_sum()),
        "pid_error" => Some(log.pid_error()),
        "debug" => (log.debug.k >= 1).then(|| log.debug.first(log.debug.k.min(3))),
        _ => panic!("trace inconnue : {key}"),
    }
}

/// La trace existe et n'est pas nulle partout (`_has_trace`) : sinon sa case est masquée.
pub fn has_trace(log: &FlightLog, key: &str) -> bool {
    trace_data(log, key).is_some_and(|d| d.data.iter().any(|&v| v != 0.0))
}

/// Moyenne glissante centrée de largeur `w` (`np.convolve(y, ones(w)/w, "same")`, bords complétés par des zéros),
/// même longueur que `y`. `w` ≤ 1 : la courbe telle quelle.
///
/// Écart connu avec numpy : si `w` dépasse la longueur de `y`, numpy rend `w` valeurs, ici `y.len()` (W-033).
pub fn smooth(y: &[f32], w: usize) -> Vec<f64> {
    if w <= 1 {
        return y.iter().map(|&v| v as f64).collect();
    }
    let k = 1.0 / w as f64;
    // fenêtre de numpy "same" : de −w/2 à w−1−w/2 (centrée si w est impair, un cran plus tôt si w est pair)
    let (lo, hi) = (-((w / 2) as isize), (w - 1 - w / 2) as isize);
    (0..y.len() as isize)
        .map(|i| {
            let mut s = 0.0;
            for j in (i + lo).max(0)..=(i + hi).min(y.len() as isize - 1) {
                s += y[j as usize] as f64 * k;
            }
            s
        })
        .collect()
}

/// Résultat de la mesure entre deux points : écart en ms et fréquence correspondante (`measure_text`).
/// Vide s'il manque un point. Virgule décimale française.
pub fn measure_text(points: &[f64]) -> String {
    let [a, b, ..] = points else { return String::new() };
    let dt = (b - a).abs();
    let ms = format!("{:.1}", dt * 1000.0).replace('.', ",");
    let hz = if dt > 0.0 {
        format!("{:.1}", 1.0 / dt).replace('.', ",")
    } else {
        "—".to_string()
    };
    format!("Δt {ms} ms · {hz} Hz")
}

/// Valeurs au curseur, une par ligne (`readout` du lecteur). `i` : indice de l'échantillon.
pub fn readout_lines(log: &FlightLog, i: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let n = log.n_motors();
    for k in (0..n.min(8)).step_by(2) {
        let cells: Vec<String> = (k..n.min(k + 2))
            .map(|j| format!("M{} : {:3.0} %", j + 1, log.motors.row(j)[i] as f64))
            .collect();
        lines.push(cells.join("   "));
    }
    for (a, name) in crate::theme::AXIS_NAMES.iter().enumerate() {
        lines.push(format!("Gyro {name:<5} : {:6.0} °/s", log.gyro.row(a)[i] as f64));
    }
    lines.push(format!("Gaz : {:.0} %", log.throttle[i] as f64));
    if let Some(v) = &log.vbat {
        lines.push(format!("Batterie : {:.2} V", v[i] as f64));
    }
    lines
}

/// Position des deux manches au curseur, de −1 à 1 (y = +1 en haut) : (gauche : lacet / gaz, droite : roulis / tangage).
pub fn stick_positions(log: &FlightLog, i: usize) -> ([f32; 2], [f32; 2]) {
    let rc = |k: usize| {
        if log.rc_command.k > k {
            log.rc_command.row(k)[i]
        } else {
            0.0
        }
    };
    let c = |v: f32| v.clamp(-1.0, 1.0);
    (
        [c(rc(2) / 500.0), c((rc(3) - 1500.0) / 500.0)],
        [c(rc(0) / 500.0), c(rc(1) / 500.0)],
    )
}
