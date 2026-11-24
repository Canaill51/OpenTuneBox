//! Écart du rejeu au vol (fin du lot 24 du plan) : à quel point la chaîne rejouée avec les réglages du log (« simulé actuel »
//! de la page Filtres) retombe sur le spectre **enregistré**, et pourquoi elle s'en écarte quand la cause est connue.
//!
//! Mesure, la même que `otb-dev corpus replay` (qui l'appelle) : par bande de fréquences, moyenne de (rejeu − log) en dB sur
//! les cases de la bande, puis moyenne du roulis et du tangage (le lacet, peu excité et sans D le plus souvent, est laissé de
//! côté). Gyro filtré et terme D (gain Kd ajusté sur les basses fréquences par `filter_replay`, donc écart de **forme**).
//! La dernière bande s'arrête à 0,45 × la cadence du log ; une bande qui n'y tient pas est omise.
//!
//! Causes connues d'un écart ([`GapCause`]) : un log qui ne garde qu'une boucle PID sur N (gyro non filtré sous-échantillonné
//! sans anti-repliement : W-067 ; le terme D du firmware s'en écarte le plus, D-076), et un firmware d'avant la 4.3 (rejoué
//! par l'ancienne chaîne, pas à l'arithmétique du firmware). Les chiffres de référence sur le corpus sont dans
//! `docs/validation.md` (« Rejeu contre le vol ») ; rien n'est promis : l'écart est **mesuré** sur le vol affiché.

use crate::filter_replay::BeforeAfter;
use crate::firmware_chain::Engine;
use otb_log::settings::Settings;

/// Bandes comparées (Hz, borne haute nominale).
pub const BANDS: [(f64, f64); 3] = [(20.0, 100.0), (100.0, 250.0), (250.0, 500.0)];
/// La dernière bande s'arrête à cette fraction de la cadence du log (sous Nyquist, loin du repliement du bord).
pub const TOP_FRACTION: f64 = 0.45;

/// Écart d'une bande, rejeu − log (dB, signé), moyenne du roulis et du tangage ; NaN quand le signal manque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandGap {
    pub lo_hz: f64,
    /// Borne haute nominale (celle de [`BANDS`], qui nomme la bande).
    pub nominal_hi_hz: f64,
    /// Borne haute mesurée : la nominale, coupée à [`TOP_FRACTION`] × cadence du log.
    pub hi_hz: f64,
    pub gyro_db: f64,
    pub d_db: f64,
}

/// Cause connue d'un écart du rejeu au vol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GapCause {
    /// Le log ne garde qu'une boucle PID sur `loops_per_sample` (cadence du log `log_hz`) : l'entrée des filtres n'y est
    /// pas à pleine cadence.
    Slowed { loops_per_sample: usize, log_hz: f64 },
    /// Firmware d'avant la 4.3 : rejoué par l'ancienne chaîne d'OpenTuneBox, pas à l'arithmétique du firmware.
    OldFirmware,
}

/// Écart du rejeu au vol d'une période d'un log.
#[derive(Debug, Clone, PartialEq)]
pub struct FlightGap {
    pub bands: Vec<BandGap>,
    pub causes: Vec<GapCause>,
}

impl FlightGap {
    /// Pire bande (|écart| le plus grand) du gyro : (|écart| dB, bande) ; `None` sans aucune bande mesurée.
    pub fn worst_gyro(&self) -> Option<(f64, BandGap)> {
        worst(&self.bands, |b| b.gyro_db)
    }

    /// Pire bande du terme D ; `None` sans terme D enregistré.
    pub fn worst_d(&self) -> Option<(f64, BandGap)> {
        worst(&self.bands, |b| b.d_db)
    }
}

fn worst(bands: &[BandGap], pick: impl Fn(&BandGap) -> f64) -> Option<(f64, BandGap)> {
    bands
        .iter()
        .filter(|b| pick(b).is_finite())
        .map(|b| (pick(b).abs(), *b))
        .fold(None, |best, x| match best {
            Some((m, _)) if m >= x.0 => best,
            _ => Some(x),
        })
}

/// Moyenne de `a − b` (dB) sur les fréquences de [`lo`, `hi`[ ; NaN sans fréquence dans la bande.
pub fn band_mean(f: &[f64], a: &[f64], b: &[f64], lo: f64, hi: f64) -> f64 {
    let v: Vec<f64> = f
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(x, _)| **x >= lo && **x < hi)
        .map(|(_, (p, q))| p - q)
        .collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Moyenne des valeurs finies ; NaN s'il n'y en a aucune.
fn finite_mean(v: impl Iterator<Item = f64>) -> f64 {
    let v: Vec<f64> = v.filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// `band_gaps` : écarts par bande ([`BandGap`]) des spectres `r` d'un log enregistré à `fs` Hz.
pub fn band_gaps(r: &BeforeAfter, fs: f64) -> Vec<BandGap> {
    let top = TOP_FRACTION * fs;
    let axes = 0..r.gyro.len().min(2);
    BANDS
        .iter()
        .filter_map(|&(lo, nominal_hi)| {
            let hi = nominal_hi.min(top);
            if hi <= lo {
                return None;
            }
            let gyro_db = finite_mean(
                axes.clone()
                    .map(|a| band_mean(&r.freqs, &r.gyro[a].current_db, &r.gyro[a].logged_db, lo, hi)),
            );
            let d_db = finite_mean(
                axes.clone()
                    .filter_map(|a| r.dterm.get(a)?.as_ref())
                    .map(|d| band_mean(&r.freqs, &d.current_db, &d.logged_db, lo, hi)),
            );
            Some(BandGap {
                lo_hz: lo,
                nominal_hi_hz: nominal_hi,
                hi_hz: hi,
                gyro_db,
                d_db,
            })
        })
        .collect()
}

/// `causes` : causes connues d'un écart pour un log de réglages `s` (rangées : log ralenti, puis ancien firmware).
pub fn causes(s: &Settings) -> Vec<GapCause> {
    let mut out = Vec::new();
    let log_hz = s.log_rate_hz();
    if log_hz > 0.0 {
        let loops_per_sample = (s.pid_rate_hz() / log_hz).round().max(1.0) as usize;
        if loops_per_sample > 1 {
            out.push(GapCause::Slowed {
                loops_per_sample,
                log_hz,
            });
        }
    }
    if Engine::in_service(s) == Engine::Model {
        out.push(GapCause::OldFirmware);
    }
    out
}

/// `flight_gap` : [`FlightGap`] des spectres `r` (réglages du log `s`, log à `fs` Hz).
pub fn flight_gap(r: &BeforeAfter, fs: f64, s: &Settings) -> FlightGap {
    FlightGap {
        bands: band_gaps(r, fs),
        causes: causes(s),
    }
}
