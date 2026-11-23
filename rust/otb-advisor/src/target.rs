//! La fiche cible d'un drone (lot 31b de `docs/plan.md`, D-072) : les dix cases de la fiche du drone, et les
//! **valeurs proposées** pour une taille.
//!
//! ⚠️ **Les valeurs proposées sont les seuils de l'assistant** pour le profil de taille, lus là où ses règles les
//! lisent (`profiles::PROFILES`, `rules`) : jamais un chiffre recopié. Une valeur proposée qui ne serait pas le
//! seuil de l'assistant afficherait, par défaut, deux chiffres différents pour une même mesure (D-072).
//!
//! Une grandeur sans seuil dans l'assistant (bas de la zone du pic, temps de montée, marge de gain) n'a **pas** de
//! valeur proposée : sa case reste vide tant que le pilote ne la remplit pas. La cible du pilote ne sert que la fiche
//! et le suivi ; l'assistant garde ses seuils dans ses constats.

use crate::profiles::{CraftProfile, profile};
use crate::rules::{PHASE_MARGIN_LOW_DEG, STEP_PEAK_HIGH};
use crate::tracked;
use otb_format::fixed;

/// Le sens d'une cible : la mesure doit rester au plus (`AtMost`, « ≤ ») ou au moins (`AtLeast`, « ≥ ») à la valeur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    AtMost,
    AtLeast,
}

impl Bound {
    pub fn sign(self) -> &'static str {
        match self {
            Bound::AtMost => "≤",
            Bound::AtLeast => "≥",
        }
    }
}

/// Une case de la fiche cible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetRow {
    /// Clé gardée dans la bibliothèque (littéral anglais, jamais traduit).
    pub key: &'static str,
    /// Groupe affiché : « Réactivité des PID » ou « Filtrage ».
    pub group: &'static str,
    pub label: &'static str,
    pub bound: Bound,
    /// Unité affichée (vide pour le pic, sans unité).
    pub unit: &'static str,
    /// Chiffres après la virgule d'une valeur proposée.
    pub decimals: usize,
    /// La grandeur suivie que la case fixe (`tracked`) ; les deux bords de la zone du pic visent la même.
    pub tracked: &'static str,
}

pub const RESPONSE: &str = "Réactivité des PID";
pub const FILTERING: &str = "Filtrage";

/// Les dix cases, dans l'ordre du prototype (`docs/maquettes/Application.dc.html`, écran `droneEdit`).
pub const ROWS: [TargetRow; 10] = [
    row(
        "latency_ms",
        RESPONSE,
        "Latence (50 %)",
        Bound::AtMost,
        "ms",
        0,
        tracked::LATENCY_MS,
    ),
    row(
        "peak_low",
        RESPONSE,
        "Pic, bas de la zone",
        Bound::AtLeast,
        "",
        2,
        tracked::PEAK,
    ),
    row(
        "peak_high",
        RESPONSE,
        "Pic, haut de la zone",
        Bound::AtMost,
        "",
        2,
        tracked::PEAK,
    ),
    row(
        "rise_ms",
        RESPONSE,
        "Temps de montée",
        Bound::AtMost,
        "ms",
        0,
        tracked::RISE_MS,
    ),
    row(
        "phase_margin_deg",
        RESPONSE,
        "Marge de phase",
        Bound::AtLeast,
        "°",
        0,
        tracked::PHASE_MARGIN_DEG,
    ),
    row(
        "gain_margin_db",
        RESPONSE,
        "Marge de gain",
        Bound::AtLeast,
        "dB",
        0,
        tracked::GAIN_MARGIN_DB,
    ),
    row(
        "gyro_noise_db",
        FILTERING,
        "Bruit gyro filtré",
        Bound::AtMost,
        "dB",
        1,
        tracked::GYRO_NOISE_DB,
    ),
    row(
        "dterm_noise_db",
        FILTERING,
        "Bruit du terme D",
        Bound::AtMost,
        "dB",
        1,
        tracked::DTERM_NOISE_DB,
    ),
    row(
        "gyro_filter_delay_ms",
        FILTERING,
        "Retard des filtres gyro",
        Bound::AtMost,
        "ms",
        2,
        tracked::GYRO_FILTER_DELAY_MS,
    ),
    row(
        "dterm_filter_delay_ms",
        FILTERING,
        "Retard des filtres D",
        Bound::AtMost,
        "ms",
        2,
        tracked::DTERM_FILTER_DELAY_MS,
    ),
];

const fn row(
    key: &'static str,
    group: &'static str,
    label: &'static str,
    bound: Bound,
    unit: &'static str,
    decimals: usize,
    tracked: &'static str,
) -> TargetRow {
    TargetRow {
        key,
        group,
        label,
        bound,
        unit,
        decimals,
        tracked,
    }
}

/// Le seuil de l'assistant qui sert de valeur proposée à une case, pour un profil ; `None` sans seuil.
///
/// - Latence : haut de la plage « habituelle » du profil (`latency_ms.1`), au-delà de laquelle `step_soft` juge la
///   réponse molle.
/// - Haut de la zone du pic : `STEP_PEAK_HIGH.0`, le conseil **sans** feedforward (choix de Cédric, 2026-11-23 : la
///   fiche ne sait pas si le drone vole avec ; le pilote qui en a la monte lui-même).
/// - Marge de phase : `PHASE_MARGIN_LOW_DEG`, commune à toutes les tailles.
/// - Bruit et retard des filtres : les seuils du profil que comparent les règles de bruit et de retard.
pub fn proposed(row: &TargetRow, p: &CraftProfile) -> Option<f64> {
    match row.key {
        "latency_ms" => Some(p.latency_ms.1),
        "peak_high" => Some(STEP_PEAK_HIGH.0),
        "phase_margin_deg" => Some(PHASE_MARGIN_LOW_DEG),
        "gyro_noise_db" => Some(p.gyro_noise_high_db),
        "dterm_noise_db" => Some(p.dterm_noise_high_db),
        "gyro_filter_delay_ms" => Some(p.gyro_delay_max_ms),
        "dterm_filter_delay_ms" => Some(p.dterm_delay_max_ms),
        _ => None,
    }
}

/// Les valeurs proposées des dix cases pour la clé d'un profil (`five`, `seven`…) ; une clé inconnue n'en propose
/// aucune.
pub fn proposed_values(profile_key: &str) -> [Option<f64>; 10] {
    let Some(p) = profile(profile_key) else {
        return [None; 10];
    };
    ROWS.map(|r| proposed(&r, p))
}

/// Les grandeurs suivies que fixe la fiche cible, une fois chacune, dans l'ordre des cases : le suivi trace un
/// graphique par grandeur (lot 31c), le pic une seule fois pour ses deux bords.
pub fn charted() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::with_capacity(ROWS.len());
    for r in &ROWS {
        if !out.contains(&r.tracked) {
            out.push(r.tracked);
        }
    }
    out
}

/// La bande visée d'une grandeur suivie, `(bas, haut)`, lue dans les cibles des dix cases (`targets`, dans l'ordre de
/// `ROWS`) : une case « ≥ » donne le bas, une case « ≤ » le haut. Un bord sans cible vaut `None` ; les deux à `None`,
/// pas de bande (D-072 : une grandeur sans seuil se trace sans bande).
pub fn band(tracked: &str, targets: &[Option<f64>; 10]) -> (Option<f64>, Option<f64>) {
    let (mut lo, mut hi) = (None, None);
    for (r, t) in ROWS.iter().zip(targets) {
        if r.tracked != tracked || t.is_none() {
            continue;
        }
        match r.bound {
            Bound::AtLeast => lo = *t,
            Bound::AtMost => hi = *t,
        }
    }
    (lo, hi)
}

/// Une valeur telle qu'elle s'affiche dans sa case : virgule décimale, `decimals` chiffres.
pub fn shown(row: &TargetRow, x: f64) -> String {
    fixed(x, row.decimals, false).replace('.', ",")
}

/// Le texte d'une case relu en nombre : vide → `Ok(None)` (pas de cible) ; virgule ou point acceptés ; tout autre
/// texte, ou un nombre infini, → `Err` (le texte lu, pour le message).
pub fn parse(text: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    match t.replace(',', ".").parse::<f64>() {
        Ok(x) if x.is_finite() => Ok(Some(x)),
        _ => Err(t.to_string()),
    }
}
