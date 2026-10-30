//! Séries de vols pour le réglage par la réponse indicielle (port de `analysis/step_series.py`, phase 4c-B(e)).
//!
//! Une série = des vols du même drone (mêmes filtres) où un seul curseur change. Ce module relève, pour chaque vol, ses curseurs, sa signature
//! de filtres, ses conditions (feedforward, part du vol en mode Angle, curseur I) et sa réponse indicielle roll / pitch (`flight_step`), puis
//! reconnaît les séries parmi les vols (`find_series`).
//!
//! ⚠️ `find_series` rend une erreur si un curseur est NaN : le tri d'une liste à NaN de Python dépend de l'algorithme (timsort) et aucun
//! curseur de l'application n'est NaN (fractions d'entiers de l'en-tête).

use crate::delay::xcorr_delay_ms;
use crate::npstat::std_f32;
use crate::open_loop::{LoopMetrics, Terms};
use crate::sliders::{PTB_CATEGORIES, slider_view};
use crate::step::{LoopInput, StepMetrics, StepOptions, step_response};
use otb_format::round_digits;
use otb_log::flightlog::{ANGLE_MODE_MASK, FlightLog, clamp_range};
use otb_log::settings::Settings;
use std::ops::Range;

/// Curseur du feedforward.
pub const FF_KEY: &str = "Stick Resp.";
/// Curseurs dont on lit les séries.
pub const SERIES_KEYS: [&str; 5] = ["Damping", "Master", "Pitch T.", "Pitch D.", FF_KEY];
/// Vol « en mode Angle » : au moins 90 % de la période analysée.
pub const ANGLE_SHARE_MIN: f64 = 0.9;

/// `FlightConditions` : conditions du protocole pour un vol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlightConditions {
    /// Gain F roll, pitch.
    pub ff: (i64, i64),
    /// Part de la période en mode Angle (NaN : modes non enregistrés).
    pub angle_share: f64,
    /// Curseur I (« Drift-Wobble »), 1,0 = 100.
    pub i_slider: f64,
}

impl FlightConditions {
    pub fn angle(&self) -> bool {
        self.angle_share.is_finite() && self.angle_share >= ANGLE_SHARE_MIN
    }

    /// Feedforward présent dans la réponse : F > 0 hors mode Angle (le firmware l'y met à 0).
    pub fn ff_active(&self) -> bool {
        self.ff.0.max(self.ff.1) > 0 && !self.angle()
    }

    /// Gain F en service sur l'axe (0 = roll, 1 = pitch) : 0 en mode Angle, quel que soit l'en-tête (L-005).
    pub fn ff_gain(&self, axis: usize) -> i64 {
        if self.angle() {
            0
        } else if axis == 0 {
            self.ff.0
        } else {
            self.ff.1
        }
    }
}

/// Un tuple imbriqué d'entiers (la signature des filtres de Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sig {
    I(i64),
    T(Vec<Sig>),
}

/// `filter_signature` : réglages de filtres qui changent la réponse ; deux vols d'une série doivent les partager.
pub fn filter_signature(s: &Settings) -> Sig {
    let lp =
        |f: &otb_log::settings::Lowpass| Sig::T(vec![Sig::I(f.static_hz), Sig::I(f.dyn_min_hz), Sig::I(f.dyn_max_hz)]);
    let pair = |p: (i64, i64)| Sig::T(vec![Sig::I(p.0), Sig::I(p.1)]);
    Sig::T(vec![
        lp(&s.gyro_lpf1),
        lp(&s.gyro_lpf2),
        lp(&s.dterm_lpf1),
        lp(&s.dterm_lpf2),
        Sig::T(s.gyro_notches.iter().map(|&n| pair(n)).collect()),
        pair(s.dterm_notch),
        Sig::I(s.dyn_notch_count),
        Sig::I(s.dyn_notch_q),
        Sig::I(s.dyn_notch_min_hz),
        Sig::I(s.rpm_harmonics),
        Sig::I(s.rpm_q),
        Sig::I(s.rpm_min_hz),
        Sig::T(s.rpm_weights.iter().map(|&w| Sig::I(w)).collect()),
    ])
}

/// `flight_conditions`.
pub fn flight_conditions(log: &FlightLog, sel: Range<usize>) -> Result<FlightConditions, String> {
    let s = &log.settings;
    let ff = (s.pids[0].f, s.pids[1].f);
    let mut share = f64::NAN;
    if let Some(flags) = &log.mode_flags {
        let f = &flags[clamp_range(sel.start, sel.end, flags.len())];
        if !f.is_empty() {
            // np.mean d'un tableau de booléens : somme exacte d'entiers 0/1 sur le nombre de valeurs
            share = f.iter().filter(|&&v| v & ANGLE_MODE_MASK != 0).count() as f64 / f.len() as f64;
        }
    }
    let view = slider_view(s)?;
    let i_slider = view
        .values
        .iter()
        .find(|(k, _)| *k == "Drift-Wobble")
        .map(|(_, v)| *v)
        .ok_or("curseur Drift-Wobble absent")?;
    Ok(FlightConditions {
        ff,
        angle_share: share,
        i_slider,
    })
}

/// `FlightStep` : un vol vu par le diagnostic.
#[derive(Debug, Clone)]
pub struct FlightStep {
    pub label: String,
    /// Catégorie PTB → fraction (1,0 = 100 %), dans l'ordre de `PTB_CATEGORIES`.
    pub sliders: Vec<(&'static str, f64)>,
    pub filters: Sig,
    pub conditions: FlightConditions,
    /// Roll, pitch.
    pub metrics: Vec<StepMetrics>,
    pub n: Vec<usize>,
    pub sp_delay_ms: Vec<f64>,
    pub loops: Vec<LoopMetrics>,
}

impl FlightStep {
    fn slider(&self, key: &str) -> Result<f64, String> {
        self.sliders
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("curseur {key} absent"))
    }
}

/// `flight_step` : réponse indicielle roll / pitch d'un vol sur `sel`, avec ses curseurs et ses conditions.
pub fn flight_step(log: &FlightLog, sel: Range<usize>, label: &str) -> Result<FlightStep, String> {
    let s = &log.settings;
    let mut f = FlightStep {
        label: if label.is_empty() {
            log.path
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned())
        } else {
            label.to_string()
        },
        sliders: slider_view(s)?.values,
        filters: filter_signature(s),
        conditions: flight_conditions(log, sel.clone())?,
        metrics: Vec::new(),
        n: Vec::new(),
        sp_delay_ms: Vec::new(),
        loops: Vec::new(),
    };
    let breaks: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let br = if breaks.is_empty() {
        None
    } else {
        Some(breaks.as_slice())
    };
    for ax in 0..2 {
        let rows = |r: &otb_log::flightlog::Rows| r.row(ax)[clamp_range(sel.start, sel.end, r.row(ax).len())].to_vec();
        let (sp32, gy32) = (rows(&log.setpoint), rows(&log.gyro));
        let up = |v: &[f32]| v.iter().map(|&x| x as f64).collect::<Vec<f64>>();
        let (sp, gy) = (up(&sp32), up(&gy32));
        let terms = log.pid_terms(ax, sel.clone());
        let (tp, ti, td, tf) = (up(&terms.p), up(&terms.i), up(&terms.d), up(&terms.f));
        let inst = terms.instrument.as_ref().map(|v| up(v));
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
        let r = step_response(&sp, &gy, log.fs, &StepOptions::default(), &input, br)?;
        f.metrics.push(r.metrics);
        f.n.push(r.n);
        // `np.std(sp) >= 5.0` sur le tableau float32 du journal
        let sd = std_f32(&sp32);
        f.sp_delay_ms.push(if sd >= 5.0f32 {
            xcorr_delay_ms(&sp, &gy, log.fs, 100.0, 0)?
        } else {
            f64::NAN
        });
        f.loops.push(r.loop_);
    }
    Ok(f)
}

/// `Series` : des vols triés par valeur du curseur qui change.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// Catégorie PTB du curseur qui change.
    pub key: &'static str,
    /// Indices dans la liste des vols, triés par valeur du curseur.
    pub flights: Vec<usize>,
    pub values: Vec<f64>,
}

impl Series {
    /// Écarts entre valeurs successives distinctes (arrondis à 3 décimales).
    pub fn steps(&self) -> Vec<f64> {
        let mut v = self.values.clone();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup();
        v.windows(2).map(|w| round_digits(w[1] - w[0], 3)).collect()
    }
}

/// `find_series` : séries à un seul curseur parmi `flights` (mêmes filtres, mêmes autres curseurs à 0,01 près).
pub fn find_series(flights: &[FlightStep]) -> Result<Vec<Series>, String> {
    let mut out = Vec::new();
    for key in SERIES_KEYS {
        // dictionnaire ordonné de Python : (filtres, autres curseurs arrondis) → indices des vols ; un NaN n'est égal à rien
        let mut groups: Vec<(&Sig, Vec<f64>, Vec<usize>)> = Vec::new();
        for (i, f) in flights.iter().enumerate() {
            let mut others = Vec::new();
            for (cat, _) in PTB_CATEGORIES.iter().filter(|(c, _)| *c != key) {
                others.push(round_digits(f.slider(cat)?, 2));
            }
            match groups.iter_mut().find(|(sig, o, _)| **sig == f.filters && *o == others) {
                Some(g) => g.2.push(i),
                None => groups.push((&f.filters, others, vec![i])),
            }
        }
        for (_, _, idx) in &groups {
            let vals: Vec<f64> = idx
                .iter()
                .map(|&i| flights[i].slider(key).map(|v| round_digits(v, 2)))
                .collect::<Result<_, _>>()?;
            if vals.iter().any(|v| v.is_nan()) {
                return Err("find_series : curseur NaN (tri non défini)".into());
            }
            let mut distinct = vals.clone();
            distinct.sort_by(|a, b| a.partial_cmp(b).unwrap());
            distinct.dedup();
            if distinct.len() < 2 {
                continue;
            }
            let mut order: Vec<usize> = (0..idx.len()).collect();
            order.sort_by(|&a, &b| vals[a].partial_cmp(&vals[b]).unwrap()); // tri stable, comme `sorted`
            out.push(Series {
                key: PTB_CATEGORIES.iter().find(|(c, _)| *c == key).map(|(c, _)| *c).unwrap(),
                flights: order.iter().map(|&j| idx[j]).collect(),
                values: order.iter().map(|&j| vals[j]).collect(),
            });
        }
    }
    Ok(out)
}
