//! Simulateur de filtres de la page Filtres (lot 22e du plan, D-058) : la chaîne de filtres du firmware **rejouée** sur le
//! gyro non filtré du log, avec les règles vérifiées étage par étage au lot 22 (`filter_stages`), et les spectres « avant /
//! après » qui en sortent.
//!
//! Rejeu ([`replay`]) :
//! - filtres calculés pour la cadence **nominale** de l'en-tête, comme le firmware (L-101) ;
//! - ordre et cadence des filtres lus dans la description de la chaîne (`chain::betaflight`, lot 23) ; chaque filtre rejoué
//!   à la cadence où le firmware l'applique : LPF2 à la cadence gyro, notchs RPM, notchs statiques, LPF1 et notch
//!   dynamique à la cadence PID, même quand le log n'enregistre qu'une boucle sur N (L-104) ;
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
    gyro_stage_own_rate, pre_boost_throttle, simulate_dterm_at, stages_before,
};
use crate::chain::{self, Brick, Signal};
use crate::dyn_notch::{DynNotchTracker, PREROLL_S, ticks_per_log_sample};
use crate::filter_stages::{model_rate_hz, tpa_d_factor};
use crate::firmware_chain::{self, Engine, every, pid_loops_per_log_sample};
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
    /// Chaîne qui a rejoué.
    pub engine: Engine,
    /// `Engine::Firmware` : gyro rejoué à la cadence PID (le terme D en part), sinon vide.
    pub gyro_pid: Vec<Vec<f32>>,
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
    replay_with(log, s, Engine::Model)
}

/// [`replay`] par la chaîne `engine` : l'actuelle, ou celle à l'arithmétique du firmware (`firmware_chain`, lot 24c).
pub fn replay_with(log: &FlightLog, s: &Settings, engine: Engine) -> Result<Option<Replay>, String> {
    if engine == Engine::Firmware {
        return replay_firmware(log, s);
    }
    let Some(gyro) = replay_gyro(log, s)? else {
        return Ok(None);
    };
    let dterm = replay_dterm(log, s, &gyro)?;
    Ok(Some(Replay {
        gyro,
        dterm,
        engine,
        gyro_pid: Vec::new(),
    }))
}

/// Rejeu par la chaîne à l'arithmétique du firmware : le bloc d'avant le notch dynamique d'une traite (cadence gyro puis
/// PID), les centres du notch dynamique suivis sur sa sortie ramenée à la cadence du log ([`dyn_centres`], comme la chaîne
/// actuelle), le notch dynamique et le terme D à la cadence PID, chaque signal ramené à la cadence du log à la fin.
fn replay_firmware(log: &FlightLog, s: &Settings) -> Result<Option<Replay>, String> {
    let desc = chain::betaflight();
    let Some(gu) = desc.recorded_input(log) else {
        return Ok(None);
    };
    let r = pid_loops_per_log_sample(s)?;
    let rows: Vec<Vec<f64>> = (0..gu.k.min(3)).map(|a| wide(gu.row(a))).collect();
    let block = desc.gyro_between(Signal::GyroUnfilt, Signal::FftFreqBeforeDynNotch)?;
    let pre = firmware_chain::gyro_steps(log, s, block, &rows, 1, None)?;
    let centres = if dyn_notch_enabled(s) {
        let pre_log: Vec<Vec<f64>> = pre.iter().map(|p| every(p, r)).collect();
        dyn_centres(log, s, &pre_log)
    } else {
        vec![Vec::new(); pre.len()]
    };
    let pre_wide: Vec<Vec<f64>> = pre.iter().map(|p| p.iter().map(|&v| v as f64).collect()).collect();
    let rest = desc.gyro_between(Signal::FftFreqBeforeDynNotch, Signal::Gyro)?;
    let gyro_pid = firmware_chain::gyro_steps(log, s, rest, &pre_wide, r, Some(&centres))?;
    let gyro: Vec<Vec<f64>> = gyro_pid.iter().map(|g| every(g, r)).collect();
    let dterm = replay_dterm_firmware(log, s, &gyro_pid)?;
    Ok(Some(Replay {
        gyro,
        dterm,
        engine: Engine::Firmware,
        gyro_pid,
    }))
}

/// Terme D de la chaîne à l'arithmétique du firmware, depuis le gyro rejoué à la cadence PID (× facteur TPA).
fn replay_dterm_firmware(log: &FlightLog, s: &Settings, gyro_pid: &[Vec<f32>]) -> Result<Matrix64, String> {
    let r = pid_loops_per_log_sample(s)?;
    let n = log.n();
    let d = firmware_chain::dterm(log, s, gyro_pid, r)?;
    let mut data = Vec::with_capacity(d.len() * n);
    for row in &d {
        let mut v = every(row, r);
        v.resize(n, 0.0);
        data.extend(v);
    }
    let mut dterm = Matrix64 { k: d.len(), n, data };
    apply_tpa(log, s, &mut dterm);
    Ok(dterm)
}

/// Le D filtré multiplié par le facteur TPA, comme le firmware (`flight/pid.c`).
fn apply_tpa(log: &FlightLog, s: &Settings, dterm: &mut Matrix64) {
    let n = dterm.n;
    if let Some(tpa) = tpa_d_factor(s, &pre_boost_throttle(s, &log.throttle, model_rate_hz(s))) {
        for row in dterm.data.chunks_mut(n) {
            row.iter_mut().zip(&tpa).for_each(|(v, f)| *v *= f);
        }
    }
}

/// Vrai si `s` et `base` ne diffèrent que par les filtres du terme D (passe-bas et notch du D), qui ne touchent pas au gyro.
fn same_gyro_chain(s: &Settings, base: &Settings) -> bool {
    let mut t = s.clone();
    t.dterm_lpf1 = base.dterm_lpf1.clone();
    t.dterm_lpf2 = base.dterm_lpf2.clone();
    t.dterm_notch = base.dterm_notch;
    t == *base
}

/// `replay` de `s`, en reprenant `cur` (rejeu de `base` sur le même log) : tel quel si rien ne change, son gyro si seuls les
/// filtres du D changent (le gyro rejoué coûte l'essentiel du calcul).
pub fn replay_from(log: &FlightLog, s: &Settings, base: &Settings, cur: &Replay) -> Result<Option<Replay>, String> {
    if s == base {
        return Ok(Some(cur.clone()));
    }
    if same_gyro_chain(s, base) {
        let dterm = match cur.engine {
            Engine::Model => replay_dterm(log, s, &cur.gyro)?,
            Engine::Firmware => replay_dterm_firmware(log, s, &cur.gyro_pid)?,
        };
        return Ok(Some(Replay { dterm, ..cur.clone() }));
    }
    replay_with(log, s, cur.engine)
}

/// Gyro filtré rejoué (une ligne par axe) ; `None` sans gyro non filtré.
fn replay_gyro(log: &FlightLog, s: &Settings) -> Result<Option<Vec<Vec<f64>>>, String> {
    let desc = chain::betaflight();
    let Some(gu) = desc.recorded_input(log) else {
        return Ok(None);
    };
    let fs = model_rate_hz(s);
    // étages d'avant le notch dynamique, chacun à la cadence de ses étapes dans la description, même si le log ne garde
    // qu'une boucle PID sur N (L-104)
    let stages = stages_before(Signal::FftFreqBeforeDynNotch)?;
    let mut pre = Vec::with_capacity(3);
    for a in 0..gu.k.min(3) {
        let mut y = wide(gu.row(a));
        for &st in &stages {
            y = gyro_stage_own_rate(st, log, s, y, fs)?;
        }
        pre.push(y);
    }
    // notch dynamique : aux centres du suivi rejoué, à la cadence de son étage
    let dyn_steps = GyroStage::DynNotch.steps()?;
    if desc.gyro[dyn_steps.clone()]
        .iter()
        .any(|st| st.brick != Brick::DynNotch)
    {
        return Err(format!(
            "{} : l'étage du notch dynamique contient un autre filtre",
            desc.firmware
        ));
    }
    let pid_loops = GyroStage::DynNotch.loops_per_sample(s, fs)?;
    let centres = if dyn_notch_enabled(s) {
        dyn_centres(log, s, &pre)
    } else {
        vec![Vec::new(); pre.len()]
    };
    let mut gyro = Vec::with_capacity(pre.len());
    for (y, c) in pre.into_iter().zip(&centres) {
        gyro.push(dyn_notch_with_centres(s, y, c, fs, pid_loops)?);
    }
    Ok(Some(gyro))
}

/// Terme D rejoué à partir du gyro rejoué `gyro` (× facteur TPA).
fn replay_dterm(log: &FlightLog, s: &Settings, gyro: &[Vec<f64>]) -> Result<Matrix64, String> {
    let fs = model_rate_hz(s);
    let n = log.n();
    let m = Matrix64 {
        k: gyro.len(),
        n,
        data: gyro.concat(),
    };
    let mut dterm = simulate_dterm_at(log, Some(s), Some(&m), fs)?;
    apply_tpa(log, s, &mut dterm);
    Ok(dterm)
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
    before_after_with(log, sel, base, modf, seg_ms, Engine::Model)
}

/// [`before_after`] avec la chaîne `engine` ([`replay_with`]).
pub fn before_after_with(
    log: &FlightLog,
    sel: Range<usize>,
    base: &Settings,
    modf: &Settings,
    seg_ms: f64,
    engine: Engine,
) -> Result<Option<BeforeAfter>, String> {
    let Some(cur) = replay_with(log, base, engine)? else {
        return Ok(None);
    };
    before_after_from(log, sel, base, &cur, modf, seg_ms)
}

/// [`before_after`] avec `cur`, rejeu de `base` déjà calculé sur ce log ([`replay`]) : l'assistant essaie plusieurs réglages
/// modifiés sur le même vol sans refaire le rejeu actuel.
pub fn before_after_from(
    log: &FlightLog,
    sel: Range<usize>,
    base: &Settings,
    cur: &Replay,
    modf: &Settings,
    seg_ms: f64,
) -> Result<Option<BeforeAfter>, String> {
    let Some(gu) = &log.gyro_unfilt else {
        return Ok(None);
    };
    let sel = clamp_range(sel.start, sel.end, log.n());
    let changed = modf != base;
    let new = if changed {
        replay_from(log, modf, base, cur)?
    } else {
        None
    };
    let new = new.as_ref().unwrap_or(cur);
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

/// Écart entre les deux chaînes sur un log (lot 24c) : spectres (dB, moyenne des fenêtres) des rejeux **exacts** de la
/// chaîne actuelle et de celle à l'arithmétique du firmware, réglages du log, et écart dans le temps.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainGap {
    pub freqs: Vec<f64>,
    /// Par axe : (chaîne actuelle, chaîne du firmware).
    pub gyro_db: Vec<(Vec<f64>, Vec<f64>)>,
    /// Par axe, `None` sans terme D enregistré.
    pub dterm_db: Vec<Option<(Vec<f64>, Vec<f64>)>>,
    /// Par axe : valeur efficace de (firmware − actuelle) rapportée à celle de l'actuelle, gyro, sur la période.
    pub gyro_rel_rms: Vec<f64>,
}

/// `chain_gap` : [`ChainGap`] de la période `sel` (fenêtres de `seg_ms`). `None` sans gyro non filtré, si la période ne
/// contient aucune fenêtre, ou si la chaîne du firmware refuse le log (version d'avant la 4.3 : `Err`).
pub fn chain_gap(log: &FlightLog, sel: Range<usize>, seg_ms: f64) -> Result<Option<ChainGap>, String> {
    let s = &log.settings;
    let (Some(old), Some(new)) = (
        replay_with(log, s, Engine::Model)?,
        replay_with(log, s, Engine::Firmware)?,
    ) else {
        return Ok(None);
    };
    let sel = clamp_range(sel.start, sel.end, log.n());
    let fs = log.fs;
    let l = 32.max(py_round(seg_ms * fs / 1000.0)?) as usize;
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let w = |x: &[f64]| windows(x, &sel, l, &br, fs);
    let mut freqs = Vec::new();
    let mut gyro_db = Vec::new();
    let mut gyro_rel_rms = Vec::new();
    for (a, b) in old.gyro.iter().zip(&new.gyro) {
        let (Some((f, pa)), Some((_, pb))) = (w(a)?, w(b)?) else {
            return Ok(None);
        };
        let nf = f.len();
        freqs = f;
        gyro_db.push((mean_db(&pa, nf), mean_db(&pb, nf)));
        let (mut e, mut r) = (0.0, 0.0);
        for i in sel.clone() {
            e += (b[i] - a[i]).powi(2);
            r += a[i] * a[i];
        }
        gyro_rel_rms.push(if r > 0.0 { (e / r).sqrt() } else { f64::NAN });
    }
    let nf = freqs.len();
    let mut dterm_db = Vec::new();
    for ax in 0..old.gyro.len() {
        if !log.d_term.row(ax).iter().any(|&v| v != 0.0) {
            dterm_db.push(None);
            continue;
        }
        match (w(old.dterm.row(ax))?, w(new.dterm.row(ax))?) {
            (Some((_, pa)), Some((_, pb))) => dterm_db.push(Some((mean_db(&pa, nf), mean_db(&pb, nf)))),
            _ => dterm_db.push(None),
        }
    }
    Ok(Some(ChainGap {
        freqs,
        gyro_db,
        dterm_db,
        gyro_rel_rms,
    }))
}
