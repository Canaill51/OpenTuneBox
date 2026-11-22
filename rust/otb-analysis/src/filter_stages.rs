//! Contrôle du modèle de filtres **étage par étage** contre le firmware (lot 22a du plan).
//!
//! Pour chaque étage dont le log enregistre l'entrée et la sortie, le modèle d'OpenTuneBox (les étages de
//! [`crate::bf_filters::simulate_gyro`] et de [`crate::bf_filters::simulate_dterm`]) filtre l'entrée **enregistrée**, et l'on
//! compare son résultat à la sortie **enregistrée**. Les erreurs ne s'additionnent pas d'un étage à l'autre : un écart
//! désigne un filtre précis. Points de mesure du firmware (`sensors/gyro_filter_impl.c`, `flight/pid.c`) :
//!
//! | Étage | Entrée → sortie | Mode debug |
//! |---|---|---|
//! | horloge | `gyroUnfilt` → `GYRO_SAMPLE[0]` (modèle : rien, seulement le décalage de L-100) | `GYRO_SAMPLE` |
//! | LPF2 | `GYRO_SAMPLE[0]` → `[1]` | `GYRO_SAMPLE` |
//! | notchs RPM | `GYRO_SAMPLE[1]` → `[2]` | `GYRO_SAMPLE` |
//! | notchs statiques + LPF1 | `GYRO_SAMPLE[2]` → `[3]` | `GYRO_SAMPLE` |
//! | LPF2 + RPM + statiques + LPF1 | `gyroUnfilt` → gyro avant le notch dynamique | `FFT_FREQ` |
//! | notch dynamique | gyro avant le notch dynamique → gyro, centres enregistrés | `FFT_FREQ` |
//! | filtres D | −dg/dt du gyro enregistré → `axisD`, gain ajusté | aucun (log à chaque boucle PID, D sans D max) |
//!
//! Mesure : fonctions de transfert de Welch (fenêtres de Hann d'environ 0,5 s, recouvrement de moitié, aucune sur un trou
//! du log), H = S(x, y) / S(x, x) pour le firmware et pour le modèle, sur la même entrée. ⚠️ Pour le firmware, l'arrondi
//! de l'entrée enregistrée (au °/s pour les canaux debug) est retiré de S(x, x) : il n'est pas dans le signal que le
//! firmware a filtré, et le garder fait croire à un firmware qui filtre plus (L-103, [`InputNoise`]). Par bande : gain
//! moyen en dB (pondéré par la cohérence firmware), écart, retard tiré de la phase, cohérence moyenne et résidu (énergie de
//! sortie − modèle rapportée à celle de la sortie, mesurée dans les fenêtres). ⚠️ Le firmware écrit `gyroUnfilt` un tick
//! gyro après avoir filtré l'échantillon précédent : tout étage qui part de `gyroUnfilt` retire ce décalage (L-100).
//!
//! Le modèle suit le firmware sur quatre points que le simulateur affiché ignore encore (lots 22a bis et 22b) :
//! - ⚠️ ses filtres sont calculés pour la cadence **nominale** de l'en-tête ([`model_rate_hz`]), pas pour celle mesurée sur
//!   le log : un contrôleur surchargé dont la boucle tourne deux fois moins vite que prévu filtre deux fois plus bas en
//!   hertz réels, et les fréquences des bandes sont comptées comme le firmware les compte (L-101) ;
//! - le terme D est multiplié par le facteur TPA de chaque instant ([`tpa_d_factor`]), tiré des gaz d'avant le throttle
//!   boost ([`pre_boost_throttle`]) ;
//! - les instants où le firmware force le D à zéro (PID coupé, récupération de vrille en lacet : `flight/pid.c`) sont
//!   écartés de l'étage D comme des trous du log ;
//! - ⚠️ le LPF2 gyro est calculé pour la cadence des échantillons gyro et appliqué à chacun d'eux
//!   ([`lpf2_at_gyro_rate`]), pas à la cadence du log (L-104).
//!
//! Seuls les logs enregistrés à [`MIN_LOG_RATE_HZ`] ou plus sont contrôlés (D-057).

use crate::bf_filters::{
    GyroStage, dyn_notch_enabled, gyro_stage_at, lpf2_at_gyro_rate, pre_boost_throttle, simulate_dterm_at,
};
use crate::spectral::{rfftfreq, row_mean, segments};
use otb_dsp::numeric::{hann, rfft_rows};
use otb_log::flightlog::{FlightLog, GyroDebugTaps, clamp_range};
use otb_log::settings::Settings;
use std::f64::consts::PI;
use std::ops::Range;

/// Cadence minimale du log pour juger un filtre (choix de Cédric, D-057) : en dessous, le signal gyro est trop dégradé.
pub const MIN_LOG_RATE_HZ: f64 = 1600.0;
/// Cohérence moyenne sous laquelle une bande est « non concluante » (l'arrondi du log au °/s y domine).
pub const MIN_COHERENCE: f64 = 0.5;
/// Bandes de fréquences rapportées (Hz) ; la dernière s'arrête à la moitié de la cadence du log.
pub const BANDS_HZ: [(f64, f64); 5] = [
    (5.0, 50.0),
    (50.0, 100.0),
    (100.0, 250.0),
    (250.0, 500.0),
    (500.0, f64::INFINITY),
];
/// Bande où le gain de l'étage D est ajusté (le gain Kd × TPA × facteur d'échelle n'est pas lu dans l'en-tête).
pub const D_GAIN_FIT_HZ: (f64, f64) = (10.0, 50.0);
/// Durée visée d'une fenêtre de Welch (s) ; arrondie à la puissance de 2 supérieure, comme `delay::phase_delay_ms`.
const WINDOW_S: f64 = 0.5;
/// Part maximale de l'arrondi dans l'énergie d'entrée d'une case pour que la case compte (L-103) : au-delà, elle sort des
/// moyennes au lieu d'être corrigée. La correction d'une case est ainsi bornée à 1 / (1 − 0,1), soit +0,9 dB. ⚠️ À 0,5,
/// des vols stationnaires très calmes (signal presque immobile d'un échantillon à l'autre : l'arrondi n'y est plus un
/// bruit blanc) s'emballaient jusqu'à +5 dB ; à 0,1, plus aucun emballement, et les logs à chaque boucle PID ne perdent que
/// 2 bandes concluantes sous 500 Hz sur 511 (lot 22b, `docs/validation.md`).
pub const MAX_INPUT_NOISE_SHARE: f64 = 0.1;
/// Plancher des gains avant le passage en dB (un notch peut rendre une case nulle).
const MIN_GAIN: f64 = 1e-12;

/// Étage contrôlé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// `gyroUnfilt` → `GYRO_SAMPLE[0]` : aucun filtre, seulement le décalage d'horloge de L-100.
    Clock,
    Lpf2,
    Rpm,
    StaticLpf1,
    /// LPF2, notchs RPM, notchs statiques et LPF1 en bloc (`FFT_FREQ`).
    GyroBlock,
    DynNotch,
    Dterm,
}

impl Stage {
    /// Clé anglaise stable (colonnes du corpus).
    pub fn key(self) -> &'static str {
        match self {
            Stage::Clock => "clock",
            Stage::Lpf2 => "lpf2",
            Stage::Rpm => "rpm",
            Stage::StaticLpf1 => "static_lpf1",
            Stage::GyroBlock => "gyro_block",
            Stage::DynNotch => "dyn_notch",
            Stage::Dterm => "dterm",
        }
    }

    /// Nom affiché.
    pub fn label(self) -> &'static str {
        match self {
            Stage::Clock => "horloge (gyroUnfilt → GYRO_SAMPLE[0])",
            Stage::Lpf2 => "LPF2",
            Stage::Rpm => "notchs RPM",
            Stage::StaticLpf1 => "notchs statiques + LPF1",
            Stage::GyroBlock => "LPF2 + RPM + notchs statiques + LPF1",
            Stage::DynNotch => "notch dynamique",
            Stage::Dterm => "filtres D",
        }
    }
}

/// Signaux d'un étage, sur la période choisie : entrée et sortie enregistrées, sortie du modèle pour la même entrée.
#[derive(Debug, Clone)]
pub struct StageSignals {
    pub stage: Stage,
    pub axis: usize,
    pub input: Vec<f64>,
    pub output: Vec<f64>,
    pub model: Vec<f64>,
    /// Retard d'horloge de la sortie sur l'entrée, absent du modèle (s) : L-100 pour un étage qui part de `gyroUnfilt`.
    pub clock_offset_s: f64,
    /// Gain du modèle ajusté sur [`D_GAIN_FIT_HZ`] (étage D) au lieu d'être pris tel quel.
    pub fit_gain: bool,
    /// Raccords propres à l'étage (indices depuis le début de la période), en plus des trous du log : instants où le
    /// firmware a forcé le D à zéro (étage D).
    pub breaks: Vec<i64>,
    /// Arrondi de l'entrée enregistrée, retiré de son spectre avant de mesurer le gain du firmware.
    pub input_noise: InputNoise,
}

/// Bruit d'arrondi de l'entrée enregistrée d'un étage. Le firmware filtre la valeur exacte, le log n'en garde que
/// l'arrondi : la différence, de variance pas² / 12, est un bruit blanc que l'estimateur S(x, y) / S(x, x) compte comme du
/// signal d'entrée. ⚠️ Non retiré, il fait croire que le firmware filtre plus que le modèle, d'autant plus que le signal
/// est faible : −0,3 dB entre 50 et 100 Hz sur un étage sans aucun filtre, −1,2 à −2,3 dB entre 250 et 500 Hz sur le D des
/// vols chirp (L-103).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputNoise {
    /// Entrée exacte (log fabriqué) ou arrondi inconnu.
    None,
    /// Entrée arrondie au pas donné (unité de l'entrée).
    Rounded(f64),
    /// Entrée = −(g[i] − g[i−1]) × cadence, g arrondi au pas donné (étage D).
    RoundedDerivative(f64),
}

impl InputNoise {
    /// Densité du bruit à la fréquence `f` : variance par échantillon, multipliée par |réponse de la dérivée|² s'il y a
    /// lieu (à multiplier par Σ w² et par le nombre de fenêtres pour une somme de périodogrammes).
    fn variance_at(self, f: f64, fs: f64) -> f64 {
        match self {
            InputNoise::None => 0.0,
            InputNoise::Rounded(step) => step * step / 12.0,
            InputNoise::RoundedDerivative(step) => step * step / 12.0 * (2.0 * fs * (PI * f / fs).sin()).powi(2),
        }
    }
}

/// Résultat d'une bande de fréquences.
#[derive(Debug, Clone, PartialEq)]
pub struct BandCheck {
    pub lo_hz: f64,
    pub hi_hz: f64,
    /// Cases de fréquence dans la bande.
    pub bins: usize,
    pub gain_measured_db: f64,
    pub gain_model_db: f64,
    /// Firmware − modèle (dB) ; positif : le firmware atténue moins que le modèle.
    pub error_db: f64,
    /// Retard du firmware sur le modèle (ms), décalage d'horloge retiré si demandé.
    pub delay_ms: f64,
    pub coherence: f64,
    /// √(énergie de (sortie − modèle) / énergie de la sortie) dans la bande.
    pub residual: f64,
    pub conclusive: bool,
}

/// Résultat d'un étage.
#[derive(Debug, Clone, PartialEq)]
pub struct StageCheck {
    pub stage: Stage,
    pub axis: usize,
    /// Décalage d'horloge retiré (ms ; 0 s'il n'y en a pas ou si la correction est coupée).
    pub clock_offset_ms: f64,
    /// Gain appliqué au modèle (1 sauf pour l'étage D).
    pub gain_factor: f64,
    pub windows: usize,
    pub bands: Vec<BandCheck>,
    /// Résidu sur toutes les fréquences sauf le continu.
    pub residual: f64,
}

/// Étages sautés, avec la raison.
pub type Skipped = Vec<(Stage, String)>;

/// Tous les étages d'un log : contrôlés, et sautés avec la raison.
#[derive(Debug, Clone, Default)]
pub struct StageChecks {
    pub checks: Vec<StageCheck>,
    pub skipped: Skipped,
}

/// Décalage d'horloge de `gyroUnfilt` (L-100) : ⌊`pid_process_denom` / 2⌋ ticks gyro (s). Déduit de `fc/core.c`
/// (`gyroFilterReady`, `pidLoopReady`) ; mesuré seulement pour un denom de 2.
pub fn clock_offset_s(s: &Settings) -> f64 {
    let rate = s.gyro_rate_hz();
    if rate > 0.0 {
        (s.pid_denom.max(1) / 2) as f64 / rate
    } else {
        0.0
    }
}

/// `tpa_d_factor` : facteur TPA du terme D à chaque instant, d'après les gaz d'avant le throttle boost (fraction 0–1).
/// Courbe « classique » du firmware (`flight/pid.c:getTpaFactorClassic`, constantes de `flight/pid_init.c`) : 1 −
/// `tpa_rate` % au-dessus de `tpa_breakpoint`, en rampe jusqu'aux pleins gaz ; sous `tpa_low_breakpoint`, 1 −
/// `tpa_low_rate` % au plus, jusqu'au premier passage au-dessus (sauf `tpa_low_always`). Le firmware l'applique au D quel que
/// soit `tpa_mode`. `None` si l'en-tête n'a pas de TPA ou une autre courbe (`tpa_curve_type` ≠ 0).
pub fn tpa_d_factor(s: &Settings, throttle: &[f64]) -> Option<Vec<f64>> {
    if s.get("tpa_breakpoint").is_none() || s.get_int("tpa_curve_type", 0) != 0 {
        return None;
    }
    let bp = ((s.get_int("tpa_breakpoint", 1350) - 1000) as f64 / 1000.0).clamp(0.0, 0.99);
    let mult = (s.get_int("tpa_rate", 0) as f64 / 100.0) / (1.0 - bp);
    let low_bp = ((s.get_int("tpa_low_breakpoint", 1000) - 1000) as f64 / 1000.0)
        .clamp(0.01, 1.0)
        .min(bp);
    let low_mult = s.get_int("tpa_low_rate", 0) as f64 / (100.0 * low_bp);
    let low_always = s.get_int("tpa_low_always", 0) != 0;
    let mut faded = false;
    Some(
        throttle
            .iter()
            .map(|&t| {
                let t = t.clamp(0.0, 1.0);
                let rate = if t >= low_bp || low_bp <= 0.01 || faded {
                    faded |= !low_always;
                    mult * (t - bp).max(0.0)
                } else {
                    low_mult * (low_bp - t)
                };
                1.0 - rate
            })
            .collect(),
    )
}

fn wide(r: &[f32]) -> Vec<f64> {
    r.iter().map(|&v| v as f64).collect()
}

/// `model_rate_hz` : cadence du log telle que le firmware la compte (en-tête : gyro, `pid_process_denom`, `P interval`).
/// Le modèle calcule ses filtres pour elle, comme le firmware, même si la cadence mesurée sur le log en diffère (L-101).
pub fn model_rate_hz(s: &Settings) -> f64 {
    s.log_rate_hz()
}

/// Raccords (indices depuis `sel.start`) qui isolent les instants où le firmware a forcé le D à zéro (`flight/pid.c`) :
/// PID coupé (gaz à zéro sans airmode, débordement du gyro : P, I, D nuls sur les trois axes) ou récupération de vrille
/// en lacet (P, I, D nuls sur roulis et tangage, I nul en lacet). Critère commun : P, I et D du roulis et du tangage **et**
/// I du lacet nuls au même instant. ⚠️ Pas moins : les termes sont arrondis à l'entier dans le log, et un seul axe à zéro
/// arrive par hasard en vol calme, jusqu'à 60 échantillons d'affilée (mesuré sur le corpus, lot 22a bis). Chaque instant
/// retenu et celui qui le suit deviennent des raccords : aucune fenêtre d'analyse n'en contient.
fn pid_off_breaks(log: &FlightLog, sel: &Range<usize>) -> Vec<i64> {
    let mut terms: Vec<&[f32]> = Vec::new();
    for (c, rows, axes) in [
        ('P', &log.p_term, 0..2),
        ('I', &log.i_term, 0..3),
        ('D', &log.d_term, 0..2),
    ] {
        terms.extend(axes.filter(|&a| log.term_present(c, a)).map(|a| rows.row(a)));
    }
    if terms.len() < 7 {
        return Vec::new(); // un terme manque : impossible de distinguer une coupure d'un hasard
    }
    let mut out = Vec::new();
    for i in sel.clone() {
        if terms.iter().all(|r| r[i] == 0.0) {
            let j = (i - sel.start) as i64;
            if out.last() != Some(&j) {
                out.push(j);
            }
            out.push(j + 1);
        }
    }
    out
}

/// Le log enregistre-t-il chaque boucle PID ?
fn every_pid_loop(s: &Settings) -> bool {
    s.log_rate_hz() >= s.pid_rate_hz() * 0.999
}

/// `stage_signals` : signaux de tous les étages que le log permet de contrôler sur `sel`, et raisons des étages sautés.
/// Le modèle tourne sur tout le log (état des filtres établi) avant d'être découpé sur `sel`.
pub fn stage_signals(log: &FlightLog, sel: Range<usize>) -> Result<(Vec<StageSignals>, Skipped), String> {
    let s = &log.settings;
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    let rate = s.log_rate_hz();
    if rate < MIN_LOG_RATE_HZ {
        let why = format!("log à {rate:.0} Hz, sous {MIN_LOG_RATE_HZ:.0} Hz (D-057)");
        for st in [Stage::GyroBlock, Stage::DynNotch, Stage::Dterm] {
            skipped.push((st, why.clone()));
        }
        return Ok((out, skipped));
    }
    let n = log.n();
    let sel = clamp_range(sel.start, sel.end, n);
    let cut = |v: &[f64]| v[sel.clone()].to_vec();
    let offset = clock_offset_s(s);
    let fs = model_rate_hz(s);
    let rpm_missing = s.rpm_harmonics > 0 && log.motor_hz.is_none();
    let model = |stages: &[GyroStage], x: &[f32]| -> Result<Vec<f64>, String> {
        let mut y = wide(x);
        for &st in stages {
            y = match st {
                GyroStage::Lpf2 => lpf2_at_gyro_rate(log, s, y, fs)?,
                _ => gyro_stage_at(st, log, s, y, 1, fs)?,
            };
        }
        Ok(y)
    };
    // pas d'arrondi des entrées (°/s) : canaux debug à l'unité, gyro et gyroUnfilt au dixième en haute résolution
    let debug_step = InputNoise::Rounded(1.0);
    let gyro_step = 1.0 / if s.high_resolution { 10.0 } else { 1.0 };
    let mut push = |stage, axis, input: &[f32], output: &[f32], m: Vec<f64>, clock_offset_s, input_noise| {
        out.push(StageSignals {
            stage,
            axis,
            input: cut(&wide(input)),
            output: cut(&wide(output)),
            model: cut(&m),
            clock_offset_s,
            fit_gain: false,
            breaks: Vec::new(),
            input_noise,
        });
    };

    match log.gyro_debug_taps() {
        Some(GyroDebugTaps::Sample {
            axis,
            before_lpf2,
            after_lpf2,
            after_rpm,
            after_static,
        }) => {
            match &log.gyro_unfilt {
                Some(gu) => push(
                    Stage::Clock,
                    axis,
                    gu.row(axis),
                    before_lpf2,
                    wide(gu.row(axis)),
                    offset,
                    InputNoise::Rounded(gyro_step),
                ),
                None => skipped.push((Stage::Clock, "pas de gyro non filtré dans ce log".into())),
            }
            push(
                Stage::Lpf2,
                axis,
                before_lpf2,
                after_lpf2,
                model(&[GyroStage::Lpf2], before_lpf2)?,
                0.0,
                debug_step,
            );
            if rpm_missing {
                skipped.push((Stage::Rpm, "filtre RPM actif mais pas d'eRPM dans le log".into()));
            } else {
                push(
                    Stage::Rpm,
                    axis,
                    after_lpf2,
                    after_rpm,
                    model(&[GyroStage::Rpm], after_lpf2)?,
                    0.0,
                    debug_step,
                );
            }
            push(
                Stage::StaticLpf1,
                axis,
                after_rpm,
                after_static,
                model(&[GyroStage::StaticLpf1], after_rpm)?,
                0.0,
                debug_step,
            );
        }
        Some(GyroDebugTaps::FftFreq { axis, before_dyn_notch }) => {
            if !dyn_notch_enabled(s) {
                let why = "notch dynamique coupé : le firmware n'écrit pas le debug FFT_FREQ";
                skipped.push((Stage::GyroBlock, why.into()));
                skipped.push((Stage::DynNotch, why.into()));
            } else {
                match &log.gyro_unfilt {
                    _ if rpm_missing => {
                        skipped.push((Stage::GyroBlock, "filtre RPM actif mais pas d'eRPM dans le log".into()))
                    }
                    Some(gu) => {
                        let block = [GyroStage::Lpf2, GyroStage::Rpm, GyroStage::StaticLpf1];
                        push(
                            Stage::GyroBlock,
                            axis,
                            gu.row(axis),
                            before_dyn_notch,
                            model(&block, gu.row(axis))?,
                            offset,
                            InputNoise::Rounded(gyro_step),
                        );
                    }
                    None => skipped.push((Stage::GyroBlock, "pas de gyro non filtré dans ce log".into())),
                }
                push(
                    Stage::DynNotch,
                    axis,
                    before_dyn_notch,
                    log.gyro.row(axis),
                    model(&[GyroStage::DynNotch], before_dyn_notch)?,
                    0.0,
                    debug_step,
                );
            }
        }
        None => {
            let why = format!("debug {} : ni GYRO_SAMPLE ni FFT_FREQ", log.debug_name());
            skipped.push((Stage::GyroBlock, why));
        }
    }

    // Filtres D : −dg/dt du gyro enregistré → axisD, sur chaque axe dont le D est enregistré et constant.
    if !every_pid_loop(s) {
        skipped.push((Stage::Dterm, "le log n'enregistre pas chaque boucle PID".into()));
    } else {
        let axes: Vec<usize> = (0..3)
            .filter(|&a| s.pids[a].d > 0 && log.term_present('D', a) && log.d_term.row(a).iter().any(|&v| v != 0.0))
            .collect();
        let constant: Vec<usize> = axes
            .iter()
            .copied()
            .filter(|&a| s.pids[a].dmax == s.pids[a].d)
            .collect();
        if axes.is_empty() {
            skipped.push((Stage::Dterm, "aucun terme D enregistré".into()));
        } else if constant.is_empty() {
            skipped.push((Stage::Dterm, "D max actif : le gain du D varie en vol".into()));
        } else {
            let mut sim = simulate_dterm_at(log, None, None, fs)?;
            // le firmware multiplie le D filtré par le facteur TPA (`flight/pid.c`, `getTpaFactor`)
            if let Some(tpa) = tpa_d_factor(s, &pre_boost_throttle(s, &log.throttle, fs)) {
                for row in sim.data.chunks_mut(n) {
                    row.iter_mut().zip(&tpa).for_each(|(v, f)| *v *= f);
                }
            }
            let off = pid_off_breaks(log, &sel);
            for a in constant {
                let g = wide(log.gyro.row(a));
                let mut x = vec![0.0; n];
                for i in 1..n {
                    x[i] = -(g[i] - g[i - 1]) * fs;
                }
                if n > 1 {
                    x[0] = x[1];
                }
                out.push(StageSignals {
                    stage: Stage::Dterm,
                    axis: a,
                    input: cut(&x),
                    output: cut(&wide(log.d_term.row(a))),
                    model: cut(sim.row(a)),
                    clock_offset_s: 0.0,
                    fit_gain: true,
                    breaks: off.clone(),
                    input_noise: InputNoise::RoundedDerivative(gyro_step),
                });
            }
        }
    }
    Ok((out, skipped))
}

type Cx = (f64, f64);

fn cmul(a: Cx, b: Cx) -> Cx {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn conj(a: Cx) -> Cx {
    (a.0, -a.1)
}

fn cabs2(a: Cx) -> f64 {
    a.0 * a.0 + a.1 * a.1
}

fn cdiv(a: Cx, b: Cx) -> Cx {
    let d = cabs2(b);
    if d > 0.0 {
        let p = cmul(a, conj(b));
        (p.0 / d, p.1 / d)
    } else {
        (f64::NAN, f64::NAN)
    }
}

fn db(a: Cx) -> f64 {
    20.0 * cabs2(a).sqrt().max(MIN_GAIN).log10()
}

/// Sommes par case de fréquence sur les fenêtres de Welch.
struct Spectra {
    freqs: Vec<f64>,
    windows: usize,
    /// Σ w² de la fenêtre de Hann (puissance d'un bruit blanc de variance 1 dans une case, par fenêtre).
    window_power: f64,
    sxx: Vec<f64>,
    syy: Vec<f64>,
    smm: Vec<f64>,
    sxy: Vec<Cx>,
    sxm: Vec<Cx>,
    /// Σ conj(Y)·M
    sym: Vec<Cx>,
}

fn spectra(x: &[f64], y: &[f64], m: &[f64], fs: f64, breaks: &[i64]) -> Option<Spectra> {
    let n = x.len().min(y.len()).min(m.len());
    let nper = ((fs * WINDOW_S).log2().ceil().exp2() as usize).min(n);
    if nper < 256 {
        return None;
    }
    let hop = nper / 2;
    let w = hann(nper as i64, false);
    let fft = |v: &[f64]| {
        let seg = segments(&v[..n], nper, hop, Some(breaks));
        let mut buf = Vec::with_capacity(seg.nseg * nper);
        for i in 0..seg.nseg {
            let row = &seg.data[i * nper..(i + 1) * nper];
            let mean = row_mean(row);
            buf.extend(row.iter().zip(&w).map(|(v, wk)| (v - mean) * wk));
        }
        (seg.nseg, rfft_rows(&buf, seg.nseg, nper))
    };
    let (rows, xs) = fft(x);
    if rows == 0 {
        return None;
    }
    let (_, ys) = fft(y);
    let (_, ms) = fft(m);
    let nf = nper / 2 + 1;
    let zero = vec![(0.0, 0.0); nf];
    let mut sp = Spectra {
        freqs: rfftfreq(nper, fs),
        windows: rows,
        window_power: w.iter().map(|v| v * v).sum(),
        sxx: vec![0.0; nf],
        syy: vec![0.0; nf],
        smm: vec![0.0; nf],
        sxy: zero.clone(),
        sxm: zero.clone(),
        sym: zero,
    };
    let add = |acc: &mut Cx, p: Cx| *acc = (acc.0 + p.0, acc.1 + p.1);
    for r in 0..rows {
        for j in 0..nf {
            let i = r * nf + j;
            let (a, b, c) = ((xs[i].re, xs[i].im), (ys[i].re, ys[i].im), (ms[i].re, ms[i].im));
            sp.sxx[j] += cabs2(a);
            sp.syy[j] += cabs2(b);
            sp.smm[j] += cabs2(c);
            add(&mut sp.sxy[j], cmul(conj(a), b));
            add(&mut sp.sxm[j], cmul(conj(a), c));
            add(&mut sp.sym[j], cmul(conj(b), c));
        }
    }
    Some(sp)
}

/// `compare` : contrôle d'un étage (voir l'en-tête du module). `correct_clock` : retirer le décalage d'horloge de
/// `sig.clock_offset_s` (toujours, sauf pour la contre-preuve). `breaks` : raccords de la période (indices relatifs).
/// `None` si la période est trop courte pour une fenêtre.
pub fn compare(sig: &StageSignals, fs: f64, breaks: &[i64], correct_clock: bool) -> Option<StageCheck> {
    let mut all: Vec<i64> = breaks.iter().chain(&sig.breaks).copied().collect();
    all.sort_unstable();
    all.dedup();
    let sp = spectra(&sig.input, &sig.output, &sig.model, fs, &all)?;
    let tau = if correct_clock { sig.clock_offset_s } else { 0.0 };
    let nf = sp.freqs.len();
    // modèle complet par case : c = K · e^(−j2πfτ), appliqué à la sortie du modèle
    let rot: Vec<Cx> = sp
        .freqs
        .iter()
        .map(|&f| ((2.0 * PI * f * tau).cos(), -(2.0 * PI * f * tau).sin()))
        .collect();
    let coh: Vec<f64> = (0..nf)
        .map(|j| {
            let d = sp.sxx[j] * sp.syy[j];
            if d > 0.0 { cabs2(sp.sxy[j]) / d } else { 0.0 }
        })
        .collect();
    // S(x, x) sans l'arrondi de l'entrée (L-103) ; NaN là où l'arrondi dépasse MAX_INPUT_NOISE_SHARE de l'énergie
    // d'entrée : la case ne mesure alors plus le gain et sort des moyennes
    let sxx_signal: Vec<f64> = (0..nf)
        .map(|j| {
            let noise = sp.windows as f64 * sp.window_power * sig.input_noise.variance_at(sp.freqs[j], fs);
            if noise <= MAX_INPUT_NOISE_SHARE * sp.sxx[j] {
                sp.sxx[j] - noise
            } else {
                f64::NAN
            }
        })
        .collect();
    let gain_fw = |j: usize| cdiv(sp.sxy[j], (sxx_signal[j], 0.0));
    let ratio = |j: usize| cdiv(gain_fw(j), cdiv(cmul(sp.sxm[j], rot[j]), (sp.sxx[j], 0.0)));
    let mut k = 1.0;
    if sig.fit_gain {
        let (mut num, mut den) = (0.0, 0.0);
        for (j, (&f, &cj)) in sp.freqs.iter().zip(&coh).enumerate().skip(1) {
            let r = ratio(j);
            if f >= D_GAIN_FIT_HZ.0 && f < D_GAIN_FIT_HZ.1 && r.0.is_finite() {
                num += cj * cabs2(r).sqrt().max(MIN_GAIN).ln();
                den += cj;
            }
        }
        if den > 0.0 {
            k = (num / den).exp();
        }
    }
    // énergie de (Y − c·M) : Syy − 2 Re(c · Σ conj(Y)M) + |c|² Smm
    let err = |j: usize| {
        let c = (rot[j].0 * k, rot[j].1 * k);
        (sp.syy[j] - 2.0 * cmul(c, sp.sym[j]).0 + cabs2(c) * sp.smm[j]).max(0.0)
    };
    let nyq = fs / 2.0;
    let mut bands = Vec::new();
    for &(lo, hi) in &BANDS_HZ {
        if lo >= nyq {
            break;
        }
        let idx: Vec<usize> = (1..nf)
            .filter(|&j| sp.freqs[j] >= lo && sp.freqs[j] < hi.min(nyq))
            .collect();
        let (mut w, mut gm, mut gmod, mut dl, mut wd) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut ey, mut ee, mut c) = (0.0, 0.0, 0.0);
        for &j in &idx {
            let f = sp.freqs[j];
            let hy = gain_fw(j);
            let hm = cdiv(cmul(sp.sxm[j], rot[j]), (sp.sxx[j], 0.0));
            let hm = (hm.0 * k, hm.1 * k);
            c += coh[j];
            ey += sp.syy[j];
            ee += err(j);
            if !(hy.0.is_finite() && hm.0.is_finite()) {
                continue;
            }
            w += coh[j];
            gm += coh[j] * db(hy);
            gmod += coh[j] * db(hm);
            let r = cdiv(hy, hm);
            if r.0.is_finite() && f > 0.0 {
                dl += coh[j] * (-r.1.atan2(r.0) / (2.0 * PI * f) * 1000.0);
                wd += coh[j];
            }
        }
        let mean = |v: f64, d: f64| if d > 0.0 { v / d } else { f64::NAN };
        let coherence = mean(c, idx.len() as f64);
        let (gain_measured_db, gain_model_db) = (mean(gm, w), mean(gmod, w));
        bands.push(BandCheck {
            lo_hz: lo,
            hi_hz: hi.min(nyq),
            bins: idx.len(),
            gain_measured_db,
            gain_model_db,
            error_db: gain_measured_db - gain_model_db,
            delay_ms: mean(dl, wd),
            coherence,
            residual: mean(ee, ey).sqrt(),
            conclusive: coherence >= MIN_COHERENCE && gain_measured_db.is_finite(),
        });
    }
    let (ey, ee): (f64, f64) = (1..nf).fold((0.0, 0.0), |(a, b), j| (a + sp.syy[j], b + err(j)));
    Some(StageCheck {
        stage: sig.stage,
        axis: sig.axis,
        clock_offset_ms: tau * 1000.0,
        gain_factor: k,
        windows: sp.windows,
        bands,
        residual: if ey > 0.0 { (ee / ey).sqrt() } else { f64::NAN },
    })
}

/// `check_stages` : tous les étages contrôlables du log sur `sel`, décalage d'horloge retiré, fréquences comptées à la
/// cadence nominale ([`model_rate_hz`]).
pub fn check_stages(log: &FlightLog, sel: Range<usize>) -> Result<StageChecks, String> {
    let sel = clamp_range(sel.start, sel.end, log.n());
    let (signals, mut skipped) = stage_signals(log, sel.clone())?;
    let breaks: Vec<i64> = log.breaks(sel, 4.0).iter().map(|&b| b as i64).collect();
    let mut checks = Vec::new();
    for sig in &signals {
        match compare(sig, model_rate_hz(&log.settings), &breaks, true) {
            Some(c) => checks.push(c),
            None => skipped.push((sig.stage, "période trop courte pour une fenêtre d'analyse".into())),
        }
    }
    Ok(StageChecks { checks, skipped })
}
