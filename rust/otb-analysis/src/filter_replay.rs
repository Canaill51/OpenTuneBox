//! Simulateur de filtres de la page Filtres (lot 22e du plan, D-058) : la chaîne de filtres du firmware **rejouée** sur le
//! gyro non filtré du log, avec les règles vérifiées étage par étage au lot 22 (`filter_stages`), et les spectres « avant /
//! après » qui en sortent.
//!
//! Rejeu ([`replay`]) :
//! - filtres calculés pour la cadence **nominale** de l'en-tête, comme le firmware (L-101) ;
//! - chaque filtre rejoué à la cadence où le firmware l'applique : LPF2 à la cadence gyro, notchs RPM, notchs statiques,
//!   LPF1 et notch dynamique à la cadence PID, même quand le log n'enregistre qu'une boucle sur N (L-104) ;
//! - centres du notch dynamique donnés par le suivi des pics du firmware rejoué sur chaque axe (`dyn_notch`, lot 22c), avec
//!   les réglages simulés : changer le nombre, le Q ou la plage déplace les notchs comme en vol (W-057) ;
//! - terme D : −dg/dt du gyro rejoué dans la chaîne D, × facteur TPA de chaque instant (gaz d'avant le throttle boost).
//!
//! Spectres ([`before_after`]), fenêtre par fenêtre, le rejeu arrondi comme le Blackbox arrondit le log (gyro au °/s, au
//! 1/10 en haute résolution ; terme D à l'unité) pour la courbe « actuel » : sans cela, là où le gyro filtré est faible, le
//! bruit d'arrondi du log domine et le rejeu paraît filtrer jusqu'à 1,7 dB de plus (L-103). ⚠️ Le rapport modifié / actuel,
//! lui, se calcule sur les rejeux **exacts** : arrondis, chacun porte son propre bruit d'arrondi, et la moyenne d'un rapport
//! de deux spectres bruités est biaisée vers le haut (tout changement, même un LPF plus bas, « ajoutait » du bruit) :
//! - « actuel » : le rejeu avec les réglages du log, tel quel. Il n'est **pas** exact par construction : son écart au
//!   spectre enregistré montre ce que vaut le modèle sur ce log ;
//! - « modifié » : spectre enregistré × (rejeu modifié / rejeu actuel). Réglages inchangés : exactement le log (D-008) ;
//!   l'erreur du modèle s'annule dans le rapport pour tout ce qui ne change pas.

use std::ops::Range;

use crate::bf_filters::{
    GyroStage, Matrix64, dyn_notch_enabled, dyn_notch_initial_centres, dyn_notch_with_centres, fft_freq_centres,
    gyro_stage_upsampled, lpf2_at_gyro_rate, pre_boost_throttle, simulate_dterm_at,
};
use crate::dyn_notch::{DynNotchTracker, PREROLL_S, ticks_per_log_sample};
use crate::filter_stages::{model_rate_hz, tpa_d_factor};
use crate::spectral::{Mode, py_round, seg_spectra, segments, to_db};
use otb_log::flightlog::{FlightLog, clamp_range, slow_gain};
use otb_log::settings::Settings;

/// Plafond du rapport modifié / actuel d'une case (×1000, +30 dB), comme le spectre « et si » de D-008.
const MAX_RATIO: f64 = 1e3;
/// Sous cette part du signal d'avant filtres (−20 dB), les filtres actuels « écrasent » la case : si les filtres
/// modifiés filtrent moins, le rejeu modifié lui-même sert de plancher (le spectre enregistré n'y a plus d'information).
const CRUSHED: f64 = 1e-2;

/// Chaîne de filtres rejouée sur tout le log.
#[derive(Debug, Clone)]
pub struct Replay {
    /// Gyro filtré rejoué, une ligne par axe.
    pub gyro: Vec<Vec<f64>>,
    /// Terme D rejoué (°/s² × facteur TPA ; le gain Kd n'y est pas), une ligne par axe.
    pub dterm: Matrix64,
}

fn wide(r: &[f32]) -> Vec<f64> {
    r.iter().map(|&v| v as f64).collect()
}

/// Centres du notch dynamique de chaque axe (`[axe][notch][échantillon]`) : suivi rejoué sur `pre` (gyro d'avant le notch).
/// Firmware sans suivi rejouable (avant la 4.3) : les centres du debug `FFT_FREQ` si les réglages du notch sont ceux du log,
/// sinon les centres initiaux, fixes.
fn dyn_centres(log: &FlightLog, s: &Settings, pre: &[Vec<f64>]) -> Vec<Vec<Vec<f64>>> {
    let n = log.n();
    if let Some(t) = DynNotchTracker::new(s) {
        let input = [0, 1, 2].map(|a| pre.get(a).map(Vec::as_slice));
        let preroll = (PREROLL_S * s.log_rate_hz()).round() as usize;
        let tracked = t.track(input, ticks_per_log_sample(s), preroll);
        return (0..pre.len())
            .map(|a| (0..tracked.count).map(|p| wide(tracked.row(a, p))).collect())
            .collect();
    }
    let ls = &log.settings;
    let same = (s.dyn_notch_count, s.dyn_notch_min_hz, s.dyn_notch_max_hz)
        == (ls.dyn_notch_count, ls.dyn_notch_min_hz, ls.dyn_notch_max_hz);
    let recorded = fft_freq_centres(log, Some(s)).filter(|_| same);
    let rows: Vec<Vec<f64>> = match recorded {
        Some(m) => (0..m.k).map(|p| m.row(p).to_vec()).collect(),
        None => dyn_notch_initial_centres(s).into_iter().map(|c| vec![c; n]).collect(),
    };
    vec![rows; pre.len()]
}

/// `replay` : chaîne de filtres de `s` rejouée sur le gyro non filtré de tout le log (voir l'en-tête du module).
/// `None` sans gyro non filtré.
pub fn replay(log: &FlightLog, s: &Settings) -> Result<Option<Replay>, String> {
    let Some(gu) = &log.gyro_unfilt else {
        return Ok(None);
    };
    let fs = model_rate_hz(s);
    // filtres de la boucle PID : à sa cadence, même si le log n'en garde qu'une sur `pid_loops` (L-104)
    let pid_loops = (s.pid_rate_hz() / fs).round().max(1.0) as usize;
    let mut pre = Vec::with_capacity(3);
    for a in 0..gu.k.min(3) {
        let mut y = lpf2_at_gyro_rate(log, s, wide(gu.row(a)), fs)?;
        for st in [GyroStage::Rpm, GyroStage::StaticLpf1] {
            y = gyro_stage_upsampled(st, log, s, y, fs, pid_loops)?;
        }
        pre.push(y);
    }
    let centres = if dyn_notch_enabled(s) {
        dyn_centres(log, s, &pre)
    } else {
        vec![Vec::new(); pre.len()]
    };
    let mut gyro = Vec::with_capacity(pre.len());
    for (y, c) in pre.into_iter().zip(&centres) {
        gyro.push(dyn_notch_with_centres(s, y, c, fs, pid_loops)?);
    }
    let n = log.n();
    let m = Matrix64 {
        k: gyro.len(),
        n,
        data: gyro.concat(),
    };
    let mut dterm = simulate_dterm_at(log, Some(s), Some(&m), fs)?;
    // le firmware multiplie le D filtré par le facteur TPA (`flight/pid.c`)
    if let Some(tpa) = tpa_d_factor(s, &pre_boost_throttle(s, &log.throttle, fs)) {
        for row in dterm.data.chunks_mut(n) {
            row.iter_mut().zip(&tpa).for_each(|(v, f)| *v *= f);
        }
    }
    Ok(Some(Replay { gyro, dterm }))
}

/// Spectres d'un signal (dB, moyenne des fenêtres) : enregistré, rejoué avec les réglages actuels, estimé avec les réglages
/// modifiés.
#[derive(Debug, Clone, PartialEq)]
pub struct BeforeAfterSpectra {
    pub logged_db: Vec<f64>,
    pub current_db: Vec<f64>,
    pub modified_db: Vec<f64>,
}

/// Spectres de la page Filtres, par axe : gyro filtré et terme D (`None` : terme D nul sur cet axe).
#[derive(Debug, Clone, PartialEq)]
pub struct BeforeAfter {
    pub freqs: Vec<f64>,
    pub gyro: Vec<BeforeAfterSpectra>,
    pub dterm: Vec<Option<BeforeAfterSpectra>>,
}

/// `x` arrondi au pas `step` du Blackbox.
fn rounded(x: &[f64], step: f64) -> Vec<f64> {
    x.iter().map(|v| (v / step).round() * step).collect()
}

/// Fréquences et spectres par fenêtre (`nseg × nf`, rangés fenêtre après fenêtre).
type Windows = (Vec<f64>, Vec<f64>);

/// Spectres par fenêtre (`nseg × nf`) d'une période d'un signal ; `None` si aucune fenêtre ne tient.
fn windows(x: &[f64], sel: &Range<usize>, l: usize, br: &[i64], fs: f64) -> Result<Option<Windows>, String> {
    let segs = segments(&x[sel.clone()], l, l / 2, Some(br));
    if segs.nseg == 0 {
        return Ok(None);
    }
    seg_spectra(&segs, fs, Mode::Psd).map(Some)
}

/// Moyenne des fenêtres (`nseg × nf`) en dB.
fn mean_db(p: &[f64], nf: usize) -> Vec<f64> {
    let nseg = p.len() / nf.max(1);
    let mut v = vec![0.0; nf];
    for row in p.chunks(nf) {
        v.iter_mut().zip(row).for_each(|(a, b)| *a += b);
    }
    to_db(&v.iter().map(|a| a / nseg as f64).collect::<Vec<_>>())
}

/// Spectre estimé avec les réglages modifiés, case par case et fenêtre par fenêtre : enregistré × (modifié / actuel),
/// plafonné à [`MAX_RATIO`] ; là où les filtres actuels écrasent le signal d'avant filtres (`before`), le rejeu modifié
/// sert de plancher quand il filtre moins.
fn estimate(logged: &[f64], cur: &[f64], modf: &[f64], before: Option<&[f64]>) -> Vec<f64> {
    (0..logged.len())
        .map(|i| {
            let ratio = if cur[i] > 0.0 {
                (modf[i] / cur[i]).min(MAX_RATIO)
            } else {
                1.0
            };
            let mut est = logged[i] * ratio;
            if let Some(b) = before
                && cur[i] < CRUSHED * b[i]
                && modf[i] > cur[i]
            {
                est = est.max(modf[i]);
            }
            est
        })
        .collect()
}

/// `before_after` : spectres gyro et D de la période `sel` (fenêtres de `seg_ms`), réglages `base` (ceux du log) et
/// `modf` (voir l'en-tête du module). `None` sans gyro non filtré ou si la période ne contient aucune fenêtre.
pub fn before_after(
    log: &FlightLog,
    sel: Range<usize>,
    base: &Settings,
    modf: &Settings,
    seg_ms: f64,
) -> Result<Option<BeforeAfter>, String> {
    let Some(gu) = &log.gyro_unfilt else {
        return Ok(None);
    };
    let sel = clamp_range(sel.start, sel.end, log.n());
    let Some(cur) = replay(log, base)? else {
        return Ok(None);
    };
    let changed = modf != base;
    let new = if changed { replay(log, modf)? } else { None };
    let new = new.as_ref().unwrap_or(&cur);
    let fs = log.fs;
    let l = 32.max(py_round(seg_ms * fs / 1000.0)?) as usize;
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let gyro_step = if base.high_resolution { 0.1 } else { 1.0 };
    let mut freqs = Vec::new();
    let mut gyro = Vec::new();
    for a in 0..cur.gyro.len() {
        let w = |x: &[f64]| windows(x, &sel, l, &br, fs);
        // rapport tiré des rejeux exacts : arrondis, leur bruit d'arrondi (indépendant dans chacun) gonfle en moyenne le
        // rapport de deux spectres, et tout changement semblait ajouter du bruit (+0,2 à +0,4 dB entre 20 et 100 Hz)
        let (Some((f, p_log)), Some((_, p_shown)), Some((_, p_cur)), Some((_, p_mod)), Some((_, p_unf))) = (
            w(&wide(log.gyro.row(a)))?,
            w(&rounded(&cur.gyro[a], gyro_step))?,
            w(&cur.gyro[a])?,
            w(&new.gyro[a])?,
            w(&wide(gu.row(a)))?,
        ) else {
            return Ok(None);
        };
        let nf = f.len();
        freqs = f;
        gyro.push(BeforeAfterSpectra {
            logged_db: mean_db(&p_log, nf),
            current_db: mean_db(&p_shown, nf),
            modified_db: mean_db(&estimate(&p_log, &p_cur, &p_mod, Some(&p_unf)), nf),
        });
    }
    let nf = freqs.len();
    let mut dterm = Vec::new();
    for a in 0..cur.gyro.len() {
        let d = log.d_term.row(a);
        if !d.iter().any(|&v| v != 0.0) {
            dterm.push(None);
            continue;
        }
        // gain Kd du D enregistré, ajusté sur les basses fréquences : le même pour l'actuel et le modifié
        let sim_cur: Vec<f32> = cur.dterm.row(a).iter().map(|&v| v as f32).collect();
        let k = slow_gain(&sim_cur, d, fs)?;
        let scaled = |m: &Matrix64| m.row(a).iter().map(|v| v * k).collect::<Vec<f64>>();
        let w = |x: &[f64]| windows(x, &sel, l, &br, fs);
        let (Some((_, p_log)), Some((_, p_shown)), Some((_, p_cur)), Some((_, p_mod))) = (
            w(&wide(d))?,
            w(&rounded(&scaled(&cur.dterm), 1.0))?,
            w(&scaled(&cur.dterm))?,
            w(&scaled(&new.dterm))?,
        ) else {
            dterm.push(None);
            continue;
        };
        dterm.push(Some(BeforeAfterSpectra {
            logged_db: mean_db(&p_log, nf),
            current_db: mean_db(&p_shown, nf),
            modified_db: mean_db(&estimate(&p_log, &p_cur, &p_mod, None), nf),
        }));
    }
    Ok(Some(BeforeAfter { freqs, gyro, dterm }))
}
