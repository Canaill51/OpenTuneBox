//! Éléments sans état de l'assistant à règles (`advisor/rules.py`) : gravités, `Finding`, textes de la réponse indicielle et de la
//! boucle ouverte, fréquences d'enregistrement, constat de déséquilibre des moteurs (phase 5a-2a).
//!
//! Les textes sont comparés caractère par caractère à ceux de Python : les nombres passent par `otb-format` (jamais `{:.2}` de Rust, qui
//! n'arrondit pas comme Python à tous les coups sur les cas limites), `.replace(".", ",")` s'applique à la chaîne entière comme en Python.
//! Les comparaisons avec NaN restent fausses (`not x > y` est vrai pour NaN) : les conditions sont recopiées dans le même sens que Python.
//!
//! Pas encore portés (5a-2b) : `Report` (son champ `measures` dépend des règles de bruit, de filtres et de moteurs), `_Analysis` et les
//! règles qui lisent un `FlightLog`.

use crate::profiles::CraftProfile;
use otb_analysis::npstat::argmax_f64;
use otb_analysis::numpy_mean;
use otb_analysis::open_loop::{LoopMetrics, MEASURED};
use otb_analysis::step::StepResult;
use otb_format::fixed;

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
