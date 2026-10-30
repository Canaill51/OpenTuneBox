//! Éléments sans état de l'assistant à règles (`advisor/rules.py`) : gravités, `Finding`, textes de la réponse indicielle et de la
//! boucle ouverte, fréquences d'enregistrement, constat de déséquilibre des moteurs (phase 5a-2a).
//!
//! Les textes sont comparés caractère par caractère à ceux de Python : les nombres passent par `otb-format` (jamais `{:.2}` de Rust, qui
//! n'arrondit pas comme Python à tous les coups sur les cas limites), `.replace(".", ",")` s'applique à la chaîne entière comme en Python.
//! Les comparaisons avec NaN restent fausses (`not x > y` est vrai pour NaN) : les conditions sont recopiées dans le même sens que Python.
//!
//! Phase 5a-2b : `Report`, `Analysis` (= `_Analysis`) et `analyze`, avec les règles qui lisent un `FlightLog` : enregistrement, réponse
//! indicielle, retard consigne → gyro, niveaux de bruit, moteurs, curseurs. **Pas portés (5a-3)** : `delay_rules`, `filter_rules` et
//! `filter_strategy` existent ici sans effet (marqués `// 5a-3`) pour que `analyze` s'exécute de bout en bout : leurs constats et leurs
//! mesures (`frame_resonance`, `dyn_notch`, `motor_harmonic_*`, `hover_lines`) manquent, aucune parité n'est prétendue pour eux.

use crate::profiles::{CraftProfile, detect_profile};
use otb_analysis::delay::{Delays, compute_delays};
use otb_analysis::motors::hover_motor_hz;
use otb_analysis::npstat::{argmax_f64, mean_f32, py_min};
use otb_analysis::numpy_mean;
use otb_analysis::open_loop::{LoopMetrics, MEASURED, Terms};
use otb_analysis::period::crash_time;
use otb_analysis::sliders::{SliderView, slider_view};
use otb_analysis::spectral::{self, Mode, PTB_PSD_OFFSET_DB};
use otb_analysis::stats::{FlightStats, flight_stats, pid_error_stats};
use otb_analysis::step::{LoopInput, StepMetrics, StepOptions, StepResult, step_response};
use otb_analysis::step_series::{FF_KEY, flight_conditions};
use otb_format::fixed;
use otb_log::flightlog::{FlightLog, Rows, clamp_range};
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
/// Repères de Brian White sur l'échelle des spectres de PIDtoolbox : terme D au plus −10 dB, gyro filtré sous −30 dB.
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
/// 5a-3 y ajoutera `frame_resonance`, `dyn_notch`, `motor_harmonic_left_db`, `motor_harmonic_raw_db` et `hover_lines`.
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
fn sel_row<'r>(r: &'r Rows, ax: usize, sel: &Range<usize>) -> &'r [f32] {
    let row = r.row(ax);
    &row[clamp_range(sel.start, sel.end, row.len())]
}

fn wide(v: &[f32]) -> Vec<f64> {
    v.iter().map(|&x| x as f64).collect()
}

/// `np.any(row)` d'une ligne `float32` : un NaN compte comme non nul.
fn any_nonzero(row: &[f32]) -> bool {
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
/// ⚠️ 5a-3 : `delay_rules`, `filter_rules` et `filter_strategy` ne produisent encore rien ; le rapport n'a donc pas leurs constats.
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
    delay_rules(&mut a)?; // 5a-3
    filter_rules(&mut a)?; // 5a-3
    motor_rules(&mut a)?;
    slider_rules(&mut a)?;
    let moving = !a.hover()?;
    if let Some(strategy) = filter_strategy(&a.f, moving) {
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

/// 5a-3 : retards des filtres gyro et D, relèvement des filtres du D (`_delay_rules`, avec `_dterm_lift` et `_dterm_lift_noise`).
/// Sans effet en attendant : aucun constat, aucune mesure.
fn delay_rules(_a: &mut Analysis) -> Result<(), String> {
    Ok(())
}

/// 5a-3 : réglages des filtres (`_filter_rules`) : passe-bas, LPF1 du gyro, filtre RPM, raie fixe, vol stationnaire, notch dynamique.
/// Sans effet en attendant : aucun constat, aucune mesure.
fn filter_rules(_a: &mut Analysis) -> Result<(), String> {
    Ok(())
}

/// 5a-3 : synthèse ordonnée des constats de filtrage (`filter_strategy`). Rend toujours `None` en attendant.
fn filter_strategy(_findings: &[Finding], _moving: bool) -> Option<Finding> {
    None
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
        let r = axis_step(log, &sel, ax, &a.br)?;
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
