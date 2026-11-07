//! `advisor/noise.py` en Rust (phase 5a-4) : diagnostic du bruit et de la stratégie de filtrage, pour la fenêtre Analyse spectrale.
//!
//! Reprend les constats de `rules::analyze` (mêmes seuils, mêmes conseils) et les range en quatre parties : conditions du log, gyro, terme D,
//! stratégie. Ajoute les niveaux de bruit avant et après filtres, axe par axe, et un bloc par filtre (filtre RPM, notch dynamique, passe-bas
//! du gyro et du D) : réglages, mesures, constats, et un verdict même quand aucune règle ne se déclenche. Les verdicts n'ont aucun seuil à
//! eux : ils reprennent les repères de `rules`.
//!
//! Textes et décisions identiques à Python au caractère près ; toute erreur de Python (`Err` ici) est rejouée, sans panique.

use crate::profiles::CraftProfile;
use crate::rules::{
    self, ATTENTION, CONSEIL, DTERM_PTB_MAX_DB, FILTER_STAGES, Finding, GYRO_PTB_MAX_DB, HARMONIC_VISIBLE_DB, INFO, OK,
    RPM_Q_MAX, RPM_WEIGHT_GAIN_MIN, RPM_WEIGHT_MARGIN_DB, Report, band_db, band_peak_ptb, dyn_notch_active,
    main_action, rpm_harmonic_levels,
};
use otb_analysis::npstat::py_min;
use otb_format::fixed;
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::Lowpass;
use std::collections::BTreeSet;
use std::ops::Range;

/// Rangement des constats par clé (`Finding.key`, comme `rules::FILTER_STAGES`). Un constat de bruit ou de filtre absent de ces listes tombe
/// dans la partie gyro.
pub const CONDITION_KEYS: [&str; 2] = ["profile_provisional", "delay_not_representative"];
pub const DTERM_KEYS: [&str; 7] = [
    "dterm_noisy",
    "dterm_noise_level",
    "dterm_filter_weak",
    "dterm_lpf1_static",
    "dterm_filter_too_strong",
    "dterm_lift",
    "dterm_delay",
];
pub const GYRO_KEYS: [&str; 21] = [
    "gyro_noisy",
    "gyro_noise_level",
    "gyro_delay",
    "gyro_filter_cautious",
    "gyro_lpf1_off",
    "gyro_lpf2_off",
    "gyro_lpf_biquad",
    "motor_noise_left",
    "rpm_q_raise",
    "rpm_weight_low",
    "rpm_harmonic_missing",
    "rpm_line_visible",
    "rpm_weight_lighten",
    "frame_line_fixed",
    "hover_line_residual",
    "dyn_notch_q_lower",
    "dyn_notch_q_raise",
    "dyn_notch_more",
    "dyn_notch_fewer",
    "hover_noise_summary",
    "low_band_oscillation",
];
pub const STRATEGY_KEY: &str = "filter_strategy";
/// En mécanique, seule la « Raie fixe » (`GYRO_KEYS`) concerne le bruit.
pub const NOISE_CATEGORIES: [&str; 2] = ["Bruit", "Filtres"];
/// Boucle PID minimale du notch dynamique (`rules::dyn_notch_active`).
pub const DYN_NOTCH_UPDATE_MIN_HZ: i64 = 2000;
/// Valeur par défaut de `rpm_filter_harmonics` (betaflight `pg/rpm_filter.c`).
pub const RPM_HARMONICS_DEFAULT: i64 = 3;

/// Clés d'une étape de `rules::FILTER_STAGES` (`_STAGES[nom]` de Python).
fn stage(name: &str) -> &'static [&'static str] {
    FILTER_STAGES
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(&[], |(_, keys)| *keys)
}

/// Filtre RPM : `_STAGES["Filtre RPM"]`.
pub fn rpm_keys() -> Vec<&'static str> {
    stage("Filtre RPM").to_vec()
}

/// Notch dynamique : `_STAGES["Notch dynamique"] + _STAGES["Mécanique"]` (la raie fixe va au notch, qui la traite).
pub fn notch_keys() -> Vec<&'static str> {
    let mut v = stage("Notch dynamique").to_vec();
    v.extend_from_slice(stage("Mécanique"));
    v
}

/// Passe-bas du gyro : l'étape, puis les constats du curseur « filtre gyro ».
pub fn gyro_lowpass_keys() -> Vec<&'static str> {
    let mut v = stage("Passe-bas du gyro").to_vec();
    v.extend_from_slice(&["gyro_lpf2_off", "gyro_delay", "gyro_noisy"]);
    v
}

/// Bruit d'un axe dans la bande 80 Hz → `hi`, sur l'échelle de PIDtoolbox (dB).
#[derive(Debug, Clone, PartialEq)]
pub struct AxisNoise {
    pub axis: String,
    /// Sommet du spectre lissé avant filtres.
    pub before_db: f64,
    /// Même chose après filtres.
    pub after_db: f64,
    /// Fréquence du sommet avant filtres.
    pub peak_hz: f64,
    /// Baisse du niveau moyen dans la bande (les sommets ne sont pas à la même fréquence).
    pub reduction_db: f64,
}

/// Un filtre : réglages actuels, ce que montre le log, constats de l'assistant, verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterBlock {
    pub title: String,
    pub settings: String,
    pub observations: Vec<String>,
    pub findings: Vec<Finding>,
    /// Seulement si aucun constat ne demande de changer un réglage.
    pub verdict: Option<Finding>,
}

impl FilterBlock {
    fn new(title: &str, settings: String, findings: Vec<Finding>) -> FilterBlock {
        FilterBlock {
            title: title.to_string(),
            settings,
            observations: Vec::new(),
            findings,
            verdict: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoiseReport {
    /// Rapport complet de l'assistant (profil, période, mesures).
    pub report: Report,
    pub band: (f64, f64),
    /// Assez de mouvements pour juger le retard des filtres.
    pub moving: bool,
    /// Gyro non filtré dans le log.
    pub has_unfiltered: bool,
    pub gyro: Vec<AxisNoise>,
    pub dterm: Vec<AxisNoise>,
    /// Hauteur des harmoniques 1–3 avant, après filtres.
    pub harmonics: Option<(Vec<f64>, Vec<f64>)>,
    pub conditions: Vec<Finding>,
    pub gyro_findings: Vec<Finding>,
    pub dterm_findings: Vec<Finding>,
    pub strategy: Vec<Finding>,
    /// Filtre RPM, notch dynamique, passe-bas.
    pub gyro_blocks: Vec<FilterBlock>,
    /// Constats du gyro hors blocs (niveau de bruit…).
    pub gyro_other: Vec<Finding>,
    pub dterm_block: Option<FilterBlock>,
}

/// Partie du diagnostic où ranger un constat de l'assistant (`None` : hors sujet) : `conditions`, `dterm`, `gyro` ou `strategy`.
pub fn section_of(f: &Finding) -> Option<&'static str> {
    if f.key == STRATEGY_KEY {
        return Some("strategy");
    }
    if f.category == "Enregistrement" || CONDITION_KEYS.contains(&f.key.as_str()) {
        return Some("conditions");
    }
    if DTERM_KEYS.contains(&f.key.as_str()) {
        return Some("dterm");
    }
    if GYRO_KEYS.contains(&f.key.as_str()) {
        return Some("gyro");
    }
    if NOISE_CATEGORIES.contains(&f.category.as_str()) {
        return Some("gyro");
    }
    None
}

/// Ligne `ax` d'un tableau sur `sel` (`IndexError` de Python si l'axe n'existe pas).
fn row_of<'r>(r: &'r Rows, ax: usize, sel: &Range<usize>) -> Result<&'r [f32], String> {
    if ax >= r.k {
        return Err("IndexError : index hors du tableau".to_string());
    }
    Ok(rules::sel_row(r, ax, sel))
}

/// `_axis_noise` : niveaux avant / après filtres des axes `0..axes` (un axe dont les valeurs filtrées sont toutes nulles est sauté ; sans
/// signal `raw`, avant et baisse valent NaN).
fn axis_noise(
    raw: Option<&Rows>,
    filt: &Rows,
    sel: &Range<usize>,
    fs: f64,
    band: (f64, f64),
    axes: usize,
    breaks: &[i64],
) -> Result<Vec<AxisNoise>, String> {
    const NAMES: [&str; 3] = ["Roll", "Pitch", "Yaw"];
    let mut out = Vec::new();
    for a in 0..axes {
        let f = row_of(filt, a, sel)?;
        if !rules::any_nonzero(f) {
            continue;
        }
        let name = NAMES.get(a).ok_or("IndexError : tuple index out of range")?.to_string();
        let br = Some(breaks);
        let (hz, before, drop) = match raw {
            Some(r) => {
                let x = row_of(r, a, sel)?;
                let (hz, before) = band_peak_ptb(x, fs, band.0, band.1, br)?;
                let drop = band_db(x, fs, band.0, band.1, br)? - band_db(f, fs, band.0, band.1, br)?;
                (hz, before, drop)
            }
            None => (f64::NAN, f64::NAN, f64::NAN),
        };
        let after = band_peak_ptb(f, fs, band.0, band.1, br)?.1;
        out.push(AxisNoise {
            axis: name,
            before_db: before,
            after_db: after,
            peak_hz: hz,
            reduction_db: drop,
        });
    }
    Ok(out)
}

/// État d'un passe-bas en texte : « coupé », « PT1 dynamique 75–150 Hz » ou « PT1 150 Hz ».
pub fn lowpass_text(lp: &Lowpass) -> String {
    if !lp.enabled() {
        return "coupé".to_string();
    }
    if lp.dynamic() {
        return format!("{} dynamique {}–{} Hz", lp.kind, lp.dyn_min_hz, lp.dyn_max_hz);
    }
    format!("{} {} Hz", lp.kind, lp.static_hz)
}

/// Un constat demande-t-il un changement de réglage (actions et gravité `attention` ou `conseil`) ?
fn actionable(findings: &[Finding]) -> bool {
    findings
        .iter()
        .any(|f| !f.actions.is_empty() && (f.severity == ATTENTION || f.severity == CONSEIL))
}

fn fx0(x: f64) -> String {
    fixed(x, 0, false)
}

fn rpm_block(log: &FlightLog, nr: &NoiseReport, findings: Vec<Finding>) -> Result<FilterBlock, String> {
    let s = &log.settings;
    let n = s.rpm_harmonics;
    // (list(s.rpm_weights) + [100] * 3)[:max(n, 1)]
    let mut w: Vec<i64> = s.rpm_weights.clone();
    w.extend_from_slice(&[100, 100, 100]);
    w.truncate(n.max(1) as usize);
    let settings = if n != 0 {
        format!(
            "rpm_filter_harmonics = {n}, rpm_filter_weights = {}, rpm_filter_q = {}, rpm_filter_min_hz = {}",
            w.iter().map(i64::to_string).collect::<Vec<_>>().join(", "),
            s.rpm_q,
            s.rpm_min_hz
        )
    } else {
        "coupé (rpm_filter_harmonics = 0)".to_string()
    };
    let mut b = FilterBlock::new("Filtre RPM", settings, findings);
    if log.motor_hz.is_none() {
        b.verdict = Some(Finding::new(
            "rpm_block_no_rpm",
            "Filtres",
            if !s.dshot_bidir { CONSEIL } else { INFO },
            "Filtre RPM : impossible à juger",
            "Pas d'eRPM dans le log : sans télémétrie RPM, le filtre RPM ne fonctionne pas et les harmoniques moteur ne peuvent pas être repérées.",
            vec![if s.dshot_bidir {
                "blackbox_disable_rpm = OFF (dshot_bidir est déjà activé).".to_string()
            } else {
                format!(
                    "dshot_bidir = ON si vos ESC le permettent (BLHeli_32, Bluejay, AM32), puis rpm_filter_harmonics = {RPM_HARMONICS_DEFAULT}."
                )
            }],
            None,
        ));
        return Ok(b);
    }
    if n == 0 {
        b.verdict = Some(Finding::new(
            "rpm_block_off",
            "Filtres",
            CONSEIL,
            "Filtre RPM coupé",
            "L'eRPM est enregistré mais le filtre RPM est coupé : c'est le filtre le plus efficace contre le bruit moteur, pour très peu de retard.",
            vec![format!(
                "rpm_filter_harmonics = {RPM_HARMONICS_DEFAULT} (valeur par défaut de Betaflight), puis réglez les poids d'après le log suivant."
            )],
            None,
        ));
        return Ok(b);
    }
    let Some((raw, left)) = &nr.harmonics else {
        b.verdict = Some(Finding::new(
            "rpm_block_unmeasurable",
            "Filtres",
            INFO,
            "Filtre RPM : efficacité non mesurable",
            "Sans gyro non filtré, on ne voit pas les harmoniques moteur avant filtrage.",
            vec!["En 4.4 : debug_mode = GYRO_SCALED. En 4.5 et plus : blackbox_disable_gyrounfilt = OFF.".to_string()],
            None,
        ));
        return Ok(b);
    };
    let idx = |v: &[f64], k: usize| -> Result<f64, String> {
        v.get(k)
            .copied()
            .ok_or_else(|| "IndexError : index hors du tableau".to_string())
    };
    for k in 0..3usize {
        let (r, l) = (idx(raw, k)?, idx(left, k)?);
        if !(r.is_finite() && l.is_finite()) {
            continue;
        }
        let state = if l <= HARMONIC_VISIBLE_DB {
            "effacée"
        } else {
            "encore visible"
        };
        let how = if (k as i64) < n {
            format!("poids {}", w.get(k).ok_or("IndexError : list index out of range")?)
        } else {
            "non traitée par le filtre RPM".to_string()
        };
        b.observations.push(format!(
            "{}e harmonique : {} → {} dB au-dessus du bruit voisin ({how}) : {state}.",
            k + 1,
            fx0(r),
            fx0(l)
        ));
    }
    if actionable(&b.findings) {
        return Ok(b);
    }
    let why = if nr.report.measures.aliased.unwrap_or(false) {
        "le log est trop lent pour proposer d'alléger les poids".to_string()
    } else if !s.version.is_empty() && s.version.as_slice() < [4i64, 5].as_slice() {
        "rpm_filter_weights n'existe qu'à partir de Betaflight 4.5".to_string()
    } else {
        format!(
            "aucun poids ne peut baisser d'au moins {} en gardant {} dB de marge sous le bruit voisin",
            fixed(RPM_WEIGHT_GAIN_MIN, 1, false),
            fx0(RPM_WEIGHT_MARGIN_DB)
        )
    };
    let q = if s.rpm_q >= RPM_Q_MAX {
        format!(
            "rpm_filter_q est déjà à {}, le maximum conseillé ({RPM_Q_MAX})",
            s.rpm_q
        )
    } else {
        "L'assistant propose de monter rpm_filter_q (notchs plus étroits, moins de retard) quand les harmoniques sont effacées et que le bruit du gyro et du D est sous les repères".to_string()
    };
    b.verdict = Some(Finding::new(
        "rpm_block_ok",
        "Filtres",
        OK,
        "Filtre RPM : rien à changer",
        &format!(
            "Les harmoniques traitées sont effacées (au plus {} dB au-dessus du bruit voisin après filtres) ; {why}. {q}.",
            fx0(HARMONIC_VISIBLE_DB)
        ),
        Vec::new(),
        None,
    ));
    Ok(b)
}

fn notch_block(log: &FlightLog, nr: &NoiseReport, findings: Vec<Finding>) -> Result<FilterBlock, String> {
    let s = &log.settings;
    let m = &nr.report.measures;
    let settings = if s.dyn_notch_count == 0 || s.dyn_notch_max_hz == 0 {
        "coupé (dyn_notch_count = 0)".to_string()
    } else {
        let mut t = format!(
            "dyn_notch_count = {}, dyn_notch_q = {}, dyn_notch_min_hz = {}, dyn_notch_max_hz = {}",
            s.dyn_notch_count, s.dyn_notch_q, s.dyn_notch_min_hz, s.dyn_notch_max_hz
        );
        if s.pid_rate_hz() < DYN_NOTCH_UPDATE_MIN_HZ as f64 {
            t += &format!(
                " ; inactif : boucle PID sous {} kHz",
                fx0(DYN_NOTCH_UPDATE_MIN_HZ as f64 / 1000.0)
            );
        }
        t
    };
    let mut b = FilterBlock::new("Notch dynamique", settings, findings);
    if let Some(fr) = &m.frame_resonance
        && !fr.near_motor
    {
        b.observations.push(format!(
            "Raie la plus marquée quel que soit le gaz : ~{} Hz, {} dB au-dessus du bruit voisin.",
            fx0(fr.freq_hz),
            fixed(fr.excess_db, 0, true)
        ));
    }
    let dn = m.dyn_notch.as_ref();
    if let Some(dn) = dn {
        let peaks: Vec<String> = dn
            .peaks
            .iter()
            .zip(&dn.widths)
            .zip(&dn.heights)
            .map(|((&f0, &w), &h)| {
                let mut t = format!("{} Hz", fx0(f0));
                if w.is_finite() && h.is_finite() {
                    t += &format!(" ({} Hz de large, {} dB)", fx0(w), fixed(h, 0, true));
                }
                t
            })
            .collect();
        let peaks = peaks.join(", ");
        b.observations.push(if !peaks.is_empty() {
            format!(
                "Pics à suivre après le filtre RPM (largeur à mi-puissance, hauteur au-dessus du bruit voisin) : {peaks}."
            )
        } else {
            "Aucun pic à suivre après le filtre RPM.".to_string()
        });
    }
    if actionable(&b.findings) {
        return Ok(b);
    }
    let rpm_on = log.motor_hz.is_some() && s.rpm_harmonics > 0;
    if !dyn_notch_active(s) {
        if rpm_on {
            b.verdict = Some(Finding::new(
                "notch_block_ok",
                "Filtres",
                OK,
                "Notch dynamique : rien à changer",
                "Il est coupé ; avec le filtre RPM, il ne sert qu'aux résonances qui ne viennent pas des moteurs, et aucune n'est signalée.",
                Vec::new(),
                None,
            ));
        } else {
            b.verdict = Some(Finding::new(
                "notch_block_both_off",
                "Filtres",
                INFO,
                "Notch dynamique et filtre RPM coupés",
                "Seuls les passe-bas filtrent le bruit moteur : activez d'abord le filtre RPM (bloc ci-dessus), puis jugez le notch dynamique sur le log suivant.",
                Vec::new(),
                None,
            ));
        }
        return Ok(b);
    }
    let Some(dn) = dn else {
        b.verdict = Some(Finding::new(
            "notch_block_unjudged",
            "Filtres",
            INFO,
            "Notch dynamique : réglage non jugé",
            "Il faut le gyro non filtré et le filtre RPM actif : sans filtre RPM, le notch dynamique suit aussi les raies moteur et son nombre de notchs ne se juge pas.",
            Vec::new(),
            None,
        ));
        return Ok(b);
    };
    let out: Vec<f64> = dn
        .peaks
        .iter()
        .copied()
        .filter(|&f| f < dn.min_hz as f64 || f > dn.max_hz as f64)
        .collect();
    let mut detail = format!(
        "{} pic(s) à suivre pour {} notch(s), plage {}–{} Hz.",
        dn.peaks.len(),
        dn.count,
        dn.min_hz,
        dn.max_hz
    );
    if dn.peaks.len() as i64 > dn.count {
        // fréquent en vol avec mouvements (les pics bougent avec les gaz) : simple remarque, pas un défaut
        detail += " Plus de pics que de notchs : les notchs suivent les plus forts ; l'assistant propose un notch par pic seulement si une raie passe les filtres ou si le gyro filtré est bruité.";
    }
    if !out.is_empty() && nr.moving {
        // en stationnaire, rules._hover_noise traite déjà chaque raie qui passe
        b.verdict = Some(Finding::new(
            "notch_block_peak_outside",
            "Filtres",
            INFO,
            "Notch dynamique : pic en dehors de la plage",
            &format!(
                "{detail} Pic à {} Hz hors de la plage : l'assistant ne propose de l'élargir que pour une raie vue en vol stationnaire ou une raie fixe.",
                out.iter().map(|&f| fx0(f)).collect::<Vec<_>>().join(", ")
            ),
            vec!["Vol stationnaire de 20 à 30 s pour confirmer la raie.".to_string()],
            None,
        ));
    } else {
        b.verdict = Some(Finding::new(
            "notch_block_ok",
            "Filtres",
            OK,
            "Notch dynamique : rien à changer",
            &format!(
                "{detail} dyn_notch_q {} : l'assistant calcule le Q d'après la largeur et la hauteur des raies à suivre ; il le baisse si une raie l'exige et le monte (jusqu'à {}) quand rien ne passe et que le bruit du gyro et du D est sous les repères.",
                s.dyn_notch_q,
                rules::DYN_NOTCH_Q_RPM.1
            ),
            Vec::new(),
            None,
        ));
    }
    Ok(b)
}

fn lowpass_block(title: &str, lps: [&Lowpass; 2], findings: Vec<Finding>, moving: bool, prefix: &str) -> FilterBlock {
    let settings = ["LPF1", "LPF2"]
        .iter()
        .zip(lps)
        .map(|(name, lp)| format!("{prefix}_{} : {}", name.to_lowercase(), lowpass_text(lp)))
        .collect::<Vec<_>>()
        .join(" ; ");
    let mut b = FilterBlock::new(title, settings, findings);
    if !actionable(&b.findings) {
        b.verdict = Some(Finding::new(
            "lowpass_block_ok",
            "Filtres",
            OK,
            &format!("{title} : rien à changer"),
            if moving {
                "Aucun constat ne demande de les modifier."
            } else {
                "Aucun constat de bruit ne demande de les modifier ; leur retard se juge sur un vol avec mouvements."
            },
            Vec::new(),
            None,
        ));
    }
    b
}

/// Moins de deux étapes de filtrage : l'assistant n'écrit pas de stratégie, on dit quoi faire quand même.
fn strategy_fallback(actionable: &[Finding], moving: bool) -> Result<Finding, String> {
    let next_log = if !moving {
        "Log suivant : vol avec des mouvements francs (flips, rolls) pour juger le retard des filtres."
    } else {
        "Log suivant : même type de vol, pour comparer."
    };
    let Some(f) = actionable.first() else {
        return Ok(Finding::new(
            "filter_no_change",
            "Filtres",
            OK,
            "Aucun changement de filtre conseillé",
            "Aucun constat de bruit ou de filtrage ne demande de modifier un réglage sur cette période.",
            vec![next_log.to_string()],
            None,
        ));
    };
    Ok(Finding::new(
        "filter_single_step",
        "Filtres",
        CONSEIL,
        "Un seul réglage de filtre à faire",
        &format!(
            "Commencez par « {} » (détail ci-dessus), puis vérifiez la température des moteurs.",
            f.title
        ),
        vec![main_action(f)?, next_log.to_string()],
        None,
    ))
}

/// `noise_report` : le diagnostic du bruit d'un vol. `sel` : période (tout le journal si absente) ; `profile` : taille choisie (détectée si absente).
pub fn noise_report(
    log: &FlightLog,
    sel: Option<Range<usize>>,
    profile: Option<&CraftProfile>,
) -> Result<NoiseReport, String> {
    let sel = sel.unwrap_or(0..log.n());
    let rep = rules::analyze(log, Some(sel.clone()), profile)?;
    let hi = py_min(500.0, 0.95 * log.fs / 2.0);
    let band = (80.0, hi);
    let moving = rep.measures.delays.as_ref().ok_or("KeyError : 'delays'")?.moving();
    let mut nr = NoiseReport {
        report: rep,
        band,
        moving,
        has_unfiltered: log.gyro_unfilt.is_some(),
        gyro: Vec::new(),
        dterm: Vec::new(),
        harmonics: None,
        conditions: Vec::new(),
        gyro_findings: Vec::new(),
        dterm_findings: Vec::new(),
        strategy: Vec::new(),
        gyro_blocks: Vec::new(),
        gyro_other: Vec::new(),
        dterm_block: None,
    };
    // aucune fenêtre sur un trou du log
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    nr.gyro = axis_noise(log.gyro_unfilt.as_ref(), &log.gyro, &sel, log.fs, band, 3, &br)?;
    let prefilter = log.d_term_prefilter()?;
    nr.dterm = axis_noise(Some(&prefilter), &log.d_term, &sel, log.fs, band, 2, &br)?;
    if log.gyro_unfilt.is_some() && log.motor_hz.is_some() {
        nr.harmonics = Some(rpm_harmonic_levels(log, &sel)?);
    }
    for f in nr.report.sorted() {
        match section_of(&f) {
            Some("conditions") => nr.conditions.push(f),
            Some("gyro") => nr.gyro_findings.push(f),
            Some("dterm") => nr.dterm_findings.push(f),
            Some("strategy") => nr.strategy.push(f),
            _ => {}
        }
    }
    let s = &log.settings;
    // `pick` : indices dans `gyro_findings` (Python compare les identités : si les listes de clés des blocs se recoupaient un jour, un même constat entrerait dans plusieurs blocs)
    let pick = |keys: &[&str]| -> Vec<usize> {
        nr.gyro_findings
            .iter()
            .enumerate()
            .filter(|(_, f)| keys.contains(&f.key.as_str()))
            .map(|(i, _)| i)
            .collect()
    };
    let take = |idx: &[usize]| -> Vec<Finding> { idx.iter().map(|&i| nr.gyro_findings[i].clone()).collect() };
    let (rpm_idx, notch_idx, low_idx) = (pick(&rpm_keys()), pick(&notch_keys()), pick(&gyro_lowpass_keys()));
    let blocks = vec![
        rpm_block(log, &nr, take(&rpm_idx))?,
        notch_block(log, &nr, take(&notch_idx))?,
        lowpass_block(
            "Passe-bas du gyro",
            [&s.gyro_lpf1, &s.gyro_lpf2],
            take(&low_idx),
            moving,
            "gyro",
        ),
    ];
    let in_blocks: BTreeSet<usize> = rpm_idx.iter().chain(&notch_idx).chain(&low_idx).copied().collect();
    let other: Vec<Finding> = nr
        .gyro_findings
        .iter()
        .enumerate()
        .filter(|(i, _)| !in_blocks.contains(i))
        .map(|(_, f)| f.clone())
        .collect();
    nr.gyro_blocks = blocks;
    nr.gyro_other = other;
    nr.dterm_block = Some(lowpass_block(
        "Passe-bas du D",
        [&s.dterm_lpf1, &s.dterm_lpf2],
        nr.dterm_findings.clone(),
        moving,
        "dterm",
    ));
    let verdicts: Vec<Finding> = nr.gyro_blocks.iter().filter_map(|b| b.verdict.clone()).collect();
    if nr.strategy.is_empty() {
        let mut acts: Vec<Finding> = nr
            .gyro_findings
            .iter()
            .chain(&nr.dterm_findings)
            .chain(&verdicts)
            .filter(|f| {
                !f.actions.is_empty() && (f.severity == ATTENTION || f.severity == CONSEIL) && f.category != "Réponse"
            })
            .cloned()
            .collect();
        acts.sort_by_key(|f| rules::severity_order(&f.severity)); // tri stable
        nr.strategy.push(strategy_fallback(&acts, moving)?);
    }
    Ok(nr)
}

/// Repère usuel affiché sous le tableau des niveaux (échelle de PIDtoolbox).
pub fn level_note(kind: &str) -> String {
    if kind == "gyro" {
        return format!("Repère après filtres : sous {} dB.", fx0(GYRO_PTB_MAX_DB));
    }
    format!(
        "Repère après filtres : au plus {} dB, idéal sous −20 dB.",
        fx0(DTERM_PTB_MAX_DB)
    )
}
