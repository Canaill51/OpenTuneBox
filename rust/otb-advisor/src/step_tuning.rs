//! `advisor/step_tuning.py` en Rust (phase 5a-5) : diagnostic du réglage par la réponse indicielle (séries de vols).
//!
//! Méthode de Brian White (`docs/spec-methodes.md`, [S32]) : séries de vols où un seul curseur change par pas de 0,2, feedforward à 0 (ou
//! mode Angle), curseur I à 0,2–0,3. Trois blocs, plus un quatrième en finitions : équilibre P/D (série Damping), niveau des gains (série
//! Master), équilibre roll / pitch, série Feedforward reconnue mais pas jugée. Seuils et origine : `docs/assistant.md`.
//!
//! Textes et décisions identiques à Python au caractère près. Toute erreur de Python (indice hors tableau, curseur inconnu) est un `Err`
//! ici, sans panique. Pièges de Python reproduits : `round(x, 2)` au pair sur la valeur binaire exacte (`otb_format::round_digits`), `max`
//! et `min` qui gardent le **premier** ex æquo, `max` d'une liste à NaN qui dépend de l'ordre, comparaisons avec NaN toujours fausses.
//! Hors du domaine de `find_series` (une valeur de curseur NaN), `sorted` de Python n'est pas reproduit : `Err`.

use crate::rules::{ATTENTION, CONSEIL, Finding, INFO, OK, STEP_MIN_WINDOWS, STEP_SURE_WINDOWS};
use otb_analysis::step::StepMetrics;
use otb_analysis::step_series::{FF_KEY, FlightStep, Series, find_series};
use otb_format::{fixed, round_digits};
use std::collections::BTreeSet;

/// Pas conseillé entre deux vols [S32].
pub const SERIES_STEP: f64 = 0.2;
/// Pas plus fin : écarts trop faibles pour se lire sur la réponse [S32].
pub const STEP_TOO_FINE: f64 = 0.15;
/// Curseur I conseillé 0,2–0,3 [S32], à 0,05 près.
pub const I_SLIDER_RANGE: (f64, f64) = (0.15, 0.35);
/// Fenêtres exploitables par axe : en dessous, le vol n'est pas lu.
pub const MIN_WINDOWS: usize = STEP_MIN_WINDOWS;
/// En dessous, mesure moins sûre (avertissement) : consigne de Brian White [S35].
pub const SURE_WINDOWS: usize = STEP_SURE_WINDOWS;
/// Pic « sans dépassement » (repère 1,03 de `rules::STEP_PEAK_FLAT`, plus le bruit d'un vol).
pub const PEAK_FLAT_MAX: f64 = 1.05;
/// Le cran suivant ne baisse plus le pic que de moins de 0,02 : D suffisant.
pub const PEAK_PLATEAU: f64 = 0.02;
/// … à condition que le pic soit déjà modéré.
pub const PEAK_PLATEAU_MAX: f64 = 1.10;
/// Pitch qui dépasse plus que le roll au Damping choisi : Pitch Damping à monter.
pub const PITCH_PEAK_GAP: f64 = 0.02;
/// Cassure nette (gardés ≤ 0,067, rejetés ≥ 0,064).
pub const RIPPLE_HIGH: f64 = 0.065;
/// Début de cassure : près de l'optimum.
pub const RIPPLE_EARLY: f64 = 0.03;
/// Cassure qui, avec une latence en plateau, suffit à arrêter.
pub const RIPPLE_PLATEAU: f64 = 0.05;
/// Au-delà, pas d'affinage entre les deux crans.
pub const RIPPLE_STRONG: f64 = 0.10;
/// Cassure qui grandit d'un cran à l'autre.
pub const RIPPLE_GROWTH: f64 = 0.02;
/// Gain de latence d'un cran de +0,2 sous lequel la latence ne baisse plus.
pub const LATENCY_PLATEAU_PCT: f64 = 4.0;
/// Écart de latence pitch − roll : équilibré.
pub const LATENCY_GAP_OK_MS: f64 = 1.0;
/// Acceptable selon le drone (inertie, moteurs) ; au-delà, à corriger.
pub const LATENCY_GAP_MAX_MS: f64 = 2.0;
/// Vagues nettement au-dessus du bruit.
pub const WAVES_RATIO_HIGH: f64 = 2.5;

const AXES2: [&str; 2] = ["Roll", "Pitch"];

/// Nom d'un curseur dans le configurateur Betaflight (`SLIDER_NAMES` : les séries gardent les clés de PIDtoolbox). `KeyError` de Python
/// pour une clé inconnue.
fn slider_name(key: &str) -> Result<&'static str, String> {
    match key {
        "Damping" => Ok("Damping"),
        "Master" => Ok("Master"),
        "Pitch T." => Ok("Pitch Tracking"),
        "Pitch D." => Ok("Pitch Damping"),
        k if k == FF_KEY => Ok("Feedforward"),
        k => Err(format!("KeyError : {k:?}")),
    }
}

/// Un bloc du diagnostic : une série (ou un vol), les mesures vol par vol, le constat, le vol conseillé.
#[derive(Debug, Clone, PartialEq)]
pub struct TuningBlock {
    pub title: String,
    pub series: Option<Series>,
    /// Mesures vol par vol.
    pub lines: Vec<String>,
    pub finding: Option<Finding>,
    /// Indice du vol conseillé.
    pub best: Option<usize>,
}

impl TuningBlock {
    fn new(title: String, series: Option<Series>) -> TuningBlock {
        TuningBlock {
            title,
            series,
            lines: Vec::new(),
            finding: None,
            best: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TuningReport {
    pub conditions: Vec<Finding>,
    pub blocks: Vec<TuningBlock>,
}

/// 1,2 / 1,15 / 0,065 : virgule, zéros inutiles retirés (au moins un chiffre après la virgule).
pub fn fmt(v: f64) -> String {
    let s = fixed(v, 3, false);
    let t = s.trim_end_matches('0');
    let t = if t.ends_with('.') {
        format!("{t}0")
    } else {
        t.to_string()
    };
    t.replace('.', ",")
}

/// Nombre à la française : virgule décimale, « – » s'il manque.
pub fn n_text(x: f64, d: usize, sign: bool) -> String {
    if !x.is_finite() {
        return "–".to_string();
    }
    fixed(x, d, sign).replace('.', ",")
}

fn n2(x: f64) -> String {
    n_text(x, 2, false)
}

/// Axes (0 = roll, 1 = pitch) lus : au moins `MIN_WINDOWS` fenêtres.
pub fn axes(f: &FlightStep) -> Vec<usize> {
    (0..2).filter(|&a| a < f.n.len() && f.n[a] >= MIN_WINDOWS).collect()
}

/// `f.metrics[a]` (`IndexError` de Python si le vol n'a pas cet axe).
fn metric(f: &FlightStep, a: usize) -> Result<&StepMetrics, String> {
    f.metrics
        .get(a)
        .ok_or_else(|| format!("IndexError : le vol {:?} n'a pas de métriques pour l'axe {a}", f.label))
}

fn flight(flights: &[FlightStep], i: usize) -> Result<&FlightStep, String> {
    flights
        .get(i)
        .ok_or_else(|| format!("IndexError : vol {i} hors de la liste de {} vols", flights.len()))
}

fn sum_n(f: &FlightStep) -> usize {
    f.n.iter().sum()
}

/// Un vol par valeur du curseur : celui qui a le plus de fenêtres (le premier à égalité). Valeurs triées, indices dans le même ordre.
pub fn pick(flights: &[FlightStep], s: &Series) -> Result<(Vec<f64>, Vec<usize>), String> {
    // dictionnaire de Python : une clé flottante égale (`==`) est la même clé ; la première insérée reste la clé
    let mut best: Vec<(f64, usize)> = Vec::new();
    for (&i, &v) in s.flights.iter().zip(&s.values) {
        match best.iter().position(|(k, _)| *k == v) {
            None => best.push((v, i)),
            Some(p) => {
                if sum_n(flight(flights, i)?) > sum_n(flight(flights, best[p].1)?) {
                    best[p].1 = i;
                }
            }
        }
    }
    if best.iter().any(|(k, _)| k.is_nan()) {
        return Err("sorted : valeur de curseur NaN (tri non défini)".to_string());
    }
    best.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(best.into_iter().unzip())
}

/// Conditions du protocole ; les vols d'une série Feedforward (`ff_series`) ont le feedforward voulu.
pub fn conditions(flights: &[FlightStep], ff_series: &BTreeSet<usize>) -> Result<Vec<Finding>, String> {
    let mut out = Vec::new();
    let ff: Vec<&str> = flights
        .iter()
        .enumerate()
        .filter(|(i, f)| f.conditions.ff_active() && !ff_series.contains(i))
        .map(|(_, f)| f.label.as_str())
        .collect();
    if !ff.is_empty() {
        out.push(Finding::new(
            "ff_active",
            "Réglage",
            CONSEIL,
            "Feedforward actif",
            &format!(
                "{} : feedforward > 0 hors mode Angle. Il gonfle le pic et raccourcit la latence : la réponse ne montre plus seulement le P et le D.",
                ff.join(", ")
            ),
            vec![
                "Pour les séries : feedforward à 0 en Acro, ou vols en mode Angle (le firmware y coupe le feedforward de la boucle de vitesse)."
                    .to_string(),
            ],
            None,
        ));
    }
    let mut angle: Vec<&str> = Vec::new();
    for &i in ff_series {
        let f = flight(flights, i)?;
        if f.conditions.angle() {
            angle.push(f.label.as_str());
        }
    }
    if !angle.is_empty() {
        out.push(Finding::new(
            "ff_series_angle",
            "Réglage",
            ATTENTION,
            "Série Feedforward en mode Angle",
            &format!(
                "{} : le firmware coupe le feedforward en mode Angle, ces vols ne mesurent pas son effet.",
                angle.join(", ")
            ),
            vec!["Série Feedforward : vols en Acro uniquement.".to_string()],
            None,
        ));
    }
    let (lo, hi) = I_SLIDER_RANGE;
    let bad_i: Vec<String> = flights
        .iter()
        .filter(|f| !(lo <= f.conditions.i_slider && f.conditions.i_slider <= hi))
        .map(|f| format!("{} ({})", f.label, fmt(f.conditions.i_slider)))
        .collect();
    if !bad_i.is_empty() {
        out.push(Finding::new(
            "i_slider_out",
            "Réglage",
            CONSEIL,
            "Curseur I hors de 0,2–0,3",
            &format!(
                "{}. Le terme I change la forme de la réponse et fausse la lecture.",
                bad_i.join(", ")
            ),
            vec![
                "Pour les séries : curseur I (Drift-Wobble) entre 0,2 et 0,3, remonté une fois le réglage terminé."
                    .to_string(),
            ],
            None,
        ));
    }
    let few: Vec<&str> = flights
        .iter()
        .filter(|f| axes(f).is_empty())
        .map(|f| f.label.as_str())
        .collect();
    if !few.is_empty() {
        out.push(Finding::new(
            "few_windows",
            "Réglage",
            ATTENTION,
            "Pas assez de mouvements",
            &format!(
                "{} : moins de {MIN_WINDOWS} fenêtres exploitables en roll et en pitch.",
                few.join(", ")
            ),
            vec!["Volez 20 à 30 s avec des flips et rolls francs sur chaque axe.".to_string()],
            None,
        ));
    }
    // `min(f.n[:2])` : les deux premiers axes (le vol a au moins un axe lu, donc `n` n'est pas vide)
    let thin: Vec<String> = flights
        .iter()
        .filter_map(|f| {
            let m = f.n.iter().take(2).min().copied()?;
            (!axes(f).is_empty() && f.n.len() >= 2 && m < SURE_WINDOWS).then(|| format!("{} (N = {m})", f.label))
        })
        .collect();
    if !thin.is_empty() {
        out.push(Finding::new(
            "few_windows_unsure",
            "Réglage",
            CONSEIL,
            "Mesure moins sûre",
            &format!(
                "{} : moins de {SURE_WINDOWS} fenêtres en roll ou en pitch. La réponse moyenne bouge encore d'un vol à l'autre ; le choix peut se jouer à un cran près.",
                thin.join(", ")
            ),
            vec![
                "Wobbles roll et pitch mêlés, au moins 20 s (environ 12 fenêtres par seconde de mouvement)."
                    .to_string(),
            ],
            None,
        ));
    }
    Ok(out)
}

/// Remarque sur un pas trop fin entre deux vols (vide si aucun).
fn step_note(s: &Series) -> String {
    let mut fine = s.steps().into_iter().filter(|&d| d < STEP_TOO_FINE);
    let Some(first) = fine.next() else {
        return String::new();
    };
    // `min` de Python : le premier, remplacé par un élément strictement plus petit
    let m = fine.fold(first, |m, d| if d < m { d } else { m });
    format!(
        " Pas de {} entre deux vols : trop fin pour se lire sur la réponse, préférez {}.",
        fmt(m),
        fmt(SERIES_STEP)
    )
}

/// Pic arrondi à 0,01 : la valeur lue à l'écran (1,052 s'affiche et se juge 1,05) ; NaN si l'axe n'est pas lu.
fn peak(f: &FlightStep, ax: usize) -> Result<f64, String> {
    if axes(f).contains(&ax) {
        Ok(round_digits(metric(f, ax)?.peak, 2))
    } else {
        Ok(f64::NAN)
    }
}

/// Série Damping (jugée sur le roll : Damping règle le D des deux axes, le pitch se corrige ensuite par Pitch D.) ou série Pitch D.
/// (jugée sur le pitch).
pub fn damping_block(flights: &[FlightStep], s: &Series) -> Result<TuningBlock, String> {
    let ax = usize::from(s.key == "Pitch D.");
    let name = slider_name(s.key)?;
    let title = if ax == 1 {
        format!("Équilibre roll / pitch (série {name})")
    } else {
        "Équilibre P/D (série Damping)".to_string()
    };
    let mut b = TuningBlock::new(title, Some(s.clone()));
    let (vals, idx) = pick(flights, s)?;
    let peaks: Vec<f64> = idx
        .iter()
        .map(|&i| peak(flight(flights, i)?, ax))
        .collect::<Result<_, _>>()?;
    for (&v, &i) in vals.iter().zip(&idx) {
        let f = flight(flights, i)?;
        let mut parts = Vec::new();
        for a in axes(f) {
            parts.push(format!("{} {}", AXES2[a].to_lowercase(), n2(metric(f, a)?.peak)));
        }
        b.lines
            .push(format!("{name} {} : pic {} ({})", fmt(v), parts.join(" / "), f.label));
    }
    let ok: Vec<usize> = (0..peaks.len()).filter(|&k| peaks[k].is_finite()).collect();
    if ok.len() < 2 {
        b.finding = Some(Finding::new(
            "series_incomplete",
            "Réponse",
            INFO,
            &format!("Série {name} incomplète"),
            "Il faut au moins deux vols exploitables.",
            Vec::new(),
            None,
        ));
        return Ok(b);
    }
    let axis = AXES2[ax].to_lowercase();
    let mut chosen: Option<usize> = None;
    let mut why = String::new();
    let mut flat = false;
    for (j, &k) in ok.iter().enumerate() {
        if peaks[k] <= PEAK_FLAT_MAX {
            chosen = Some(k);
            flat = true;
            why = format!("premier pic {axis} sans dépassement visible (≤ {})", fmt(PEAK_FLAT_MAX));
            break;
        }
        if j + 1 < ok.len() && peaks[k] <= PEAK_PLATEAU_MAX && peaks[k] - peaks[ok[j + 1]] < PEAK_PLATEAU - 1e-9 {
            chosen = Some(k);
            why = format!(
                "le cran suivant ne baisse plus le pic {axis} (moins de {})",
                fmt(PEAK_PLATEAU)
            );
            break;
        }
    }
    let note = step_note(s);
    let Some(chosen) = chosen else {
        let last = vals[ok[ok.len() - 1]];
        b.finding = Some(Finding::new(
            "series_more_overshoot",
            "Réponse",
            CONSEIL,
            &format!("Encore du dépassement en {axis}"),
            &format!(
                "Le pic {axis} reste au-dessus de {} jusqu'à {name} {}.{note}",
                fmt(PEAK_FLAT_MAX),
                fmt(last)
            ),
            vec![format!("Volez avec {name} {}, puis comparez.", fmt(last + SERIES_STEP))],
            None,
        ));
        return Ok(b);
    };
    b.best = Some(idx[chosen]);
    if chosen == ok[0] && flat {
        let prv = vals[chosen] - SERIES_STEP;
        b.finding = Some(Finding::new(
            "series_already_flat",
            "Réponse",
            CONSEIL,
            &format!("{name} {} : déjà sans dépassement", fmt(vals[chosen])),
            &format!(
                "Pic {axis} {} dès la plus petite valeur testée : une valeur plus basse suffit peut-être.{note}",
                n2(peaks[chosen])
            ),
            vec![format!("Volez avec {name} {} pour trouver la limite.", fmt(prv))],
            None,
        ));
        return Ok(b);
    }
    let mut detail = format!("Pic {axis} {} : {why}.{note}", n2(peaks[chosen]));
    let mut actions = vec![format!(
        "Gardez {name} {}{}",
        fmt(vals[chosen]),
        if ax == 0 { " pour la série Master." } else { "." }
    )];
    let mut sev = OK;
    let pitch = peak(flight(flights, idx[chosen])?, 1)?;
    if ax == 0 && pitch > PEAK_FLAT_MAX && pitch - peaks[chosen] >= PITCH_PEAK_GAP {
        sev = CONSEIL;
        detail += &format!(
            " Le pitch dépasse encore ({}) : il a plus d'inertie, son D se règle à part.",
            n2(pitch)
        );
        actions.push(format!(
            "Pitch Damping (simplified_pitch_d_gain) +{} pour amortir le pitch, puis comparez son pic.",
            fmt(SERIES_STEP)
        ));
    }
    b.finding = Some(Finding::new(
        "series_choice",
        "Réponse",
        sev,
        &format!("{name} conseillé : {}", fmt(vals[chosen])),
        &detail,
        actions,
        None,
    ));
    Ok(b)
}

/// Plus forte cassure des deux axes et son écart-type.
fn ripple(f: &FlightStep) -> Result<(f64, f64), String> {
    let mut best = (f64::NAN, f64::NAN);
    for a in axes(f) {
        let m = metric(f, a)?;
        // `not (m.ripple <= best[0])` : vrai tant que `best` est NaN (`m.ripple` est fini ici)
        if m.ripple.is_finite() && (m.ripple > best.0 || best.0.is_nan()) {
            best = (m.ripple, m.ripple_std);
        }
    }
    Ok(best)
}

fn latency(f: &FlightStep) -> Result<f64, String> {
    let mut v = Vec::new();
    for a in axes(f) {
        v.push(metric(f, a)?.latency_ms);
    }
    if v.is_empty() {
        return Ok(f64::NAN);
    }
    // `sum(v) / len(v)` : sommes d'un ou deux termes (au plus deux axes), identiques à l'addition simple
    Ok(v.iter().fold(0.0, |s, x| s + x) / v.len() as f64)
}

/// Plus fortes vagues des axes assez fournis (au moins `SURE_WINDOWS` fenêtres) ; NaN sinon.
fn waves(f: &FlightStep) -> Result<f64, String> {
    let mut v = Vec::new();
    for a in axes(f) {
        if f.n[a] >= SURE_WINDOWS {
            v.push(metric(f, a)?.waves_ratio);
        }
    }
    let Some((&first, rest)) = v.split_first() else {
        return Ok(f64::NAN);
    };
    // `max` de Python : le premier, remplacé par un élément strictement plus grand (un NaN ne remplace rien et ne se fait pas remplacer)
    Ok(rest.iter().fold(first, |m, &x| if x > m { x } else { m }))
}

/// (signes de gains trop hauts, premiers signes) d'un cran de la série Master.
fn master_signs(rip: (f64, f64), prev_rip: f64, gain: f64, waves: f64) -> (Vec<String>, Vec<String>) {
    let mut strong = Vec::new();
    let mut early = Vec::new();
    if rip.0 >= RIPPLE_HIGH {
        strong.push(format!(
            "cassure {} (≥ {})",
            n_text(rip.0, 3, false),
            n_text(RIPPLE_HIGH, 3, false)
        ));
    } else if rip.0 >= RIPPLE_EARLY {
        early.push(format!("début de cassure ({})", n_text(rip.0, 3, false)));
    }
    if gain.is_finite() && gain < LATENCY_PLATEAU_PCT {
        let text = format!("latence presque plus améliorée ({} %)", n_text(gain, 1, false));
        if rip.0 >= RIPPLE_PLATEAU && rip.0 - prev_rip >= RIPPLE_GROWTH {
            strong.push(text);
        } else {
            early.push(text);
        }
    }
    if waves >= WAVES_RATIO_HIGH {
        strong.push(format!("vagues sur le palier (× {} le bruit)", n_text(waves, 1, false)));
    }
    (strong, early)
}

fn waves_text(w: f64) -> String {
    if w.is_finite() {
        format!("vagues × {}", n_text(w, 1, false))
    } else {
        format!("vagues non lues (moins de {SURE_WINDOWS} fenêtres)")
    }
}

pub fn master_block(flights: &[FlightStep], s: &Series) -> Result<TuningBlock, String> {
    let mut b = TuningBlock::new("Niveau des gains (série Master)".to_string(), Some(s.clone()));
    let (vals, idx) = pick(flights, s)?;
    let mut rip = Vec::new();
    let mut lat = Vec::new();
    let mut wav = Vec::new();
    for &i in &idx {
        let f = flight(flights, i)?;
        rip.push(ripple(f)?);
        lat.push(latency(f)?);
        wav.push(waves(f)?);
    }
    let mut gains = vec![f64::NAN];
    for k in 1..idx.len() {
        gains.push(if lat[k - 1] > 0.0 {
            100.0 * (1.0 - lat[k] / lat[k - 1])
        } else {
            f64::NAN
        });
    }
    let mut signs: Vec<(Vec<String>, Vec<String>)> = vec![(Vec::new(), Vec::new())];
    for k in 1..idx.len() {
        signs.push(master_signs(rip[k], rip[k - 1].0, gains[k], wav[k]));
    }
    for (k, (&v, &i)) in vals.iter().zip(&idx).enumerate() {
        let gain = if k > 0 {
            format!(", gain {} %", n_text(gains[k], 1, false))
        } else {
            String::new()
        };
        let unc = if rip[k].1.is_finite() {
            format!(" ± {}", n_text(rip[k].1, 3, false))
        } else {
            String::new()
        };
        let state = if !signs[k].0.is_empty() {
            "trop haut"
        } else if !signs[k].1.is_empty() {
            "premiers signes"
        } else {
            "propre"
        };
        b.lines.push(format!(
            "Master {} : latence {} ms{gain}, cassure {}{unc}, {} — {state} ({})",
            fmt(v),
            n_text(lat[k], 1, false),
            n_text(rip[k].0, 3, true),
            waves_text(wav[k]),
            flight(flights, i)?.label
        ));
    }
    let note = step_note(s);
    let stop = (1..idx.len()).find(|&k| !signs[k].0.is_empty());
    let Some(stop) = stop else {
        let last = idx.len().checked_sub(1).ok_or("IndexError : série Master sans vol")?;
        b.best = Some(idx[last]);
        let nxt = fmt(vals[last] + SERIES_STEP);
        if !signs[last].1.is_empty() {
            b.finding = Some(Finding::new(
                "master_near_optimum",
                "Réponse",
                CONSEIL,
                &format!("Master {} : proche de l'optimum", fmt(vals[last])),
                &format!("Premiers signes : {}.{note}", signs[last].1.join(", ")),
                vec![
                    format!(
                        "Vous pouvez garder Master {}, ou voler à {nxt} pour confirmer : si la cassure devient nette, revenez à la valeur d'avant.",
                        fmt(vals[last])
                    ),
                    "Surveillez aussi la chaleur des moteurs et le bruit (fenêtre Spectre).".to_string(),
                ],
                None,
            ));
        } else {
            b.finding = Some(Finding::new(
                "master_no_sign",
                "Réponse",
                CONSEIL,
                &format!("Master {} sans signe de gains trop hauts", fmt(vals[last])),
                &format!("Latence encore en baisse, sans cassure ni vagues.{note}"),
                vec![
                    format!("Volez avec Master {nxt}, puis comparez."),
                    "Surveillez aussi la chaleur des moteurs et le bruit (fenêtre Spectre).".to_string(),
                ],
                None,
            ));
        }
        return Ok(b);
    };
    let keep = stop - 1;
    b.best = Some(idx[keep]);
    let mut actions = vec![format!("Gardez Master {}.", fmt(vals[keep]))];
    let strong = rip[stop].0 >= RIPPLE_STRONG || wav[stop] >= WAVES_RATIO_HIGH;
    if !strong && vals[stop] - vals[keep] > 0.1 + 1e-9 {
        actions.push(format!(
            "Affinage possible : Master {}.",
            fmt((vals[keep] + vals[stop]) / 2.0)
        ));
    }
    if rip[stop].1.is_finite() && rip[stop].1 > 0.03 {
        actions.push(
            "Cassure mesurée avec une grande incertitude : un vol de plus à cette valeur la confirmerait.".to_string(),
        );
    }
    b.finding = Some(Finding::new(
        "master_choice",
        "Réponse",
        OK,
        &format!("Master conseillé : {}", fmt(vals[keep])),
        &format!("À Master {} : {}.{note}", fmt(vals[stop]), signs[stop].0.join(", ")),
        actions,
        None,
    ));
    Ok(b)
}

/// Latence pitch − roll (ms) ; NaN si un axe manque.
pub fn gap(f: &FlightStep) -> Result<f64, String> {
    if axes(f) != [0, 1] {
        return Ok(f64::NAN);
    }
    Ok(metric(f, 1)?.latency_ms - metric(f, 0)?.latency_ms)
}

fn gap_line(f: &FlightStep) -> Result<String, String> {
    let (roll, pitch) = (metric(f, 0)?, metric(f, 1)?);
    Ok(format!(
        "latence roll {} ms, pitch {} ms (écart {} ms), cassure pitch {}",
        n_text(roll.latency_ms, 1, false),
        n_text(pitch.latency_ms, 1, false),
        n_text(gap(f)?, 1, true),
        n_text(pitch.ripple, 3, true)
    ))
}

pub fn gap_action(g: f64) -> String {
    let sign = if g > 0.0 { "+" } else { "−" };
    format!(
        "Pitch Tracking (simplified_pitch_pi_gain) {sign}0,2 pour rapprocher les latences ; vérifiez qu'aucune cassure n'apparaît sur le pitch."
    )
}

/// Série Pitch T. (latences) si elle existe, sinon le vol `reference` (conseillé par la série Master, ou le vol le mieux mesuré).
pub fn roll_pitch_block(
    flights: &[FlightStep],
    s: Option<&Series>,
    reference: Option<usize>,
) -> Result<Option<TuningBlock>, String> {
    if let Some(s) = s {
        let name = slider_name(s.key)?;
        let mut b = TuningBlock::new(format!("Équilibre roll / pitch (série {name})"), Some(s.clone()));
        let (vals, idx) = pick(flights, s)?;
        for (&v, &i) in vals.iter().zip(&idx) {
            let f = flight(flights, i)?;
            b.lines
                .push(format!("{name} {} : {} ({})", fmt(v), gap_line(f)?, f.label));
        }
        let mut ok = Vec::new();
        for (k, &i) in idx.iter().enumerate() {
            let f = flight(flights, i)?;
            let r = metric(f, 1)?.ripple;
            // `not r >= RIPPLE_HIGH` : vrai aussi quand la cassure est NaN
            if gap(f)?.is_finite() && (r < RIPPLE_HIGH || r.is_nan()) {
                ok.push(k);
            }
        }
        if ok.is_empty() {
            b.finding = Some(Finding::new(
                "pitch_series_incomplete",
                "Réponse",
                INFO,
                "Série incomplète ou cassure sur le pitch",
                "Aucun vol avec les deux axes mesurés et sans cassure nette sur le pitch.",
                Vec::new(),
                None,
            ));
            return Ok(Some(b));
        }
        // `min(ok, key=…)` : le premier de l'écart le plus faible
        let mut k = ok[0];
        let mut kmin = gap(flight(flights, idx[k])?)?.abs();
        for &c in &ok[1..] {
            let v = gap(flight(flights, idx[c])?)?.abs();
            if v < kmin {
                k = c;
                kmin = v;
            }
        }
        b.best = Some(idx[k]);
        let g = gap(flight(flights, idx[k])?)?;
        let sev = if g.abs() <= LATENCY_GAP_OK_MS {
            OK
        } else if g.abs() <= LATENCY_GAP_MAX_MS {
            CONSEIL
        } else {
            ATTENTION
        };
        b.finding = Some(Finding::new(
            "series_choice",
            "Réponse",
            sev,
            &format!("{name} conseillé : {}", fmt(vals[k])),
            &format!(
                "Écart de latence pitch − roll le plus faible sans cassure nette : {} ms (visé : {} à {} ms au plus).{}",
                n_text(g, 1, true),
                n_text(LATENCY_GAP_OK_MS, 0, false),
                n_text(LATENCY_GAP_MAX_MS, 0, false),
                step_note(s)
            ),
            if sev == OK { Vec::new() } else { vec![gap_action(g)] },
            None,
        ));
        return Ok(Some(b));
    }
    let Some(r) = reference else {
        return Ok(None);
    };
    let f = flight(flights, r)?;
    let g = gap(f)?;
    if !g.is_finite() {
        return Ok(None);
    }
    let mut b = TuningBlock::new("Équilibre roll / pitch".to_string(), None);
    b.lines.push(format!("{} ({})", gap_line(f)?, f.label));
    b.best = Some(r);
    let ok_ms = n_text(LATENCY_GAP_OK_MS, 0, false);
    let max_ms = n_text(LATENCY_GAP_MAX_MS, 0, false);
    b.finding = Some(if g.abs() <= LATENCY_GAP_OK_MS {
        Finding::new(
            "pitch_balance_ok",
            "Réponse",
            OK,
            "Roll et pitch équilibrés",
            &format!("Écart de latence {} ms (≤ {ok_ms} ms).", n_text(g, 1, true)),
            Vec::new(),
            None,
        )
    } else if g.abs() <= LATENCY_GAP_MAX_MS {
        Finding::new(
            "pitch_balance_near",
            "Réponse",
            INFO,
            "Roll et pitch presque équilibrés",
            &format!(
                "Écart de latence {} ms : acceptable (1 à 2 ms selon le drone).",
                n_text(g, 1, true)
            ),
            vec![gap_action(g) + " (facultatif)"],
            None,
        )
    } else {
        Finding::new(
            "pitch_balance_slower",
            "Réponse",
            CONSEIL,
            &format!("Le {} répond plus lentement", if g > 0.0 { "pitch" } else { "roll" }),
            &format!("Écart de latence {} ms (> {max_ms} ms).", n_text(g, 1, true)),
            vec![gap_action(g)],
            None,
        )
    });
    Ok(Some(b))
}

/// Série Feedforward (Acro) reconnue : la réponse indicielle, faussée par le feedforward, ne la juge pas ; les retards consigne → gyro
/// se lisent dans l'Assistant (`rules::setpoint_delay`) et sous les logs du guide.
pub fn ff_block(flights: &[FlightStep], s: &Series) -> Result<TuningBlock, String> {
    let (vals, idx) = pick(flights, s)?;
    let mut b = TuningBlock::new("Feedforward (série Réponse des sticks)".to_string(), Some(s.clone()));
    for (&v, &i) in vals.iter().zip(&idx) {
        b.lines
            .push(format!("Feedforward {} : {}", fmt(v), flight(flights, i)?.label));
    }
    b.finding = Some(Finding::new(
        "ff_series_info",
        "Réponse",
        INFO,
        "Série Feedforward : lisez les retards dans l'Assistant",
        "Le feedforward fausse la réponse indicielle : elle ne sert pas à choisir sa valeur. L'Assistant donne, pour chaque vol, le retard consigne → gyro en roll et en pitch.",
        Vec::new(),
        None,
    ));
    Ok(b)
}

/// Diagnostic du réglage d'une liste de vols (conditions du protocole, un bloc par série, équilibre roll / pitch).
pub fn step_tuning(flights: &[FlightStep]) -> Result<TuningReport, String> {
    let series = find_series(flights)?;
    let ff_series: BTreeSet<usize> = series
        .iter()
        .filter(|s| s.key == FF_KEY)
        .flat_map(|s| s.flights.iter().copied())
        .collect();
    let mut rep = TuningReport {
        conditions: conditions(flights, &ff_series)?,
        blocks: Vec::new(),
    };
    let mut master_best: Option<usize> = None;
    for s in &series {
        if s.key == FF_KEY {
            rep.blocks.push(ff_block(flights, s)?);
        } else if s.key == "Damping" || s.key == "Pitch D." {
            rep.blocks.push(damping_block(flights, s)?);
        } else if s.key == "Master" {
            let b = master_block(flights, s)?;
            master_best = b.best;
            rep.blocks.push(b);
        }
    }
    let pitch: Vec<&Series> = series.iter().filter(|s| s.key == "Pitch T.").collect();
    for s in &pitch {
        if let Some(b) = roll_pitch_block(flights, Some(s), None)? {
            rep.blocks.push(b);
        }
    }
    if pitch.is_empty() {
        let reference = match master_best {
            Some(m) => Some(m),
            None => {
                // `max(…, key=sum(n), default=None)` : le premier vol de somme maximale
                let mut best: Option<(usize, usize)> = None;
                for (i, f) in flights.iter().enumerate() {
                    if axes(f) == [0, 1] {
                        let k = sum_n(f);
                        if best.is_none_or(|(_, bk)| k > bk) {
                            best = Some((i, k));
                        }
                    }
                }
                best.map(|(i, _)| i)
            }
        };
        if let Some(b) = roll_pitch_block(flights, None, reference)? {
            rep.blocks.push(b);
        }
    }
    Ok(rep)
}
