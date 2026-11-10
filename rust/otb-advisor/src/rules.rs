//! Éléments sans état de l'assistant à règles (`advisor/rules.py`) : gravités, `Finding`, textes de la réponse indicielle et de la
//! boucle ouverte, fréquences d'enregistrement, constat de déséquilibre des moteurs (phase 5a-2a).
//!
//! Les textes sont comparés caractère par caractère à ceux de Python : les nombres passent par `otb-format` (jamais `{:.2}` de Rust, qui
//! n'arrondit pas comme Python à tous les coups sur les cas limites), `.replace(".", ",")` s'applique à la chaîne entière comme en Python.
//! Les comparaisons avec NaN restent fausses (`not x > y` est vrai pour NaN) : les conditions sont recopiées dans le même sens que Python.
//!
//! Phase 5a-2b : `Report`, `Analysis` (= `_Analysis`) et `analyze`, avec les règles qui lisent un `FlightLog` : enregistrement, réponse
//! indicielle, retard consigne → gyro, niveaux de bruit, moteurs, curseurs. **Phase 5a-3a** : `delay_rules` (retards des filtres,
//! relèvement des filtres du D), `predict_filter_change`, `gyro_lpf1_off` et `filter_strategy`.
//! **Phase 5a-3b** : `lowpass_setup_rules`, `rpm_harmonic_levels`, `rpm_weight_rules`, `rpm_q_raise`, les raies du vol stationnaire
//! (`hover_noise`, `residual_line`, `line_shape`) et le notch statique (`static_notch_cutoff`, `static_notch_case`, `dyn_notch_line_actions`).
//! **Phase 5a-3c** : `frame_resonance`, `line_follows_motors`, `frame_resonance_finding`, `dyn_notch_view`, `dyn_notch_q_n_rules` et
//! `filter_rules`, qui assemble toutes les règles de filtres : `analyze` est complet et ses constats, ses mesures et sa stratégie de
//! filtrage sont comparés à Python sur tout le corpus (`otb-parity check-advisor`).

use crate::profiles::{CraftProfile, detect_profile};
use otb_analysis::bf_filters::{self, Target, WhatIfCache, WhatIfSpectrum};
use otb_analysis::delay::{Delays, MOVING_SETPOINT_DPS, compute_delays};
use otb_analysis::motors::{SpectrumLine, hover_motor_hz};
use otb_analysis::npstat::{argmax_f64, mean_f32, median_f32, py_min};
use otb_analysis::numpy_mean;
use otb_analysis::open_loop::{LoopMetrics, MEASURED, Terms};
use otb_analysis::period::crash_time;
use otb_analysis::sliders::{SliderView, slider_view};
use otb_analysis::spectral::{self, Mode, PTB_PSD_OFFSET_DB};
use otb_analysis::stats::{FlightStats, flight_stats, pid_error_stats};
use otb_analysis::step::{LoopInput, StepMetrics, StepOptions, StepResult, step_response};
use otb_analysis::step_series::{FF_KEY, flight_conditions};
use otb_format::{fixed, general};
use otb_log::flightlog::{FlightLog, Rows, clamp_range};
use otb_log::settings::Settings;
use std::ops::Range;

// Stabilité (boucle ouverte mesurée), en conseil : 7 dB et 35° retenus sur le corpus du 2026-10-05.
pub const SENSITIVITY_HIGH_DB: f64 = 7.0;
/// Pic de réponse indicielle (conseil, attention), sans feedforward en service.
pub const STEP_PEAK_HIGH: (f64, f64) = (1.20, 1.25);
/// Idem avec feedforward en service (F > 0 hors mode Angle), qui gonfle le pic (D-025).
pub const STEP_PEAK_HIGH_FF: (f64, f64) = (1.60, 1.70);
pub const STEP_PEAK_FLAT: f64 = 1.03;
/// Réponse molle : pic sous le p10 des axes du corpus et latence au-dessus du p90 de la taille.
pub const STEP_PEAK_SOFT: f64 = 1.01;
/// Fenêtres de la réponse indicielle : lue dès 20 par axe, « mesure moins sûre » sous 200.
pub const STEP_MIN_WINDOWS: usize = 20;
pub const STEP_SURE_WINDOWS: usize = 200;
/// Feedforward : pas de pas chiffré dans un conseil à un seul vol, renvoi à la série du guide.
pub const FF_SERIES_ADVICE: &str = "série Feedforward du guide (curseur « Réponse des sticks » : 0, 0,5, 1… en Acro), en comparant \
le retard consigne → gyro d'un vol à l'autre";
pub const PHASE_MARGIN_LOW_DEG: f64 = 35.0;
/// Repères usuels sur l'échelle des spectres de PIDtoolbox : terme D au plus −10 dB, gyro filtré sous −30 dB.
pub const DTERM_PTB_MAX_DB: f64 = -10.0;
pub const GYRO_PTB_MAX_DB: f64 = -30.0;
/// D dominant au croisement (|Ĉd/Ĉe|) : au-delà, la marge faible ne se corrige plus par du D.
pub const D_DOMINANT_RATIO: f64 = 1.0;

// Niveaux de gravité
pub const INFO: &str = "info";
pub const OK: &str = "ok";
pub const CONSEIL: &str = "conseil";
pub const ATTENTION: &str = "attention";
pub const ALERTE: &str = "alerte";
pub const SEVERITY_ORDER: [(&str, i64); 5] = [(ALERTE, 0), (ATTENTION, 1), (CONSEIL, 2), (INFO, 3), (OK, 4)];

/// Réglages CLI dont une règle a besoin de savoir qu'un constat précédent les conseille déjà (`Finding.settings`).
pub const TRACKED_SETTINGS: [&str; 2] = ["rpm_filter_q", "dyn_notch_count"];
/// Marges de la boucle ouverte mesurée.
pub const LOOP_MARGIN_KEYS: [&str; 2] = ["loop_sensitivity_high", "loop_phase_margin_low"];

// Fréquence d'enregistrement minimale : 1,6 kHz (et 6 × la fréquence moteur si connue) ; 1 kHz pour régler les PID.
pub const LOG_RATE_MIN_HZ: f64 = 1600.0;
/// `blackbox_sample_rate` : 1/1 … 1/16 de la boucle PID.
pub const LOG_RATE_FRACTIONS: [i64; 5] = [1, 2, 4, 8, 16];
pub const PID_LOG_RATE_MIN_HZ: f64 = 1000.0;
pub const PID_LOG_RATE_FLOOR_HZ: f64 = 500.0;

/// Rang d'une gravité pour le tri (`SEVERITY_ORDER.get(severity, 9)`).
pub fn severity_order(severity: &str) -> i64 {
    SEVERITY_ORDER
        .iter()
        .find(|(s, _)| *s == severity)
        .map_or(9, |(_, o)| *o)
}

/// Un constat de l'assistant. ⚠️ Les règles, le diagnostic du bruit et les tests le reconnaissent par sa clé `key`, jamais par son titre
/// (D-028).
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub key: String,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub actions: Vec<String>,
    pub axis: Option<String>,
    /// Réglages de `TRACKED_SETTINGS` que les actions citent.
    pub settings: Vec<String>,
}

impl Finding {
    pub fn new(
        key: &str,
        category: &str,
        severity: &str,
        title: &str,
        detail: &str,
        actions: Vec<String>,
        axis: Option<String>,
    ) -> Finding {
        Finding {
            key: key.to_string(),
            category: category.to_string(),
            severity: severity.to_string(),
            title: title.to_string(),
            detail: detail.to_string(),
            actions,
            axis,
            settings: Vec::new(),
        }
    }
}

/// `AxisStep` : la réponse indicielle d'un axe.
///
/// ⚠️ Dans un rapport rendu par [`analyze`], `result.responses` est **toujours vide** : les fenêtres individuelles sont
/// libérées (voir `step_section`), seules la moyenne, l'écart-type et le nombre de fenêtres restent. Qui a besoin des
/// fenêtres appelle `step_response` lui-même, comme le fait l'outil « Réponse indicielle » (L-094).
#[derive(Debug, Clone)]
pub struct AxisStep {
    pub axis: String,
    pub result: StepResult,
}

/// `Report.sorted()` : tri **stable** par gravité (`sorted` de Python), les gravités inconnues en dernier.
pub fn sorted(findings: &[Finding]) -> Vec<Finding> {
    let mut v = findings.to_vec();
    v.sort_by_key(|f| severity_order(&f.severity));
    v
}

/// « 3 trous dans la période analysée (0,4 s perdues, 1,2 %) ».
pub fn gaps_text(n_gaps: i64, lost_s: f64, span_s: f64) -> String {
    let pct = if span_s + lost_s > 0.0 {
        100.0 * lost_s / (span_s + lost_s)
    } else {
        0.0
    };
    format!(
        "{n_gaps} trou{} dans la période analysée ({} s perdues, {} %)",
        if n_gaps > 1 { "s" } else { "" },
        fixed(lost_s, 1, false),
        fixed(pct, 1, false)
    )
    .replace('.', ",")
}

/// Nombre à deux décimales, virgule à la française (paliers cités dans les textes).
pub fn fr(x: f64) -> String {
    fixed(x, 2, false).replace('.', ",")
}

/// `min(liste)` de Python sur des flottants : le premier des plus petits (un NaN ne remplace rien, et ne se laisse remplacer par rien s'il est en tête).
fn py_min_list(v: &[f64]) -> Option<f64> {
    let mut it = v.iter().copied();
    let mut best = it.next()?;
    for x in it {
        if x < best {
            best = x;
        }
    }
    Some(best)
}

/// La plus lente des fractions de la boucle PID qui atteint `need` (à 5 % près) ; la boucle PID elle-même si aucune n'y suffit.
fn slowest_fraction(pid_hz: f64, need: f64) -> f64 {
    let ok: Vec<f64> = LOG_RATE_FRACTIONS
        .iter()
        .map(|&d| pid_hz / d as f64)
        .filter(|&r| r >= 0.95 * need)
        .collect();
    py_min_list(&ok).unwrap_or(pid_hz)
}

/// Fréquence d'enregistrement à viser : la plus lente des fractions de la boucle PID qui atteint 1,6 kHz (et 6 × la fréquence moteur si
/// elle est connue) ; la boucle PID elle-même si aucune n'y suffit.
pub fn log_rate_needed(pid_hz: f64, motor_hz: Option<f64>) -> f64 {
    let need = match motor_hz {
        None => LOG_RATE_MIN_HZ,
        // max(LOG_RATE_MIN_HZ, 6 × motor_hz) de Python : le premier, sauf si le second est strictement plus grand (NaN : le premier)
        Some(m) => {
            let b = 6.0 * m;
            if b > LOG_RATE_MIN_HZ { b } else { LOG_RATE_MIN_HZ }
        }
    };
    slowest_fraction(pid_hz, need)
}

/// Fréquence d'enregistrement à viser pour un vol de réglage des PID : la plus lente des fractions qui atteint 1 kHz.
pub fn pid_log_rate_needed(pid_hz: f64) -> f64 {
    slowest_fraction(pid_hz, PID_LOG_RATE_MIN_HZ)
}

/// Valeur de `blackbox_sample_rate` pour enregistrer à `rate_hz` : la fraction la plus proche, la première à égalité.
pub fn sample_rate_cli(pid_hz: f64, rate_hz: f64) -> String {
    let key = |d: i64| (pid_hz / d as f64 - rate_hz).abs();
    let mut best = LOG_RATE_FRACTIONS[0];
    let mut best_key = key(best);
    for &d in &LOG_RATE_FRACTIONS[1..] {
        let k = key(d);
        if k < best_key {
            best = d;
            best_key = k;
        }
    }
    format!("1/{best}")
}

/// Précision ajoutée au constat quand la boucle ouverte n'a pas pu être mesurée.
pub fn loop_caveat(lp: &LoopMetrics) -> String {
    if lp.method == MEASURED {
        return String::new();
    }
    let why = if lp.notes.is_empty() {
        String::new()
    } else {
        format!(" ({})", lp.notes.join("; "))
    };
    format!(
        " Valeur approchée{why} : calculée sans séparer le D (sur le gyro) et le feedforward (sur la consigne), elle peut s'écarter de \
plusieurs dB ou dizaines de degrés."
    )
}

/// Précision pour un whoop : marge faible fréquente et réelle, seuil théorique et non percentile du corpus.
pub fn whoop_margin_note(profile: &CraftProfile) -> String {
    if profile.key != "whoop" {
        return String::new();
    }
    " Fréquent sur un whoop : la boucle est plus rapide (croisement vers 20 Hz) et le D y domine, si bien que les filtres du D retardent \
beaucoup la boucle au croisement. Le seuil vient de la théorie (pas d'une comparaison avec les autres whoops) : la marge est vraiment faible."
        .to_string()
}

/// Pic au bord haut de la bande mesurée : la mesure s'arrête où s'arrête l'énergie des mouvements (W-004).
pub fn edge_actions(lp: &LoopMetrics) -> Vec<String> {
    if !lp.sensitivity_peak_at_edge {
        return Vec::new();
    }
    vec![format!(
        "Mesure arrêtée à {} Hz, au bord de ce que vos mouvements excitent : le vrai pic peut être plus haut. Des wobbles plus rapides \
et plus secs, au hasard sur le roll et le pitch en même temps, la font aller plus loin.",
        fixed(lp.sensitivity_peak_hz, 0, false)
    )]
}

/// Que faire d'une marge faible. Quand le D domine déjà le P et le I au croisement (|Cd/Ce| > 1), plus de D apporte peu de marge.
pub fn margin_actions(lp: &LoopMetrics) -> Vec<String> {
    if lp.method != MEASURED {
        return vec![
            "Master −0,1 à −0,2.".to_string(),
            "Pour une mesure exacte, enregistrez les termes P, I, D et F (champs PID du blackbox).".to_string(),
        ];
    }
    if lp.d_ratio.is_finite() && lp.d_ratio > D_DOMINANT_RATIO {
        return vec![format!(
            "Master −0,1 à −0,2. Le D domine déjà au croisement ({} × P et I) : plus de D apporterait peu de marge et peut augmenter le \
pic de sensibilité.",
            fixed(lp.d_ratio, 1, false)
        )];
    }
    vec!["Damping (D) +0,1, ou Master −0,1 à −0,2.".to_string()]
}

/// Interprète le déséquilibre pour un quad en X (ordre Betaflight : 1 AR-D, 2 AV-D, 3 AR-G, 4 AV-G). `Err` là où Python lève (aucun moteur ;
/// moyenne nulle avec 4 moteurs : division par zéro).
pub fn imbalance_finding(m: &[f64], imbalance: f64) -> Result<Finding, String> {
    let values = m
        .iter()
        .enumerate()
        .map(|(i, v)| format!("M{} {} %", i + 1, fixed(*v, 0, false)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut actions = vec![
        "Un moteur qui force seul peut aussi être un moteur abîmé, une hélice tordue ou un ESC fatigué.".to_string(),
    ];
    let worst = argmax_f64(m).ok_or("ValueError : argmax d'une liste vide")?;
    let mut cause = format!(
        "Le moteur {} travaille {} % de plus que la moyenne.",
        worst + 1,
        fixed(imbalance, 0, false)
    );
    if m.len() == 4 {
        let mean = numpy_mean(m);
        let front = (m[1] + m[3]) / 2.0;
        let rear = (m[0] + m[2]) / 2.0;
        let left = (m[2] + m[3]) / 2.0;
        let right = (m[0] + m[1]) / 2.0;
        let diag_a = (m[0] + m[3]) / 2.0;
        let diag_b = (m[1] + m[2]) / 2.0;
        let gaps = [
            ("avant", front - rear),
            ("arrière", rear - front),
            ("gauche", left - right),
            ("droite", right - left),
        ];
        // max(gaps.items(), key=…) : le premier des plus grands
        let (mut side, mut gap) = gaps[0];
        for &(s, g) in &gaps[1..] {
            if g > gap {
                side = s;
                gap = g;
            }
        }
        let dgap = (diag_a - diag_b).abs();
        if mean == 0.0 {
            return Err("ZeroDivisionError : float division by zero".to_string());
        }
        if gap / mean > 0.12 && gap >= dgap {
            let pct = fixed(100.0 * gap / mean, 0, false);
            cause = if side == "avant" || side == "arrière" {
                format!("Les moteurs côté {side} travaillent {pct} % de plus : centre de gravité décalé vers l'{side}")
            } else {
                format!("Les moteurs côté {side} travaillent {pct} % de plus : centre de gravité décalé à {side}.")
            };
            actions.insert(
                0,
                format!("Déplacez la batterie (ou la caméra) pour recentrer le poids, à l'opposé du côté {side}."),
            );
        } else if dgap / mean > 0.12 {
            let pair = if diag_a > diag_b { "M1/M4" } else { "M2/M3" };
            cause = format!(
                "Les moteurs en diagonale {pair} travaillent {} % de plus : le drone compense un couple en lacet (yaw).",
                fixed(100.0 * dgap / mean, 0, false)
            );
            actions.insert(
                0,
                "Vérifiez le sens et le modèle des hélices, et qu'un moteur ne frotte pas ; un châssis vrillé ou un moteur penché donne \
le même symptôme."
                    .to_string(),
            );
        }
    }
    Ok(Finding::new(
        "motor_imbalance",
        "Mécanique",
        CONSEIL,
        "Moteurs déséquilibrés",
        &format!("{cause} ({values})."),
        actions,
        None,
    ))
}

/// Règles de la réponse indicielle d'un axe : rien sous `STEP_MIN_WINDOWS` fenêtres ; sous `STEP_SURE_WINDOWS`, chaque constat (sauf les
/// marges de la boucle ouverte, qui ont leur propre réserve) reçoit la mention « mesure moins sûre ». `ff_gain` : gain F en service (0 en
/// mode Angle) ; `ff_cut` : F > 0 dans l'en-tête mais coupé par le mode Angle.
pub fn step_rules(f: &mut Vec<Finding>, st: &AxisStep, profile: &CraftProfile, ff_gain: i64, ff_cut: bool) {
    let r = &st.result;
    if r.n < STEP_MIN_WINDOWS {
        return;
    }
    let first = f.len();
    step_findings(f, st, profile, ff_gain, ff_cut);
    if r.n < STEP_SURE_WINDOWS {
        for x in f[first..].iter_mut() {
            if LOOP_MARGIN_KEYS.contains(&x.key.as_str()) {
                continue; // boucle ouverte : précision non mesurée ici, elle a sa propre réserve (loop_caveat)
            }
            x.detail += &format!(
                " Mesure moins sûre : {} fenêtres ({STEP_SURE_WINDOWS} conseillées, soit environ 20 s de wobbles roll et pitch mêlés).",
                r.n
            );
        }
    }
}

/// Constats de la réponse indicielle d'un axe (dépassement, réponse molle, réponse correcte) puis de la boucle ouverte (marges).
pub fn step_findings(f: &mut Vec<Finding>, st: &AxisStep, profile: &CraftProfile, ff_gain: i64, ff_cut: bool) {
    let r = &st.result;
    let ax = st.axis.as_str();
    let axl = ax.to_lowercase();
    let lp = &r.loop_;
    let (pk, lat) = (r.metrics.peak, r.metrics.latency_ms);
    let (advice, strong) = if ff_gain > 0 { STEP_PEAK_HIGH_FF } else { STEP_PEAK_HIGH };
    let with_ff = if ff_gain > 0 {
        format!(" avec feedforward (F = {ff_gain})")
    } else if ff_cut {
        " sans feedforward (coupé par le mode Angle)".to_string()
    } else {
        " sans feedforward".to_string()
    };
    let (lat0, lat1) = profile.latency_ms;
    let axis = Some(ax.to_string());
    if pk > advice && ff_gain > 0 && lat.is_finite() && lat < lat0 {
        // Réponse plus rapide que ce que P et D permettent seuls, puis dépassement : c'est le feedforward qui pousse.
        f.push(Finding::new(
            "step_ff_overshoot",
            "Réponse",
            if pk <= strong { CONSEIL } else { ATTENTION },
            &format!("Dépassement dû au feedforward en {axl}"),
            &format!(
                "Pic {} atteint après une montée très rapide (latence {} ms, plus courte que les {}–{} ms habituels pour {}). \
Le feedforward (F = {ff_gain}) anticipe le mouvement et le drone dépasse la consigne.",
                fixed(pk, 2, false),
                fixed(lat, 1, false),
                fixed(lat0, 0, false),
                fixed(lat1, 0, false),
                profile.label
            ),
            vec![
                format!("Refaites la {FF_SERIES_ADVICE} ; ou feedforward_boost plus bas."),
                "Si vous aimez cette vivacité (freestyle, course), ce n'est pas un défaut : vérifiez seulement qu'il n'y a pas de rebond à \
la fin des flips."
                    .to_string(),
                "Le D n'est pas en cause tant que les vols calmes ne dépassent pas.".to_string(),
            ],
            axis,
        ));
        return;
    }
    if pk > strong {
        f.push(Finding::new(
            "step_overshoot_strong",
            "Réponse",
            ATTENTION,
            &format!("Fort dépassement en {axl}"),
            &format!(
                "Pic {} (> {}{with_ff}, plus que 95 % des axes du corpus) : le drone dépasse la consigne puis revient, signe d'un manque \
d'amortissement.",
                fixed(pk, 2, false),
                fr(strong)
            ),
            vec!["Curseur Damping (D) : +0,2.".to_string()],
            axis.clone(),
        ));
    } else if pk > advice {
        f.push(Finding::new(
            "step_overshoot_light",
            "Réponse",
            CONSEIL,
            &format!("Léger dépassement en {axl}"),
            &format!(
                "Pic {} (> {}{with_ff}, plus que 90 % des drones du corpus). Sans feedforward, une réponse sans dépassement visible donne \
environ {} ; le feedforward fait monter le pic.",
                fixed(pk, 2, false),
                fr(advice),
                fr(STEP_PEAK_FLAT)
            ),
            vec!["Curseur Damping (D) : +0,1.".to_string()],
            axis.clone(),
        ));
    } else if pk < STEP_PEAK_SOFT && lat.is_finite() && lat > lat1 {
        f.push(Finding::new(
            "step_soft",
            "Réponse",
            CONSEIL,
            &format!("Réponse molle en {axl}"),
            &format!(
                "Pic {} et latence {} ms (repère {}–{} ms pour {}) : trop amorti.",
                fixed(pk, 2, false),
                fixed(lat, 1, false),
                fixed(lat0, 0, false),
                fixed(lat1, 0, false),
                profile.label
            ),
            vec![
                "Curseur Damping (D) : −0,1.".to_string(),
                format!("Pour la réactivité au manche : {FF_SERIES_ADVICE}."),
            ],
            axis.clone(),
        ));
    } else {
        f.push(Finding::new(
            "step_ok",
            "Réponse",
            OK,
            &format!("Réponse indicielle {axl}"),
            &format!(
                "Pic {}, latence {} ms, {} fenêtres analysées.",
                fixed(pk, 2, false),
                fixed(lat, 1, false),
                r.n
            ),
            Vec::new(),
            axis.clone(),
        ));
    }
    if lp.sensitivity_peak_db.is_finite() && lp.sensitivity_peak_db > SENSITIVITY_HIGH_DB {
        let at_least = if lp.sensitivity_peak_at_edge { "au moins " } else { "" };
        let mut actions = margin_actions(lp);
        actions.push("Réduire le retard des filtres si le bruit le permet.".to_string());
        actions.extend(edge_actions(lp));
        f.push(Finding::new(
            "loop_sensitivity_high",
            "Réponse",
            CONSEIL,
            &format!("Marge de stabilité faible en {axl}"),
            &format!(
                "Pic de sensibilité {at_least}{} dB à {} Hz (> {} dB) : le drone amplifie les perturbations à cette fréquence et risque \
d'osciller.{}{}",
                fixed(lp.sensitivity_peak_db, 1, false),
                fixed(lp.sensitivity_peak_hz, 0, false),
                fixed(SENSITIVITY_HIGH_DB, 0, false),
                loop_caveat(lp),
                whoop_margin_note(profile)
            ),
            actions,
            axis,
        ));
    } else if lp.phase_margin_deg.is_finite() && lp.phase_margin_deg < PHASE_MARGIN_LOW_DEG {
        f.push(Finding::new(
            "loop_phase_margin_low",
            "Réponse",
            CONSEIL,
            &format!("Marge de phase réduite en {axl}"),
            &format!(
                "Marge de phase {}° (< {}°) au croisement de {} Hz.{}{}",
                fixed(lp.phase_margin_deg, 0, false),
                fixed(PHASE_MARGIN_LOW_DEG, 0, false),
                fixed(lp.crossover_hz, 0, false),
                loop_caveat(lp),
                whoop_margin_note(profile)
            ),
            margin_actions(lp),
            axis,
        ));
    }
}

// ---------------------------------------------------------------- phase 5a-2b : règles qui lisent un journal de vol

const AXES: [&str; 3] = ["Roll", "Pitch", "Yaw"];

fn fx(x: f64, prec: usize) -> String {
    fixed(x, prec, false)
}

/// `max(a, b)` de Python : `a`, sauf si `b > a` (un NaN en `a` reste, un NaN en `b` ne remplace rien).
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// `max(liste)` de Python : le premier des plus grands, dans l'ordre (dépend de la place d'un NaN). `None` si vide (Python lève).
fn py_max_list(v: &[f64]) -> Option<f64> {
    let (first, rest) = v.split_first()?;
    Some(rest.iter().fold(*first, |m, &x| py_max(m, x)))
}

/// Mesures du rapport (`Report.measures`, un dict en Python) sous les noms de Python. `None` : la section n'a pas tourné (toutes sont `Some`
/// dans un rapport rendu par `analyze`), sauf `hover_motor_hz` où `None` est la valeur `None` de Python.
/// Exceptions : `frame_resonance` et `dyn_notch` valent `None` comme en Python quand il n'y a pas de raie ou pas de notch dynamique à
/// juger ; `motor_harmonic_left_db` et `motor_harmonic_raw_db` n'existent en Python que si le journal a l'eRPM, le gyro non filtré et un
/// filtre RPM (clé absente = `None`), de même que `hover_lines` (écrit par `hover_noise`, seulement en vol stationnaire).
#[derive(Debug, Clone, Default)]
pub struct Measures {
    pub hover_motor_hz: Option<f64>,
    pub log_rate_needed_hz: Option<(f64, f64)>,
    pub aliased: Option<bool>,
    /// `{axe: métriques}` dans l'ordre Roll, Pitch, Yaw.
    pub steps: Option<Vec<(String, StepMetrics)>>,
    pub delays: Option<Delays>,
    pub gyro_noise_db: Option<Vec<f64>>,
    pub dterm_noise_db: Option<Vec<f64>>,
    pub low_band_db: Option<Vec<f64>>,
    pub gyro_peak_ptb_db: Option<f64>,
    pub dterm_peak_ptb_db: Option<f64>,
    pub stats: Option<FlightStats>,
    pub pid_error_sd: Option<Vec<f64>>,
    pub iterm_mean: Option<Vec<f64>>,
    pub sliders: Option<SliderView>,
    /// Raies hors moteurs du vol stationnaire (`_hover_noise`, 5a-3b), la plus haute d'abord.
    pub hover_lines: Option<Vec<SpectrumLine>>,
    /// Raie la plus marquée quel que soit le gaz (`frame_resonance`) ; `None` : le journal ne couvre pas assez de tranches de gaz.
    pub frame_resonance: Option<FrameResonance>,
    /// Pics que le notch dynamique a à suivre (`dyn_notch_view`) ; `None` : pas de notch dynamique en service avec le filtre RPM.
    pub dyn_notch: Option<DynNotch>,
    /// Niveau de la fréquence de rotation des moteurs dans le gyro filtré, puis non filtré (dB, maximum du roll et du pitch).
    pub motor_harmonic_left_db: Option<f64>,
    pub motor_harmonic_raw_db: Option<f64>,
}

/// `Report` : le diagnostic d'un vol.
#[derive(Debug, Clone)]
pub struct Report {
    pub profile: CraftProfile,
    pub profile_reason: String,
    pub period: (f64, f64),
    pub findings: Vec<Finding>,
    pub steps: Vec<AxisStep>,
    pub measures: Measures,
}

impl Report {
    pub fn sorted(&self) -> Vec<Finding> {
        sorted(&self.findings)
    }
}

/// `_Analysis` : état partagé par les sections d'`analyze` (le journal, la période, le profil, les constats accumulés et les mesures), puis
/// ce qu'une section calcule pour les suivantes.
pub struct Analysis<'a> {
    pub log: &'a FlightLog,
    pub sel: Range<usize>,
    pub profile: &'a CraftProfile,
    /// Constats accumulés, dans l'ordre d'affichage à gravité égale (`F`).
    pub f: Vec<Finding>,
    pub meas: Measures,
    /// Spectre replié : on n'allège aucun filtre sur cette base (`recording_rules`).
    pub aliased: bool,
    pub steps: Vec<AxisStep>,
    /// Bande de bruit 80 Hz → `hi`, roll / pitch (`noise_rules`).
    pub hi: f64,
    pub g_db: Vec<f64>,
    pub d_db: Vec<f64>,
    pub low_db: Vec<f64>,
    pub g_ptb: f64,
    pub d_ptb: f64,
    pub gyro_noisy: bool,
    pub dterm_noisy: bool,
    pub delays: Option<Delays>,
    /// Raccords du journal : aucune fenêtre d'analyse ne les chevauche (D-020).
    pub br: Vec<i64>,
}

impl Analysis<'_> {
    pub fn fs(&self) -> f64 {
        self.log.fs
    }

    /// Vol stationnaire : `not self.delays.moving` (les retards sont calculés par `noise_rules`).
    pub fn hover(&self) -> Result<bool, String> {
        Ok(!self
            .delays
            .as_ref()
            .ok_or("AttributeError : delays n'est pas calculé")?
            .moving())
    }
}

/// Ligne `ax` d'un tableau, sur `sel` (une tranche Python borne ses indices).
pub(crate) fn sel_row<'r>(r: &'r Rows, ax: usize, sel: &Range<usize>) -> &'r [f32] {
    let row = r.row(ax);
    &row[clamp_range(sel.start, sel.end, row.len())]
}

fn wide(v: &[f32]) -> Vec<f64> {
    v.iter().map(|&x| x as f64).collect()
}

/// `np.any(row)` d'une ligne `float32` : un NaN compte comme non nul.
pub(crate) fn any_nonzero(row: &[f32]) -> bool {
    row.iter().any(|&v| v != 0.0)
}

/// `_band_db` : niveau moyen (dB, densité physique) du spectre de `x` entre `lo` et `hi` (NaN si la bande est vide).
pub fn band_db(x: &[f32], fs: f64, lo: f64, hi: f64, breaks: Option<&[i64]>) -> Result<f64, String> {
    let s = spectral::spectrum(&wide(x), fs, Mode::Psd, 2.0, 0, breaks)?;
    let hi = py_min(hi, 0.95 * fs / 2.0);
    let m: Vec<f64> = s
        .freqs
        .iter()
        .zip(&s.values)
        .filter(|(f, _)| **f >= lo && **f <= hi)
        .map(|(_, v)| *v)
        .collect();
    Ok(if m.is_empty() { f64::NAN } else { numpy_mean(&m) })
}

/// Sommet du spectre lissé dans la bande : (fréquence en Hz, niveau sur l'échelle de PIDtoolbox).
pub fn band_peak_ptb(x: &[f32], fs: f64, lo: f64, hi: f64, breaks: Option<&[i64]>) -> Result<(f64, f64), String> {
    let s = spectral::spectrum(&wide(x), fs, Mode::Psd, 2.0, 2, breaks)?;
    let hi = py_min(hi, 0.95 * fs / 2.0);
    let (freqs, values): (Vec<f64>, Vec<f64>) = s
        .freqs
        .iter()
        .zip(&s.values)
        .filter(|(f, _)| **f >= lo && **f <= hi)
        .map(|(f, v)| (*f, *v))
        .unzip();
    let Some(k) = argmax_f64(&values) else {
        return Ok((f64::NAN, f64::NAN));
    };
    Ok((freqs[k], values[k] - PTB_PSD_OFFSET_DB))
}

/// Sommet du spectre lissé dans la bande, sur l'échelle de PIDtoolbox (ce que l'œil lit sur sa courbe).
pub fn band_peak_ptb_db(x: &[f32], fs: f64, lo: f64, hi: f64, breaks: Option<&[i64]>) -> Result<f64, String> {
    Ok(band_peak_ptb(x, fs, lo, hi, breaks)?.1)
}

/// `analyze` : le diagnostic complet d'un vol. `sel` : période (tout le journal si absente) ; `profile` : taille choisie (détectée si absente).
pub fn analyze(log: &FlightLog, sel: Option<Range<usize>>, profile: Option<&CraftProfile>) -> Result<Report, String> {
    let sel = sel.unwrap_or(0..log.n());
    let (profile, reason) = match profile {
        Some(p) => (p, "choisi par l'utilisateur".to_string()),
        None => detect_profile(log),
    };
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let mut a = Analysis {
        log,
        sel: sel.clone(),
        profile,
        f: Vec::new(),
        meas: Measures::default(),
        aliased: false,
        steps: Vec::new(),
        hi: 500.0,
        g_db: Vec::new(),
        d_db: Vec::new(),
        low_db: Vec::new(),
        g_ptb: f64::NAN,
        d_ptb: f64::NAN,
        gyro_noisy: false,
        dterm_noisy: false,
        delays: None,
        br,
    };
    if profile.provisional {
        a.f.push(Finding::new(
            "profile_provisional",
            "Réglage",
            INFO,
            "Seuils provisoires pour cette taille",
            &format!(
                "Le corpus de calibration compte trop peu de drones « {} » : les seuils de bruit et de mécanique sont indicatifs.",
                profile.label
            ),
            Vec::new(),
            None,
        ));
    }
    // log.t[sel.start] et log.t[min(sel.stop, n) - 1] : un indice hors du journal fait lever Python (IndexError)
    let n = log.n();
    let t0 = *log.t.get(sel.start).ok_or("IndexError : index hors du journal")?;
    let last = sel.end.min(n);
    let t1_idx = if last == 0 { n.checked_sub(1) } else { Some(last - 1) }; // indice -1 de Python : le dernier
    let t1 = *t1_idx
        .and_then(|i| log.t.get(i))
        .ok_or("IndexError : index hors du journal")?;
    recording_rules(&mut a, t1 - t0)?;
    step_section(&mut a)?;
    noise_rules(&mut a)?;
    delay_rules(&mut a)?;
    filter_rules(&mut a)?;
    motor_rules(&mut a)?;
    slider_rules(&mut a)?;
    let moving = !a.hover()?;
    if let Some(strategy) = filter_strategy(&a.f, moving)? {
        a.f.insert(0, strategy);
    }
    Ok(Report {
        profile: profile.clone(),
        profile_reason: reason,
        period: (t0, t1),
        findings: a.f,
        steps: a.steps,
        measures: a.meas,
    })
}

// ---------------------------------------------------------------- phase 5a-3a : retards des filtres, relèvement du D, stratégie

/// Terme D « idéalement sous −20 dB » PIDtoolbox : en dessous, le simulateur cherche jusqu'où relever ses filtres.
pub const DTERM_PTB_IDEAL_DB: f64 = -20.0;
/// Borne du curseur « filtre D » hors mode expert du configurateur.
pub const DTERM_SLIDER_BASIC_MAX: i64 = 120;
/// Sans curseur, ou au-delà de 1,2 : nombre de pas au plus (coupures × 1,1 par pas).
pub const DTERM_LIFT_MAX_STEPS: i64 = 2;
/// Filtre RPM : `rpm_filter_q` monte par pas de 100 jusqu'à 1000 au plus.
pub const RPM_Q_STEP: i64 = 100;
pub const RPM_Q_MAX: i64 = 1000;

/// Étapes de la stratégie de filtrage : (étape, clés des constats qui en relèvent), dans l'ordre d'affichage.
pub const FILTER_STAGES: [(&str, &[&str]); 7] = [
    (
        "Enregistrement",
        &["log_rate_low", "log_rate_low_filters", "log_rate_harmonics"],
    ),
    (
        "À corriger d'abord",
        &["gyro_lpf2_off", "dterm_filter_weak", "gyro_noisy", "dterm_noisy"],
    ),
    ("Mécanique", &["frame_line_fixed"]),
    (
        "Filtre RPM",
        &[
            "rpm_q_raise",
            "rpm_harmonic_missing",
            "rpm_weight_low",
            "rpm_line_visible",
            "motor_noise_left",
            "rpm_weight_lighten",
        ],
    ),
    (
        "Notch dynamique",
        &[
            "hover_line_residual",
            "dyn_notch_q_lower",
            "dyn_notch_q_raise",
            "dyn_notch_more",
            "dyn_notch_fewer",
        ],
    ),
    (
        "Passe-bas du gyro",
        &["gyro_lpf1_off", "gyro_filter_cautious", "gyro_lpf_biquad"],
    ),
    (
        "Passe-bas du D",
        &["dterm_lpf1_static", "dterm_filter_too_strong", "dterm_lift"],
    ),
];
/// Raies qui passent les filtres (mécanique ou stationnaire).
pub const LINE_KEYS: [&str; 2] = ["frame_line_fixed", "hover_line_residual"];
/// Stratégie affichée à partir de 2 étapes.
pub const STRATEGY_MIN_STAGES: usize = 2;

/// Effet prévu de réglages de filtres modifiés (`predict_filter_change`) : variation du bruit moyen et du sommet (dB, par axe roll / pitch)
/// du gyro filtré et du terme D, et gain de retard (ms) des chaînes gyro et D.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    pub gyro_mean: Vec<f64>,
    pub gyro_peak: Vec<f64>,
    pub d_mean: Vec<f64>,
    pub d_peak: Vec<f64>,
    pub gyro_delay_gain_ms: f64,
    pub d_delay_gain_ms: f64,
}

/// `np.convolve(v, np.ones(5) / 5, mode="same")` : `v` non vide. Pour au moins 5 valeurs, même longueur que `v` ; en dessous, numpy échange
/// les deux tableaux et rend 5 valeurs (décalage `(len − 1) / 2` dans la convolution complète).
///
/// Pour au moins 5 valeurs, **identique au bit près** à numpy (mesuré : 0 écart sur 52 240 valeurs de longueurs 5 à 1 001, et sur 10 000
/// valeurs aléatoires ; `scripts/smooth_cases.py`) : au centre, somme **croissante sans FMA** (`acc + x × 0,2`) ; sur les deux premières et les
/// deux dernières valeurs (fenêtre tronquée à 3 ou 4 termes), somme croissante **avec** FMA, comme le produit scalaire de numpy (OpenBLAS) pour
/// les fenêtres courtes. Pour 15 valeurs, [`smooth15`] fusionne partout (L-061). En dessous de 5 valeurs, aucune variante essayée (ordre
/// croissant / décroissant, avec ou sans FMA) ne rend les bits de numpy : somme décroissante sans FMA, juste à la tolérance près.
/// ⚠️ Un autre ordre de somme ou un FMA mal placé diffère au dernier bit, invisible à la tolérance du corpus (L-061).
pub fn smooth5(v: &[f64]) -> Vec<f64> {
    let k = 5; // largeur du noyau
    let kern = 1.0f64 / 5.0; // np.ones(5) / 5 : cinq fois 0,2 (1/5 arrondi)
    let n = v.len();
    if n >= k {
        return (0..n)
            .map(|i| {
                let window = &v[i.saturating_sub(2)..(i + 3).min(n)];
                if i < 2 || i >= n - 2 {
                    window.iter().fold(0.0f64, |acc, &x| x.mul_add(kern, acc))
                } else {
                    window.iter().fold(0.0f64, |acc, &x| acc + x * kern)
                }
            })
            .collect();
    }
    let full_len = n + k - 1;
    let full: Vec<f64> = (0..full_len)
        .map(|i| {
            // somme de v[i - j] × kern pour j = 0..5, dans l'ordre des termes de la convolution complète (v décroissant)
            let mut acc = 0.0f64;
            for j in 0..k {
                if i >= j && i - j < n {
                    acc += v[i - j] * kern;
                }
            }
            acc
        })
        .collect();
    let off = (n.min(k) - 1) / 2;
    full[off..off + k].to_vec()
}

/// `np.max` : un NaN l'emporte. Vide : `None` (numpy lève).
fn np_max_nan(v: &[f64]) -> Option<f64> {
    let (first, rest) = v.split_first()?;
    let mut m = *first;
    for &x in rest {
        if m.is_nan() {
            break;
        }
        if x.is_nan() || x > m {
            m = x;
        }
    }
    Some(m)
}

/// `deltas` de `predict_filter_change` : (variation du niveau moyen, variation du sommet du spectre lissé) entre 80 Hz et `hi`.
/// `None` si aucune fréquence n'est dans la bande (session de 0,1 à 0,2 s) : pas de prévision pour cet axe, comme Python.
fn whatif_deltas(w: &WhatIfSpectrum, hi: f64) -> Result<Option<(f64, f64)>, String> {
    let idx: Vec<usize> = (0..w.freqs.len())
        .filter(|&i| w.freqs[i] >= 80.0 && w.freqs[i] <= hi)
        .collect();
    let cur: Vec<f64> = idx.iter().map(|&i| w.current_db[i]).collect();
    let new: Vec<f64> = idx.iter().map(|&i| w.modified_db[i]).collect();
    if idx.is_empty() {
        return Ok(None);
    }
    let peak = |v: &[f64]| np_max_nan(&smooth5(v)).ok_or("ValueError : max() d'une liste vide".to_string());
    Ok(Some((numpy_mean(&new) - numpy_mean(&cur), peak(&new)? - peak(&cur)?)))
}

fn widen_row(r: &Rows, ax: usize) -> Vec<f64> {
    wide(r.row(ax))
}

/// `predict_filter_change` : effet de réglages de filtres `modf` (simulateur `whatif_spectrum`, exact quand rien ne change) sur le bruit
/// et sur les retards des chaînes gyro et D (au gaz médian).
pub fn predict_filter_change(
    log: &FlightLog,
    sel: &Range<usize>,
    modf: &Settings,
    hi: f64,
) -> Result<Prediction, String> {
    let s = &log.settings;
    let mut out = Prediction {
        gyro_mean: Vec::new(),
        gyro_peak: Vec::new(),
        d_mean: Vec::new(),
        d_peak: Vec::new(),
        gyro_delay_gain_ms: 0.0,
        d_delay_gain_ms: 0.0,
    };
    let mut cache = WhatIfCache::default(); // réponses des filtres par fenêtre, communes aux deux axes et au gyro / D
    let mut prefilter: Option<Rows> = None; // `log.d_term_prefilter` (propriété mise en cache en Python)
    for a in 0..2 {
        if let Some(unf) = &log.gyro_unfilt {
            let w = bf_filters::whatif_spectrum(
                &widen_row(&log.gyro, a),
                Some(&widen_row(unf, a)),
                log,
                sel.clone(),
                s,
                modf,
                Target::Gyro,
                300.0,
                &mut cache,
            )?;
            if let Some(g) = whatif_deltas(&w, hi)? {
                out.gyro_mean.push(g.0);
                out.gyro_peak.push(g.1);
            }
        }
        if any_nonzero(log.d_term.row(a)) {
            if prefilter.is_none() {
                prefilter = Some(log.d_term_prefilter()?);
            }
            let pre = prefilter.as_ref().unwrap_or(&log.d_term);
            let w = bf_filters::whatif_spectrum(
                &widen_row(&log.d_term, a),
                Some(&widen_row(pre, a)),
                log,
                sel.clone(),
                s,
                modf,
                Target::Dterm,
                300.0,
                &mut cache,
            )?;
            if let Some(d) = whatif_deltas(&w, hi)? {
                out.d_mean.push(d.0);
                out.d_peak.push(d.1);
            }
        }
    }
    let thr = median_f32(&log.throttle[clamp_range(sel.start, sel.end, log.throttle.len())]) as f64 / 100.0;
    let delay =
        |chain: &bf_filters::Chain| -> Result<f64, String> { Ok(bf_filters::chain_delay_ms(chain, 10.0)?.phase_ms) };
    out.gyro_delay_gain_ms = delay(&bf_filters::gyro_chain(s, thr, None, Some(&[]))?)?
        - delay(&bf_filters::gyro_chain(modf, thr, None, Some(&[]))?)?;
    out.d_delay_gain_ms = delay(&bf_filters::dterm_chain(s, thr))? - delay(&bf_filters::dterm_chain(modf, thr))?;
    Ok(out)
}

/// `_noise_after` : bruits prévus (maximum roll / pitch) : gyro moyen, D moyen, sommet du D (échelle PIDtoolbox). `None` si la mesure manque.
pub fn noise_after(
    pred: &Prediction,
    d_db: &[f64],
    d_ptb: f64,
    g_db: Option<&[f64]>,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    let sum_max = |base: &[f64], delta: &[f64]| -> Option<f64> {
        let v: Vec<f64> = base.iter().zip(delta).map(|(x, dx)| x + dx).collect();
        py_max_list(&v)
    };
    let g = g_db.and_then(|g| sum_max(g, &pred.gyro_mean));
    let d = sum_max(d_db, &pred.d_mean);
    let dp = if d_ptb.is_finite() {
        py_max_list(&pred.d_peak).map(|m| d_ptb + m)
    } else {
        None
    };
    (g, d, dp)
}

/// Un curseur de l'en-tête (`Settings.sliders().get(clé)`).
fn slider_of(s: &Settings, key: &str) -> Option<i64> {
    s.sliders().into_iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// Multiplicateur du curseur « filtre D » s'il est actif, sinon -1.
fn dterm_mult(s: &Settings) -> i64 {
    if slider_of(s, "dterm_filter").unwrap_or(0) > 0 {
        slider_of(s, "dterm_mult").unwrap_or(-1)
    } else {
        -1
    }
}

/// Copie des réglages avec les passe-bas du D multipliés par `factor`.
fn scaled_dterm(s: &Settings, factor: f64) -> Result<Settings, String> {
    let mut m = s.clone();
    m.dterm_lpf1 = bf_filters::scale_lowpass(&s.dterm_lpf1, factor)?;
    m.dterm_lpf2 = bf_filters::scale_lowpass(&s.dterm_lpf2, factor)?;
    Ok(m)
}

/// `_gyro_lpf1_off` : LPF1 du gyro superflu avec le filtre RPM, si le simulateur prévoit un bruit encore sous les repères.
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
pub fn gyro_lpf1_off(
    log: &FlightLog,
    sel: &Range<usize>,
    hi: f64,
    profile: &CraftProfile,
    g_db: &[f64],
    d_db: &[f64],
    d_ptb: f64,
    hover: bool,
) -> Result<Option<Finding>, String> {
    let s = &log.settings;
    let mut modf = s.clone();
    modf.gyro_lpf1 = bf_filters::lowpass_off(&s.gyro_lpf1);
    let pred = predict_filter_change(log, sel, &modf, hi)?;
    let (g, d, dp) = noise_after(&pred, d_db, d_ptb, Some(g_db));
    let (Some(g), Some(d), Some(dp)) = (g, d, dp) else {
        return Ok(None);
    };
    if g > profile.gyro_noise_high_db || d > profile.dterm_noise_high_db || dp > DTERM_PTB_MAX_DB {
        return Ok(None);
    }
    let lp = &s.gyro_lpf1;
    let now = if lp.dynamic() {
        format!("{}–{} Hz dynamique", lp.dyn_min_hz, lp.dyn_max_hz)
    } else {
        format!("{} Hz", lp.static_hz)
    };
    let hover_note = if hover {
        " Vol stationnaire : bruit vu à un seul régime, à confirmer sur un vol avec des gaz plus hauts."
    } else {
        ""
    };
    Ok(Some(Finding::new(
        "gyro_lpf1_off",
        "Filtres",
        CONSEIL,
        "Le LPF1 du gyro peut être coupé",
        &format!(
            "Avec le filtre RPM, le LPF1 du gyro ({} {now}) est souvent superflu. Sans lui, le simulateur \
             prévoit {} ms de retard en moins, un gyro filtré à {} dB \
             (seuil {}), un terme D à {} dB (seuil {}) \
             et un sommet du D à {} dB PIDtoolbox (repère {}). Retard calculé \
             d'après les réglages des filtres.{hover_note}",
            lp.kind,
            fx(pred.gyro_delay_gain_ms, 2),
            fx(g, 1),
            fx(profile.gyro_noise_high_db, 1),
            fx(d, 1),
            fx(profile.dterm_noise_high_db, 1),
            fx(dp, 0),
            fx(DTERM_PTB_MAX_DB, 0),
        ),
        vec![
            "gyro_lpf1_static_hz = 0 et gyro_lpf1_dyn_min_hz = 0 (le curseur « filtre gyro » ne rallume pas un \
             filtre coupé)."
                .to_string(),
            "Vol d'essai court, puis vérifiez la température des moteurs et le spectre du log suivant.".to_string(),
        ],
        None,
    )))
}

/// `_dterm_lift` : retard du D au-dessus du repère avec un D peu bruité : relever ses filtres seulement si le simulateur prévoit un D encore
/// sous les repères avec le curseur « filtre D » +0,1.
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
pub fn dterm_lift(
    log: &FlightLog,
    sel: &Range<usize>,
    hi: f64,
    profile: &CraftProfile,
    delay_ms: f64,
    d_db: &[f64],
    d_ptb: f64,
    d_goal: &str,
) -> Result<Finding, String> {
    let s = &log.settings;
    let mult = dterm_mult(s);
    // curseur +0,1 = multiplicateur +10 (simplified_tuning.c)
    let factor = if mult > 0 {
        (mult + 10) as f64 / mult as f64
    } else {
        1.1
    };
    let modf = scaled_dterm(s, factor)?;
    let pred = if any_nonzero(log.d_term.row(0)) {
        Some(predict_filter_change(log, sel, &modf, hi)?)
    } else {
        None
    };
    let head = format!(
        "Retard des filtres D {} ms (repère {} ms) avec un D peu bruité.",
        fx(delay_ms, 2),
        fx(profile.dterm_delay_max_ms, 1)
    );
    let Some(pred) = pred else {
        return Ok(Finding::new(
            "dterm_filter_too_strong",
            "Filtres",
            CONSEIL,
            "Filtrage du D peut-être trop fort",
            &format!("{head}{d_goal}"),
            vec!["Curseur « filtre D » : +0,1.".to_string()],
            None,
        ));
    };
    let (_, d, dp) = noise_after(&pred, d_db, d_ptb, None);
    // `{d:.1f}` d'un `None` fait lever Python (TypeError) ; seule la branche de repli du `if` plus bas lit `d` et `dp` sans les tester
    let (d_txt, dp_txt) = match (d, dp) {
        (Some(d), Some(dp)) => (fx(d, 1), fx(dp, 0)),
        _ => return Err("TypeError : unsupported format string passed to NoneType.__format__".into()),
    };
    let forecast = format!(
        " Avec le curseur « filtre D » +0,1 (coupures ×{}), le simulateur prévoit {} ms de retard en moins, un terme D à {} dB (seuil {}) \
         et un sommet à {} dB PIDtoolbox (repère {}).",
        fx(factor, 2),
        fx(pred.d_delay_gain_ms, 2),
        d_txt,
        fx(profile.dterm_noise_high_db, 1),
        dp_txt,
        fx(DTERM_PTB_MAX_DB, 0)
    );
    let (d, dp) = (d.unwrap_or(f64::NAN), dp.unwrap_or(f64::NAN));
    if d <= profile.dterm_noise_high_db && dp <= DTERM_PTB_MAX_DB {
        return Ok(Finding::new(
            "dterm_filter_too_strong",
            "Filtres",
            CONSEIL,
            "Filtrage du D peut-être trop fort",
            &format!("{head}{forecast}{d_goal}"),
            vec![
                "Curseur « filtre D » : +0,1.".to_string(),
                "Vol d'essai court, puis vérifiez la température des moteurs : moins de filtrage du D les fait \
                 chauffer plus vite que tout autre réglage."
                    .to_string(),
            ],
            None,
        ));
    }
    Ok(Finding::new(
        "dterm_delay",
        "Filtres",
        INFO,
        "Retard des filtres D",
        &format!("{head}{forecast} Le D dépasserait les repères : mieux vaut ne pas relever ses filtres.{d_goal}"),
        Vec::new(),
        None,
    ))
}

/// `x:.1f` avec la virgule à la française (`f1` des actions de `_dterm_lift_noise`).
fn fr1(x: f64) -> String {
    fx(x, 1).replace('.', ",")
}

/// `_dterm_lift_noise` : terme D sous −20 dB PIDtoolbox : plus grand relèvement de ses filtres (pas de +0,1 du curseur « filtre D », ou
/// coupures × 1,1 sans curseur) pour lequel le simulateur prévoit un D encore sous les repères.
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
pub fn dterm_lift_noise(
    log: &FlightLog,
    sel: &Range<usize>,
    hi: f64,
    profile: &CraftProfile,
    d_db: &[f64],
    d_ptb: f64,
    hover: bool,
    delay_ms: f64,
) -> Result<Option<Finding>, String> {
    let s = &log.settings;
    let mult = dterm_mult(s);
    let k_max: i64 = if mult > 0 {
        (if mult < DTERM_SLIDER_BASIC_MAX {
            (DTERM_SLIDER_BASIC_MAX - mult).max(0)
        } else {
            10 * DTERM_LIFT_MAX_STEPS
        })
        .div_euclid(10)
    } else {
        DTERM_LIFT_MAX_STEPS
    };
    struct Best {
        k: i64,
        lpf: (otb_log::settings::Lowpass, otb_log::settings::Lowpass),
        d: f64,
        dp: f64,
        gain: f64,
    }
    let mut best: Option<Best> = None;
    // du plus grand pas au plus petit : le plus souvent une seule simulation
    for k in (1..=k_max).rev() {
        let factor = if mult > 0 {
            (mult + 10 * k) as f64 / mult as f64
        } else {
            1.1f64.powf(k as f64)
        };
        let modf = scaled_dterm(s, factor)?;
        if (&modf.dterm_lpf1, &modf.dterm_lpf2) == (&s.dterm_lpf1, &s.dterm_lpf2) {
            continue; // coupures plafonnées à 1000 Hz : rien ne change
        }
        let pred = predict_filter_change(log, sel, &modf, hi)?;
        let (_, d, dp) = noise_after(&pred, d_db, d_ptb, None);
        if let (Some(d), Some(dp)) = (d, dp)
            && d <= profile.dterm_noise_high_db
            && dp <= DTERM_PTB_MAX_DB
        {
            best = Some(Best {
                k,
                lpf: (modf.dterm_lpf1.clone(), modf.dterm_lpf2.clone()),
                d,
                dp,
                gain: pred.d_delay_gain_ms,
            });
            break;
        }
    }
    let Some(best) = best else {
        return Ok(None);
    };
    let k = best.k;
    let action = if mult > 0 {
        let (now, new) = (mult as f64 / 100.0, (mult + 10 * k) as f64 / 100.0);
        let mut a = format!("Curseur « filtre D » : {} → {}", fr1(now), fr1(new));
        if k > 1 {
            let steps: Vec<String> = (1..k).map(|i| fr1(now + 0.1 * i as f64)).collect();
            a += &format!(
                ", en passant par {} avec un vol d'essai à chaque pas.",
                steps.join(", ")
            );
        } else {
            a += ".";
        }
        if mult + 10 * k > DTERM_SLIDER_BASIC_MAX {
            a += " Au-delà de 1,2, le configurateur demande le mode expert.";
        }
        a
    } else {
        let (lp1, lp2) = &best.lpf;
        let p1 = if lp1.dynamic() {
            format!(
                "dterm_lpf1_dyn_min_hz = {}, dterm_lpf1_dyn_max_hz = {}",
                lp1.dyn_min_hz, lp1.dyn_max_hz
            )
        } else if lp1.enabled() {
            format!("dterm_lpf1_static_hz = {}", lp1.static_hz)
        } else {
            String::new()
        };
        let p2 = if lp2.enabled() {
            format!("dterm_lpf2_static_hz = {}", lp2.static_hz)
        } else {
            String::new()
        };
        let parts: Vec<String> = [p1, p2].into_iter().filter(|p| !p.is_empty()).collect();
        format!("Coupures du D (sans curseur) : {} (× 1,1 par pas).", parts.join(" ; "))
    };
    let delay = if delay_ms.is_finite() {
        format!(" Retard mesuré du D : {} ms.", fx(delay_ms, 2))
    } else {
        String::new()
    };
    let hover_note = if hover {
        " Vol stationnaire : bruit vu à un seul régime, à confirmer sur un vol avec des gaz plus hauts."
    } else {
        ""
    };
    Ok(Some(Finding::new(
        "dterm_lift",
        "Filtres",
        CONSEIL,
        "Terme D propre : ses filtres peuvent être relevés",
        &format!(
            "Sommet du terme D à {} dB sur l'échelle de PIDtoolbox, sous l'idéal de {} dB.{delay} En relevant ses filtres de \
             {k} pas, le simulateur prévoit {} ms de retard en moins, un terme D à {} dB (seuil {}) et un sommet à {} dB PIDtoolbox (repère \
             {}).{hover_note}",
            fx(d_ptb, 1),
            fx(DTERM_PTB_IDEAL_DB, 0),
            fx(best.gain, 2),
            fx(best.d, 1),
            fx(profile.dterm_noise_high_db, 1),
            fx(best.dp, 0),
            fx(DTERM_PTB_MAX_DB, 0)
        ),
        vec![
            action,
            "Après chaque vol d'essai, touchez les moteurs : moins de filtrage du D les fait chauffer plus vite que tout autre réglage ; \
             tièdes = OK, chauds = revenir au pas précédent."
                .to_string(),
        ],
        None,
    )))
}

/// `_delay_rules` (3b) : retards des filtres gyro et D (vol avec mouvements seulement, D-010) et relèvement des filtres du D.
fn delay_rules(a: &mut Analysis) -> Result<(), String> {
    let (log, sel, profile) = (a.log, a.sel.clone(), a.profile);
    let s = &log.settings;
    let delays = a.delays.clone().ok_or("AttributeError : delays n'est pas calculé")?;
    let (hi, d_ptb) = (a.hi, a.d_ptb);
    let hover = a.hover()?;
    let moving = delays.moving();
    let target = crate::profiles::filter_delay_target_ms(profile.nominal_inch);
    // `nominal_inch:g` n'est lu que si `target` existe (donc la taille aussi)
    let inch = |t: &Option<(f64, f64)>| t.map(|_| general(profile.nominal_inch.unwrap_or(f64::NAN), 6));
    let g_goal = match (&target, inch(&target)) {
        (Some(t), Some(i)) => format!(" Objectif pour un {i} pouces : environ {} ms.", fx(t.0, 2)),
        _ => String::new(),
    };
    if !moving && (delays.gyro_ms.is_finite() || delays.dterm_ms.is_finite()) {
        let vals: Vec<String> = [("gyro", delays.gyro_ms), ("D", delays.dterm_ms)]
            .iter()
            .filter(|(_, v)| v.is_finite())
            .map(|(name, v)| format!("{name} {} ms", fx(*v, 2)))
            .collect();
        a.f.push(Finding::new(
            "delay_not_representative",
            "Filtres",
            INFO,
            "Retard des filtres peu représentatif",
            &format!(
                "Retards mesurés : {}. Le vol contient trop peu de mouvements (consigne au-delà de {} °/s moins de 5 % du temps, 95e centile \
                 {} °/s) : en vol stationnaire, les filtres dynamiques (LPF dynamiques, notchs RPM et dynamiques) ne travaillent qu'à un \
                 régime, et le retard ne se mesure pas bien. Aucun conseil fondé sur le retard mesuré ; le vol stationnaire sert en revanche \
                 à identifier le bruit (harmoniques, résonances, bruit résiduel).{g_goal}",
                vals.join(" ; "),
                fx(MOVING_SETPOINT_DPS, 0),
                fx(delays.stick_p95_dps, 0)
            ),
            vec![
                "Pour juger le retard des filtres : log avec des mouvements francs (flips, rolls, wobbles), pas seulement du vol \
                 stationnaire."
                    .to_string(),
            ],
            None,
        ));
    }
    if moving && delays.gyro_ms.is_finite() {
        let max_ms = profile.gyro_delay_max_ms;
        if delays.gyro_ms > max_ms && !a.gyro_noisy && !a.dterm_noisy {
            let mut actions = vec![
                "Curseur « filtre gyro » : +0,2 (moins de retard → meilleure réactivité, moins de propwash)."
                    .to_string(),
            ];
            let mut settings: Vec<String> = Vec::new();
            if s.rpm_harmonics > 0 && s.rpm_q < RPM_Q_MAX {
                actions.push(format!(
                    "Filtre RPM : rpm_filter_q {} → {} (notchs plus étroits, moins de retard), puis par pas de {RPM_Q_STEP} jusqu'à \
                     {RPM_Q_MAX} au plus, en vérifiant à chaque pas que les raies moteur ne réapparaissent pas.",
                    s.rpm_q,
                    (s.rpm_q + RPM_Q_STEP).min(RPM_Q_MAX)
                ));
                settings.push("rpm_filter_q".to_string());
            }
            actions.push("Contrôlez la température des moteurs après chaque changement.".to_string());
            let mut f = Finding::new(
                "gyro_filter_cautious",
                "Filtres",
                CONSEIL,
                "Filtrage gyro peut-être trop prudent",
                &format!(
                    "Retard des filtres gyro {} ms (repère {} ms pour {}, plus que 90 % des drones du corpus) alors que le bruit est \
                     faible.{g_goal}",
                    fx(delays.gyro_ms, 2),
                    fx(max_ms, 1),
                    profile.label
                ),
                actions,
                None,
            );
            f.settings = settings;
            a.f.push(f);
        } else if delays.gyro_ms > max_ms {
            a.f.push(Finding::new(
                "gyro_delay",
                "Filtres",
                INFO,
                "Retard des filtres gyro",
                &format!(
                    "{} ms, au-dessus du repère ({} ms pour {}), mais le bruit ne permet pas de filtrer moins.{g_goal}",
                    fx(delays.gyro_ms, 2),
                    fx(max_ms, 1),
                    profile.label
                ),
                Vec::new(),
                None,
            ));
        } else {
            a.f.push(Finding::new(
                "gyro_delay",
                "Filtres",
                OK,
                "Retard des filtres gyro",
                &format!(
                    "{} ms (repère ≤ {} ms pour {}).{g_goal}",
                    fx(delays.gyro_ms, 2),
                    fx(max_ms, 1),
                    profile.label
                ),
                Vec::new(),
                None,
            ));
        }
    }
    if moving && delays.dterm_ms.is_finite() {
        let max_ms = profile.dterm_delay_max_ms;
        let d_goal = match (&target, inch(&target)) {
            (Some(t), Some(i)) => format!(
                " Objectif pour un {i} pouces : environ {} ms lus par PIDtoolbox, qui mesure le retard du D environ 0,5 ms \
                 plus court qu'ici.",
                fx(t.1, 2)
            ),
            _ => String::new(),
        };
        if delays.dterm_ms > max_ms && !a.dterm_noisy {
            let f = if !a.aliased {
                dterm_lift(log, &sel, hi, profile, delays.dterm_ms, &a.d_db, d_ptb, &d_goal)?
            } else {
                Finding::new(
                    "dterm_delay",
                    "Filtres",
                    INFO,
                    "Retard des filtres D",
                    &format!(
                        "{} ms, au-dessus du repère ({} ms), mais le log est trop lent pour juger le bruit : pas de relèvement proposé.{d_goal}",
                        fx(delays.dterm_ms, 2),
                        fx(max_ms, 1)
                    ),
                    Vec::new(),
                    None,
                )
            };
            a.f.push(f);
        } else if delays.dterm_ms <= max_ms {
            a.f.push(Finding::new(
                "dterm_delay",
                "Filtres",
                OK,
                "Retard des filtres D",
                &format!(
                    "{} ms (repère ≤ {} ms pour {}).{d_goal}",
                    fx(delays.dterm_ms, 2),
                    fx(max_ms, 1),
                    profile.label
                ),
                Vec::new(),
                None,
            ));
        }
    }
    if !a.aliased
        && !a.dterm_noisy
        && !a.gyro_noisy
        && d_ptb.is_finite()
        && d_ptb < DTERM_PTB_IDEAL_DB
        && any_nonzero(log.d_term.row(0))
    {
        let delay_ms = if moving { delays.dterm_ms } else { f64::NAN };
        if let Some(f) = dterm_lift_noise(log, &sel, hi, profile, &a.d_db, d_ptb, hover, delay_ms)? {
            // remplace le conseil +0,1 fondé sur le retard, qui en dit moins
            a.f.retain(|x| !(x.key == "dterm_filter_too_strong" && x.severity == CONSEIL));
            a.f.push(f);
        }
    }
    Ok(())
}

/// `main_action` : l'action qui change un réglage, sinon la première. Python lève (`IndexError`) si le constat n'a aucune action.
pub fn main_action(f: &Finding) -> Result<String, String> {
    f.actions
        .iter()
        .find(|a| a.contains('=') || a.contains('→') || a.contains("Curseur"))
        .or_else(|| f.actions.first())
        .cloned()
        .ok_or_else(|| "IndexError : list index out of range".to_string())
}

/// `filter_strategy` : synthèse ordonnée des constats de filtrage : quelles étapes modifier, dans quel ordre. `None` sous
/// `STRATEGY_MIN_STAGES` étapes : une seule étape, le constat seul dit déjà quoi faire.
pub fn filter_strategy(findings: &[Finding], moving: bool) -> Result<Option<Finding>, String> {
    let mut steps: Vec<(&str, Vec<&Finding>)> = Vec::new();
    for (stage, keys) in FILTER_STAGES.iter() {
        let hits: Vec<&Finding> = findings
            .iter()
            .filter(|f| {
                !f.actions.is_empty()
                    && (f.severity == ATTENTION || f.severity == CONSEIL)
                    && keys.contains(&f.key.as_str())
            })
            .collect();
        if !hits.is_empty() {
            steps.push((stage, hits));
        }
    }
    if steps.len() < STRATEGY_MIN_STAGES {
        return Ok(None);
    }
    let detail: Vec<String> = steps
        .iter()
        .enumerate()
        .map(|(i, (stage, hits))| {
            let titles: Vec<String> = hits.iter().map(|f| format!("« {} »", f.title)).collect();
            format!("{}. {stage} : {}", i + 1, titles.join(", "))
        })
        .collect();
    let mut actions = Vec::new();
    for (i, (stage, hits)) in steps.iter().enumerate() {
        let more = if hits.len() > 1 {
            format!(" (+ {} autre(s) constat(s))", hits.len() - 1)
        } else {
            String::new()
        };
        actions.push(format!("{}. {stage} — {}{more}", i + 1, main_action(hits[0])?));
    }
    actions.push(format!(
        "Un seul changement à la fois, vol d'essai court, puis température des moteurs ; nouveau log {}",
        if !moving {
            "avec mouvements pour juger le retard."
        } else {
            "après chaque étape."
        }
    ));
    Ok(Some(Finding::new(
        "filter_strategy",
        "Filtres",
        CONSEIL,
        "Stratégie de filtrage",
        &format!(
            "Ordre conseillé des réglages de filtres d'après les constats ci-dessous : {}.",
            detail.join(" ; ")
        ),
        actions,
        None,
    )))
}

/// 1. Qualité de l'enregistrement : fréquence du log, champs manquants, trous, crash.
fn recording_rules(a: &mut Analysis, span_s: f64) -> Result<(), String> {
    let (log, sel) = (a.log, a.sel.clone());
    let s = &log.settings;
    let fs = log.fs;
    let hz = hover_motor_hz(log);
    a.meas.hover_motor_hz = hz;
    let base_hz = log_rate_needed(s.pid_rate_hz(), None);
    let harm_hz = log_rate_needed(s.pid_rate_hz(), hz);
    a.meas.log_rate_needed_hz = Some((base_hz, harm_hz));
    a.aliased = fs < 0.95 * base_hz;
    a.meas.aliased = Some(a.aliased);
    let loops = format!(
        "gyro à {} kHz, boucle PID à {} kHz",
        fx(s.gyro_rate_hz() / 1000.0, 1),
        fx(s.pid_rate_hz() / 1000.0, 1)
    );
    let best = py_max(base_hz, harm_hz);
    let filter_fix = vec![format!(
        "Pour un vol de réglage des filtres : blackbox_sample_rate = {} ({} Hz).",
        sample_rate_cli(s.pid_rate_hz(), best),
        fx(best, 0)
    )];
    let pid_hz = pid_log_rate_needed(s.pid_rate_hz());
    if fs < 0.95 * PID_LOG_RATE_MIN_HZ {
        let mut actions = vec![format!(
            "Pour un vol de réglage des PID : blackbox_sample_rate = {} ({} Hz).",
            sample_rate_cli(s.pid_rate_hz(), pid_hz),
            fx(pid_hz, 0)
        )];
        actions.extend(filter_fix);
        a.f.push(Finding::new(
            "log_rate_low",
            "Enregistrement",
            ATTENTION,
            "Fréquence d'enregistrement trop basse",
            &format!(
                "Le log est enregistré à {} Hz ({loops}) : il faut au moins {} Hz pour régler les PID et {} Hz pour juger les filtres. \
Le blackbox garde un échantillon sur N sans filtre anti-repliement : tout bruit au-dessus de {} Hz se replie dans le spectre (fausses raies).",
                fx(fs, 0),
                fx(PID_LOG_RATE_MIN_HZ, 0),
                fx(base_hz, 0),
                fx(fs / 2.0, 0)
            ),
            actions,
            None,
        ));
    } else if fs < 0.95 * base_hz {
        a.f.push(Finding::new(
            "log_rate_low_filters",
            "Enregistrement",
            CONSEIL,
            "Fréquence d'enregistrement trop basse pour les filtres",
            &format!(
                "Le log est enregistré à {} Hz ({loops}) : assez pour régler les PID (au moins {} Hz), pas pour juger les filtres \
(au moins {} Hz). Tout bruit au-dessus de {} Hz se replie dans le spectre (fausses raies) : aucun filtre n'est allégé d'après ce log.",
                fx(fs, 0),
                fx(PID_LOG_RATE_MIN_HZ, 0),
                fx(base_hz, 0),
                fx(fs / 2.0, 0)
            ),
            filter_fix,
            None,
        ));
    }
    if let Some(hz) = hz
        && 3.0 * hz > fs / 2.0
    {
        let action = if harm_hz > fs * 1.05 {
            format!(
                "blackbox_sample_rate = {} ({} Hz), au moins sur un vol court pour régler les filtres.",
                sample_rate_cli(s.pid_rate_hz(), harm_hz),
                fx(harm_hz, 0)
            )
        } else {
            "Boucle PID trop lente pour enregistrer plus vite : les harmoniques au-delà restent repliées.".to_string()
        };
        a.f.push(Finding::new(
            "log_rate_harmonics",
            "Enregistrement",
            CONSEIL,
            "Harmoniques moteur au-delà de la plage visible",
            &format!(
                "Les moteurs tournent à ~{} Hz en vol : la 3e harmonique ({} Hz) dépasse la moitié de la fréquence d'enregistrement \
({} Hz) et se replie dans le spectre (petit drone : moteurs rapides).",
                fx(hz, 0),
                fx(3.0 * hz, 0),
                fx(fs / 2.0, 0)
            ),
            vec![action],
            None,
        ));
    }
    if log.gyro_unfilt.is_none() {
        a.f.push(Finding::new(
            "gyro_unfilt_missing",
            "Enregistrement",
            CONSEIL,
            "Gyro non filtré absent",
            "Sans gyro non filtré, impossible de mesurer l'efficacité des filtres ni leur retard.",
            vec![
                "En 4.4 : debug_mode = GYRO_SCALED. En 4.5 et plus : ne désactivez pas le champ gyroUnfilt."
                    .to_string(),
            ],
            None,
        ));
    }
    if log.motor_hz.is_none() {
        a.f.push(Finding::new(
            "rpm_missing",
            "Enregistrement",
            CONSEIL,
            "Pas de télémétrie RPM",
            "Sans eRPM, le filtre RPM est inactif et les harmoniques moteur ne peuvent qu'être estimées.",
            vec!["Activez dshot_bidir si vos ESC le permettent (BLHeli_32, Bluejay, AM32).".to_string()],
            None,
        ));
    }
    let (n_gaps, lost_s) = log.recording_gaps(sel, 4.0);
    if n_gaps != 0 {
        // les fenêtres d'analyse qui chevauchent un trou sont écartées (D-020)
        a.f.push(Finding::new(
            "recording_gaps",
            "Enregistrement",
            INFO,
            "Trous dans l'enregistrement",
            &format!(
                "{}, dus à des pauses d'écriture de la carte SD ou de la mémoire flash. Les fenêtres d'analyse qui chevauchent un trou \
sont écartées : il reste moins de données à analyser.",
                gaps_text(n_gaps as i64, lost_s, span_s)
            ),
            vec!["Si c'est fréquent : carte SD plus rapide, ou fréquence d'enregistrement plus basse.".to_string()],
            None,
        ));
    }
    if let Some(ct) = crash_time(log, 300.0, 60.0)? {
        a.f.push(Finding::new(
            "crash",
            "Vol",
            INFO,
            "Crash détecté",
            &format!("Crash vers {} s : la période d'analyse s'arrête avant.", fx(ct, 1)),
            Vec::new(),
            None,
        ));
    }
    Ok(())
}

/// Réponse indicielle d'un axe : mêmes entrées que `step_series::flight_step` (consigne, gyro et termes PID de la période, raccords).
fn axis_step(log: &FlightLog, sel: &Range<usize>, ax: usize, br: &[i64]) -> Result<StepResult, String> {
    let (sp, gy) = (wide(sel_row(&log.setpoint, ax, sel)), wide(sel_row(&log.gyro, ax, sel)));
    let terms = log.pid_terms(ax, sel.clone());
    let (tp, ti, td, tf) = (wide(&terms.p), wide(&terms.i), wide(&terms.d), wide(&terms.f));
    let inst = terms.instrument.as_ref().map(|v| wide(v));
    let input = if terms.note.is_empty() {
        LoopInput::Terms(Terms {
            p: &tp,
            i: &ti,
            d: &td,
            f: &tf,
            instrument: inst.as_deref(),
        })
    } else {
        LoopInput::Unmeasurable(&terms.note)
    };
    step_response(&sp, &gy, log.fs, &StepOptions::default(), &input, Some(br))
}

/// 2. Réponse indicielle : pic, latence, marges de la boucle ouverte, écart roll / pitch.
fn step_section(a: &mut Analysis) -> Result<(), String> {
    let (log, sel, profile) = (a.log, a.sel.clone(), a.profile);
    let mut steps = Vec::new();
    for (ax, name) in AXES.iter().enumerate() {
        let mut r = axis_step(log, &sel, ax, &a.br)?;
        // ⚠️ Le rapport ne garde **pas** les fenêtres individuelles : le cache des outils en garde une copie par profil
        // demandé, et elles pesaient 69 % du rapport (jusqu'à 72 Mo sur une session de 174 s à 2 kHz,
        // `docs/validation.md`). Personne ne les lit ici : les règles se servent de `n`, `metrics` et `loop_`, la page
        // Assistant trace `mean`, `std` et `t_ms`. L'outil « Réponse indicielle », lui, refait son propre calcul et les
        // garde (case « Brutes »). `n` reste le nombre de fenêtres mesurées (L-094).
        r.responses = Vec::new();
        steps.push(AxisStep {
            axis: name.to_string(),
            result: r,
        });
    }
    a.meas.steps = Some(steps.iter().map(|st| (st.axis.clone(), st.result.metrics)).collect());
    let cond = flight_conditions(log, sel)?;
    for (ax, st) in steps[..2].iter().enumerate() {
        step_rules(
            &mut a.f,
            st,
            profile,
            cond.ff_gain(ax),
            log.settings.pids[ax].f > 0 && cond.angle(),
        );
    }
    let (r0, r1) = (&steps[0].result, &steps[1].result);
    if r0.n < STEP_MIN_WINDOWS && r1.n < STEP_MIN_WINDOWS {
        a.f.push(Finding::new(
            "step_few_windows",
            "Réponse",
            ATTENTION,
            "Pas assez de mouvements de manche",
            &format!(
                "Seulement {} fenêtres exploitables en roll et {} en pitch (il faut des mouvements de plus de 20 °/s).",
                r0.n, r1.n
            ),
            vec!["Volez 20 à 30 s avec des mouvements francs et des flips/rolls rapides sur chaque axe.".to_string()],
            None,
        ));
    }
    if r0.n >= STEP_MIN_WINDOWS
        && r1.n >= STEP_MIN_WINDOWS
        && r0.metrics.peak.is_finite()
        && r1.metrics.peak.is_finite()
    {
        let d = r1.metrics.peak - r0.metrics.peak;
        if d.abs() > profile.roll_pitch_gap_high {
            let more = if d > 0.0 { "pitch" } else { "roll" };
            a.f.push(Finding::new(
                "step_roll_pitch_gap",
                "Réponse",
                CONSEIL,
                "Roll et pitch se comportent différemment",
                &format!(
                    "Pic {} en roll contre {} en pitch : le {more} dépasse davantage (écart {}, seuil {}).",
                    fx(r0.metrics.peak, 2),
                    fx(r1.metrics.peak, 2),
                    fx(d.abs(), 2),
                    fx(profile.roll_pitch_gap_high, 2)
                ),
                vec![format!(
                    "Curseur « D pitch » (pitch_d) : {}0,1 pour équilibrer.",
                    if d > 0.0 { "+" } else { "−" }
                )],
                None,
            ));
        }
    }
    a.steps = steps;
    Ok(())
}

/// 3a. Retard consigne → gyro, niveaux de bruit du gyro filtré et du terme D (moyenne 80 Hz → hi, sommet sur l'échelle de PIDtoolbox) et
/// constats « bruité ».
fn noise_rules(a: &mut Analysis) -> Result<(), String> {
    let (log, sel, profile) = (a.log, a.sel.clone(), a.profile);
    let fs = log.fs;
    let delays = compute_delays(log, sel.clone())?;
    a.meas.delays = Some(delays.clone());
    let sp = setpoint_delay(log, &sel, &delays.sp_gyro_ms)?;
    a.f.extend(sp);
    a.delays = Some(delays);
    let nyq = fs / 2.0;
    // Bandes vérifiées sur le corpus (docs/assistant.md, « Bandes de bruit ») : 500 Hz = plus haute borne commune aux logs à 1 kHz ;
    // changer une borne oblige à recalibrer les seuils p90 des profils
    let hi = py_min(500.0, 0.95 * nyq);
    a.hi = hi;
    let br = Some(a.br.as_slice());
    let mut g_db = Vec::new();
    for x in 0..2 {
        g_db.push(band_db(sel_row(&log.gyro, x, &sel), fs, 80.0, hi, br)?);
    }
    let mut d_db = Vec::new();
    for x in 0..2 {
        if any_nonzero(log.d_term.row(x)) {
            d_db.push(band_db(sel_row(&log.d_term, x, &sel), fs, 80.0, hi, br)?);
        }
    }
    let mut low_db = Vec::new();
    for x in 0..2 {
        low_db.push(band_db(sel_row(&log.gyro, x, &sel), fs, 20.0, 80.0, br)?);
    }
    let mut g_peaks = Vec::new();
    for x in 0..2 {
        g_peaks.push(band_peak_ptb_db(sel_row(&log.gyro, x, &sel), fs, 80.0, hi, br)?);
    }
    let g_ptb = py_max_list(&g_peaks).ok_or("ValueError : max() d'une liste vide")?;
    let mut d_peaks = Vec::new();
    for x in 0..2 {
        if any_nonzero(log.d_term.row(x)) {
            d_peaks.push(band_peak_ptb_db(sel_row(&log.d_term, x, &sel), fs, 80.0, hi, br)?);
        }
    }
    let d_ptb = py_max_list(&d_peaks).unwrap_or(f64::NAN); // max(..., default=nan)
    a.g_db = g_db.clone();
    a.d_db = d_db.clone();
    a.low_db = low_db.clone();
    a.g_ptb = g_ptb;
    a.d_ptb = d_ptb;
    a.meas.gyro_noise_db = Some(g_db.clone());
    a.meas.dterm_noise_db = Some(d_db.clone());
    a.meas.low_band_db = Some(low_db);
    a.meas.gyro_peak_ptb_db = Some(g_ptb);
    a.meas.dterm_peak_ptb_db = Some(d_ptb);
    let g_max = py_max_list(&g_db).ok_or("ValueError : max() d'une liste vide")?;
    a.gyro_noisy = g_max > profile.gyro_noise_high_db;
    let dterm_corpus = py_max_list(&d_db).is_some_and(|m| m > profile.dterm_noise_high_db);
    let dterm_ptb = d_ptb.is_finite() && d_ptb > DTERM_PTB_MAX_DB;
    a.dterm_noisy = dterm_corpus || dterm_ptb;
    if a.dterm_noisy {
        let mut why = Vec::new();
        if dterm_corpus {
            why.push(format!(
                "bruit moyen {} dB (80–{} Hz), plus que 90 % des drones de cette taille (seuil {} dB)",
                fx(py_max_list(&d_db).unwrap_or(f64::NAN), 1),
                fx(hi, 0),
                fx(profile.dterm_noise_high_db, 1)
            ));
        }
        if dterm_ptb {
            why.push(format!(
                "sommet du spectre à {} dB sur l'échelle de PIDtoolbox, au-dessus du repère de {} dB (idéal : sous −20 dB)",
                fx(d_ptb, 0),
                fx(DTERM_PTB_MAX_DB, 0)
            ));
        }
        a.f.push(Finding::new(
            "dterm_noisy",
            "Bruit",
            ATTENTION,
            "Terme D bruité",
            &format!(
                "Terme D : {}. Un D bruité fait chauffer les moteurs et use les ESC.",
                why.join(" ; ")
            ),
            vec![
                "Vérifiez d'abord la mécanique (hélices, roulements, vis du châssis).".to_string(),
                "Curseur « filtre D » : −0,1 à −0,2 (filtrage plus fort), ou baisser le D (curseur Damping) si la réponse le permet.".to_string(),
                "Après le vol : moteurs tièdes = OK ; chauds = réduire D ou filtrer plus.".to_string(),
            ],
            None,
        ));
    }
    if a.gyro_noisy {
        a.f.push(Finding::new(
            "gyro_noisy",
            "Bruit",
            ATTENTION,
            "Gyro filtré bruité",
            &format!(
                "Bruit du gyro filtré {} dB (80–{} Hz), plus que 90 % des drones de cette taille (seuil {} dB). Sommet du spectre à {} dB sur \
l'échelle de PIDtoolbox (repère : sous {} dB).",
                fx(g_max, 1),
                fx(hi, 0),
                fx(profile.gyro_noise_high_db, 1),
                fx(g_ptb, 0),
                fx(GYRO_PTB_MAX_DB, 0)
            ),
            vec![
                "Contrôlez hélices et moteurs (vibrations), la fixation du contrôleur de vol (silentblocs).".to_string(),
                "Curseur « filtre gyro » : −0,2.".to_string(),
            ],
            None,
        ));
    } else if g_ptb.is_finite() {
        a.f.push(Finding::new(
            "gyro_noise_level",
            "Bruit",
            if g_ptb <= GYRO_PTB_MAX_DB { OK } else { INFO },
            "Bruit du gyro filtré",
            &format!(
                "Sommet du spectre à {} dB sur l'échelle de PIDtoolbox (repère : sous {} dB) ; bruit moyen {} dB, dans les 90 % du corpus \
(seuil {} dB).",
                fx(g_ptb, 0),
                fx(GYRO_PTB_MAX_DB, 0),
                fx(g_max, 1),
                fx(profile.gyro_noise_high_db, 1)
            ),
            Vec::new(),
            None,
        ));
    }
    if !a.dterm_noisy && d_ptb.is_finite() {
        a.f.push(Finding::new(
            "dterm_noise_level",
            "Bruit",
            OK,
            "Bruit du terme D",
            &format!(
                "Sommet du spectre à {} dB sur l'échelle de PIDtoolbox (repère : au plus {} dB, idéal sous −20 dB).",
                fx(d_ptb, 0),
                fx(DTERM_PTB_MAX_DB, 0)
            ),
            Vec::new(),
            None,
        ));
    }
    Ok(())
}

/// Retard consigne → gyro (intercorrélation) roll / pitch : la mesure de la série Feedforward des finitions (pas de 0,5, en Acro), lue ici
/// plutôt que sur la réponse indicielle, que le feedforward fausse.
pub fn setpoint_delay(log: &FlightLog, sel: &Range<usize>, sp_ms: &[f64; 3]) -> Result<Vec<Finding>, String> {
    if !sp_ms[..2].iter().any(|v| v.is_finite()) {
        return Ok(Vec::new());
    }
    let cond = flight_conditions(log, sel.clone())?;
    let ff = slider_view(&log.settings)?
        .values
        .iter()
        .find(|(k, _)| *k == FF_KEY)
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("KeyError : '{FF_KEY}'"))?;
    let mode = if cond.angle() {
        "mode Angle : le firmware y coupe le feedforward, volez en Acro pour la série"
    } else {
        "Acro"
    };
    let txt = sp_ms[..2]
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .map(|(a, v)| format!("{} {} ms", AXES[a].to_lowercase(), fx(*v, 1)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut chars = txt.chars();
    let head: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let txt = format!("{head}{} (feedforward {}", chars.as_str(), fx(ff, 1)).replace('.', ",");
    Ok(vec![Finding::new(
        "setpoint_delay",
        "Réponse",
        INFO,
        "Retard consigne → gyro",
        &format!(
            "{txt}, {mode}). Série Feedforward (0, 0,5, 1… en Acro) : comparez ce retard d'un vol à l'autre ; plus il est court, plus le drone \
suit le manche tôt."
        ),
        Vec::new(),
        None,
    )])
}

/// 4. Moteurs et mécanique : saturation, déséquilibre, terme I décentré.
fn motor_rules(a: &mut Analysis) -> Result<(), String> {
    let (log, sel, profile) = (a.log, a.sel.clone(), a.profile);
    let fst = flight_stats(log, sel.clone());
    if fst.motor_saturation_pct > profile.saturation_high_pct {
        a.f.push(Finding::new(
            "motor_saturation",
            "Moteurs",
            ATTENTION,
            "Moteurs souvent saturés",
            &format!(
                "Au moins un moteur est à fond {} % du temps (seuil {} %) : le contrôleur n'a plus de marge pour corriger.",
                fx(fst.motor_saturation_pct, 1),
                fx(profile.saturation_high_pct, 1)
            ),
            vec![
                "Si c'est pendant les manœuvres : baisser les rates ou le Master.".to_string(),
                "Sinon : drone trop lourd pour ses moteurs/hélices, ou batterie faible.".to_string(),
            ],
            None,
        ));
    }
    if fst.motor_imbalance_pct.is_finite() && fst.motor_imbalance_pct > profile.imbalance_high_pct {
        let means: Vec<f64> = fst.motors.iter().map(|m| m.mean).collect();
        a.f.push(imbalance_finding(&means, fst.motor_imbalance_pct)?);
    }
    a.meas.stats = Some(fst);
    let pe = pid_error_stats(log, sel.clone(), 100.0, 50.0);
    a.meas.pid_error_sd = Some(pe.sd.clone());
    let i_mean: Vec<f64> = (0..3).map(|x| mean_f32(sel_row(&log.i_term, x, &sel)) as f64).collect();
    a.meas.iterm_mean = Some(i_mean.clone());
    for (x, axis) in AXES[..2].iter().enumerate() {
        if i_mean[x].abs() > profile.iterm_offset_high {
            let axl = axis.to_lowercase();
            a.f.push(Finding::new(
                "iterm_offset",
                "Mécanique",
                CONSEIL,
                &format!("Terme I décentré en {axl}"),
                &format!(
                    "Le terme I {axl} reste en moyenne à {} (seuil ±{}) : le drone compense en permanence un déséquilibre.",
                    fx(i_mean[x], 0),
                    fx(profile.iterm_offset_high, 0)
                ),
                vec![
                    "Centrez la batterie / la caméra (centre de gravité).".to_string(),
                    "Vérifiez qu'aucun moteur ne tourne moins vite.".to_string(),
                ],
                None,
            ));
        }
    }
    Ok(())
}

/// 5. Réglage simplifié : positions des curseurs, estimées si les PID ont été saisis à la main.
fn slider_rules(a: &mut Analysis) -> Result<(), String> {
    let sv = slider_view(&a.log.settings)?;
    if !sv.from_header {
        let pos = sv
            .values
            .iter()
            .map(|(k, v)| format!("{k} {}", fx(*v, 2)))
            .collect::<Vec<_>>()
            .join(", ");
        a.f.push(Finding::new(
            "sliders_off",
            "Réglage",
            INFO,
            "Réglage simplifié désactivé",
            "Les PID ont été saisis à la main : les conseils « curseur » sont donnés à titre indicatif.",
            vec![format!("Positions estimées : {pos}")],
            None,
        ));
    }
    a.meas.sliders = Some(sv);
    Ok(())
}

// ---------------------------------------------------------------- phase 5a-3b : passe-bas, filtre RPM, raies du vol stationnaire

/// Filtre RPM : plus petit `rpm_filter_q` du firmware (`cli/settings.c`).
pub const RPM_Q_MIN: i64 = 250;
/// Poids RPM : marge (dB) au-dessus du bruit voisin, et gain minimal du poids pour le conseiller.
pub const RPM_WEIGHT_MARGIN_DB: f64 = 3.0;
pub const RPM_WEIGHT_GAIN_MIN: f64 = 0.5;
/// Au-delà de cette hauteur (dB) au-dessus du bruit voisin, une raie est « encore visible » (calibration du 2026-10-12).
pub const HARMONIC_VISIBLE_DB: f64 = 4.0;
/// Notch dynamique : marge autour d'une raie, bornes de `dyn_notch_min_hz` et de `dyn_notch_max_hz`, pas et plancher du Q.
pub const DYN_NOTCH_MARGIN_HZ: i64 = 20;
pub const DYN_NOTCH_MIN_HZ: i64 = 20;
pub const DYN_NOTCH_MAX_MIN_HZ: i64 = 200;
pub const DYN_NOTCH_MAX_MAX_HZ: i64 = 1000;
pub const DYN_NOTCH_Q_STEP: i64 = 50;
pub const DYN_NOTCH_Q_LOW: i64 = 250;
/// Notch statique : bornes du Q (× 100), repli de la coupure (× la fréquence) sans mesure de la raie.
pub const STATIC_NOTCH_Q_LOW: i64 = 100;
pub const STATIC_NOTCH_Q_HIGH: i64 = 450;
pub const STATIC_NOTCH_FALLBACK: f64 = 0.7;
/// Vol stationnaire : raies listées dès 6 dB ; sous 100 Hz, prudence avant de les filtrer.
pub const HOVER_LINE_MIN_DB: f64 = 6.0;
pub const HOVER_LOW_LINE_HZ: f64 = 100.0;
/// Notch dynamique : boucle PID minimale (Hz) et nombre maximal de notchs.
pub const DYN_NOTCH_PID_MIN_HZ: i64 = 2000;
pub const DYN_NOTCH_COUNT_MAX: i64 = 7;

/// `int(x)` de Python : tronque vers zéro ; NaN → `ValueError`, infini → `OverflowError`. (Au-delà de i64, la conversion sature : aucun
/// appelant n'y arrive, et un Q saturé est de toute façon ramené à 450.)
fn py_int(x: f64) -> Result<i64, String> {
    if x.is_nan() {
        return Err("ValueError : cannot convert float NaN to integer".into());
    }
    if x.is_infinite() {
        return Err("OverflowError : cannot convert float infinity to integer".into());
    }
    Ok(x.trunc() as i64)
}

/// `a // b` de Python sur des flottants (`float_floor_div`, identique à `np.floor_divide`) : passe par `fmod`, d'où le NaN de `inf // b`.
fn py_floordiv(a: f64, b: f64) -> f64 {
    let m = a % b;
    let mut div = (a - m) / b;
    if m != 0.0 && (b < 0.0) != (m < 0.0) {
        div -= 1.0;
    }
    if div != 0.0 {
        let mut fl = div.floor();
        if div - fl > 0.5 {
            fl += 1.0;
        }
        fl
    } else {
        0.0f64.copysign(a / b)
    }
}

/// `np.fmax(a, b)` : un NaN d'un côté laisse l'autre.
fn fmax(a: f64, b: f64) -> f64 {
    if a >= b || b.is_nan() { a } else { b }
}

/// Raccords de `sel` pour les analyses à fenêtres (`log.breaks(sel)`).
fn breaks_of(log: &FlightLog, sel: &Range<usize>) -> Vec<i64> {
    log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect()
}

/// `log.motor_hz[:, sel]`.
pub(crate) fn sel_rows(r: &Rows, sel: &Range<usize>) -> Rows {
    let rg = clamp_range(sel.start, sel.end, r.n);
    let mut data = Vec::with_capacity(r.k * rg.len());
    for k in 0..r.k {
        data.extend_from_slice(&r.row(k)[rg.clone()]);
    }
    Rows {
        k: r.k,
        n: rg.len(),
        data,
    }
}

/// `list[k]` de Python pour un indice entier relatif (négatif : depuis la fin ; hors bornes : `IndexError`).
fn at(v: &[f64], k: i64) -> Result<f64, String> {
    let i = if k < 0 { k + v.len() as i64 } else { k };
    if i < 0 || i >= v.len() as i64 {
        return Err("IndexError : list index out of range".into());
    }
    Ok(v[i as usize])
}

/// `_lowpass_setup_rules` : réglages de passe-bas déconseillés par Betaflight (notes de réglage 4.2 / 4.3, aide du configurateur).
pub fn lowpass_setup_rules(s: &Settings) -> Vec<Finding> {
    let mut out = Vec::new();
    let order = |kind: &str| match kind {
        "PT1" => 1,
        "PT2" => 2,
        "PT3" => 3,
        "BIQUAD" => 2,
        _ => 1,
    };
    if s.pid_rate_hz() < 0.99 * s.gyro_rate_hz() && !s.gyro_lpf2.enabled() {
        let slow = s.pid_rate_hz() < 3000.0;
        out.push(Finding::new(
            "gyro_lpf2_off",
            "Filtres",
            if slow { ATTENTION } else { CONSEIL },
            "LPF2 du gyro coupé : risque de repliement",
            &format!(
                "La boucle PID ({} kHz) est plus lente que le gyro ({} kHz) : le LPF2 du gyro sert de filtre anti-repliement avant ce \
sous-échantillonnage. Sans lui, du bruit haute fréquence se replie dans la bande utile.",
                fx(s.pid_rate_hz() / 1000.0, 1),
                fx(s.gyro_rate_hz() / 1000.0, 1)
            ),
            vec![format!(
                "gyro_lpf2_type = PT1, gyro_lpf2_static_hz = {} ({}).",
                if slow { 500 } else { 1000 },
                if slow {
                    "indispensable à 2 kHz"
                } else {
                    "à 4 kHz : de 500 à 1000 Hz"
                }
            )],
            None,
        ));
    }
    let d_order: i64 = [&s.dterm_lpf1, &s.dterm_lpf2]
        .iter()
        .filter(|lp| lp.enabled())
        .map(|lp| order(lp.kind))
        .sum();
    if d_order < 2 {
        out.push(Finding::new(
            "dterm_filter_weak",
            "Filtres",
            ATTENTION,
            "Filtrage du D insuffisant",
            "Le terme D n'a qu'un filtre du premier ordre (ou aucun) : Betaflight demande au moins deux PT1 ou un filtre du second ordre. \
Risque de moteurs brûlants ou de départ incontrôlé à l'armement.",
            vec![
                "Réactivez le LPF2 du D (dterm_lpf2_type = PT1, dterm_lpf2_static_hz = valeur haute de la plage dynamique du LPF1), ou \
passez le LPF1 du D en PT2."
                    .to_string(),
            ],
            None,
        ));
    }
    if s.dterm_lpf1.static_hz != 0 && !s.dterm_lpf1.dynamic() {
        let f = s.dterm_lpf1.static_hz;
        out.push(Finding::new(
            "dterm_lpf1_static",
            "Filtres",
            INFO,
            "LPF1 du D en mode statique",
            &format!(
                "Le LPF1 du D est fixe à {f} Hz. Betaflight déconseille le mode statique pour le D : en mode dynamique, il filtre plus \
à bas régime (grondement moteur, armement) et moins en vol (moins de retard, moins de propwash)."
            ),
            vec![format!(
                "dterm_lpf1_dyn_min_hz = {f}, dterm_lpf1_dyn_max_hz = {} (rapport des valeurs par défaut 75–150 Hz, gardé par le curseur \
« filtre D »).",
                2 * f
            )],
            None,
        ));
    }
    for (name, lp) in [("LPF1", &s.gyro_lpf1), ("LPF2", &s.gyro_lpf2)] {
        if lp.enabled() && lp.kind == "BIQUAD" {
            out.push(Finding::new(
                "gyro_lpf_biquad",
                "Filtres",
                INFO,
                &format!("Biquad sur le {name} du gyro"),
                "Le biquad peut osciller sur les variations brusques et dépasser près de sa coupure ; Betaflight recommande le PT2, qui \
filtre presque autant avec moins de retard.",
                vec![format!("gyro_{}_type = PT2.", name.to_lowercase())],
                None,
            ));
        }
    }
    out
}

/// `rpm_harmonic_levels` : hauteur de chaque harmonique moteur au-dessus du bruit voisin (dB, maximum du roll et du pitch), avant filtres
/// (gyro non filtré) puis après (gyro filtré). Python lève (`TypeError`) sans gyro non filtré ou sans eRPM.
pub fn rpm_harmonic_levels(log: &FlightLog, sel: &Range<usize>) -> Result<(Vec<f64>, Vec<f64>), String> {
    let unf = log
        .gyro_unfilt
        .as_ref()
        .ok_or("TypeError : 'NoneType' object is not subscriptable")?;
    let mhz = sel_rows(
        log.motor_hz
            .as_ref()
            .ok_or("TypeError : 'NoneType' object is not subscriptable")?,
        sel,
    );
    let br = breaks_of(log, sel);
    let mut raw = Vec::new();
    for a in 0..2 {
        raw.push(otb_analysis::motors::harmonic_prominence(
            &wide(sel_row(unf, a, sel)),
            &mhz,
            log.fs,
            3,
            300.0,
            20.0,
            Some(&br),
        )?);
    }
    let mut left = Vec::new();
    for a in 0..2 {
        left.push(otb_analysis::motors::harmonic_prominence(
            &wide(sel_row(&log.gyro, a, sel)),
            &mhz,
            log.fs,
            3,
            300.0,
            20.0,
            Some(&br),
        )?);
    }
    let f = |v: &[Vec<f64>]| -> Vec<f64> { v[0].iter().zip(&v[1]).map(|(&a, &b)| fmax(a, b)).collect() };
    Ok((f(&raw), f(&left)))
}

/// Poids allégé d'une harmonique de rang 2 ou 3 (`_rpm_weight_rules`) : `np.ceil(10 x - 1e-9) / 10` pour le besoin `x` de la raie (marge
/// comprise), si le gain `cur - need` atteint `RPM_WEIGHT_GAIN_MIN` à 1e-9 près. Les deux tolérances de 1e-9 absorbent l'arrondi des flottants :
/// 0,7 − 0,2 vaut 0,49999999999999994, et `10 x` peut dépasser 2 d'un rien. ⚠️ Tolérances à garder, comme en Python (L-061).
pub fn lighten_weight(cur: f64, prominence_db: f64) -> Option<f64> {
    let need = (10.0 * otb_analysis::motors::rpm_weight_needed(prominence_db) - 1e-9).ceil() / 10.0;
    (cur - need >= RPM_WEIGHT_GAIN_MIN - 1e-9).then_some(need)
}

/// `_rpm_weight_rules` : poids du filtre RPM : raie restée visible avec un poids réduit, ou poids plus fort que nécessaire (seulement si
/// `lighten`, c'est-à-dire si le bruit moteur est bien éliminé).
pub fn rpm_weight_rules(
    log: &FlightLog,
    sel: &Range<usize>,
    lighten: bool,
    h1_flagged: bool,
) -> Result<Vec<Finding>, String> {
    let s = &log.settings;
    let (raw, left) = rpm_harmonic_levels(log, sel)?;
    let n = s.rpm_harmonics.min(3);
    let nu = n.max(0) as usize; // `range(n)` est vide pour n ≤ 0
    let mut w: Vec<i64> = s.rpm_weights.clone();
    w.extend([100, 100, 100]);
    let cur: Vec<f64> = w[..3].iter().map(|&x| x as f64 / 100.0).collect();
    let mut out = Vec::new();
    let low: Vec<usize> = (0..nu)
        .filter(|&k| left[k] > HARMONIC_VISIBLE_DB && cur[k] < 1.0)
        .collect();
    if !low.is_empty() {
        let mut f = Finding::new(
            "rpm_weight_low",
            "Filtres",
            CONSEIL,
            "Poids du filtre RPM trop bas",
            &format!(
                "Raie moteur encore visible après filtrage : {}.",
                low.iter()
                    .map(|&k| format!(
                        "{}e harmonique {} dB au-dessus du bruit voisin, poids {}",
                        k + 1,
                        fx(left[k], 1),
                        fx(cur[k] * 100.0, 0)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ; ")
            ),
            vec![
                "rpm_filter_weights : remontez le poids de cette harmonique (vers 100) avant de toucher à rpm_filter_q.".to_string(),
            ],
            None,
        );
        f.settings = vec!["rpm_filter_q".to_string()]; // Q attend les poids : rpm_q_raise se tait
        out.push(f);
    }
    // `range(n, 3)` : un n négatif (jamais vu) y compterait des indices relatifs, comme Python
    let mut missing: Vec<i64> = Vec::new();
    for k in n..3 {
        let l = at(&left, k)?;
        if l.is_finite() && l > HARMONIC_VISIBLE_DB {
            missing.push(k);
        }
    }
    if !missing.is_empty() {
        let mx = *missing.iter().max().unwrap();
        let mut parts = Vec::new();
        for &k in &missing {
            parts.push(format!(
                "la {}e reste {} dB au-dessus du bruit voisin après filtrage",
                k + 1,
                fx(at(&left, k)?, 0)
            ));
        }
        out.push(Finding::new(
            "rpm_harmonic_missing",
            "Filtres",
            CONSEIL,
            "Harmonique moteur hors du filtre RPM",
            &format!("Le filtre RPM ne traite que {n} harmonique(s) ; {}.", parts.join(" ; ")),
            vec![format!(
                "rpm_filter_harmonics = {} (actuellement {n}), en réglant son poids avec rpm_filter_weights (4.5 et plus).",
                mx + 1
            )],
            None,
        ));
    }
    // 1re harmonique déjà signalée par « Bruit moteur mal éliminé » : pas de doublon
    let full: Vec<usize> = (0..nu)
        .filter(|&k| left[k] > HARMONIC_VISIBLE_DB && cur[k] >= 1.0 && !(k == 0 && h1_flagged))
        .collect();
    if !full.is_empty() {
        let q_new = RPM_Q_MIN.max(s.rpm_q - RPM_Q_STEP); // comme « Bruit moteur mal éliminé »
        let mut f = Finding::new(
            "rpm_line_visible",
            "Filtres",
            CONSEIL,
            "Raie moteur encore visible après le filtre RPM",
            &format!(
                "Malgré un poids de 100 : {}. Les notchs RPM sont trop étroits pour la raie, ou mal placés.",
                full.iter()
                    .map(|&k| format!(
                        "{}e harmonique {} dB avant filtres, {} dB après (au-dessus du bruit voisin)",
                        k + 1,
                        fx(raw[k], 0),
                        fx(left[k], 0)
                    ))
                    .collect::<Vec<_>>()
                    .join(" ; ")
            ),
            vec![
                "Vérifiez motor_poles (14 pour la plupart des moteurs, 12 pour certains micro-moteurs) : un mauvais nombre de pôles \
décale tous les notchs."
                    .to_string(),
                format!(
                    "rpm_filter_q {} → {q_new} (notchs plus larges, un peu plus de retard), puis par pas de {RPM_Q_STEP} tant que la raie \
reste visible.",
                    s.rpm_q
                ),
            ],
            None,
        );
        f.settings = vec!["rpm_filter_q".to_string()];
        out.push(f);
    }
    // rpm_filter_weights n'existe qu'à partir de Betaflight 4.5
    if !lighten || (!s.version.is_empty() && s.version[..] < [4, 5][..]) {
        return Ok(out);
    }
    let mut new = cur.clone();
    for k in 1..nu {
        // 2e et 3e harmoniques, comme la documentation de Betaflight
        if !(raw[k].is_finite() && left[k] <= HARMONIC_VISIBLE_DB) {
            continue;
        }
        if let Some(w) = lighten_weight(cur[k], raw[k] + RPM_WEIGHT_MARGIN_DB) {
            new[k] = w;
        }
    }
    if new != cur {
        let fmt = |w: &[f64]| {
            w[..(n.max(1) as usize).min(3)]
                .iter()
                .map(|x| fx(x * 100.0, 0))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let changed: Vec<usize> = (0..nu).filter(|&k| new[k] != cur[k]).collect();
        out.push(Finding::new(
            "rpm_weight_lighten",
            "Filtres",
            CONSEIL,
            "Filtre RPM plus fort que nécessaire",
            &format!(
                "Avant filtrage, {} : un poids de {} suffit à l'effacer (marge de {} dB). Moins de filtrage = moins de retard.",
                changed
                    .iter()
                    .map(|&k| if raw[k] > 0.0 {
                        format!("la {}e harmonique ne dépasse le bruit voisin que de {} dB", k + 1, fx(raw[k], 1))
                    } else {
                        format!("la {}e harmonique ne dépasse pas le bruit voisin", k + 1)
                    })
                    .collect::<Vec<_>>()
                    .join(" ; "),
                changed
                    .iter()
                    .map(|&k| fx(new[k] * 100.0, 0))
                    .collect::<Vec<_>>()
                    .join(", "),
                fx(RPM_WEIGHT_MARGIN_DB, 0)
            ),
            vec![
                format!("rpm_filter_weights = {} (actuellement {}).", fmt(&new), fmt(&cur)),
                "Vérifiez sur le log suivant que les raies moteur restent invisibles dans le spectre du gyro filtré et du terme D."
                    .to_string(),
            ],
            None,
        ));
    }
    Ok(out)
}

/// `_rpm_q_raise` : toutes les harmoniques traitées effacées avec le Q actuel et bruit sous les repères : des notchs plus étroits suffiraient
/// (moins de retard). Pas de 100 jusqu'à 1000 ; décision de Cédric (2026-10-10). `findings` : les constats déjà posés (`F`).
pub fn rpm_q_raise(
    log: &FlightLog,
    sel: &Range<usize>,
    findings: &[Finding],
    hover: bool,
) -> Result<Option<Finding>, String> {
    let s = &log.settings;
    let (_raw, left) = rpm_harmonic_levels(log, sel)?;
    let treated: Vec<usize> = (0..s.rpm_harmonics.clamp(0, 3) as usize)
        .filter(|&k| left[k].is_finite())
        .collect();
    if treated.is_empty() || treated.iter().any(|&k| left[k] > HARMONIC_VISIBLE_DB) {
        return Ok(None);
    }
    let levels = treated
        .iter()
        .map(|&k| format!("{}e {} dB", k + 1, fx(left[k], 0)))
        .collect::<Vec<_>>()
        .join(", ");
    let then = if findings.iter().any(|f| f.key == "rpm_weight_lighten") {
        " Un réglage à la fois : d'abord les poids (« Filtre RPM plus fort que nécessaire »), puis Q."
    } else {
        ""
    };
    let hover_note = if hover {
        " Vol stationnaire : bruit vu à un seul régime, à confirmer sur un vol avec des gaz plus hauts."
    } else {
        ""
    };
    let mut f = Finding::new(
        "rpm_q_raise",
        "Filtres",
        CONSEIL,
        "Filtre RPM : Q peut monter",
        &format!(
            "Les harmoniques traitées sont effacées après filtres (au-dessus du bruit voisin : {levels} ; visible au-delà de {} dB) et le \
bruit du gyro et du D est sous les repères : des notchs plus étroits suffiraient, avec moins de retard.{then}{hover_note}",
            fx(HARMONIC_VISIBLE_DB, 0)
        ),
        vec![
            format!(
                "rpm_filter_q {} → {}, puis par pas de {RPM_Q_STEP} jusqu'à {RPM_Q_MAX} au plus, en vérifiant à chaque pas que les raies \
moteur ne réapparaissent pas dans le gyro filtré.",
                s.rpm_q,
                (s.rpm_q + RPM_Q_STEP).min(RPM_Q_MAX)
            ),
            "Température des moteurs après chaque vol d'essai.".to_string(),
        ],
        None,
    );
    f.settings = vec!["rpm_filter_q".to_string()];
    Ok(Some(f))
}

/// `dyn_notch_active` : notch dynamique en service : au moins un notch et boucle PID ≥ 2 kHz.
pub fn dyn_notch_active(s: &Settings) -> bool {
    s.dyn_notch_count > 0 && s.dyn_notch_max_hz > 0 && s.pid_rate_hz() >= DYN_NOTCH_PID_MIN_HZ as f64
}

/// `static_notch_case` : raison de conseiller un notch statique pour une raie fixe à `f0` (cas exceptionnel : le notch dynamique ne peut pas
/// la traiter), ou `None`. Jamais sous `HOVER_LOW_LINE_HZ`.
pub fn static_notch_case(s: &Settings, f0: f64) -> Option<String> {
    if f0 < HOVER_LOW_LINE_HZ {
        return None;
    }
    if s.pid_rate_hz() < DYN_NOTCH_PID_MIN_HZ as f64 {
        return Some(format!(
            "boucle PID sous {} kHz : le notch dynamique ne peut pas fonctionner",
            fx(DYN_NOTCH_PID_MIN_HZ as f64 / 1000.0, 0)
        ));
    }
    if f0 + DYN_NOTCH_MARGIN_HZ as f64 > DYN_NOTCH_MAX_MAX_HZ as f64 {
        return Some(format!(
            "raie au-delà de la portée du notch dynamique ({DYN_NOTCH_MAX_MAX_HZ} Hz au plus, à {DYN_NOTCH_MARGIN_HZ} Hz près)"
        ));
    }
    if s.dyn_notch_count >= DYN_NOTCH_COUNT_MAX && dyn_notch_active(s) {
        return Some(format!(
            "les {DYN_NOTCH_COUNT_MAX} notchs dynamiques sont déjà en service"
        ));
    }
    None
}

/// `_dyn_notch_line_actions` : réglages du notch dynamique pour qu'il traite une raie à `f0` (plage, nombre de notchs, mise en service) :
/// (actions, réglages de `TRACKED_SETTINGS` cités). `Err` si `f0` n'est pas fini (`int()` lève en Python).
pub fn dyn_notch_line_actions(s: &Settings, f0: f64, fixed: bool) -> Result<(Vec<String>, Vec<String>), String> {
    let count = vec!["dyn_notch_count".to_string()];
    let margin = DYN_NOTCH_MARGIN_HZ as f64;
    if !dyn_notch_active(s) {
        if s.pid_rate_hz() < DYN_NOTCH_PID_MIN_HZ as f64 {
            return Ok((Vec::new(), Vec::new()));
        }
        let lo = DYN_NOTCH_MIN_HZ.max(py_int(py_floordiv(f0 - margin, 10.0) * 10.0)?);
        let hi = DYN_NOTCH_MAX_MAX_HZ.min(DYN_NOTCH_MAX_MIN_HZ.max(py_int(-py_floordiv(-(f0 + margin), 10.0) * 10.0)?));
        return Ok((
            vec![format!(
                "Mettez en service le notch dynamique : dyn_notch_count = 1, dyn_notch_min_hz ≤ {lo}, dyn_notch_max_hz ≥ {hi}."
            )],
            count,
        ));
    }
    let (lo, hi) = (f0 - margin, f0 + margin);
    if lo < s.dyn_notch_min_hz as f64 || hi > s.dyn_notch_max_hz as f64 {
        let new_lo = DYN_NOTCH_MIN_HZ.max(s.dyn_notch_min_hz.min(py_int(py_floordiv(lo, 10.0) * 10.0)?));
        let new_hi = DYN_NOTCH_MAX_MAX_HZ.min(s.dyn_notch_max_hz.max(py_int(-py_floordiv(-hi, 10.0) * 10.0)?));
        return Ok((
            vec![format!(
                "Notch dynamique : plage {}–{} Hz → dyn_notch_min_hz = {new_lo}, dyn_notch_max_hz = {new_hi} ({DYN_NOTCH_MARGIN_HZ} Hz de \
marge autour de la raie).",
                s.dyn_notch_min_hz, s.dyn_notch_max_hz
            )],
            Vec::new(),
        ));
    }
    if fixed {
        return Ok((
            vec![format!(
                "Elle est dans la plage du notch dynamique ({}–{} Hz), qui peut la suivre si un notch est libre.",
                s.dyn_notch_min_hz, s.dyn_notch_max_hz
            )],
            Vec::new(),
        ));
    }
    Ok((
        vec![format!(
            "La raie est dans la plage du notch dynamique mais passe quand même : dyn_notch_count {} → {} (si le Q est trop haut pour \
elle, le constat « Notch dynamique : Q trop haut » le dit).",
            s.dyn_notch_count,
            (s.dyn_notch_count + 1).min(DYN_NOTCH_COUNT_MAX)
        )],
        count,
    ))
}

/// `_static_notch_action` : notch statique, seulement dans un cas exceptionnel (`static_notch_case`).
pub fn static_notch_action(s: &Settings, f0: f64, cutoff: (i64, bool), lead: &str) -> Vec<String> {
    match static_notch_case(s, f0) {
        Some(why) => vec![format!(
            "{lead} notch statique ({why}) : {}.",
            static_notch_text(f0, cutoff)
        )],
        None => Vec::new(),
    }
}

/// `dyn_notch_q_need` : plus haut Q du notch dynamique (× 100, comme Betaflight) qui ramène chaque raie à `HARMONIC_VISIBLE_DB` au plus du
/// bruit voisin sur sa demi-largeur : `Q = f0·h / (2d·√(1 − h²))` pour une atténuation `h`. `None` sans raie mesurable ; `inf` si aucune
/// n'en a besoin.
pub fn dyn_notch_q_need(peaks: &[f64], widths: &[f64], heights: &[f64]) -> Option<f64> {
    let mut qs = Vec::new();
    for ((&f0, &w), &ht) in peaks.iter().zip(widths).zip(heights) {
        if !(w.is_finite() && w > 0.0 && ht.is_finite()) {
            continue;
        }
        let need_db = ht - HARMONIC_VISIBLE_DB;
        if need_db <= 0.0 {
            qs.push(f64::INFINITY);
            continue;
        }
        let h = 10f64.powf(-need_db / 20.0);
        qs.push(100.0 * f0 * h / (w * (1.0 - h * h).sqrt())); // 2d = w
    }
    py_min_list(&qs)
}

/// `_q_round` : `int(max(low, q // 50 * 50))`. `Err` si le résultat n'est pas fini (`int()` lève) ; `inf // 50` vaut NaN en Python, donc `low`.
pub fn q_round(q: f64, low: i64) -> Result<i64, String> {
    py_int(py_max(
        low as f64,
        py_floordiv(q, DYN_NOTCH_Q_STEP as f64) * DYN_NOTCH_Q_STEP as f64,
    ))
}

/// `static_notch_cutoff` : coupure (Hz) de `gyro_notch1` pour une raie de largeur à mi-puissance `width` et de hauteur `height` (dB) : Q
/// nécessaire (arrondi comme le notch dynamique, de 100 à 450), puis `c = f0·(√(1 + 4Q²) − 1) / (2Q)`, arrondie vers le bas. Renvoie aussi
/// si la raie a été mesurée (sinon repli à 0,7 × la fréquence).
pub fn static_notch_cutoff(f0: f64, width: f64, height: f64) -> Result<(i64, bool), String> {
    let q = dyn_notch_q_need(&[f0], &[width], &[height]);
    let Some(q) = q.filter(|q| q.is_finite()) else {
        return Ok((py_int(f0 * STATIC_NOTCH_FALLBACK)?, false));
    };
    let qq = q_round(q, STATIC_NOTCH_Q_LOW)?.min(STATIC_NOTCH_Q_HIGH) as f64 / 100.0;
    Ok((py_int(f0 * ((1.0 + 4.0 * qq * qq).sqrt() - 1.0) / (2.0 * qq))?, true))
}

/// `_line_shape` : largeur et hauteur d'une raie dans le gyro non filtré, la plus large et la plus haute du roll et du pitch ; NaN sans gyro
/// non filtré ou sans eRPM.
pub fn line_shape(log: &FlightLog, sel: &Range<usize>, f0: f64) -> Result<(f64, f64), String> {
    let (Some(unf), Some(mhz)) = (&log.gyro_unfilt, &log.motor_hz) else {
        return Ok((f64::NAN, f64::NAN));
    };
    let mhz = sel_rows(mhz, sel);
    let br = breaks_of(log, sel);
    let mut sh = Vec::new();
    for a in 0..2 {
        let r = otb_analysis::motors::line_shapes(
            &wide(sel_row(unf, a, sel)),
            &mhz,
            log.fs,
            &[f0],
            log.settings.rpm_harmonics,
            300.0,
            Some(&br),
            Some(&log.settings),
        )?;
        sh.push(r[0]);
    }
    // `worst` : np.nanmax si une valeur est finie, sinon NaN
    let worst = |vals: [f64; 2]| {
        if vals.iter().any(|v| v.is_finite()) {
            vals.iter()
                .copied()
                .filter(|v| !v.is_nan())
                .fold(f64::NEG_INFINITY, f64::max)
        } else {
            f64::NAN
        }
    };
    Ok((worst([sh[0].0, sh[1].0]), worst([sh[0].1, sh[1].1])))
}

/// `_static_notch_text`.
pub fn static_notch_text(f0: f64, cutoff: (i64, bool)) -> String {
    let how = if cutoff.1 {
        "calculée d'après la largeur de la raie"
    } else {
        "0,7 × la fréquence : raie trop faible ou absente dans le spectre moyen du gyro non filtré"
    };
    format!(
        "gyro_notch1_hz = {}, gyro_notch1_cutoff = {} (coupure {how})",
        fx(f0, 0),
        cutoff.0
    )
}

/// `_low_line_caveat` : mise en garde pour une raie sous `HOVER_LOW_LINE_HZ` (bande de travail du PID).
pub fn low_line_caveat(f0: f64) -> String {
    if f0 >= HOVER_LOW_LINE_HZ {
        return String::new();
    }
    format!(
        " Sous {} Hz, un notch ajoute du retard là où le PID travaille, et la raie peut venir d'une oscillation de réglage (P ou D) \
plutôt que de la mécanique : vérifiez la réponse sur un vol avec mouvements avant de filtrer.",
        fx(HOVER_LOW_LINE_HZ, 0)
    )
}

/// `_residual_line` : raie hors moteurs encore visible après filtrage : où la traiter.
pub fn residual_line(s: &Settings, ln: &SpectrumLine, cutoff: Option<(i64, bool)>) -> Result<Finding, String> {
    let f0 = ln.freq_hz;
    let mut actions = vec![
        "Cherchez d'abord une cause mécanique : vis, antennes, câbles, caméra, pile de cartes, hélices.".to_string(),
    ];
    let (notch, settings) = dyn_notch_line_actions(s, f0, false)?;
    actions.extend(notch);
    let cutoff = match cutoff {
        Some(c) => c,
        None => (py_int(f0 * STATIC_NOTCH_FALLBACK)?, false),
    };
    actions.extend(static_notch_action(s, f0, cutoff, "Cas exceptionnel,"));
    let caveat = low_line_caveat(f0);
    let mut f = Finding::new(
        "hover_line_residual",
        "Filtres",
        CONSEIL,
        &format!("Raie à {} Hz encore visible après filtrage", fx(f0, 0)),
        &format!(
            "En vol stationnaire, une raie qui ne vient pas des moteurs dépasse le bruit voisin de {} dB avant filtres et de {} dB après : \
les filtres actuels ne la traitent pas.{caveat}",
            fx(ln.raw_db, 0),
            fx(ln.left_db, 0)
        ),
        actions,
        None,
    );
    f.settings = settings;
    Ok(f)
}

/// Fusion d'une raie du vol stationnaire avec celles déjà gardées (`_hover_noise` : `near = [o for o in lines if …]`, puis `near[0]`) : la
/// **première** raie voisine (à 10 % de sa fréquence) est remplacée si la nouvelle est plus haute avant filtres, sinon rien ; sans voisine, la
/// nouvelle s'ajoute. Deux raies déjà gardées peuvent être voisines de la nouvelle (le voisinage se mesure à la fréquence de la raie gardée).
pub fn merge_hover_line(lines: &mut Vec<SpectrumLine>, ln: SpectrumLine) {
    let near = lines
        .iter()
        .position(|o| (o.freq_hz - ln.freq_hz).abs() <= 0.1 * o.freq_hz);
    match near {
        None => lines.push(ln),
        Some(i) if ln.raw_db > lines[i].raw_db => lines[i] = ln,
        Some(_) => {}
    }
}

/// `_hover_noise` : vol stationnaire : régime stable, le meilleur log pour identifier le bruit. Bilan des harmoniques moteur, des raies qui
/// n'en viennent pas (résonances, bruit résiduel) et du bruit restant ; conseil pour chaque raie encore visible après filtrage. Écrit
/// `meas.hover_lines`. (Le paramètre `profile` de Python n'est pas lu : il n'est pas repris.) Python lève sans gyro non filtré ou sans eRPM.
pub fn hover_noise(
    log: &FlightLog,
    sel: &Range<usize>,
    g_ptb: f64,
    d_ptb: f64,
    fixed_hz: Option<f64>,
    meas: &mut Measures,
    aliased: bool,
) -> Result<Vec<Finding>, String> {
    let s = &log.settings;
    let hz = hover_motor_hz(log);
    let mut lines: Vec<SpectrumLine> = Vec::new();
    let unf = log
        .gyro_unfilt
        .as_ref()
        .ok_or("TypeError : 'NoneType' object is not subscriptable")?;
    let mhz = sel_rows(
        log.motor_hz
            .as_ref()
            .ok_or("TypeError : 'NoneType' object is not subscriptable")?,
        sel,
    );
    let br = breaks_of(log, sel);
    for a in 0..2 {
        let found = otb_analysis::motors::spectrum_lines(
            &wide(sel_row(unf, a, sel)),
            &wide(sel_row(&log.gyro, a, sel)),
            &mhz,
            log.fs,
            (20.0, 900.0),
            HOVER_LINE_MIN_DB,
            8,
            300.0,
            Some(&br),
        )?;
        for ln in found {
            merge_hover_line(&mut lines, ln);
        }
    }
    // `lines.sort(key=-raw_db)` : tri stable
    lines.sort_by(|a, b| (-a.raw_db).total_cmp(&(-b.raw_db)));
    meas.hover_lines = Some(lines.clone());
    let mut parts: Vec<String> = Vec::new();
    if let Some(hz) = hz {
        parts.push(format!(
            "moteurs à {} Hz (harmoniques {} Hz)",
            fx(hz, 0),
            [1.0, 2.0, 3.0]
                .iter()
                .map(|h| fx(h * hz, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if s.rpm_harmonics > 0 {
        let (raw, left) = rpm_harmonic_levels(log, sel)?;
        let harm: Vec<String> = (0..3)
            .filter(|&k| raw[k].is_finite() && left[k].is_finite())
            .map(|k| format!("{}e {} → {} dB", k + 1, fx(raw[k], 0), fx(left[k], 0)))
            .collect();
        if !harm.is_empty() {
            parts.push(format!(
                "hauteur des harmoniques avant → après filtres : {}",
                harm.join(", ")
            ));
        }
    }
    parts.push(format!(
        "autres raies : {}",
        if lines.is_empty() {
            format!("aucune au-dessus de {} dB", fx(HOVER_LINE_MIN_DB, 0))
        } else {
            lines
                .iter()
                .take(5)
                .map(|o| {
                    format!(
                        "{} Hz ({} → {} dB)",
                        fx(o.freq_hz, 0),
                        fx(o.raw_db, 0),
                        fx(o.left_db, 0)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    parts.push(if d_ptb.is_finite() {
        format!(
            "bruit restant : gyro {} dB (repère {}), D {} dB (repère {}) sur l'échelle de PIDtoolbox",
            fx(g_ptb, 0),
            fx(GYRO_PTB_MAX_DB, 0),
            fx(d_ptb, 0),
            fx(DTERM_PTB_MAX_DB, 0)
        )
    } else {
        format!(
            "bruit restant : gyro {} dB (repère {}) sur l'échelle de PIDtoolbox",
            fx(g_ptb, 0),
            fx(GYRO_PTB_MAX_DB, 0)
        )
    });
    let mut out = vec![Finding::new(
        "hover_noise_summary",
        "Filtres",
        INFO,
        "Vol stationnaire : bruit identifié",
        &format!(
            "Régime stable, raies moteur repérées par l'eRPM : {}.{}",
            parts.join(" ; "),
            if aliased {
                " Log trop lent : certaines raies peuvent être des repliements."
            } else {
                ""
            }
        ),
        Vec::new(),
        None,
    )];
    if aliased {
        return Ok(out);
    }
    for ln in &lines {
        if ln.left_db.is_nan() || ln.left_db <= HARMONIC_VISIBLE_DB {
            continue;
        }
        if let Some(fh) = fixed_hz
            && (ln.freq_hz - fh).abs() <= 0.1 * fh
        {
            continue; // déjà traitée par le constat « Raie fixe »
        }
        // notch statique : cas exceptionnel seulement
        let cutoff = if static_notch_case(s, ln.freq_hz).is_some() {
            let (w, h) = line_shape(log, sel, ln.freq_hz)?;
            Some(static_notch_cutoff(ln.freq_hz, w, h)?)
        } else {
            None
        };
        out.push(residual_line(s, ln, cutoff)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------- phase 5a-3c : raie fixe, notch dynamique, `filter_rules`

/// Q du notch dynamique avec filtre RPM : de 300 à 500 (décision de Cédric, 2026-10-13).
pub const DYN_NOTCH_Q_RPM: (i64, i64) = (300, 500);
/// « Raie fixe » : harmoniques 2 et 3 écartées à ± 8 % de l'harmonique de la vitesse en stationnaire, fondamentale à ± 15 % (test des
/// deux trajets : raie moteur si le trajet moteur est plus haut de plus de −2 dB en moyenne, D-027).
pub const RESONANCE_HARMONIC_MARGIN: f64 = 0.08;
pub const RESONANCE_FUND_MARGIN: f64 = 0.15;
pub const RESONANCE_MOTOR_SCORE_DB: f64 = -2.0;

/// `meas["frame_resonance"]` : raie la plus marquée au-dessus du bruit voisin, quel que soit le gaz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameResonance {
    pub freq_hz: f64,
    /// Excès au-dessus du bruit voisin (dB), `-inf` si la raie est collée au bord bas.
    pub excess_db: f64,
    /// La raie suit les moteurs (fondamentale ou harmonique 2 ou 3 de la vitesse en stationnaire).
    pub near_motor: bool,
}

/// `meas["dyn_notch"]` (dict en Python) : ce que le notch dynamique a à suivre.
#[derive(Debug, Clone, PartialEq)]
pub struct DynNotch {
    pub count: i64,
    pub min_hz: i64,
    pub max_hz: i64,
    pub peaks: Vec<f64>,
    pub widths: Vec<f64>,
    pub heights: Vec<f64>,
    /// Hauteur de chaque pic dans le gyro filtré.
    pub left: Vec<f64>,
    /// `None` : aucune raie mesurable ; `inf` : aucune n'a besoin d'un Q plus bas.
    pub q_need: Option<f64>,
}

/// `np.convolve(v, np.ones(15) / 15, mode="same")` pour au moins 15 valeurs : somme **croissante** des produits, chacun fusionné à la somme
/// (FMA), comme le produit scalaire de numpy (OpenBLAS) ; mesuré identique au bit près sur 3 000 valeurs. En dessous de 15 valeurs, numpy
/// échange les tableaux et rend 15 valeurs : la soustraction qui suit lève (`ValueError`), d'où l'erreur.
/// ⚠️ Un autre ordre de somme ou sans FMA diffère au dernier bit, invisible à la tolérance du corpus (L-061).
pub fn smooth15(v: &[f64]) -> Result<Vec<f64>, String> {
    let n = v.len();
    if n < 15 {
        return Err("ValueError : operands could not be broadcast together".into());
    }
    let kern = 1.0f64 / 15.0;
    Ok((0..n)
        .map(|i| {
            let lo = i.saturating_sub(7);
            let hi = (i + 8).min(n);
            v[lo..hi].iter().fold(0.0f64, |acc, &x| x.mul_add(kern, acc))
        })
        .collect())
}

/// `20 * np.log10(np.maximum(x, 1e-9))` : un NaN reste NaN.
fn db_of(x: f64) -> f64 {
    20.0 * (if x.is_nan() { x } else { x.max(1e-9) }).log10()
}

/// `np.nanmedian` d'une colonne : NaN si tout est NaN.
fn nanmedian(v: &[f64]) -> f64 {
    let k: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
    otb_analysis::npstat::median_f64(&k)
}

/// `np.argmin` : le premier NaN l'emporte, sinon le premier minimum. Tableau non vide.
fn argmin_first(v: &[f64]) -> usize {
    let mut best = 0;
    for (i, &x) in v.iter().enumerate() {
        if v[best].is_nan() {
            break;
        }
        if x.is_nan() || x < v[best] {
            best = i;
        }
    }
    best
}

/// `np.floor(t).astype(int)` puis `np.clip(·, 0, nb - 1)` : NaN et valeurs hors de i64 donnent `i64::MIN` (x86), donc la tranche 0.
fn throttle_bin(t: f32, nb: usize) -> usize {
    let fl = (t as f64).floor();
    let k = if fl.is_nan() || fl >= i64::MAX as f64 || fl < i64::MIN as f64 {
        i64::MIN
    } else {
        fl as i64
    };
    k.clamp(0, nb as i64 - 1) as usize
}

/// `log.throttle[sel]`.
fn throttle_sel(log: &FlightLog, sel: &Range<usize>) -> Vec<f64> {
    let t = &log.throttle;
    t[clamp_range(sel.start, sel.end, t.len())]
        .iter()
        .map(|&v| v as f64)
        .collect()
}

/// `frame_resonance` : raie la plus marquée au-dessus du bruit voisin, quel que soit le gaz. `None` si le journal ne couvre pas assez de
/// tranches de gaz. `Err` comme Python lève (spectres, ou moins de 15 fréquences dans la bande : `ValueError` du lissage).
pub fn frame_resonance(log: &FlightLog, sel: &Range<usize>, lo_hz: f64) -> Result<Option<FrameResonance>, String> {
    let x = log.gyro_unfilt.as_ref().unwrap_or(&log.gyro);
    let (lo, hi) = (lo_hz, py_min(0.9 * log.fs / 2.0, 900.0));
    let br = breaks_of(log, sel);
    let thr = throttle_sel(log, sel);
    let (mut maps, mut prof): (Vec<Vec<f64>>, Vec<Vec<f64>>) = (Vec::new(), Vec::new());
    let (mut f, mut ok) = (Vec::new(), Vec::new());
    for a in 0..2 {
        let ft = spectral::freq_throttle(
            &wide(sel_row(x, a, sel)),
            &thr,
            log.fs,
            Mode::Amp,
            300.0,
            100,
            (0.0, 100.0),
            Some(&br),
        )?;
        let nb = ft.bins.len();
        ok = ft.counts.iter().map(|&c| c > 3).collect();
        if ok.iter().filter(|&&o| o).count() < 8 {
            return Ok(None);
        }
        prof.push(
            (0..ft.freqs.len())
                .map(|fi| {
                    let col: Vec<f64> = (0..nb).filter(|&b| ok[b]).map(|b| ft.z[fi * nb + b]).collect();
                    nanmedian(&col)
                })
                .collect(),
        );
        f = ft.freqs;
        maps.push(ft.z);
    }
    let p: Vec<f64> = (0..f.len()).map(|i| db_of((prof[0][i] + prof[1][i]) / 2.0)).collect();
    let (fm, pm): (Vec<f64>, Vec<f64>) = f
        .iter()
        .zip(&p)
        .filter(|(fi, _)| **fi >= lo && **fi <= hi)
        .map(|(fi, pi)| (*fi, *pi))
        .unzip();
    if pm.len() < 10 {
        return Ok(None);
    }
    let base = smooth15(&pm)?;
    // dès 20 Hz, sans les 5 premiers Hz : une raie collée au bord vient des mouvements du drone ou d'un test d'excitation
    let excess: Vec<f64> = (0..pm.len())
        .map(|i| {
            if fm[i] < lo + 5.0 {
                f64::NEG_INFINITY
            } else {
                pm[i] - base[i]
            }
        })
        .collect();
    let k = argmax_f64(&excess).ok_or("ValueError : argmax d'un tableau vide")?;
    let f0 = fm[k];
    // une raie moteur se déplace avec le gaz : harmoniques 2 et 3 écartées autour de la vitesse en stationnaire, fondamentale jugée par le
    // test des deux trajets (D-027)
    let mut near_motor = false;
    if let Some(hz) = hover_motor_hz(log) {
        if (f0 - hz).abs() < RESONANCE_FUND_MARGIN * hz {
            let z: Vec<f64> = maps[0].iter().zip(&maps[1]).map(|(a, b)| (a + b) / 2.0).collect();
            let score = line_follows_motors(log, sel, f0, &z, &f, &ok)?;
            near_motor = !(score.is_finite() && score <= RESONANCE_MOTOR_SCORE_DB);
        } else {
            near_motor = [2.0, 3.0]
                .iter()
                .any(|h| (f0 - h * hz).abs() < RESONANCE_HARMONIC_MARGIN * h * hz);
        }
    }
    Ok(Some(FrameResonance {
        freq_hz: f0,
        excess_db: excess[k],
        near_motor,
    }))
}

/// `line_follows_motors` : test des deux trajets pour une raie à `f0` près de la fondamentale : dans chaque tranche de gaz (carte `z`
/// fréquence × tranche, `nf × ok.len()` à plat, tranches `ok`), excès de la raie au-dessus du bruit voisin à la vitesse moyenne des moteurs
/// de la tranche, moins son excès à `f0`. Moyenne en dB sur les tranches où les deux trajets se séparent : > 0 raie moteur, < 0 raie fixe ;
/// NaN sous 3 tranches (D-027).
pub fn line_follows_motors(
    log: &FlightLog,
    sel: &Range<usize>,
    f0: f64,
    z: &[f64],
    f: &[f64],
    ok: &[bool],
) -> Result<f64, String> {
    let Some(mhz) = &log.motor_hz else {
        return Ok(f64::NAN);
    };
    let nb = ok.len();
    let nf = f.len();
    if z.len() != nf * nb {
        return Err("ValueError : carte et fréquences incohérentes".into());
    }
    let mm = otb_analysis::motors::mean_axis0(&sel_rows(mhz, sel));
    // vitesse moyenne des moteurs par tranche de gaz de 1 %, avec ses deux voisines comme freq_throttle
    let thr = &log.throttle[clamp_range(sel.start, sel.end, log.throttle.len())];
    let (mut sums, mut cnt) = (vec![0.0f64; nb], vec![0i64; nb]);
    for (i, &t) in thr.iter().enumerate() {
        let k = throttle_bin(t, nb);
        sums[k] += mm[i] as f64;
        cnt[k] += 1;
    }
    let s3 = |b: usize| {
        let l = if b > 0 { sums[b - 1] } else { 0.0 };
        let r = if b + 1 < nb { sums[b + 1] } else { 0.0 };
        (sums[b] + l) + r
    };
    let c3 = |b: usize| cnt[b] + if b > 0 { cnt[b - 1] } else { 0 } + if b + 1 < nb { cnt[b + 1] } else { 0 };
    if nf < 2 {
        return Err("IndexError : index 1 is out of bounds".into());
    }
    let df = f[1] - f[0];
    let win: Vec<usize> = (0..nf).filter(|&i| f[i] >= 0.6 * f0 && f[i] <= 1.6 * f0).collect();
    let fw: Vec<f64> = win.iter().map(|&i| f[i]).collect();
    let mut d = Vec::new();
    for b in (0..nb).filter(|&b| ok[b]) {
        if c3(b) == 0 {
            continue;
        }
        let mb = s3(b) / c3(b) as f64;
        if (mb - f0).abs() < py_max(3.0 * df, 0.04 * f0) {
            continue; // trajets confondus dans cette tranche
        }
        let (Some(&first), Some(&last)) = (fw.first(), fw.last()) else {
            return Err("IndexError : index 0 is out of bounds".into());
        };
        if !(first <= mb && mb <= last) {
            continue; // raie moteur hors de la fenêtre
        }
        let pdb: Vec<f64> = (0..nf).map(|i| db_of(z[i * nb + b])).collect();
        let sm = smooth15(&pdb)?;
        let ex: Vec<f64> = win.iter().map(|&i| pdb[i] - sm[i]).collect();
        let at_hz = |x: f64| {
            let dist: Vec<f64> = fw.iter().map(|v| (v - x).abs()).collect();
            let j = argmin_first(&dist);
            np_max_nan(&ex[j.saturating_sub(1)..(j + 2).min(ex.len())]).unwrap_or(f64::NAN) // raie entre deux cases
        };
        d.push(at_hz(mb) - at_hz(f0));
    }
    Ok(if d.len() >= 3 { numpy_mean(&d) } else { f64::NAN })
}

/// `_frame_resonance` : raie de fréquence fixe quel que soit le gaz (résonance du châssis ou pièce desserrée) ; `res` vient de
/// `frame_resonance`. `sel` absente : tout le journal.
pub fn frame_resonance_finding(
    log: &FlightLog,
    res: Option<&FrameResonance>,
    profile: &CraftProfile,
    sel: Option<&Range<usize>>,
) -> Result<Option<Finding>, String> {
    let Some(res) = res else {
        return Ok(None);
    };
    let (f0, excess) = (res.freq_hz, res.excess_db);
    if excess < profile.resonance_high_db || res.near_motor {
        return Ok(None);
    }
    let mut actions =
        vec!["Resserrez les vis, fixez antennes et câbles, vérifiez la caméra et la pile de cartes.".to_string()];
    let s = &log.settings;
    let (notch, settings) = dyn_notch_line_actions(s, f0, true)?;
    actions.extend(notch);
    if static_notch_case(s, f0).is_some() {
        let whole = 0..log.n();
        let (w, h) = line_shape(log, sel.unwrap_or(&whole), f0)?;
        let cutoff = static_notch_cutoff(f0, w, h)?;
        actions.extend(static_notch_action(
            s,
            f0,
            cutoff,
            "Si elle persiste, cas exceptionnel :",
        ));
    }
    let mut f = Finding::new(
        "frame_line_fixed",
        "Mécanique",
        CONSEIL,
        &format!("Raie fixe vers {} Hz", fx(f0, 0)),
        &format!(
            "Une vibration à ~{} Hz reste présente quel que soit le gaz (+{} dB au-dessus du bruit voisin) : résonance du châssis, d'une \
antenne, d'une caméra ou pièce desserrée.{}",
            fx(f0, 0),
            fx(excess, 0),
            low_line_caveat(f0)
        ),
        actions,
        None,
    );
    f.settings = settings;
    Ok(Some(f))
}

/// `worst` de `dyn_notch_view` : `np.nanmax` si une valeur est finie, sinon NaN.
fn worst_of(vals: &[f64]) -> f64 {
    if vals.iter().any(|v| v.is_finite()) {
        vals.iter()
            .copied()
            .filter(|v| !v.is_nan())
            .fold(f64::NEG_INFINITY, f64::max)
    } else {
        f64::NAN
    }
}

/// `dyn_notch_view` : pics que le notch dynamique a à suivre (roll ou pitch, celui qui en compte le plus, plus la raie fixe signalée
/// `fixed_hz`), avec filtre RPM actif et gyro non filtré ; `None` sinon. La recherche déborde de deux cases SDFT de chaque côté de la plage.
pub fn dyn_notch_view(log: &FlightLog, sel: &Range<usize>, fixed_hz: Option<f64>) -> Result<Option<DynNotch>, String> {
    let s = &log.settings;
    let (Some(unf), Some(mhz_all)) = (&log.gyro_unfilt, &log.motor_hz) else {
        return Ok(None);
    };
    if !dyn_notch_active(s) || s.rpm_harmonics == 0 {
        return Ok(None);
    }
    let band = (
        (s.dyn_notch_min_hz - 37).max(DYN_NOTCH_MIN_HZ) as f64,
        (s.dyn_notch_max_hz + 37) as f64,
    );
    let mhz = sel_rows(mhz_all, sel);
    let br = breaks_of(log, sel);
    let mut t = Vec::new();
    for a in 0..2 {
        t.push(otb_analysis::motors::dyn_notch_targets(
            &wide(sel_row(unf, a, sel)),
            Some(&mhz),
            log.fs,
            band,
            Some(s),
            300.0,
            18.5,
            Some(&br),
        )?);
    }
    // `max(t, key=…)` : le premier des plus longs
    let best = if t[1].freqs.len() > t[0].freqs.len() {
        &t[1]
    } else {
        &t[0]
    };
    let mut peaks: Vec<f64> = best.freqs.clone();
    if let Some(fh) = fixed_hz
        && peaks.iter().all(|f| (f - fh).abs() > 37.0)
    {
        peaks.push(fh);
    }
    peaks.sort_by(f64::total_cmp);
    // forme de chaque raie : la plus large et la plus haute du roll et du pitch (côté prudent)
    let mut sh = Vec::new();
    for a in 0..2 {
        sh.push(otb_analysis::motors::line_shapes(
            &wide(sel_row(unf, a, sel)),
            &mhz,
            log.fs,
            &peaks,
            s.rpm_harmonics,
            300.0,
            Some(&br),
            Some(s),
        )?);
    }
    let widths: Vec<f64> = (0..peaks.len()).map(|i| worst_of(&[sh[0][i].0, sh[1][i].0])).collect();
    let heights: Vec<f64> = (0..peaks.len()).map(|i| worst_of(&[sh[0][i].1, sh[1][i].1])).collect();
    // hauteur de chaque raie dans le gyro filtré (après tous les filtres, notch dynamique compris)
    let left = if peaks.is_empty() {
        Vec::new()
    } else {
        let mut lh = Vec::new();
        for a in 0..2 {
            lh.push(otb_analysis::motors::line_heights(
                &wide(sel_row(&log.gyro, a, sel)),
                log.fs,
                &peaks,
                300.0,
                Some(&br),
            )?);
        }
        (0..peaks.len()).map(|i| worst_of(&[lh[0][i], lh[1][i]])).collect()
    };
    let q_need = dyn_notch_q_need(&peaks, &widths, &heights);
    Ok(Some(DynNotch {
        count: s.dyn_notch_count,
        min_hz: s.dyn_notch_min_hz,
        max_hz: s.dyn_notch_max_hz,
        peaks,
        widths,
        heights,
        left,
        q_need,
    }))
}

/// `_dyn_notch_q_n_rules` : Q et nombre de notchs du notch dynamique (avec filtre RPM) d'après les raies à suivre (décision de Cédric,
/// 2026-10-10) : Q baissé si une raie est trop large ou trop haute pour le Q actuel et qu'une raie à suivre dépasse encore
/// `HARMONIC_VISIBLE_DB` dans le gyro filtré, monté (500 au plus) si rien ne passe et que le bruit est faible ; un notch de plus par pic à
/// suivre quand une raie passe ou que le gyro est bruité. `findings` : les constats déjà posés (`F`).
pub fn dyn_notch_q_n_rules(
    s: &Settings,
    dn: &DynNotch,
    lines_pass: bool,
    gyro_noisy: bool,
    dterm_noisy: bool,
    findings: &[Finding],
    hover: bool,
) -> Result<Vec<Finding>, String> {
    let mut out = Vec::new();
    let (q, need) = (s.dyn_notch_q, dn.q_need);
    let shapes = dn
        .peaks
        .iter()
        .zip(&dn.widths)
        .zip(&dn.heights)
        .filter(|((_, w), h)| w.is_finite() && h.is_finite())
        .map(|((f0, w), h)| {
            format!(
                "{} Hz ({} Hz de large, {} dB)",
                fx(*f0, 0),
                fx(*w, 0),
                fixed(*h, 0, true)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let passing: Vec<(f64, f64)> = dn
        .peaks
        .iter()
        .zip(&dn.left)
        .filter(|(_, h)| **h > HARMONIC_VISIBLE_DB)
        .map(|(f0, h)| (*f0, *h))
        .collect();
    let lower = match need {
        Some(n) if n.is_finite() && q_round(n, DYN_NOTCH_Q_LOW)? < q && !passing.is_empty() => Some(n),
        _ => None,
    };
    if let Some(need) = lower {
        let new = q_round(need, DYN_NOTCH_Q_LOW)?;
        let seen = passing
            .iter()
            .map(|(f0, h)| format!("{} Hz ({} dB)", fx(*f0, 0), fixed(*h, 0, true)))
            .collect::<Vec<_>>()
            .join(", ");
        let floor = need < DYN_NOTCH_Q_LOW as f64; // raie trop large ou trop haute pour un notch dynamique raisonnable
        let why = if floor {
            format!(
                "le calcul (forme du notch de Betaflight) demanderait un Q de {}, sous {DYN_NOTCH_Q_LOW}, le plus bas Q utilisé avec filtre \
RPM : un notch aussi large ajouterait beaucoup de retard.",
                fx(need, 0)
            )
        } else {
            format!("il faut un Q de {new} au plus (calculé d'après la forme du notch de Betaflight).")
        };
        let mut actions = vec![format!(
            "dyn_notch_q {q} → {new} (notchs plus larges, un peu plus de retard)."
        )];
        if floor {
            actions.push(
                "Si la raie reste visible : cherchez sa cause mécanique (vis, hélices, roulements, antenne, caméra, pile de cartes) plutôt \
que d'élargir encore le notch."
                    .to_string(),
            );
        }
        out.push(Finding::new(
            "dyn_notch_q_lower",
            "Filtres",
            CONSEIL,
            "Notch dynamique : Q trop haut pour les raies à suivre",
            &format!(
                "Raies à suivre après le filtre RPM (largeur à mi-puissance, hauteur au-dessus du bruit voisin) : {shapes}. Encore visible \
dans le gyro filtré : {seen}. Avec dyn_notch_q {q}, le notch est trop étroit pour ramener la plus exigeante à {} dB du bruit voisin sur \
toute sa largeur ; {why}",
                fx(HARMONIC_VISIBLE_DB, 0)
            ),
            actions,
            None,
        ));
    } else if !(lines_pass || gyro_noisy || dterm_noisy) && q < DYN_NOTCH_Q_RPM.1 {
        let (new, why) = match need {
            None => (
                // aucune raie mesurable : pas de calcul, petit pas
                (q + DYN_NOTCH_Q_STEP).min(DYN_NOTCH_Q_RPM.1),
                "aucune raie à suivre n'est mesurable".to_string(),
            ),
            Some(n) => {
                let new = if !n.is_finite() {
                    DYN_NOTCH_Q_RPM.1
                } else {
                    q_round(n, DYN_NOTCH_Q_LOW)?.min(DYN_NOTCH_Q_RPM.1)
                };
                let why = if shapes.is_empty() {
                    "les raies à suivre dépassent à peine le bruit voisin".to_string()
                } else {
                    format!("raies à suivre : {shapes} ; un Q de {new} suffit encore à les effacer")
                };
                (new, why)
            }
        };
        if new > q {
            let seen = if hover {
                "aucune raie ne passe les filtres en vol stationnaire"
            } else {
                "aucune raie fixe n'est signalée"
            };
            out.push(Finding::new(
                "dyn_notch_q_raise",
                "Filtres",
                CONSEIL,
                "Notch dynamique : Q peut monter",
                &format!(
                    "Avec le filtre RPM, le notch dynamique ne traite que les résonances qui ne viennent pas des moteurs ; {seen} et le \
bruit du gyro et du D est sous les repères ({why}). Un Q plus haut donne des notchs plus étroits, avec moins de retard."
                ),
                vec![format!(
                    "dyn_notch_q {q} → {new} ({} au plus avec filtre RPM), puis vérifiez que les raies de résonance ne réapparaissent pas \
dans le gyro filtré.",
                    DYN_NOTCH_Q_RPM.1
                )],
                None,
            ));
        }
    }
    let n_peaks = dn.peaks.len() as i64;
    let said = findings
        .iter()
        .any(|f| f.settings.iter().any(|x| x == "dyn_notch_count")); // une raie qui passe demande déjà un notch de plus
    if n_peaks > dn.count && (lines_pass || gyro_noisy) && !said {
        let n = n_peaks.min(7);
        let list = dn.peaks.iter().map(|f| fx(*f, 0)).collect::<Vec<_>>().join(", ");
        let mut f = Finding::new(
            "dyn_notch_more",
            "Filtres",
            CONSEIL,
            "Notch dynamique : plus de notchs",
            &format!(
                "{n_peaks} pics à suivre après le filtre RPM ({list} Hz) pour {} notch(s), et {}",
                dn.count,
                if lines_pass {
                    "une raie passe les filtres."
                } else {
                    "le gyro filtré est bruité."
                }
            ),
            vec![format!(
                "dyn_notch_count {} → {n} (un notch par pic à suivre).",
                dn.count
            )],
            None,
        );
        f.settings = vec!["dyn_notch_count".to_string()];
        out.push(f);
    }
    Ok(out)
}

/// `np.nanmax` de deux valeurs (NaN si les deux le sont).
fn nanmax2(a: f64, b: f64) -> f64 {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => f64::NAN,
        (true, false) => b,
        (false, true) => a,
        _ => a.max(b),
    }
}

/// `_filter_rules` : réglages des filtres : passe-bas déconseillés, LPF1 du gyro, filtre RPM, oscillations 20–80 Hz, raie fixe, bilan du
/// vol stationnaire, notch dynamique. Écrit `frame_resonance`, `dyn_notch`, `motor_harmonic_*` et `hover_lines` dans les mesures.
fn filter_rules(a: &mut Analysis) -> Result<(), String> {
    let (log, sel, profile) = (a.log, a.sel.clone(), a.profile);
    let s = &log.settings;
    let (fs, br, hi, aliased) = (log.fs, a.br.clone(), a.hi, a.aliased);
    let fr = frame_resonance(log, &sel, 20.0)?;
    let fixed_hz = fr
        .filter(|r| r.excess_db >= profile.resonance_high_db && !r.near_motor)
        .map(|r| r.freq_hz);
    let dn = dyn_notch_view(log, &sel, fixed_hz)?;
    a.meas.frame_resonance = fr;
    a.meas.dyn_notch = dn.clone();
    a.f.extend(lowpass_setup_rules(s));
    if !aliased
        && s.gyro_lpf1.enabled()
        && log.gyro_unfilt.is_some()
        && log.motor_hz.is_some()
        && s.rpm_harmonics > 0
        && !a.gyro_noisy
        && !a.dterm_noisy
    {
        let hover = a.hover()?;
        if let Some(f) = gyro_lpf1_off(log, &sel, hi, profile, &a.g_db, &a.d_db, a.d_ptb, hover)? {
            a.f.push(f);
        }
    }
    if let (Some(mhz_all), Some(unf)) = (&log.motor_hz, &log.gyro_unfilt)
        && s.rpm_harmonics > 0
    {
        let mhz = sel_rows(mhz_all, &sel);
        let (mut mn, mut mp) = (Vec::new(), Vec::new());
        for x in 0..2 {
            mn.push(otb_analysis::motors::motor_noise(
                &wide(sel_row(&log.gyro, x, &sel)),
                &mhz,
                fs,
                3,
                300.0,
                20.0,
                Some(&br),
            )?);
        }
        for x in 0..2 {
            mp.push(otb_analysis::motors::motor_noise(
                &wide(sel_row(unf, x, &sel)),
                &mhz,
                fs,
                3,
                300.0,
                20.0,
                Some(&br),
            )?);
        }
        let left = nanmax2(mn[0].mean_db[0], mn[1].mean_db[0]);
        let raw = nanmax2(mp[0].mean_db[0], mp[1].mean_db[0]);
        a.meas.motor_harmonic_left_db = Some(left);
        a.meas.motor_harmonic_raw_db = Some(raw);
        if left > profile.motor_left_high_db {
            let mut f = Finding::new(
                "motor_noise_left",
                "Filtres",
                CONSEIL,
                "Bruit moteur mal éliminé",
                &format!(
                    "Il reste {} dB sur la fréquence de rotation des moteurs après filtrage (avant filtrage : {} dB).",
                    fx(left, 1),
                    fx(raw, 1)
                ),
                vec![
                    "Vérifiez motor_poles (14 pour la plupart des moteurs, 12 pour certains micro-moteurs)."
                        .to_string(),
                    format!(
                        "Filtre RPM : rpm_filter_q {} → {} (notchs plus larges), puis par pas de {RPM_Q_STEP} tant que la raie reste visible.",
                        s.rpm_q,
                        RPM_Q_MIN.max(s.rpm_q - RPM_Q_STEP)
                    ),
                ],
                None,
            );
            f.settings = vec!["rpm_filter_q".to_string()];
            a.f.push(f);
        }
        let lighten = left <= profile.motor_left_high_db && !aliased;
        let weights = rpm_weight_rules(log, &sel, lighten, left > profile.motor_left_high_db)?;
        a.f.extend(weights);
        let q_said = a.f.iter().any(|f| f.settings.iter().any(|x| x == "rpm_filter_q")); // retard, raie visible : déjà conseillé
        if !aliased && !a.gyro_noisy && !a.dterm_noisy && s.rpm_q < RPM_Q_MAX && !q_said {
            let hover = a.hover()?;
            if let Some(f) = rpm_q_raise(log, &sel, &a.f, hover)? {
                a.f.push(f);
            }
        }
    }
    let low_max = py_max_list(&a.low_db).ok_or("ValueError : max() arg is an empty sequence")?;
    if low_max > profile.low_band_high_db {
        a.f.push(Finding::new(
            "low_band_oscillation",
            "Réponse",
            CONSEIL,
            "Oscillations basses fréquences (20–80 Hz)",
            &format!(
                "Énergie du gyro filtré entre 20 et 80 Hz : {} dB (seuil {} dB, 90 % des drones de cette taille font mieux). C'est la zone \
du propwash et des oscillations de réglage.",
                fx(low_max, 1),
                fx(profile.low_band_high_db, 1)
            ),
            vec![
                "Si cela arrive en descente/gaz coupés : propwash → plus de D (Damping +0,1) ou D max.".to_string(),
                "Si cela arrive en vol stable : oscillation → Master −0,1 à −0,2.".to_string(),
                "Regardez l'outil Fréquence × gaz pour voir à quel gaz le phénomène apparaît.".to_string(),
            ],
            None,
        ));
    }
    let res = frame_resonance_finding(log, fr.as_ref(), profile, Some(&sel))?;
    if let Some(r) = &res {
        a.f.push(r.clone());
    }
    if a.hover()? && log.motor_hz.is_some() && log.gyro_unfilt.is_some() {
        let fixed = if res.is_some() { fr.map(|r| r.freq_hz) } else { None };
        let found = hover_noise(log, &sel, a.g_ptb, a.d_ptb, fixed, &mut a.meas, aliased)?;
        a.f.extend(found);
    }
    let lines_pass = a.f.iter().any(|f| LINE_KEYS.contains(&f.key.as_str()));
    if !aliased && let Some(dn) = &dn {
        let hover = a.hover()?;
        let more = dyn_notch_q_n_rules(s, dn, lines_pass, a.gyro_noisy, a.dterm_noisy, &a.f, hover)?;
        a.f.extend(more);
        if dn.count > (dn.peaks.len() as i64).max(1) {
            let n = (dn.peaks.len() as i64).max(1);
            let found = if dn.peaks.is_empty() {
                "aucun pic à suivre".to_string()
            } else {
                format!(
                    "{} pic(s) à suivre ({} Hz)",
                    dn.peaks.len(),
                    dn.peaks.iter().map(|f| fx(*f, 0)).collect::<Vec<_>>().join(", ")
                )
            };
            let mut f = Finding::new(
                "dyn_notch_fewer",
                "Filtres",
                INFO,
                "Notch dynamique : moins de notchs suffiraient",
                &format!(
                    "Entre {} et {} Hz, après le filtre RPM, {found} pour {} notchs. Chaque notch filtre en permanence, même sans pic à \
suivre (il reste à sa dernière fréquence), et ajoute du retard.",
                    dn.min_hz, dn.max_hz, dn.count
                ),
                vec![format!(
                    "dyn_notch_count = {n} (actuellement {}), puis vérifiez sur le log suivant que les raies de résonance ne réapparaissent \
pas dans le gyro filtré.",
                    dn.count
                )],
                None,
            );
            f.settings = vec!["dyn_notch_count".to_string()];
            a.f.push(f);
        }
    }
    Ok(())
}
