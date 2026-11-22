//! Suivi des pics du **notch dynamique** du firmware, rejoué sur le gyro (lot 22c du plan, W-057).
//!
//! Réimplémenté d'après la description du firmware (`flight/dyn_notch_filter.c`, `common/sdft.c`), sans en copier le code
//! (D-004). Le firmware, à chaque boucle PID :
//! 1. additionne le gyro de chaque axe **avant** le notch dynamique (après LPF2, notchs RPM, notchs statiques, LPF1) ;
//!    toutes les `sample_count` boucles, la moyenne devient un échantillon de la SDFT (transformée de Fourier glissante sur
//!    72 échantillons, amortie par r = 0,9999, une SDFT par axe) ; la mise à jour des cases est répartie sur les
//!    `sample_count` boucles suivantes (« batches ») ;
//! 2. fait avancer une machine à 4 étapes, un axe après l'autre (12 boucles pour les trois axes), relancée pour 12 boucles
//!    à chaque nouvel échantillon : fenêtre de Hann appliquée dans le domaine fréquentiel et puissance par case ; recherche
//!    des `count` plus grands maximums locaux, rangés par case croissante ; plancher de bruit = puissance moyenne hors des
//!    pics (le pic et ¾ de ses deux voisines retirés), × 2 ; pour chaque pic au-dessus du plancher, centre affiné par une
//!    parabole sur trois cases et borné à [min, max], puis lissé par un PT1 de 4 Hz × (pic / plancher, borné à 1–10),
//!    échantillonné toutes les 12 boucles ; enfin les notchs de l'axe sont recalculés.
//!
//! Variantes reproduites, d'après l'historique du firmware (dates de compilation lues dans l'en-tête, `Firmware date`) :
//! - **4.3** : `maxHz` ≥ 2 × `minHz`, sans borne de Nyquist ; `sample_count` = cadence / (2 × max) ; première case ≥ 2 ;
//!   plancher compté sur les cases intérieures (première et dernière exclues) ;
//! - **4.4 à 2026.6** : `maxHz` ≥ `minHz` et ≤ Nyquist ; première case ≥ 1 ; plancher sur toute la plage ;
//! - **compilé à partir du 2026-09-05** : plancher compté au-dessus de 60 Hz seulement (le mouvement du drone en est exclu) ;
//!   à partir du 2026-10-07, un pic posé sur la première case du plancher n'en est plus retiré ;
//! - cadence de la boucle en `float` : `1 / looptime × 1e6` jusqu'à la 2025.12, `1 / (looptime × 1e-6)` ensuite (un peu
//!   sous la valeur exacte), exacte à partir du 2026-11-20. ⚠️ L'arrondi change `sample_count` quand Nyquist / `max` tombe
//!   juste (500 Hz à 4 ou 8 kHz) : 3 au lieu de 4, d'où une autre résolution (L-102).
//!
//! Écarts connus avec le firmware : calcul en `f32` mais cosinus et sinus exacts (le firmware les approche par un
//! polynôme) ; répartition des cases en « batches » du firmware actuel pour toutes les versions (seul le moment où une case
//! reçoit l'échantillon change, pas le spectre une fois l'échantillon passé) ; la phase de la machine à étapes au début du
//! log est inconnue (au plus 12 boucles d'écart) ; l'état au début du log aussi : on le reconstruit en rejouant d'abord le
//! début du log ([`PREROLL_S`]).

use crate::bf_filters::{GyroStage, dyn_notch_enabled, gyro_stage_at};
use otb_log::flightlog::FlightLog;
use otb_log::settings::Settings;

/// Taille de la SDFT du firmware (`SDFT_SAMPLE_SIZE`) ; cases utiles : la moitié.
const SDFT_SIZE: usize = 72;
const SDFT_BINS: usize = SDFT_SIZE / 2;
/// Amortissement de la SDFT (`SDFT_R`), qui la garde stable.
const SDFT_R: f32 = 0.9999;
/// Nombre maximal de notchs par axe (`DYN_NOTCH_COUNT_MAX`).
pub const MAX_NOTCHES: usize = 7;
/// Coupure de base du lissage des centres (`DYN_NOTCH_SMOOTH_HZ`).
const SMOOTH_HZ: f32 = 4.0;
/// Boucles pour mettre à jour les trois axes (3 axes × 4 étapes).
const CALC_TICKS: usize = 12;
/// Plancher de bruit compté au-dessus de cette fréquence (firmware compilé à partir du 2026-09-05).
const FLOOR_MIN_HZ: f32 = 60.0;
/// Durée du début du log rejouée une première fois pour établir l'état du suivi (SDFT pleine, centres accrochés).
pub const PREROLL_S: f64 = 1.0;

/// Règle du plancher de bruit selon la version du firmware.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloorRule {
    /// 4.3 : cases intérieures de la plage, diviseur `fin − début − pics − 1`.
    Inner,
    /// 4.4 à 2026.6 : toute la plage, diviseur `fin − début − pics + 1`.
    Full,
    /// Compilé à partir du 2026-09-05 : au-dessus de 60 Hz ; `strict` (2026-10-07) : un pic sur la première case du
    /// plancher n'en est pas retiré.
    Above60 { strict: bool },
}

/// Réglages du suivi, tels que le firmware les calcule à l'initialisation (`dynNotchInit`).
#[derive(Clone, Debug, PartialEq)]
pub struct DynNotchTracker {
    pub count: usize,
    pub min_hz: f32,
    pub max_hz: f32,
    /// Boucles PID moyennées par échantillon de la SDFT.
    pub sample_count: usize,
    /// Largeur d'une case (Hz).
    pub resolution_hz: f32,
    pub start_bin: usize,
    pub end_bin: usize,
    /// Première case comptée dans le plancher de bruit.
    pub floor_start_bin: usize,
    pub floor: FloorRule,
    /// Pas de temps du lissage des centres (12 boucles).
    pub pt1_dt_s: f32,
}

/// Date de compilation du firmware (`Firmware date`, « Nov 13 2026 15:43:59 ») en `AAAAMMJJ`.
pub fn firmware_build_date(s: &Settings) -> Option<i64> {
    let raw = s.get("Firmware date")?;
    let mut it = raw.split_whitespace();
    let month = match it.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let day: i64 = it.next()?.parse().ok()?;
    let year: i64 = it.next()?.parse().ok()?;
    Some(year * 10_000 + month * 100 + day)
}

/// `lrintf` : arrondi au plus proche, à égalité vers le pair (mode d'arrondi par défaut).
fn lrint(x: f32) -> i64 {
    x.round_ties_even() as i64
}

impl DynNotchTracker {
    /// Réglages du suivi pour ce log. `None` : notch dynamique coupé (ou cadence PID sous 2 kHz), ou firmware d'avant
    /// la SDFT (avant la 4.3 : une FFT, que ce module ne rejoue pas).
    pub fn new(s: &Settings) -> Option<Self> {
        if !dyn_notch_enabled(s) || s.version.is_empty() || s.version < vec![4, 3] {
            return None;
        }
        let looptime = s.looptime_us.checked_mul(s.pid_denom.max(1)).filter(|&l| l > 0)?;
        let date = firmware_build_date(s);
        let built_after = |d: i64| date.is_some_and(|x| x >= d);
        let legacy = s.version < vec![4, 4];
        let v2026_6 = s.version >= vec![2026, 6];
        // corrections datées de la branche de développement 2026.12 ; sans date, on la suppose à jour
        let dev = |d: i64| s.version >= vec![2026, 12] && (date.is_none() || built_after(d));
        // ⚠️ cadence de la boucle en float, comme le firmware de chaque version : l'arrondi change `sample_count` (L-102)
        let lt = looptime as f32;
        let looprate = if dev(20261120) {
            1e6 / lt
        } else if v2026_6 {
            1.0 / (lt * 1e-6)
        } else {
            1.0 / lt * 1e6
        };
        let nyquist = looprate / 2.0;
        let min_hz = s.dyn_notch_min_hz as f32;
        let (max_hz, sample_count) = if legacy {
            let max = (2.0 * min_hz).max(s.dyn_notch_max_hz as f32);
            (max, (looprate / (2.0 * max)).max(1.0) as usize)
        } else {
            let max = min_hz.max(s.dyn_notch_max_hz as f32).min(nyquist);
            (max, (nyquist / max).max(1.0) as usize)
        };
        let sample_count = sample_count.max(1);
        let resolution_hz = looprate / sample_count as f32 / SDFT_SIZE as f32;
        let (start_bin, end_bin) = if legacy {
            // `(int)(x + 0.5f)` en 4.3
            let start = ((min_hz / resolution_hz + 0.5) as i64).max(2);
            let end = ((max_hz / resolution_hz + 0.5) as i64).min(SDFT_BINS as i64 - 1);
            (start, end)
        } else {
            let start = lrint(min_hz / resolution_hz).max(1);
            let end = lrint(max_hz / resolution_hz).min(SDFT_BINS as i64 - 1);
            (start, end)
        };
        let start_bin = start_bin.clamp(0, SDFT_BINS as i64 - 1) as usize;
        let end_bin = end_bin.clamp(start_bin as i64, SDFT_BINS as i64 - 1) as usize;
        let floor = if legacy {
            FloorRule::Inner
        } else if dev(20260905) {
            FloorRule::Above60 { strict: dev(20261007) }
        } else {
            FloorRule::Full
        };
        let floor_start_bin = match floor {
            FloorRule::Above60 { .. } => {
                let f = (lrint(FLOOR_MIN_HZ / resolution_hz).max(start_bin as i64)) as usize;
                if f as i64 > end_bin as i64 - 3 { start_bin } else { f }
            }
            _ => start_bin,
        };
        let count = usize::try_from(s.dyn_notch_count).ok()?.min(MAX_NOTCHES);
        (count > 0 && end_bin > start_bin + 1).then_some(DynNotchTracker {
            count,
            min_hz,
            max_hz,
            sample_count,
            resolution_hz,
            start_bin,
            end_bin,
            floor_start_bin,
            floor,
            pt1_dt_s: CALC_TICKS as f32 / looprate,
        })
    }

    /// Centres initiaux, répartis sur la plage : `(p + 0,5)(max − min)/count + min`.
    pub fn initial_centres(&self) -> Vec<f32> {
        (0..self.count)
            .map(|p| (p as f32 + 0.5) * (self.max_hz - self.min_hz) / self.count as f32 + self.min_hz)
            .collect()
    }

    /// Rejoue le suivi sur le gyro d'avant le notch dynamique de chaque axe (`None` : axe absent, compté nul), enregistré
    /// toutes les `ticks_per_sample` boucles PID (entre deux échantillons du log, le signal est interpolé linéairement).
    /// Rend les centres après chaque échantillon. Le début du log (`preroll` échantillons) est rejoué une première fois
    /// pour établir l'état ; les centres rendus sont ceux du second passage.
    pub fn track(&self, input: [Option<&[f64]>; 3], ticks_per_sample: usize, preroll: usize) -> TrackedCentres {
        let n = input.iter().flatten().map(|x| x.len()).max().unwrap_or(0);
        let d = ticks_per_sample.max(1);
        let mut st = State::new(self);
        let value = |a: usize, i: usize| -> f32 { input[a].and_then(|x| x.get(i)).copied().unwrap_or(0.0) as f32 };
        let run = |st: &mut State, i: usize, prev: &mut [f32; 3]| {
            let cur = [value(0, i), value(1, i), value(2, i)];
            for j in 1..=d {
                let t = j as f32 / d as f32;
                let x = [0, 1, 2].map(|a| prev[a] + (cur[a] - prev[a]) * t);
                st.tick(self, x);
            }
            *prev = cur;
        };
        let first = [value(0, 0), value(1, 0), value(2, 0)];
        let mut prev = first;
        for i in 0..preroll.min(n) {
            run(&mut st, i, &mut prev);
        }
        prev = first;
        let mut data = vec![0.0f32; 3 * self.count * n];
        for i in 0..n {
            run(&mut st, i, &mut prev);
            for a in 0..3 {
                for p in 0..self.count {
                    data[(a * self.count + p) * n + i] = st.centre[a][p];
                }
            }
        }
        TrackedCentres {
            count: self.count,
            n,
            data,
        }
    }
}

/// Centres suivis : pour chaque axe et chaque notch, un centre (Hz) par échantillon du log.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackedCentres {
    pub count: usize,
    pub n: usize,
    data: Vec<f32>,
}

impl TrackedCentres {
    /// Centres du notch `p` de l'axe `axis`.
    pub fn row(&self, axis: usize, p: usize) -> &[f32] {
        let i = (axis * self.count + p) * self.n;
        &self.data[i..i + self.n]
    }
}

#[derive(Clone, Copy, Default)]
struct C32 {
    re: f32,
    im: f32,
}

impl C32 {
    fn mul(self, o: C32) -> C32 {
        C32 {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
    fn add_re(self, v: f32) -> C32 {
        C32 {
            re: self.re + v,
            ..self
        }
    }
    fn sub_half_sum(self, a: C32, b: C32) -> C32 {
        C32 {
            re: self.re - 0.5 * (a.re + b.re),
            im: self.im - 0.5 * (a.im + b.im),
        }
    }
    fn norm2(self) -> f32 {
        self.re * self.re + self.im * self.im
    }
}

/// Une SDFT (`common/sdft.c`).
struct Sdft {
    idx: usize,
    start: usize,
    end: usize,
    batch_size: usize,
    batches: usize,
    samples: [f32; SDFT_SIZE],
    data: [C32; SDFT_BINS],
}

impl Sdft {
    fn new(start: usize, end: usize, batches: usize) -> Self {
        let batches = batches.max(1);
        Sdft {
            idx: 0,
            start,
            end,
            batch_size: (end - start + 1) / batches,
            batches,
            samples: [0.0; SDFT_SIZE],
            data: [C32::default(); SDFT_BINS],
        }
    }

    /// Ajoute l'échantillon à la part `batch` des cases (la dernière part avance aussi le tampon circulaire).
    fn push_batch(&mut self, tw: &[C32; SDFT_BINS], r_pow_n: f32, sample: f32, batch: usize) {
        let delta = sample - r_pow_n * self.samples[self.idx];
        let lo = self.batch_size * batch + self.start;
        let hi = if batch == self.batches - 1 {
            self.samples[self.idx] = sample;
            self.idx = (self.idx + 1) % SDFT_SIZE;
            self.end + 1
        } else {
            lo + self.batch_size
        };
        let hi = hi.min(SDFT_BINS);
        for (d, w) in self.data[lo..hi].iter_mut().zip(&tw[lo..hi]) {
            *d = w.mul(d.add_re(delta));
        }
        // cases juste hors de la plage, pour la fenêtre des bords
        if self.start > 0 && batch == 0 {
            let i = self.start - 1;
            self.data[i] = tw[i].mul(self.data[i].add_re(delta));
        }
        if self.end < SDFT_BINS - 1 && batch == self.batches - 1 {
            let i = self.end + 1;
            self.data[i] = tw[i].mul(self.data[i].add_re(delta));
        }
    }

    /// Puissance par case avec la fenêtre de Hann appliquée en fréquence : X[k] − (X[k−1] + X[k+1]) / 2.
    fn win_sq(&self, out: &mut [f32; SDFT_BINS]) {
        for (i, o) in out.iter_mut().enumerate().take(self.end + 1).skip(self.start) {
            let lo = if i == 0 { self.data[1] } else { self.data[i - 1] };
            let hi = if i == SDFT_BINS - 1 {
                self.data[i - 1]
            } else {
                self.data[i + 1]
            };
            let v = if i == 0 || i == SDFT_BINS - 1 {
                C32 {
                    re: self.data[i].re - lo.re,
                    im: self.data[i].im - lo.im,
                }
            } else {
                self.data[i].sub_half_sum(lo, hi)
            };
            *o = v.norm2();
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Peak {
    bin: usize,
    value: f32,
}

/// État du suivi : SDFT des trois axes, machine à étapes, centres.
struct State {
    tw: [C32; SDFT_BINS],
    r_pow_n: f32,
    sdft: [Sdft; 3],
    acc: [f32; 3],
    avg: [f32; 3],
    sample_index: usize,
    ticks_left: usize,
    step: usize,
    axis: usize,
    spectrum: [f32; SDFT_BINS],
    noise: f32,
    peaks: [Peak; MAX_NOTCHES],
    centre: [[f32; MAX_NOTCHES]; 3],
}

impl State {
    fn new(t: &DynNotchTracker) -> Self {
        let c = 2.0 * std::f32::consts::PI / SDFT_SIZE as f32;
        let tw = std::array::from_fn(|i| {
            let phi = c * i as f32;
            C32 {
                re: SDFT_R * phi.cos(),
                im: SDFT_R * phi.sin(),
            }
        });
        let init = t.initial_centres();
        let mut centre = [[0.0; MAX_NOTCHES]; 3];
        for row in &mut centre {
            row[..t.count].copy_from_slice(&init);
        }
        State {
            tw,
            r_pow_n: SDFT_R.powi(SDFT_SIZE as i32),
            sdft: std::array::from_fn(|_| Sdft::new(t.start_bin, t.end_bin, t.sample_count)),
            acc: [0.0; 3],
            avg: [0.0; 3],
            sample_index: 0,
            ticks_left: 0,
            step: 0,
            axis: 0,
            spectrum: [0.0; SDFT_BINS],
            noise: 0.0,
            peaks: [Peak::default(); MAX_NOTCHES],
            centre,
        }
    }

    /// Une boucle PID : le gyro de chaque axe entre dans l'accumulateur (`dynNotchPush`), puis `dynNotchUpdate`.
    fn tick(&mut self, t: &DynNotchTracker, x: [f32; 3]) {
        for (acc, v) in self.acc.iter_mut().zip(x) {
            *acc += v;
        }
        if self.sample_index == t.sample_count {
            self.sample_index = 0;
            for a in 0..3 {
                self.avg[a] = self.acc[a] * (1.0 / t.sample_count as f32);
                self.acc[a] = 0.0;
            }
            self.ticks_left = CALC_TICKS;
        }
        for a in 0..3 {
            self.sdft[a].push_batch(&self.tw, self.r_pow_n, self.avg[a], self.sample_index);
        }
        self.sample_index += 1;
        if self.ticks_left > 0 {
            self.process(t);
            self.ticks_left -= 1;
        }
    }

    fn process(&mut self, t: &DynNotchTracker) {
        let (start, end, fl) = (t.start_bin, t.end_bin, t.floor_start_bin);
        match self.step {
            0 => {
                self.sdft[self.axis].win_sq(&mut self.spectrum);
                let range = match t.floor {
                    FloorRule::Inner => start + 1..end,
                    FloorRule::Full => start..end + 1,
                    FloorRule::Above60 { .. } => fl..end + 1,
                };
                self.noise = range.map(|b| self.spectrum[b]).sum();
            }
            1 => {
                let n = t.count;
                self.peaks[..n].fill(Peak::default());
                let mut bin = start + 1;
                while bin < end {
                    let v = self.spectrum[bin];
                    if v > self.spectrum[bin - 1] && v > self.spectrum[bin + 1] {
                        if let Some(p) = (0..n).find(|&p| v > self.peaks[p].value) {
                            self.peaks.copy_within(p..n - 1, p + 1);
                            self.peaks[p] = Peak { bin, value: v };
                        }
                        bin += 1; // une case voisine d'un pic n'en est pas un
                    }
                    bin += 1;
                }
                // tri par case croissante, les pics vides (case 0) restant à la fin
                for p in (1..n).rev() {
                    for k in 0..p {
                        if self.peaks[k].bin > self.peaks[k + 1].bin && self.peaks[k + 1].bin != 0 {
                            self.peaks.swap(k, k + 1);
                        }
                    }
                }
            }
            2 => {
                let mut found = 0i64;
                for p in 0..t.count {
                    let b = self.peaks[p].bin;
                    let counted = match t.floor {
                        FloorRule::Inner | FloorRule::Full => b != 0,
                        FloorRule::Above60 { strict: false } => b >= fl,
                        FloorRule::Above60 { strict: true } => b > fl,
                    };
                    if counted {
                        self.noise -= 0.75 * self.spectrum[b - 1];
                        self.noise -= self.spectrum[b];
                        self.noise -= 0.75 * self.spectrum[b + 1];
                        found += 1;
                    }
                }
                let divisor = match t.floor {
                    FloorRule::Inner => end as i64 - start as i64 - found - 1,
                    FloorRule::Full => end as i64 - start as i64 - found + 1,
                    FloorRule::Above60 { .. } => end as i64 - fl as i64 - found + 1,
                };
                self.noise = self.noise / divisor as f32 * 2.0;
                for p in 0..t.count {
                    let pk = self.peaks[p];
                    if pk.bin == 0 || pk.value <= self.noise {
                        continue;
                    }
                    let (y0, y1, y2) = (
                        self.spectrum[pk.bin - 1],
                        self.spectrum[pk.bin],
                        self.spectrum[pk.bin + 1],
                    );
                    let mut mean_bin = pk.bin as f32;
                    let denom = 2.0 * (y0 - 2.0 * y1 + y2);
                    if denom != 0.0 {
                        mean_bin += (y0 - y2) / denom;
                    }
                    let f = (mean_bin * t.resolution_hz).clamp(t.min_hz, t.max_hz);
                    let mult = (pk.value / self.noise).clamp(1.0, 10.0);
                    let omega = 2.0 * std::f32::consts::PI * SMOOTH_HZ * mult * t.pt1_dt_s;
                    let gain = omega / (omega + 1.0);
                    let c = &mut self.centre[self.axis][p];
                    *c += gain * (f - *c);
                }
            }
            _ => self.axis = (self.axis + 1) % 3,
        }
        self.step = (self.step + 1) % 4;
    }
}

/// Facteur entre la cadence PID et celle du log (`P interval`), au moins 1.
pub fn ticks_per_log_sample(s: &Settings) -> usize {
    let (num, den) = s.p_interval;
    if num <= 0 || den <= 0 {
        return 1;
    }
    ((den as f64 / num as f64).round() as usize).max(1)
}

/// Gyro d'avant le notch dynamique selon le modèle (LPF2, notchs RPM, notchs statiques, LPF1 appliqués au gyro non
/// filtré), pour les trois axes, filtres calculés pour la cadence nominale du log (L-101). `None` sans gyro non filtré.
pub fn modelled_input(log: &FlightLog, s: &Settings) -> Result<Option<Vec<Vec<f64>>>, String> {
    let Some(gu) = &log.gyro_unfilt else { return Ok(None) };
    let fs = s.log_rate_hz();
    let mut out = Vec::with_capacity(gu.k);
    for a in 0..gu.k.min(3) {
        let mut y: Vec<f64> = gu.row(a).iter().map(|&v| v as f64).collect();
        for st in [GyroStage::Lpf2, GyroStage::Rpm, GyroStage::StaticLpf1] {
            y = gyro_stage_at(st, log, s, y, 1, fs)?;
        }
        out.push(y);
    }
    Ok(Some(out))
}

/// Centres du notch dynamique `s` suivis sur tout le log, à partir du gyro modélisé d'avant le notch ([`modelled_input`]).
/// `None` : notch coupé, firmware non pris en charge, ou pas de gyro non filtré.
pub fn track_log(log: &FlightLog, s: &Settings) -> Result<Option<TrackedCentres>, String> {
    let Some(tracker) = DynNotchTracker::new(s) else {
        return Ok(None);
    };
    let Some(x) = modelled_input(log, s)? else {
        return Ok(None);
    };
    let input = [0, 1, 2].map(|a| x.get(a).map(Vec::as_slice));
    let preroll = (PREROLL_S * s.log_rate_hz()).round() as usize;
    Ok(Some(tracker.track(input, ticks_per_log_sample(s), preroll)))
}

/// Écart entre des centres simulés et les centres enregistrés, sur les échantillons où le centre enregistré existe.
#[derive(Clone, Debug, PartialEq)]
pub struct CentreError {
    /// Écart absolu médian (Hz).
    pub median_abs_hz: f64,
    /// 90e centile de l'écart absolu (Hz).
    pub p90_abs_hz: f64,
    /// Écart signé médian (simulé − enregistré, Hz).
    pub median_hz: f64,
    /// Part des échantillons à moins d'une case de la SDFT.
    pub within_bin: f64,
}

/// Contrôle du suivi d'un notch, sur l'axe debug.
#[derive(Clone, Debug, PartialEq)]
pub struct NotchCheck {
    pub notch: usize,
    pub samples: usize,
    /// Centre enregistré médian (Hz).
    pub recorded_hz: f64,
    /// Suivi rejoué sur le gyro enregistré avant le notch (debug `FFT_FREQ`) : contrôle du suivi seul.
    pub from_debug: CentreError,
    /// Suivi rejoué sur le gyro d'avant le notch selon le modèle ([`modelled_input`]) : ce que le simulateur peut faire
    /// sans `FFT_FREQ`. `None` sans gyro non filtré.
    pub from_model: Option<CentreError>,
    /// Centres initiaux fixes, comme le simulateur sans `FFT_FREQ` avant le lot 22c.
    pub initial: CentreError,
}

/// Résultat de [`check_tracking`].
#[derive(Clone, Debug, PartialEq)]
pub struct TrackingCheck {
    pub axis: usize,
    pub tracker: DynNotchTracker,
    pub notches: Vec<NotchCheck>,
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = q * (sorted.len() - 1) as f64;
    let (i, f) = (pos.floor() as usize, pos.fract());
    let next = sorted[(i + 1).min(sorted.len() - 1)];
    sorted[i] + f * (next - sorted[i])
}

fn centre_error(sim: &[f64], rec: &[f64], bin_hz: f64) -> CentreError {
    let mut signed: Vec<f64> = sim.iter().zip(rec).map(|(s, r)| s - r).collect();
    let mut abs: Vec<f64> = signed.iter().map(|e| e.abs()).collect();
    signed.sort_by(f64::total_cmp);
    abs.sort_by(f64::total_cmp);
    let within = abs.iter().filter(|&&e| e <= bin_hz).count() as f64 / abs.len().max(1) as f64;
    CentreError {
        median_abs_hz: percentile(&abs, 0.5),
        p90_abs_hz: percentile(&abs, 0.9),
        median_hz: percentile(&signed, 0.5),
        within_bin: within,
    }
}

/// `check_tracking` : compare, sur `sel` et sur l'axe debug d'un log `FFT_FREQ`, les centres enregistrés par le firmware
/// aux centres du suivi rejoué (depuis le gyro enregistré avant le notch, et depuis le modèle) et aux centres initiaux.
/// `None` : pas de debug `FFT_FREQ`, notch coupé ou firmware non pris en charge.
pub fn check_tracking(log: &FlightLog, sel: std::ops::Range<usize>) -> Result<Option<TrackingCheck>, String> {
    let s = &log.settings;
    let Some(tracker) = DynNotchTracker::new(s) else {
        return Ok(None);
    };
    let Some(recorded) = crate::bf_filters::fft_freq_centres(log, None) else {
        return Ok(None);
    };
    let Some(otb_log::flightlog::GyroDebugTaps::FftFreq { axis, before_dyn_notch }) = log.gyro_debug_taps() else {
        return Ok(None);
    };
    let n = log.n();
    let sel = otb_log::flightlog::clamp_range(sel.start, sel.end, n);
    let d = ticks_per_log_sample(s);
    let preroll = (PREROLL_S * s.log_rate_hz()).round() as usize;
    let dbg: Vec<f64> = before_dyn_notch.iter().map(|&v| v as f64).collect();
    let mut input: [Option<&[f64]>; 3] = [None; 3];
    input[axis] = Some(&dbg);
    let from_debug = tracker.track(input, d, preroll);
    let from_model =
        modelled_input(log, s)?.map(|x| tracker.track([0, 1, 2].map(|a| x.get(a).map(Vec::as_slice)), d, preroll));
    let init = tracker.initial_centres();
    let bin = tracker.resolution_hz as f64;
    let mut notches = Vec::new();
    for (p, &centre0) in init.iter().enumerate().take(recorded.k) {
        let rec = recorded.row(p);
        let idx: Vec<usize> = sel.clone().filter(|&i| rec[i] > 0.0).collect();
        if idx.is_empty() {
            continue;
        }
        let pick = |v: &[f32]| -> Vec<f64> { idx.iter().map(|&i| v[i] as f64).collect() };
        let r: Vec<f64> = idx.iter().map(|&i| rec[i]).collect();
        let mut rs = r.clone();
        rs.sort_by(f64::total_cmp);
        notches.push(NotchCheck {
            notch: p,
            samples: idx.len(),
            recorded_hz: percentile(&rs, 0.5),
            from_debug: centre_error(&pick(from_debug.row(axis, p)), &r, bin),
            from_model: from_model
                .as_ref()
                .map(|m| centre_error(&pick(m.row(axis, p)), &r, bin)),
            initial: centre_error(&vec![centre0 as f64; r.len()], &r, bin),
        });
    }
    Ok(Some(TrackingCheck { axis, tracker, notches }))
}
