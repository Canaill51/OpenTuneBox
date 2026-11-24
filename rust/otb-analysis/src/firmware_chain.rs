//! Chaîne de filtres Betaflight **assemblée** avec les briques à l'arithmétique du firmware (lot 24c du plan, D-062).
//!
//! Les briques (`otb_dsp::firmware::betaflight`) sont identiques au firmware compilé sur PC au bit près (lot 24b) ; ce module
//! les enchaîne comme la boucle du firmware. **Chaîne en service depuis la bascule (lot 24d)** : le rejeu de la page Filtres,
//! l'assistant et le contrôle par étage la prennent par [`Engine::in_service`] dès la 4.3 ; l'ancienne chaîne (`bf_filters`,
//! `Engine::Model`) ne sert plus qu'aux logs d'avant la 4.3 et aux mesures (`otb-dev corpus … --chain model`). Ordre et cadence des étapes :
//! lus dans la description de la chaîne (`chain::betaflight`), jamais écrits ici.
//!
//! Recopié du firmware, version par version (sources lues : `sensors/gyro.c`, `gyro_init.c`, `gyro_filter_impl.c`,
//! `flight/rpm_filter.c`, `pid.c`, `pid_init.c`, `mixer.c`, `dyn_notch_filter.c`, `drivers/dshot.c`, `fc/core.c` des
//! versions 4.3.2, 4.4.3, 4.5.5, 2025.12.5 et 2026.6.2) :
//! - **deux cadences** réellement exécutées : LPF2 à la cadence gyro, puis passage à la cadence PID (dernière sortie du LPF2,
//!   ou moyenne des échantillons sans lui), puis les autres filtres à la cadence PID ; l'entrée enregistrée est reconstruite
//!   à la cadence de la première étape par sinus cardinal (`bf_filters::upsample`, L-104) ;
//! - **notchs RPM recalculés à tour de rôle** : `notchUpdatesPerIteration` notchs par boucle (de quoi tous les revoir en
//!   1 ms), harmonique puis moteur, fondu près de `rpm_filter_min_hz`, plafond à 0,48 × cadence PID ; fréquence des moteurs
//!   lissée par un PT1 à chaque boucle PID. Le recalcul a lieu dans la boucle PID, donc **après** le filtrage de la même
//!   boucle, jusqu'à la 2025.12 ; avant lui depuis la 2026.6 (`rpmFilterRun`) ;
//! - **LPF dynamiques recalculés par le mixeur** (après le filtrage, donc à la boucle suivante) quand les gaz **d'avant le
//!   throttle boost**, arrondis au centième, changent, au plus une fois toutes les 5 ms ;
//! - ⚠️ jusqu'à la 4.5, les passe-bas gyro PT2 et PT3 **fixes** sont initialisés avec le gain d'un **PT1** (`gyro_init.c`,
//!   `gain = pt1FilterGain(…)` passé à `pt2FilterInit`) : ils coupent plus bas que leur réglage. Recalculés (LPF1
//!   dynamique), ils prennent le bon gain. Le terme D, lui, a toujours eu le bon gain ;
//! - terme D comme `pid.c` : notch D, LPF1 D, LPF2 D sur le gyro filtré, puis `−(g − g_préc) × pidFrequency`, en float32.
//!
//! Ce qui n'est pas recopié (W-081) : le suivi des pics du notch dynamique (centres donnés : suivi rejoué ou debug
//! `FFT_FREQ`, coefficients recalculés dès que le centre change), la correction `schedulerGetCycleTimeMultiplier()` des
//! notchs RPM (2025.12 et après ; prise à 1, le log ne l'enregistre pas), la fusion multiplication-addition de la carte de
//! vol. Avant la 4.3 : aucune arithmétique vérifiée, la chaîne refuse le log. Le facteur TPA du terme D reste appliqué par
//! l'appelant, comme pour la chaîne actuelle.

use crate::bf_filters::{dyn_notch_enabled, fft_freq_centres, pre_boost_throttle, upsample};
use crate::chain::{self, Brick, LowpassSetting, NotchLimit, Rate, Step};
use otb_dsp::firmware::betaflight::{
    Arith, Biquad, BiquadKind, Decimator, Pt1, Pt2, Pt3, RpmNotch, SvfLowpass, SvfNotch, notch_q,
};
use otb_log::flightlog::FlightLog;
use otb_log::settings::{Lowpass, Settings};
use std::ops::Range;

/// Chaîne qui rejoue les filtres : l'ancienne (float64, réponses idéales) ou l'assemblée ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    /// L'ancienne chaîne (`bf_filters`), celle de l'écran et de l'assistant jusqu'au lot 24c : logs d'avant la 4.3 et mesures.
    Model,
    /// La chaîne de ce module, à l'arithmétique du firmware : celle de l'écran et de l'assistant depuis le lot 24d.
    Firmware,
}

impl Engine {
    /// Chaîne en service pour un log de réglages `s` (bascule du lot 24d, D-062) : celle du firmware dès la 4.3, l'ancienne
    /// avant (aucune arithmétique vérifiée, et aucun log d'avant la 4.3 au corpus : rien ne change pour eux).
    pub fn in_service(s: &Settings) -> Engine {
        if Release::of(s).is_some() {
            Engine::Firmware
        } else {
            Engine::Model
        }
    }

    /// `model` ou `firmware`.
    pub fn parse(s: &str) -> Result<Engine, String> {
        match s {
            "model" => Ok(Engine::Model),
            "firmware" => Ok(Engine::Firmware),
            _ => Err(format!("chaîne inconnue « {s} » (model ou firmware)")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Engine::Model => "model",
            Engine::Firmware => "firmware",
        }
    }
}

/// Version de Betaflight, au grain où la chaîne change : arithmétique des briques **et** calendrier des recalculs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Release {
    /// 4.3 : notchs RPM appliqués moteur puis harmonique, fréquence des moteurs lissée en eRPM.
    V4_3,
    /// 4.4 : notchs RPM appliqués harmonique puis moteur.
    V4_4,
    /// 4.5 : poids des harmoniques RPM, fréquence des moteurs lissée au début de la tâche de filtrage.
    V4_5,
    /// 2025.12.
    V2025_12,
    /// 2026.6 et après : filtres SVF, notchs RPM recalculés avant d'être appliqués.
    V2026_6,
}

impl Release {
    /// Version de l'en-tête ; `None` avant la 4.3 (rien de vérifié).
    pub fn of(s: &Settings) -> Option<Release> {
        let (major, minor) = (*s.version.first()?, *s.version.get(1)?);
        match (major, minor) {
            (4, 3) => Some(Release::V4_3),
            (4, 4) => Some(Release::V4_4),
            (4, 5) => Some(Release::V4_5),
            (2025, _) => Some(Release::V2025_12),
            (y, _) if y >= 2026 => Some(Release::V2026_6),
            _ => None,
        }
    }

    pub fn arith(self) -> Arith {
        match self {
            Release::V4_3 | Release::V4_4 => Arith::V4_3,
            Release::V4_5 => Arith::V4_5,
            Release::V2025_12 => Arith::V2025_12,
            Release::V2026_6 => Arith::V2026_6,
        }
    }
}

/// Cadences du firmware, en microsecondes entières comme `gyro.sampleLooptime` et `gyro.targetLooptime`.
#[derive(Clone, Copy, Debug)]
struct Timing {
    gyro_us: u32,
    pid_us: u32,
    /// Échantillons gyro par boucle PID (`pid_process_denom`).
    denom: usize,
    /// Boucles PID par échantillon du log (`P interval`), au moins 1.
    per_log: usize,
}

impl Timing {
    fn of(s: &Settings) -> Result<Timing, String> {
        if s.looptime_us <= 0 || s.pid_denom <= 0 {
            return Err("en-tête sans cadence gyro ou PID".into());
        }
        let gyro_us = s.looptime_us as u32;
        let denom = s.pid_denom as usize;
        let per_log = (s.pid_rate_hz() / s.log_rate_hz()).round().max(1.0) as usize;
        Ok(Timing {
            gyro_us,
            pid_us: gyro_us * denom as u32,
            denom,
            per_log,
        })
    }

    fn pid_dt(self) -> f32 {
        self.pid_us as f32 * 1e-6
    }
}

/// Réglages et version lus une fois.
struct Ctx<'a> {
    s: &'a Settings,
    rel: Release,
    t: Timing,
}

impl<'a> Ctx<'a> {
    fn new(s: &'a Settings) -> Result<Ctx<'a>, String> {
        let rel = Release::of(s).ok_or("version de Betaflight antérieure à la 4.3 : arithmétique non vérifiée")?;
        Ok(Ctx {
            s,
            rel,
            t: Timing::of(s)?,
        })
    }

    fn arith(&self) -> Arith {
        self.rel.arith()
    }
}

/// Un passe-bas du firmware.
#[derive(Clone, Debug)]
enum Lp {
    Off,
    Pt1(Pt1),
    Pt2(Pt2),
    Pt3(Pt3),
    /// Biquad en forme directe I (`biquadFilterApplyDF1`, choisi pour un passe-bas qui peut être recalculé).
    Df1(Biquad),
    /// Biquad en forme directe II transposée (`biquadFilterApply`).
    Df2(Biquad),
    Svf(SvfLowpass),
}

impl Lp {
    fn apply(&mut self, x: f32) -> f32 {
        match self {
            Lp::Off => x,
            Lp::Pt1(f) => f.apply(x),
            Lp::Pt2(f) => f.apply(x),
            Lp::Pt3(f) => f.apply(x),
            Lp::Df1(f) => f.apply_df1(x),
            Lp::Df2(f) => f.apply_df2(x),
            Lp::Svf(f) => f.apply(x),
        }
    }

    /// Nouvelle coupure (`dynLpfGyroUpdate`, `dynLpfDTermUpdate`), filtre calculé à `us`.
    fn set_cutoff(&mut self, arith: Arith, hz: f32, us: u32) {
        let dt = us as f32 * 1e-6;
        match self {
            Lp::Off => {}
            Lp::Pt1(f) => f.set_gain(arith.pt1_gain(hz, dt)),
            Lp::Pt2(f) => f.set_gain(arith.pt2_gain(hz, dt)),
            Lp::Pt3(f) => f.set_gain(arith.pt3_gain(hz, dt)),
            Lp::Df1(f) | Lp::Df2(f) => f.update_lowpass(hz, us),
            // 2026.6 : coupure bornée sous Nyquist avant le recalcul
            Lp::Svf(f) => f.update(hz.min(0.475 / dt), dt),
        }
    }
}

/// Passe-bas `which` tel que le firmware l'initialise (`gyroInitLowpassFilterLpf`, `pidInitFilters`).
fn new_lowpass(c: &Ctx, which: LowpassSetting) -> Lp {
    let lp = which.get(c.s);
    let arith = c.arith();
    let svf = arith.uses_svf();
    match which {
        LowpassSetting::GyroLpf1 | LowpassSetting::GyroLpf2 => {
            let us = if which == LowpassSetting::GyroLpf2 {
                c.t.gyro_us
            } else {
                c.t.pid_us
            };
            let init = if which == LowpassSetting::GyroLpf1 && lp.dyn_min_hz > 0 {
                lp.dyn_min_hz
            } else {
                lp.static_hz
            };
            if init <= 0 {
                return Lp::Off;
            }
            let (hz, dt) = (init as f32, us as f32 * 1e-6);
            let nyquist = 1_000_000 / 2 / us;
            // ⚠️ jusqu'à la 4.5, PT2 et PT3 reçoivent le gain d'un PT1 (voir l'en-tête du module)
            let pt1_gain_for_all = c.rel <= Release::V4_5;
            match lp.kind {
                "PT1" => Lp::Pt1(Pt1::new(arith.pt1_gain(hz, dt))),
                "BIQUAD" if init as u32 <= nyquist => {
                    if svf {
                        Lp::Svf(SvfLowpass::new(hz, dt))
                    } else {
                        Lp::Df1(Biquad::lowpass(arith, hz, us))
                    }
                }
                "PT2" => Lp::Pt2(Pt2::new(if pt1_gain_for_all {
                    arith.pt1_gain(hz, dt)
                } else {
                    arith.pt2_gain(hz, dt)
                })),
                "PT3" => Lp::Pt3(Pt3::new(if pt1_gain_for_all {
                    arith.pt1_gain(hz, dt)
                } else {
                    arith.pt3_gain(hz, dt)
                })),
                _ => Lp::Off,
            }
        }
        LowpassSetting::DtermLpf1 | LowpassSetting::DtermLpf2 => {
            let us = c.t.pid_us;
            let dt = c.t.pid_dt();
            let nyquist = (1.0 / dt / 2.0) as u32;
            let first = which == LowpassSetting::DtermLpf1;
            let init = if first && lp.dyn_min_hz > 0 {
                lp.dyn_min_hz
            } else {
                lp.static_hz
            };
            if init <= 0 || (!first && lp.static_hz <= 0) {
                return Lp::Off;
            }
            let hz = init as f32;
            // biquad : jusqu'à la 2025.12, le LPF1 compare sa coupure **fixe** à Nyquist ; en 2026.6, celle de départ
            let checked = if first && !svf { lp.static_hz } else { init };
            match lp.kind {
                "PT1" => Lp::Pt1(Pt1::new(arith.pt1_gain(hz, dt))),
                "BIQUAD" if (checked as u32) < nyquist => {
                    if svf {
                        Lp::Svf(SvfLowpass::new(hz, dt))
                    } else if first {
                        Lp::Df1(Biquad::lowpass(arith, hz, us))
                    } else {
                        Lp::Df2(Biquad::lowpass(arith, hz, us))
                    }
                }
                "PT2" => Lp::Pt2(Pt2::new(arith.pt2_gain(hz, dt))),
                "PT3" => Lp::Pt3(Pt3::new(arith.pt3_gain(hz, dt))),
                _ => Lp::Off,
            }
        }
    }
}

/// `dynThrottle` (`sensors/gyro.c`).
fn dyn_throttle(t: f32) -> f32 {
    t * (1.0 - (t * t) / 3.0) * 1.5
}

/// Coupure d'un passe-bas dynamique pour des gaz `t` déjà arrondis au centième (`dynLpfGyroUpdate`, `dynLpfCutoffFreq`).
pub fn dyn_lpf_cutoff(lp: &Lowpass, t: f32) -> f32 {
    if lp.expo > 0 {
        let expof = lp.expo as f32 / 10.0;
        let curve = t * (1.0 - t) * expof + t;
        (lp.dyn_max_hz - lp.dyn_min_hz) as f32 * curve + lp.dyn_min_hz as f32
    } else {
        (dyn_throttle(t) * lp.dyn_max_hz as f32).max(lp.dyn_min_hz as f32)
    }
}

/// Délai minimal entre deux recalculs des LPF dynamiques (`DYN_LPF_THROTTLE_UPDATE_DELAY_US`, `mixer.c`).
const DYN_LPF_DELAY_US: u64 = 5000;

/// Moment des recalculs des LPF dynamiques (`updateDynLpfCutoffs`, `mixer.c`).
#[derive(Clone, Copy, Debug)]
pub struct DynLpfClock {
    last_us: u64,
    previous: i32,
}

impl DynLpfClock {
    /// Horloge réglée comme si le mixeur venait de recalculer pour `throttle` à l'instant 0 : le log commence en vol, le
    /// filtre a déjà sa coupure. Rend aussi les gaz arrondis de ce premier recalcul.
    pub fn start(throttle: f32) -> (DynLpfClock, f32) {
        let q = quantize(throttle);
        (
            DynLpfClock {
                last_us: 0,
                previous: q,
            },
            q as f32 / 100.0,
        )
    }

    /// Passage du mixeur à `now_us` : les gaz arrondis du recalcul, ou `None` s'il n'y en a pas.
    pub fn tick(&mut self, now_us: u64, throttle: f32) -> Option<f32> {
        if now_us.saturating_sub(self.last_us) < DYN_LPF_DELAY_US {
            return None;
        }
        let q = quantize(throttle);
        if q == self.previous {
            return None;
        }
        self.previous = q;
        self.last_us = now_us;
        Some(q as f32 / 100.0)
    }
}

/// `lrintf(throttle * DYN_LPF_THROTTLE_STEPS)`.
fn quantize(throttle: f32) -> i32 {
    (throttle * 100.0).round_ties_even() as i32
}

/// Un notch statique.
#[derive(Clone, Debug)]
enum Notch {
    Off,
    Df2(Biquad),
    Svf(SvfNotch),
}

impl Notch {
    fn apply(&mut self, x: f32) -> f32 {
        match self {
            Notch::Off => x,
            Notch::Df2(f) => f.apply_df2(x),
            Notch::Svf(f) => f.apply(x),
        }
    }
}

/// Notch statique `(centre, coupure)` ramené sous Nyquist comme le firmware (`calculateNyquistAdjustedNotchHz`,
/// `pidInitFilters`). Coupure au-dessus du centre : aucun notch (comme la chaîne actuelle ; le firmware calculerait un Q
/// négatif).
fn new_notch(c: &Ctx, setting: Option<(i64, i64)>, limit: NotchLimit) -> Notch {
    let Some((center, cutoff)) = setting else {
        return Notch::Off;
    };
    let us = c.t.pid_us;
    let dt = c.t.pid_dt();
    let nyquist: i64 = match limit {
        NotchLimit::Pid95 => {
            let n = 1_000_000 / 2 / us;
            if c.rel >= Release::V2026_6 {
                (n as f32 * 0.95) as i64
            } else {
                n as i64
            }
        }
        NotchLimit::Pid => (1.0 / dt / 2.0) as u32 as i64,
    };
    let mut hz = center;
    if hz > nyquist {
        hz = if cutoff < nyquist { nyquist } else { 0 };
    }
    if hz <= 0 || cutoff <= 0 || cutoff >= hz {
        return Notch::Off;
    }
    let q = notch_q(hz as f32, cutoff as f32);
    if c.arith().uses_svf() {
        Notch::Svf(SvfNotch::new(hz as f32, dt, q))
    } else {
        Notch::Df2(Biquad::new(c.arith(), hz as f32, us, q, BiquadKind::Notch, 1.0))
    }
}

/// Nombre de notchs RPM recalculés par boucle (`rpmFilterInit`) : de quoi les revoir tous en 1 ms.
pub fn rpm_updates_per_loop(rel: Release, notches_per_axis: usize, pid_us: u32) -> usize {
    let loops = 0.001_f32 / (pid_us as f32 * 1e-6);
    let ratio = notches_per_axis as f32 / loops;
    if rel == Release::V4_3 {
        (ratio + 0.49).round_ties_even() as usize
    } else {
        ratio.ceil() as usize
    }
}

/// Banc de notchs RPM : un par moteur et par harmonique, sur chaque axe (`flight/rpm_filter.c`).
struct RpmBank {
    rel: Release,
    harmonics: usize,
    motors: usize,
    weights: Vec<f32>,
    min_hz: f32,
    max_hz: f32,
    fade_hz: f32,
    q: f32,
    us: u32,
    dt: f32,
    per_loop: usize,
    motor: usize,
    harmonic: usize,
    erpm_to_hz: f32,
    lpf: Vec<Pt1>,
    motor_hz: Vec<f32>,
    /// Jusqu'à la 2025.12 : `[ligne][moteur × harmoniques + harmonique]`.
    biquads: Vec<Vec<Biquad>>,
    /// 2026.6 : `[moteur × harmoniques + harmonique]`, trois axes chacun.
    svf: Vec<RpmNotch>,
    /// eRPM brut (centaines de tours électriques par minute, comme la télémétrie DShot), `[moteur][échantillon du log]`.
    erpm: Vec<Vec<f32>>,
}

impl RpmBank {
    /// `None` : pas de filtre RPM (harmoniques à 0, ou pas d'eRPM dans le log).
    fn new(c: &Ctx, log: &FlightLog, rows: usize) -> Option<RpmBank> {
        let s = c.s;
        let mhz = log.motor_hz.as_ref().filter(|_| s.rpm_harmonics > 0)?;
        let (rel, arith) = (c.rel, c.arith());
        let harmonics = s.rpm_harmonics as usize;
        let motors = mhz.k;
        let us = c.t.pid_us;
        let dt = us as f32 * 1e-6;
        let poles = s.motor_poles as f32;
        // eRPM entier retrouvé depuis la fréquence du log (`motor_hz` = eRPM × 100 / 60 / (pôles / 2))
        let back = s.motor_poles as f64 / 2.0 * 60.0 / 100.0;
        let erpm = (0..motors)
            .map(|m| mhz.row(m).iter().map(|&v| (v as f64 * back).round() as f32).collect())
            .collect();
        let weights = (0..harmonics)
            .map(|h| {
                if rel >= Release::V4_5 {
                    let w = s.rpm_weights.get(h).copied().unwrap_or(100);
                    (w as f32 / 100.0).clamp(0.0, 1.0)
                } else {
                    1.0
                }
            })
            .collect();
        let q = s.rpm_q as f32 / 100.0;
        let min_hz = s.rpm_min_hz as f32;
        let mut biquads = Vec::new();
        let mut svf = Vec::new();
        if arith.uses_svf() {
            for _ in 0..motors {
                for h in 0..harmonics {
                    svf.push(RpmNotch::new(min_hz * (h + 1) as f32, dt, q, 0.0));
                }
            }
        } else {
            let mut row = Vec::with_capacity(motors * harmonics);
            for _ in 0..motors {
                for h in 0..harmonics {
                    row.push(Biquad::new(arith, min_hz * h as f32, us, q, BiquadKind::Notch, 0.0));
                }
            }
            biquads = vec![row; rows];
        }
        let lpf_gain = arith.pt1_gain(s.rpm_lpf_hz as f32, dt);
        Some(RpmBank {
            rel,
            harmonics,
            motors,
            weights,
            min_hz,
            max_hz: 0.48_f32 * 1e6_f32 / us as f32,
            fade_hz: s.rpm_fade_hz as f32,
            q,
            us,
            dt,
            per_loop: rpm_updates_per_loop(rel, motors * harmonics, us),
            motor: 0,
            harmonic: 0,
            erpm_to_hz: 100.0_f32 / 60.0 / (poles / 2.0),
            lpf: vec![Pt1::new(lpf_gain); motors],
            motor_hz: vec![0.0; motors],
            biquads,
            svf,
            erpm,
        })
    }

    /// Télémétrie de la boucle : fréquence des moteurs lissée (`rpmFilterUpdate` en 4.3–4.4, `updateDshotTelemetry` après).
    fn telemetry(&mut self, i: usize) {
        for m in 0..self.motors {
            let e = self.erpm[m].get(i).copied().unwrap_or(0.0);
            self.motor_hz[m] = if self.rel == Release::V4_3 {
                self.erpm_to_hz * self.lpf[m].apply(e)
            } else {
                self.lpf[m].apply(self.erpm_to_hz * e)
            };
        }
    }

    /// Recalcul de `per_loop` notchs, à tour de rôle (`rpmFilterUpdate`).
    fn update(&mut self) {
        for _ in 0..self.per_loop {
            let (h, m) = (self.harmonic, self.motor);
            if self.rel < Release::V4_5 || self.weights[h] > 0.0 {
                let f = constrainf((h + 1) as f32 * self.motor_hz[m], self.min_hz, self.max_hz);
                let mut weight = 1.0_f32;
                if self.rel == Release::V4_3 {
                    if f < self.min_hz + self.fade_hz {
                        weight = (f - self.min_hz) / self.fade_hz;
                    }
                } else {
                    let margin = f - self.min_hz;
                    if margin < self.fade_hz {
                        weight = margin / self.fade_hz;
                    }
                }
                if self.rel >= Release::V4_5 {
                    weight *= self.weights[h];
                }
                let idx = m * self.harmonics + h;
                if self.svf.is_empty() {
                    for row in &mut self.biquads {
                        row[idx].update(f, self.us, self.q, BiquadKind::Notch, weight);
                    }
                } else {
                    self.svf[idx].update(f, self.dt, self.q, weight);
                }
            }
            self.harmonic = (h + 1) % self.harmonics;
            if self.harmonic == 0 {
                self.motor = (m + 1) % self.motors;
            }
        }
    }

    /// Les notchs appliqués aux lignes `v` (`rpmFilterApply`, `applyFilter`).
    fn apply(&mut self, v: &mut [f32]) {
        let hn = self.harmonics;
        // harmonique sautée quand son poids est nul (4.5 et après ; avant, tous les poids valent 1)
        let order: Vec<usize> = rpm_apply_order(self.rel, self.motors, hn)
            .into_iter()
            .filter(|&(_, h)| self.weights[h] > 0.0)
            .map(|(m, h)| m * hn + h)
            .collect();
        if !self.svf.is_empty() {
            let mut lanes = [0.0_f32; 3];
            lanes[..v.len()].copy_from_slice(v);
            for &i in &order {
                self.svf[i].apply(&mut lanes);
            }
            v.copy_from_slice(&lanes[..v.len()]);
            return;
        }
        for (x, row) in v.iter_mut().zip(&mut self.biquads) {
            for &i in &order {
                *x = row[i].apply_df1_weighted(*x);
            }
        }
    }
}

/// Ordre dans lequel le firmware applique les notchs RPM (`rpmFilterApply`, `applyFilter`) : `(moteur, harmonique)`,
/// moteur puis harmonique en 4.3, harmonique puis moteur ensuite. ⚠️ Des notchs dont la fréquence bouge ne commutent pas :
/// l'ordre change le résultat (L-120). Seule source de cet ordre, pour les deux chaînes.
pub fn rpm_apply_order(rel: Release, motors: usize, harmonics: usize) -> Vec<(usize, usize)> {
    if rel == Release::V4_3 {
        (0..motors).flat_map(|m| (0..harmonics).map(move |h| (m, h))).collect()
    } else {
        (0..harmonics).flat_map(|h| (0..motors).map(move |m| (m, h))).collect()
    }
}

/// `constrainf` du firmware : borne basse d'abord.
fn constrainf(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// Un notch dynamique.
#[derive(Clone, Debug)]
enum DynNotchF {
    Df1(Biquad),
    Svf(SvfNotch),
}

/// Notchs dynamiques d'une ligne, à des centres donnés (`[notch][échantillon du log]`).
struct DynRow {
    notches: Vec<(DynNotchF, f32, Vec<f32>)>,
}

/// Notchs dynamiques (`dyn_notch_filter.c`) : coefficients recalculés dès que le centre donné change.
struct DynBank {
    rows: Vec<DynRow>,
    us: u32,
    dt: f32,
    q: f32,
}

impl DynBank {
    /// `centres[ligne][notch][échantillon du log]` ; un notch dont tous les centres sont nuls est ignoré.
    fn new(c: &Ctx, centres: &[Vec<Vec<f64>>]) -> DynBank {
        let (us, dt, arith) = (c.t.pid_us, c.t.pid_dt(), c.arith());
        let q = c.s.dyn_notch_q as f32 / 100.0;
        let count = c.s.dyn_notch_count.max(0) as usize;
        let rows = centres
            .iter()
            .map(|row| DynRow {
                notches: row
                    .iter()
                    .take(count)
                    .filter_map(|cs| {
                        let cs: Vec<f32> = cs.iter().map(|&v| v as f32).collect();
                        let first = *cs.iter().find(|&&v| v > 0.0)?;
                        let f = if arith.uses_svf() {
                            DynNotchF::Svf(SvfNotch::new(first, dt, q))
                        } else {
                            DynNotchF::Df1(Biquad::new(arith, first, us, q, BiquadKind::Notch, 1.0))
                        };
                        Some((f, first, cs))
                    })
                    .collect(),
            })
            .collect();
        DynBank { rows, us, dt, q }
    }

    fn apply(&mut self, v: &mut [f32], i: usize) {
        for (x, row) in v.iter_mut().zip(&mut self.rows) {
            for (f, current, cs) in &mut row.notches {
                let c = cs.get(i).copied().unwrap_or(0.0);
                if c > 0.0 && c != *current {
                    *current = c;
                    match f {
                        DynNotchF::Df1(b) => b.update(c, self.us, self.q, BiquadKind::Notch, 1.0),
                        DynNotchF::Svf(n) => n.update(c, self.dt, self.q),
                    }
                }
                *x = match f {
                    DynNotchF::Df1(b) => b.apply_df1(*x),
                    DynNotchF::Svf(n) => n.apply(*x),
                };
            }
        }
    }
}

/// Une étape de la chaîne, avec l'état de ses filtres pour chaque ligne.
enum Unit {
    Lowpass {
        rows: Vec<Lp>,
        /// Réglage et horloge du recalcul, si la coupure suit les gaz.
        dynamic: Option<(LowpassSetting, DynLpfClock)>,
        us: u32,
    },
    Notch(Vec<Notch>),
    Rpm(Option<Box<RpmBank>>),
    Dyn(Option<DynBank>),
}

/// Gaz d'avant le throttle boost (fraction 0–1) de chaque échantillon du log, en float32.
fn throttle(log: &FlightLog, s: &Settings) -> Vec<f32> {
    pre_boost_throttle(s, &log.throttle, s.log_rate_hz())
        .into_iter()
        .map(|v| v as f32)
        .collect()
}

fn build(
    c: &Ctx,
    log: &FlightLog,
    step: &Step,
    rows: usize,
    thr0: f32,
    centres: Option<&[Vec<Vec<f64>>]>,
) -> Result<Unit, String> {
    Ok(match step.brick {
        Brick::Lowpass(which) => {
            let mut lps: Vec<Lp> = (0..rows).map(|_| new_lowpass(c, which)).collect();
            let us = match step.rate {
                Rate::Gyro => c.t.gyro_us,
                Rate::Pid => c.t.pid_us,
            };
            let lp = which.get(c.s);
            let can_move = matches!(which, LowpassSetting::GyroLpf1 | LowpassSetting::DtermLpf1) && lp.dyn_min_hz > 0;
            let dynamic = if can_move {
                let (clock, t) = DynLpfClock::start(thr0);
                let hz = dyn_lpf_cutoff(lp, t);
                lps.iter_mut().for_each(|f| f.set_cutoff(c.arith(), hz, us));
                Some((which, clock))
            } else {
                None
            };
            Unit::Lowpass { rows: lps, dynamic, us }
        }
        Brick::StaticNotch(setting, limit) => {
            Unit::Notch((0..rows).map(|_| new_notch(c, setting.get(c.s), limit)).collect())
        }
        Brick::RpmNotch => Unit::Rpm(RpmBank::new(c, log, rows).map(Box::new)),
        Brick::DynNotch => {
            if !dyn_notch_enabled(c.s) {
                Unit::Dyn(None)
            } else {
                let owned;
                let cs = match centres {
                    Some(cs) => cs,
                    None => match fft_freq_centres(log, Some(c.s)) {
                        Some(m) => {
                            let one: Vec<Vec<f64>> = (0..m.k).map(|p| m.row(p).to_vec()).collect();
                            owned = vec![one; rows];
                            &owned
                        }
                        None => return Ok(Unit::Dyn(None)),
                    },
                };
                Unit::Dyn(Some(DynBank::new(c, cs)))
            }
        }
    })
}

impl Unit {
    /// Début de la tâche de filtrage de la boucle (échantillon `i` du log).
    fn before(&mut self, rel: Release, i: usize) {
        if let Unit::Rpm(Some(b)) = self {
            if rel >= Release::V4_5 {
                b.telemetry(i);
            }
            if rel >= Release::V2026_6 {
                b.update();
            }
        }
    }

    fn apply(&mut self, v: &mut [f32], i: usize) {
        match self {
            Unit::Lowpass { rows, .. } => v.iter_mut().zip(rows).for_each(|(x, f)| *x = f.apply(*x)),
            Unit::Notch(rows) => v.iter_mut().zip(rows).for_each(|(x, f)| *x = f.apply(*x)),
            Unit::Rpm(Some(b)) => b.apply(v),
            Unit::Dyn(Some(b)) => b.apply(v, i),
            Unit::Rpm(None) | Unit::Dyn(None) => {}
        }
    }

    /// Une valeur de la ligne `r` : étapes à la cadence gyro (passe-bas seulement : le LPF2 de Betaflight).
    fn apply_row(&mut self, r: usize, x: f32) -> Result<f32, String> {
        match self {
            Unit::Lowpass { rows, .. } => Ok(rows[r].apply(x)),
            _ => Err("seul un passe-bas tourne à la cadence gyro".into()),
        }
    }

    /// Après le filtrage : boucle PID (notchs RPM jusqu'à la 2025.12), puis mixeur (LPF dynamiques).
    fn after(&mut self, c: &Ctx, i: usize, now_us: u64, thr: &[f32]) {
        match self {
            Unit::Rpm(Some(b)) => {
                if c.rel <= Release::V4_4 {
                    b.telemetry(i);
                }
                if c.rel <= Release::V2025_12 {
                    b.update();
                }
            }
            Unit::Lowpass {
                rows,
                dynamic: Some((which, clock)),
                us,
            } => {
                let t = thr.get(i).copied().unwrap_or(0.0);
                if let Some(q) = clock.tick(now_us, t) {
                    let hz = dyn_lpf_cutoff(which.get(c.s), q);
                    rows.iter_mut().for_each(|f| f.set_cutoff(c.arith(), hz, *us));
                }
            }
            _ => {}
        }
    }
}

/// Boucles PID par échantillon du log (`P interval`), comme la chaîne les compte.
pub fn pid_loops_per_log_sample(s: &Settings) -> Result<usize, String> {
    Ok(Timing::of(s)?.per_log)
}

/// `every` : la valeur d'une boucle sur `r` (la dernière de chaque groupe, celle qu'enregistre le log), en float64.
pub fn every(v: &[f32], r: usize) -> Vec<f64> {
    let r = r.max(1);
    v.chunks(r).filter(|c| c.len() == r).map(|c| c[r - 1] as f64).collect()
}

/// `gyro_steps` : étapes `steps` de la chaîne gyro appliquées aux lignes `x` (même longueur, `x_per_log` valeurs par
/// échantillon du log : 1 pour un signal du log, les boucles PID par échantillon pour une sortie de cette fonction).
/// Rend chaque ligne **à la cadence PID** ([`every`] la ramène à celle du log). `centres` : centres des notchs dynamiques
/// `[ligne][notch][échantillon du log]` ; sans eux, ceux du debug `FFT_FREQ`, sinon aucun notch dynamique.
pub fn gyro_steps(
    log: &FlightLog,
    s: &Settings,
    steps: Range<usize>,
    x: &[Vec<f64>],
    x_per_log: usize,
    centres: Option<&[Vec<Vec<f64>>]>,
) -> Result<Vec<Vec<f32>>, String> {
    let c = Ctx::new(s)?;
    let desc = chain::betaflight();
    let all = desc.gyro.get(steps.clone()).ok_or("étapes hors de la chaîne gyro")?;
    let rows = x.len();
    if rows == 0 || rows > 3 {
        return Err(format!("{rows} lignes : la chaîne en filtre de 1 à 3"));
    }
    let n_in = x[0].len();
    let x_per_log = x_per_log.max(1);
    let n_log = n_in / x_per_log;
    let gyro_part = all.iter().take_while(|st| st.rate == Rate::Gyro).count();
    if all[gyro_part..].iter().any(|st| st.rate == Rate::Gyro) {
        return Err("une étape à la cadence gyro après une étape à la cadence PID".into());
    }
    let thr = throttle(log, s);
    let thr0 = thr.first().copied().unwrap_or(0.0);
    let mut units = Vec::with_capacity(all.len());
    for st in all {
        units.push(build(&c, log, st, rows, thr0, centres)?);
    }
    let (mut gyro_units, mut pid_units) = {
        let rest = units.split_off(gyro_part);
        (units, rest)
    };
    let t = c.t;
    // entrée portée à la cadence de la première étape
    let per_input = if gyro_part > 0 { t.per_log * t.denom } else { t.per_log };
    if per_input % x_per_log != 0 {
        return Err(format!(
            "entrée à {x_per_log} valeurs par échantillon du log, chaîne à {per_input} : rapport non entier"
        ));
    }
    let up = per_input / x_per_log;
    let input: Vec<Vec<f32>> = x
        .iter()
        .map(|row| {
            let r = if up > 1 { upsample(row, up) } else { row.clone() };
            r.into_iter().map(|v| v as f32).collect()
        })
        .collect();
    let decimate = gyro_part > 0;
    let lpf2_on = gyro_units
        .iter()
        .any(|u| matches!(u, Unit::Lowpass { rows, .. } if !matches!(rows[0], Lp::Off)));
    let mut dec: Vec<Decimator> = (0..rows).map(|_| Decimator::new(lpf2_on)).collect();
    let n_pid = n_log * t.per_log;
    let mut out: Vec<Vec<f32>> = vec![Vec::with_capacity(n_pid); rows];
    let mut v = vec![0.0_f32; rows];
    for j in 0..n_pid {
        let i = j / t.per_log;
        for u in &mut pid_units {
            u.before(c.rel, i);
        }
        if decimate {
            for k in 0..t.denom {
                let idx = j * t.denom + k;
                for (r, d) in dec.iter_mut().enumerate() {
                    let mut y = input[r][idx];
                    for u in &mut gyro_units {
                        y = u.apply_row(r, y)?;
                    }
                    d.push(y);
                }
            }
            for (r, d) in dec.iter_mut().enumerate() {
                v[r] = d.take();
            }
        } else {
            for r in 0..rows {
                v[r] = input[r][j];
            }
        }
        for u in &mut pid_units {
            u.apply(&mut v, i);
        }
        let now = j as u64 * t.pid_us as u64;
        for u in &mut pid_units {
            u.after(&c, i, now, &thr);
        }
        for r in 0..rows {
            out[r].push(v[r]);
        }
    }
    Ok(out)
}

/// `dterm` : terme D du firmware (`pid.c`) à partir du gyro filtré `gyro` (lignes à la cadence PID, `per_log` boucles par
/// échantillon du log) : étapes de la chaîne D d'avant la dérivée, puis `signe × (g − g_préc) × pidFrequency`, puis celles
/// d'après ; à la cadence PID, sans le facteur TPA. La première boucle reprend la dérivée de la deuxième (pas de valeur
/// précédente).
pub fn dterm(log: &FlightLog, s: &Settings, gyro: &[Vec<f32>], per_log: usize) -> Result<Vec<Vec<f32>>, String> {
    let c = Ctx::new(s)?;
    let desc = chain::betaflight();
    let rows = gyro.len();
    let thr = throttle(log, s);
    let thr0 = thr.first().copied().unwrap_or(0.0);
    let mut before = Vec::new();
    let mut after = Vec::new();
    for (k, st) in desc.dterm.iter().enumerate() {
        if !matches!(st.brick, Brick::Lowpass(_) | Brick::StaticNotch(..)) || st.rate != Rate::Pid {
            return Err(format!(
                "{} : étape {} du terme D non prise en charge",
                desc.firmware, st.id
            ));
        }
        let u = build(&c, log, st, rows, thr0, None)?;
        if k < desc.derivative.at {
            before.push(u);
        } else {
            after.push(u);
        }
    }
    let sign = desc.derivative.sign as f32;
    let pid_frequency = 1.0_f32 / c.t.pid_dt();
    let per_log = per_log.max(1);
    let n = gyro.first().map_or(0, Vec::len);
    let mut out: Vec<Vec<f32>> = vec![Vec::with_capacity(n); rows];
    let mut prev = vec![0.0_f32; rows];
    let mut v = vec![0.0_f32; rows];
    for j in 0..n {
        let i = j / per_log;
        for (x, g) in v.iter_mut().zip(gyro) {
            *x = g[j];
        }
        for u in &mut before {
            u.apply(&mut v, i);
        }
        for (x, p) in v.iter_mut().zip(prev.iter_mut()) {
            let g = *x;
            *x = if j == 0 { 0.0 } else { sign * ((g - *p) * pid_frequency) };
            *p = g;
        }
        for u in &mut after {
            u.apply(&mut v, i);
        }
        let now = j as u64 * c.t.pid_us as u64;
        for u in before.iter_mut().chain(after.iter_mut()) {
            u.after(&c, i, now, &thr);
        }
        for (o, x) in out.iter_mut().zip(&v) {
            o.push(*x);
        }
    }
    for row in &mut out {
        if row.len() > 1 {
            row[0] = row[1];
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notchs_rpm_par_boucle() {
        // 4 moteurs × 3 harmoniques, boucle PID à 4 kHz (250 µs) : 4 boucles par ms, 12 / 4 = 3 notchs par boucle
        assert_eq!(rpm_updates_per_loop(Release::V4_5, 12, 250), 3);
        // à 8 kHz : 8 boucles par ms, 12 / 8 = 1,5 → 2 (plafond), et en 4.3 rintf(1,5 + 0,49) = 2
        assert_eq!(rpm_updates_per_loop(Release::V4_5, 12, 125), 2);
        assert_eq!(rpm_updates_per_loop(Release::V4_3, 12, 125), 2);
        // 1 kHz : 12 notchs en une seule boucle
        assert_eq!(rpm_updates_per_loop(Release::V2026_6, 12, 1000), 12);
        // 4.3 : 4 × 1 harmonique à 8 kHz, 0,5 + 0,49 → 1 (le plafond donnerait 1 aussi) ; 4 × 2 à 3,2 kHz : 8 / 3,2 = 2,5
        assert_eq!(rpm_updates_per_loop(Release::V4_3, 4, 125), 1);
        assert_eq!(rpm_updates_per_loop(Release::V4_3, 8, 312), 3);
        assert_eq!(rpm_updates_per_loop(Release::V4_5, 8, 312), 3);
    }

    #[test]
    fn ordre_des_notchs_rpm() {
        // 4.4 et après : harmonique puis moteur
        assert_eq!(
            rpm_apply_order(Release::V4_5, 2, 2),
            vec![(0, 0), (1, 0), (0, 1), (1, 1)]
        );
        assert_eq!(
            rpm_apply_order(Release::V2026_6, 2, 2),
            rpm_apply_order(Release::V4_4, 2, 2)
        );
        // 4.3 : moteur puis harmonique
        assert_eq!(
            rpm_apply_order(Release::V4_3, 2, 2),
            vec![(0, 0), (0, 1), (1, 0), (1, 1)]
        );
    }

    #[test]
    fn lpf_dynamique_cinq_ms_et_centieme() {
        let (mut clk, t0) = DynLpfClock::start(0.304);
        assert_eq!(t0, 0.3);
        // moins de 5 ms après : pas de recalcul, même si les gaz ont changé
        assert_eq!(clk.tick(4_999, 0.5), None);
        // 5 ms : recalcul
        assert_eq!(clk.tick(5_000, 0.5), Some(0.5));
        // même centième : rien
        assert_eq!(clk.tick(20_000, 0.504), None);
        // 0,505 × 100 = 50,5 en float32 (50,499998) → 50
        assert_eq!(clk.tick(20_000, 0.505), None);
        assert_eq!(clk.tick(20_000, 0.506), Some(0.51));
    }

    #[test]
    fn coupure_dynamique_comme_le_firmware() {
        let lp = Lowpass {
            kind: "PT1",
            static_hz: 0,
            dyn_min_hz: 250,
            dyn_max_hz: 500,
            expo: 5,
        };
        // courbe : t(1 − t)·0,5 + t à mi-gaz = 0,625 → 250 × 0,625 + 250
        assert_eq!(dyn_lpf_cutoff(&lp, 0.5), 406.25);
        let old = Lowpass { expo: 0, ..lp };
        // ancienne loi : max(min, max × t(1 − t²/3)·1,5)
        assert_eq!(dyn_lpf_cutoff(&old, 0.0), 250.0);
        assert!((dyn_lpf_cutoff(&old, 1.0) - 500.0).abs() < 1e-3);
    }
}
