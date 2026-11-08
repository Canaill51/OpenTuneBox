//! Chaînes de filtres Betaflight de `analysis/bf_filters.py` (D-035, phase 4b-3) : étages biquad d'un filtre, chaînes gyro et
//! D lues dans l'en-tête, réponse en fréquence, retard. La simulation temporelle et `whatif_spectrum` (4b-4)
//! suivent plus bas.
//!
//! Les chaînes (liste de filtres avec leur cadence) ne font qu'arithmétique, arrondis et entiers : elles sont identiques à
//! Python au bit près sur toute plate-forme. La réponse, les coefficients et le retard passent par `sin`, `cos`, `atan2`
//! (bibliothèque mathématique du système) et par `np.convolve` (BLAS) : tolérance mesurée (L-048).

use crate::spectral::{Mode, py_round, row_mean, seg_spectra, segments, to_db};
use otb_dsp::{K_LPF, K_NOTCH, K_PT1, Sections, biquad, notch_q, pt1_gain, pt1_track, run_section, sections_response};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::{Lowpass, Settings};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::f64::consts::PI;
use std::ops::Range;

/// Corrections de coupure : n PT1 identiques en cascade → −3 dB à fc (`filter.c`).
pub const CUTOFF_CORRECTION_PT2: f64 = 1.553773974;
pub const CUTOFF_CORRECTION_PT3: f64 = 1.961459177;
/// Coupure maximale d'un passe-bas dans le firmware (`LPF_MAX_HZ`, `DYN_LPF_MAX_HZ`).
pub const LPF_MAX_HZ: i64 = 1000;

/// Q de Butterworth, `1/√2` (calculé comme Python : `1.0 / math.sqrt(2.0)` ≠ la constante `FRAC_1_SQRT_2` d'un bit).
pub fn butterworth_q() -> f64 {
    1.0 / 2.0_f64.sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pt1,
    Pt2,
    Pt3,
    BiquadLpf,
    Notch,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind, String> {
        Ok(match s {
            "pt1" => Kind::Pt1,
            "pt2" => Kind::Pt2,
            "pt3" => Kind::Pt3,
            "biquad_lpf" => Kind::BiquadLpf,
            "notch" => Kind::Notch,
            _ => return Err(format!("type de filtre inconnu : {s}")),
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Pt1 => "pt1",
            Kind::Pt2 => "pt2",
            Kind::Pt3 => "pt3",
            Kind::BiquadLpf => "biquad_lpf",
            Kind::Notch => "notch",
        }
    }
}

/// Un filtre élémentaire : type, coupure ou centre, Q (notch, biquad), poids du notch RPM `y = w·notch(x) + (1−w)·x`.
#[derive(Clone, Debug, PartialEq)]
pub struct FilterSpec {
    pub kind: Kind,
    pub f_hz: f64,
    pub q: f64,
    pub weight: f64,
}

impl FilterSpec {
    /// Q de Butterworth et poids 1, comme les valeurs par défaut de Python.
    pub fn new(kind: Kind, f_hz: f64) -> Self {
        FilterSpec {
            kind,
            f_hz,
            q: butterworth_q(),
            weight: 1.0,
        }
    }

    /// Poids 1 et Q donné (notch dynamique).
    pub fn new_q(kind: Kind, f_hz: f64, q: f64) -> Self {
        FilterSpec {
            kind,
            f_hz,
            q,
            weight: 1.0,
        }
    }

    pub fn with(kind: Kind, f_hz: f64, q: f64, weight: f64) -> Self {
        FilterSpec { kind, f_hz, q, weight }
    }
}

/// Un filtre et la cadence (Hz) à laquelle il tourne.
pub type Chain = Vec<(FilterSpec, f64)>;

/// Élément d'une liste passée à `chain_delay_ms` : un filtre seul (cadence donnée à part) ou un filtre avec sa cadence.
pub enum ChainItem {
    Spec(FilterSpec),
    WithRate(FilterSpec, f64),
}

/// `_as_chain` : un `FilterSpec` seul demande `fs` (erreur sinon).
pub fn as_chain(items: Vec<ChainItem>, fs: Option<f64>) -> Result<Chain, String> {
    items
        .into_iter()
        .map(|it| match it {
            ChainItem::Spec(sp) => fs
                .map(|f| (sp, f))
                .ok_or_else(|| "fs requis pour une liste de FilterSpec".to_string()),
            ChainItem::WithRate(sp, r) => Ok((sp, r)),
        })
        .collect()
}

/// Retard de la chaîne en ms : (retard de groupe, retard de phase).
#[derive(Debug, PartialEq)]
pub struct ChainDelay {
    pub group_ms: f64,
    pub phase_ms: f64,
}

/// `np.clip(x, lo, hi)` : `min(max(x, lo), hi)`, un NaN reste NaN, à égalité la valeur de `x` est gardée (`-0.0` reste `-0.0`)
/// (si `lo > hi`, le résultat est `hi`).
fn clip(x: f64, lo: f64, hi: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    let a = if x < lo { lo } else { x }; // à égalité numpy garde x (zéro signé)
    if a > hi { hi } else { a }
}

/// Étage d'un filtre : (code, fréquence corrigée, q, poids).
pub type Section = (i64, f64, f64, f64);

/// `_sections` : un filtre en étages biquad (PT2 et PT3 : 2 et 3 PT1 à coupure corrigée).
pub fn sections(spec: &FilterSpec) -> Vec<Section> {
    match spec.kind {
        Kind::Pt1 => vec![(K_PT1, spec.f_hz, 0.0, 1.0)],
        Kind::Pt2 => vec![(K_PT1, spec.f_hz * CUTOFF_CORRECTION_PT2, 0.0, 1.0); 2],
        Kind::Pt3 => vec![(K_PT1, spec.f_hz * CUTOFF_CORRECTION_PT3, 0.0, 1.0); 3],
        Kind::BiquadLpf => vec![(K_LPF, spec.f_hz, spec.q, 1.0)],
        Kind::Notch => vec![(K_NOTCH, spec.f_hz, spec.q, spec.weight)],
    }
}

/// `np.convolve(a, v)` (mode complet) : si `v` est plus long, ils s'échangent ; chaque sortie est une somme de produits
/// dans l'ordre croissant des indices, en partant de +0,0 (le `ddot` de BLAS, pour un tout petit tableau).
fn convolve(a: &[f64], v: &[f64]) -> Vec<f64> {
    let (a, v) = if v.len() > a.len() { (v, a) } else { (a, v) };
    (0..a.len() + v.len() - 1)
        .map(|k| {
            let lo = k.saturating_sub(v.len() - 1);
            let hi = k.min(a.len() - 1);
            let mut acc = 0.0;
            for m in lo..=hi {
                acc += a[m] * v[k - m];
            }
            acc
        })
        .collect()
}

/// `coefficients` : `H(z) = b(z⁻¹) / a(z⁻¹)` du filtre à la cadence `fs`, avec `a[0] = 1`.
pub fn coefficients(spec: &FilterSpec, fs: f64) -> (Vec<f64>, Vec<f64>) {
    let (mut b, mut a) = (vec![1.0], vec![1.0]);
    for (kind, f, q, w) in sections(spec) {
        let [b0, b1, b2, a1, a2] = biquad(kind, f, q, w, fs);
        let (bs, as_): (Vec<f64>, Vec<f64>) = if kind == K_PT1 {
            (vec![b0], vec![1.0, a1])
        } else {
            (vec![b0, b1, b2], vec![1.0, a1, a2])
        };
        b = convolve(&b, &bs);
        a = convolve(&a, &as_);
    }
    (b, a)
}

/// `chain_response` : réponse complexe d'une chaîne à cadences mixtes (parties réelle et imaginaire), chaque étage évalué à
/// sa propre cadence. Même noyau qu'`otb_dsp::sections_response` que Python appelle déjà.
pub fn chain_response(chain: &Chain, freqs: &[f64]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let rows: Vec<(Section, f64)> = chain
        .iter()
        .flat_map(|(sp, fs)| sections(sp).into_iter().map(move |s| (s, *fs)))
        .collect();
    let kinds: Vec<i64> = rows.iter().map(|r| r.0.0).collect();
    let fcs: Vec<f64> = rows.iter().map(|r| r.0.1).collect();
    let qs: Vec<f64> = rows.iter().map(|r| r.0.2).collect();
    let ws: Vec<f64> = rows.iter().map(|r| r.0.3).collect();
    let rates: Vec<f64> = rows.iter().map(|r| r.1).collect();
    sections_response(
        &Sections {
            kinds: &kinds,
            fcs: &fcs,
            qs: &qs,
            ws: &ws,
            rates: &rates,
        },
        freqs,
    )
}

/// `freq_response` : une suite de filtres tous à la cadence `fs`.
pub fn freq_response(specs: &[FilterSpec], freqs: &[f64], fs: f64) -> Result<(Vec<f64>, Vec<f64>), String> {
    let chain: Chain = specs.iter().map(|s| (s.clone(), fs)).collect();
    chain_response(&chain, freqs)
}

/// Reste d'une division à la Python (`np.mod` : du signe du diviseur).
fn py_mod(a: f64, b: f64) -> f64 {
    let m = a % b;
    if b == 0.0 {
        return m;
    }
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) { m + b } else { m }
    } else {
        0.0_f64.copysign(b)
    }
}

/// `np.unwrap` de trois valeurs (période 2π).
fn unwrap3(p: [f64; 3]) -> [f64; 3] {
    let period = 2.0 * PI;
    let (high, low) = (period / 2.0, -(period / 2.0));
    let mut pc = [0.0; 2];
    for i in 0..2 {
        let dd = p[i + 1] - p[i];
        let mut ddmod = py_mod(dd - low, period) + low;
        if ddmod == low && dd > 0.0 {
            ddmod = high;
        }
        pc[i] = if dd.abs() < high { 0.0 } else { ddmod - dd };
    }
    [p[0], p[1] + pc[0], p[2] + (pc[0] + pc[1])]
}

/// `chain_delay_ms` : retard de la chaîne à la fréquence `f_hz` (groupe : −dφ/dω ; phase : −φ/ω).
pub fn chain_delay_ms(chain: &Chain, f_hz: f64) -> Result<ChainDelay, String> {
    let a = f_hz * 1e-3;
    let df = if a > 1e-3 { a } else { 1e-3 };
    let f = [f_hz - df, f_hz, f_hz + df];
    let (re, im) = chain_response(chain, &f)?;
    let angle = |i: usize| im[i].atan2(re[i]);
    let phase = unwrap3([angle(0), angle(1), angle(2)]);
    let group = -(phase[2] - phase[0]) / (2.0 * PI * 2.0 * df);
    let ph = -phase[1] / (2.0 * PI * f_hz);
    Ok(ChainDelay {
        group_ms: group * 1e3,
        phase_ms: ph * 1e3,
    })
}

/// Coupure du passe-bas dynamique selon les gaz (0–1), comme le firmware : gaz quantifiés au centième (arrondi au pair).
/// `expo > 0` : courbe `t(1−t)·expo/10 + t` ; `expo = 0` : ancienne loi `max(min, max·t(1 − t²/3)·1,5)`.
pub fn dyn_lpf_cutoff(min_hz: f64, max_hz: f64, expo: f64, throttle_frac: f64) -> f64 {
    let t = (clip(throttle_frac, 0.0, 1.0) * 100.0).round_ties_even() / 100.0;
    if expo > 0.0 {
        let curve = t * (1.0 - t) * (expo / 10.0) + t;
        (max_hz - min_hz) * curve + min_hz
    } else {
        let v = max_hz * (t * (1.0 - t * t / 3.0) * 1.5);
        // np.maximum : un NaN l'emporte
        if v.is_nan() || v > min_hz { v } else { min_hz }
    }
}

/// `scale_lowpass` : coupures multipliées par `factor`, plafonnées à 1000 Hz ; un filtre coupé le reste (le firmware ne
/// recalcule que les coupures non nulles). Erreur si `v × factor` n'est pas un nombre fini (NaN, infini) ; un produit fini
/// énorme est saturé puis plafonné à 1000 Hz.
pub fn scale_lowpass(lp: &Lowpass, factor: f64) -> Result<Lowpass, String> {
    let k = |v: i64| -> Result<i64, String> {
        if v == 0 {
            return Ok(0);
        }
        Ok(LPF_MAX_HZ.min(py_round(v as f64 * factor)?))
    };
    Ok(Lowpass {
        kind: lp.kind,
        static_hz: k(lp.static_hz)?,
        dyn_min_hz: k(lp.dyn_min_hz)?,
        dyn_max_hz: k(lp.dyn_max_hz)?,
        expo: lp.expo,
    })
}

/// `lowpass_off` : passe-bas désactivé (coupures à 0).
pub fn lowpass_off(lp: &Lowpass) -> Lowpass {
    Lowpass {
        kind: lp.kind,
        static_hz: 0,
        dyn_min_hz: 0,
        dyn_max_hz: 0,
        expo: lp.expo,
    }
}

/// `_LPF_KIND` : type d'en-tête (`PT1`, `BIQUAD`, `PT2`, `PT3`) → type de filtre.
fn lpf_kind(lp: &Lowpass) -> Kind {
    match lp.kind {
        "BIQUAD" => Kind::BiquadLpf,
        "PT2" => Kind::Pt2,
        "PT3" => Kind::Pt3,
        _ => Kind::Pt1,
    }
}

/// `_lowpass_spec` : passe-bas statique ou dynamique (`dyn_min > 0` remplace la valeur statique).
fn lowpass_spec(lp: &Lowpass, throttle_frac: f64) -> Option<FilterSpec> {
    let f = if lp.dynamic() {
        dyn_lpf_cutoff(
            lp.dyn_min_hz as f64,
            lp.dyn_max_hz as f64,
            lp.expo as f64,
            throttle_frac,
        )
    } else if lp.static_hz > 0 {
        lp.static_hz as f64
    } else {
        return None;
    };
    Some(FilterSpec::new(lpf_kind(lp), f))
}

/// `_static_notch` : notch statique dont le centre est ramené sous Nyquist comme le firmware.
fn static_notch(center: i64, cutoff: i64, nyquist: f64) -> Option<FilterSpec> {
    let mut center = center;
    if center as f64 > nyquist {
        center = if (cutoff as f64) < nyquist { nyquist as i64 } else { 0 };
    }
    if center <= 0 || cutoff <= 0 || cutoff >= center {
        return None;
    }
    Some(FilterSpec::with(
        Kind::Notch,
        center as f64,
        notch_q(center as f64, cutoff as f64),
        1.0,
    ))
}

/// `rpm_notch_freq_weight` : fréquence et poids d'un notch RPM (harmonique `harmonic`, 0 = fondamentale), pour chaque valeur
/// de `motor_hz`. Erreur si l'indice de l'harmonique sort de la liste des poids (négatif trop grand).
pub fn rpm_notch_freq_weight(
    motor_hz: &[f64],
    harmonic: i64,
    s: &Settings,
    rate_hz: f64,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let max_hz = 0.48 * rate_hz;
    let (min_hz, fade_hz) = (s.rpm_min_hz as f64, s.rpm_fade_hz as f64);
    let denom = if fade_hz > 1e-9 { fade_hz } else { 1e-9 };
    let weights: &[i64] = if s.rpm_weights.is_empty() {
        &[100]
    } else {
        &s.rpm_weights
    };
    let len = weights.len() as i64;
    let hw = if harmonic < len {
        let i = if harmonic < 0 { harmonic + len } else { harmonic };
        *weights
            .get(i as usize)
            .filter(|_| i >= 0)
            .ok_or("list index out of range")?
    } else {
        100
    };
    let hw = clip(hw as f64 / 100.0, 0.0, 1.0);
    let mut f = Vec::with_capacity(motor_hz.len());
    let mut w = Vec::with_capacity(motor_hz.len());
    for &m in motor_hz {
        let fi = clip((harmonic + 1) as f64 * m, min_hz, max_hz);
        let margin = fi - min_hz;
        let fade = if margin < fade_hz { margin / denom } else { 1.0 };
        f.push(fi);
        w.push(hw * fade);
    }
    Ok((f, w))
}

/// `dyn_notch_enabled` : notch dynamique actif si `count > 0` et cadence PID ≥ 2 kHz.
pub fn dyn_notch_enabled(s: &Settings) -> bool {
    s.dyn_notch_count > 0 && s.pid_rate_hz() >= 2000.0 - 1e-6
}

/// `dyn_notch_initial_centres` : centres initiaux du notch dynamique, `(p + 0,5)(max − min)/count + min`.
pub fn dyn_notch_initial_centres(s: &Settings) -> Vec<f64> {
    let n = s.dyn_notch_count;
    let (lo, hi) = (s.dyn_notch_min_hz, s.dyn_notch_max_hz);
    (0..n.max(0))
        .map(|p| (p as f64 + 0.5) * (hi - lo) as f64 / n as f64 + lo as f64)
        .collect()
}

/// Case de la SDFT du firmware à 8 kHz (`dyn_notch_filter.c`).
pub const DYN_NOTCH_BIN_HZ: f64 = 18.5;
/// Borne basse de `dyn_notch_min_hz` dans le firmware (`cli/settings.c`).
pub const DYN_NOTCH_MIN_LIMIT_HZ: f64 = 20.0;

/// `complete_dyn_centres` (D-041) : centres du notch dynamique `s` quand on en ajoute au-delà de ceux enregistrés dans le log.
/// 1. centres enregistrés (`known`) ramenés dans la plage de `s` et limités à son nombre de notchs ;
/// 2. puis les pics prévus (`peaks`, du plus puissant au plus faible) dans la plage, à plus de 1,5 case d'un centre déjà pris ;
/// 3. sans pic pour un notch : son centre initial.
pub fn complete_dyn_centres(known: &[f64], s: &Settings, peaks: &[f64]) -> Vec<f64> {
    let (lo, hi) = (s.dyn_notch_min_hz as f64, s.dyn_notch_max_hz as f64);
    // min(max(v, lo), hi) de Python : max(v, lo) rend lo si lo > v ; min(x, hi) rend hi si hi < x
    let mut c: Vec<f64> = known
        .iter()
        .map(|&v| {
            let a = if lo > v { lo } else { v };
            if hi < a { hi } else { a }
        })
        .collect();
    c.truncate(py_take(c.len(), s.dyn_notch_count));
    let tol = 1.5 * DYN_NOTCH_BIN_HZ;
    for &f in peaks {
        if c.len() as i64 >= s.dyn_notch_count {
            break;
        }
        if lo <= f && f <= hi && c.iter().all(|&v| (f - v).abs() > tol) {
            c.push(f);
        }
    }
    let initial = dyn_notch_initial_centres(s);
    c.extend(initial.iter().skip(c.len()));
    c
}

/// `1e6 / 2 / looptime_us / pid_denom * 0.95` : Nyquist du log, avec la division par zéro de Python en erreur.
fn nyquist(s: &Settings) -> Result<f64, String> {
    if s.looptime_us == 0 || s.pid_denom == 0 {
        return Err("float division by zero".into());
    }
    Ok(1e6 / 2.0 / s.looptime_us as f64 / s.pid_denom as f64 * 0.95)
}

/// `gyro_chain` : chaîne gyro complète dans l'ordre du firmware (LPF2 à la cadence gyro ; à la cadence PID : notchs RPM, notchs
/// statiques 1 et 2, LPF1, notchs dynamiques). `motor_hz` : une fréquence par moteur ; `dyn_notch_hz` : `None` = centres
/// initiaux, liste vide = pas de notch dynamique.
pub fn gyro_chain(
    s: &Settings,
    throttle_frac: f64,
    motor_hz: Option<&[f64]>,
    dyn_notch_hz: Option<&[f64]>,
) -> Result<Chain, String> {
    let (gyro_rate, pid_rate) = (s.gyro_rate_hz(), s.pid_rate_hz());
    let mut chain: Chain = Vec::new();
    if let Some(sp) = lowpass_spec(&s.gyro_lpf2, throttle_frac) {
        chain.push((sp, gyro_rate));
    }
    if let Some(mhz) = motor_hz.filter(|_| s.rpm_harmonics > 0) {
        let q = s.rpm_q as f64 / 100.0;
        let mut fw = Vec::new();
        for h in 0..s.rpm_harmonics {
            fw.push(rpm_notch_freq_weight(mhz, h, s, pid_rate)?);
        }
        for m in 0..mhz.len() {
            for (f, w) in &fw {
                if w[m] > 0.0 {
                    chain.push((FilterSpec::with(Kind::Notch, f[m], q, w[m]), pid_rate));
                }
            }
        }
    }
    let nyq = nyquist(s)?;
    for &(center, cutoff) in s.gyro_notches.iter().take(2) {
        if let Some(sp) = static_notch(center, cutoff, nyq) {
            chain.push((sp, pid_rate));
        }
    }
    if let Some(sp) = lowpass_spec(&s.gyro_lpf1, throttle_frac) {
        chain.push((sp, pid_rate));
    }
    if dyn_notch_enabled(s) {
        let initial;
        let centres: &[f64] = match dyn_notch_hz {
            Some(c) => c,
            None => {
                initial = dyn_notch_initial_centres(s);
                &initial
            }
        };
        let q = s.dyn_notch_q as f64 / 100.0;
        for &c in centres.iter().take(s.dyn_notch_count as usize) {
            if c > 0.0 && q > 0.0 {
                chain.push((FilterSpec::new_q(Kind::Notch, c, q), pid_rate));
            }
        }
    }
    Ok(chain)
}

/// `dterm_chain` : chaîne du terme D (cadence PID) : notch D, LPF1 D, LPF2 D.
pub fn dterm_chain(s: &Settings, throttle_frac: f64) -> Chain {
    let pid_rate = s.pid_rate_hz();
    let mut chain: Chain = Vec::new();
    let (center, cutoff) = s.dterm_notch;
    if let Some(sp) = static_notch(center, cutoff, pid_rate / 2.0) {
        chain.push((sp, pid_rate));
    }
    for lp in [&s.dterm_lpf1, &s.dterm_lpf2] {
        if let Some(sp) = lowpass_spec(lp, throttle_frac) {
            chain.push((sp, pid_rate));
        }
    }
    chain
}

// ---------------------------------------------------------------- simulation temporelle et « et si » (phase 4b-4)

/// Matrice `k × n` de float64 rangée ligne après ligne (le `(k, n)` de numpy).
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix64 {
    pub k: usize,
    pub n: usize,
    pub data: Vec<f64>,
}

impl Matrix64 {
    pub fn row(&self, i: usize) -> &[f64] {
        &self.data[i * self.n..(i + 1) * self.n]
    }

    fn from_rows(r: &Rows) -> Matrix64 {
        Matrix64 {
            k: r.k,
            n: r.n,
            data: r.data.iter().map(|&v| v as f64).collect(),
        }
    }
}

/// `np.maximum` / `np.minimum` : un NaN l'emporte (le `max` de Rust l'ignorerait).
fn np_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

fn np_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// `_apply` : applique un filtre (fréquence et poids constants ou par échantillon) à `k` lignes de `n` valeurs, via ses étages
/// biquad. Une fréquence au-delà de 0,48·fs du log est ignorée (la composante est de toute façon repliée).
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
fn apply(
    x: &[f64],
    k: usize,
    n: usize,
    kind: Kind,
    freq: &[f64],
    fs: f64,
    q: f64,
    weight: &[f64],
) -> Result<Vec<f64>, String> {
    // np.broadcast_to(·, (n,)) : une valeur seule se répète, sinon la longueur doit être n
    let broadcast = |v: &[f64]| -> Result<Vec<f64>, String> {
        if v.len() == n {
            Ok(v.to_vec())
        } else if v.len() == 1 {
            Ok(vec![v[0]; n])
        } else {
            Err(format!("impossible de diffuser {} valeurs vers ({n},)", v.len()))
        }
    };
    let (freq, w) = (broadcast(freq)?, broadcast(weight)?);
    let (corr, reps) = match kind {
        Kind::Pt2 => (CUTOFF_CORRECTION_PT2, 2),
        Kind::Pt3 => (CUTOFF_CORRECTION_PT3, 3),
        _ => (1.0, 1),
    };
    let code = match kind {
        Kind::Pt1 | Kind::Pt2 | Kind::Pt3 => K_PT1,
        Kind::BiquadLpf => K_LPF,
        Kind::Notch => K_NOTCH,
    };
    let limit = 0.48 * fs;
    let f_eff: Vec<f64> = freq.iter().map(|&f| if f < limit { f * corr } else { 0.0 }).collect();
    let mut cur = x.to_vec();
    for _ in 0..reps {
        cur = run_section(&cur, k, code, &f_eff, q, &w, fs, f64::INFINITY)?;
    }
    Ok(cur)
}

/// `_lowpass_sim` : passe-bas simulé (statique, ou dynamique selon les gaz `thr` en fraction 0–1).
fn lowpass_sim(x: Vec<f64>, k: usize, n: usize, lp: &Lowpass, thr: &[f64], fs: f64) -> Result<Vec<f64>, String> {
    if !lp.enabled() {
        return Ok(x);
    }
    let f: Vec<f64> = if lp.dynamic() {
        thr.iter()
            .map(|&t| dyn_lpf_cutoff(lp.dyn_min_hz as f64, lp.dyn_max_hz as f64, lp.expo as f64, t))
            .collect()
    } else {
        vec![lp.static_hz as f64]
    };
    apply(&x, k, n, lpf_kind(lp), &f, fs, butterworth_q(), &[1.0])
}

/// Gaz du journal en fraction 0–1 : `clip(throttle / 100, 0, 1)`.
fn throttle_frac(log: &FlightLog) -> Vec<f64> {
    log.throttle.iter().map(|&t| clip(t as f64 / 100.0, 0.0, 1.0)).collect()
}

/// `fft_freq_centres` : centres des notchs dynamiques lus dans le debug FFT_FREQ, `count × n`, ou `None`. 2025.12+ :
/// `debug[1..count]` ; 4.x : `debug[p]`, mais `debug[3]` contient le gyro (le 4e centre est perdu : 0, notch ignoré).
pub fn fft_freq_centres(log: &FlightLog, settings: Option<&Settings>) -> Option<Matrix64> {
    let s = settings.unwrap_or(&log.settings);
    if s.debug_mode != "FFT_FREQ" || s.dyn_notch_count <= 0 {
        return None;
    }
    let count = s.dyn_notch_count.min(7) as usize;
    let new = matches!(s.family(), "2025.12" | "2026.6");
    let n = log.n();
    let mut data = Vec::with_capacity(count * n);
    for p in 0..count {
        let idx = if new { p + 1 } else { p };
        if (!new && idx == 3) || idx >= log.debug.k {
            data.extend(std::iter::repeat_n(0.0, n));
        } else {
            data.extend(log.debug.row(idx).iter().map(|&v| v as f64));
        }
    }
    Some(Matrix64 { k: count, n, data })
}

/// `simulate_gyro` : rejoue la chaîne de filtres gyro sur `gyro_unfilt` (`3 × n`) → gyro filtré simulé. Mêmes approximations
/// que Python : filtres calculés à la cadence du log, ceux au-delà de 0,48·fs ignorés ; notchs RPM variables (fréquences
/// moteur lissées par le PT1 du firmware) ; LPF dynamique selon les gaz ; notchs dynamiques d'après le debug FFT_FREQ.
pub fn simulate_gyro(log: &FlightLog, settings: Option<&Settings>) -> Result<Matrix64, String> {
    let gu = log.gyro_unfilt.as_ref().ok_or("pas de gyro non filtré dans ce log")?;
    let s = settings.unwrap_or(&log.settings);
    let (fs, k, n) = (log.fs, gu.k, gu.n);
    let thr = throttle_frac(log);
    let mut x = Matrix64::from_rows(gu).data;

    x = lowpass_sim(x, k, n, &s.gyro_lpf2, &thr, fs)?;
    if let Some(mhz) = log.motor_hz.as_ref().filter(|_| s.rpm_harmonics > 0) {
        let q = s.rpm_q as f64 / 100.0;
        let kk = if s.rpm_lpf_hz > 0 {
            pt1_gain(s.rpm_lpf_hz as f64, fs)
        } else {
            1.0
        };
        for m in 0..mhz.k {
            let row: Vec<f64> = mhz.row(m).iter().map(|&v| v as f64).collect();
            let tracked = pt1_track(&row, kk);
            for h in 0..s.rpm_harmonics {
                let (f, w) = rpm_notch_freq_weight(&tracked, h, s, s.pid_rate_hz())?;
                if !w.iter().any(|&v| v > 0.0) {
                    continue;
                }
                x = apply(&x, k, n, Kind::Notch, &f, fs, q, &w)?;
            }
        }
    }
    let nyq = nyquist(s)?;
    for &(center, cutoff) in s.gyro_notches.iter().take(2) {
        if let Some(sp) = static_notch(center, cutoff, nyq) {
            x = apply(&x, k, n, Kind::Notch, &[sp.f_hz], fs, sp.q, &[1.0])?;
        }
    }
    x = lowpass_sim(x, k, n, &s.gyro_lpf1, &thr, fs)?;
    if dyn_notch_enabled(s)
        && let Some(centres) = fft_freq_centres(log, Some(s))
    {
        let q = s.dyn_notch_q as f64 / 100.0;
        for r in 0..centres.k {
            let c = centres.row(r);
            if q > 0.0 && c.iter().any(|&v| v > 0.0) {
                x = apply(&x, k, n, Kind::Notch, c, fs, q, &[1.0])?;
            }
        }
    }
    Ok(Matrix64 { k, n, data: x })
}

/// `simulate_dterm` : dérivée du gyro filtré vue par le terme D, `−dg/dt` passée dans la chaîne D (°/s²). `gyro` : gyro filtré à
/// utiliser (par défaut celui du journal, ou par exemple `simulate_gyro` avec d'autres réglages).
pub fn simulate_dterm(
    log: &FlightLog,
    settings: Option<&Settings>,
    gyro: Option<&Matrix64>,
) -> Result<Matrix64, String> {
    let s = settings.unwrap_or(&log.settings);
    let fs = log.fs;
    let own;
    let g = match gyro {
        Some(g) => g,
        None => {
            own = Matrix64::from_rows(&log.gyro);
            &own
        }
    };
    let (k, n) = (g.k, g.n);
    if n == 0 {
        return Err("index 0 is out of bounds for axis 1 with size 0".into());
    }
    let mut delta = vec![0.0; k * n];
    for r in 0..k {
        let row = g.row(r);
        for i in 1..n {
            delta[r * n + i] = -(row[i] - row[i - 1]) * fs;
        }
        delta[r * n] = if n > 1 { delta[r * n + 1] } else { 0.0 };
    }
    let thr = throttle_frac(log);
    let (center, cutoff) = s.dterm_notch;
    let mut x = delta;
    if let Some(sp) = static_notch(center, cutoff, s.pid_rate_hz() / 2.0) {
        x = apply(&x, k, n, Kind::Notch, &[sp.f_hz], fs, sp.q, &[1.0])?;
    }
    x = lowpass_sim(x, k, n, &s.dterm_lpf1, &thr, fs)?;
    x = lowpass_sim(x, k, n, &s.dterm_lpf2, &thr, fs)?;
    Ok(Matrix64 { k, n, data: x })
}

/// Cible du spectre « et si » : la chaîne gyro seule, ou la chaîne gyro puis la chaîne D (`target == "dterm"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Gyro,
    Dterm,
}

impl Target {
    pub fn parse(s: &str) -> Target {
        if s == "dterm" { Target::Dterm } else { Target::Gyro }
    }
}

/// Spectre enregistré et spectre qu'aurait donné la modification (dB, moyenne des fenêtres).
#[derive(Debug, Clone, PartialEq)]
pub struct WhatIfSpectrum {
    pub freqs: Vec<f64>,
    pub current_db: Vec<f64>,
    pub modified_db: Vec<f64>,
}

/// |H|² actuel et modifié, une valeur par fréquence.
pub type Response2 = (Vec<f64>, Vec<f64>);
/// Clé du cache gyro : nombre de fréquences, gaz arrondis, moteurs et centres des notchs dynamiques arrondis (bits des f64).
pub type GyroKey = (usize, u64, Option<Vec<u64>>, Option<Vec<u64>>);

/// Cache de `whatif_spectrum` (le `dict` de Python) : les réponses de filtres de chaque fenêtre ne sont calculées qu'une fois.
/// ⚠️ Comme Python, il ne sait pas si les réglages ont changé : un cache réutilisé avec d'autres `base` / `mod` rend les
/// anciennes réponses. À partager seulement entre appels sur le même journal, la même période et les mêmes réglages.
/// Depuis D-041 la réponse d'une fenêtre dépend aussi des pics prévus, donc de la période : un cache par (journal, période,
/// réglages actuels, réglages modifiés).
#[derive(Debug, Default)]
pub struct WhatIfCache {
    pub gyro: HashMap<GyroKey, Response2>,
    pub dterm: HashMap<(usize, u64), Response2>,
    /// Pics prévus pour les notchs dynamiques ajoutés : (début, fin de la période, plage min, plage max du notch modifié).
    pub peaks: HashMap<(usize, usize, i64, i64), Vec<f64>>,
}

pub fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// `[…][:count]` de Python (compte négatif : tout sauf les derniers).
fn py_take(len: usize, count: i64) -> usize {
    if count >= 0 {
        (count as usize).min(len)
    } else {
        len.saturating_sub(count.unsigned_abs() as usize)
    }
}

/// `_dyn_peaks` : pics que le notch dynamique `modf` aurait à suivre sur la période `sel` : roll du gyro non filtré × réponse des
/// notchs RPM des réglages actuels (`motors::dyn_notch_targets`), du plus puissant au plus faible. Vide sans gyro non filtré.
fn dyn_peaks(log: &FlightLog, sel: &Range<usize>, base: &Settings, modf: &Settings) -> Result<Vec<f64>, String> {
    let Some(unf) = &log.gyro_unfilt else {
        return Ok(Vec::new());
    };
    let mhz = match &log.motor_hz {
        Some(m) if base.rpm_harmonics > 0 => {
            let rg = clamp(sel, m.n);
            let mut data = Vec::with_capacity(m.k * rg.len());
            for k in 0..m.k {
                data.extend_from_slice(&m.row(k)[rg.clone()]);
            }
            Some(Rows {
                k: m.k,
                n: rg.len(),
                data,
            })
        }
        _ => None,
    };
    let lo = (modf.dyn_notch_min_hz as f64 - 2.0 * DYN_NOTCH_BIN_HZ).max(DYN_NOTCH_MIN_LIMIT_HZ);
    let hi = modf.dyn_notch_max_hz as f64 + 2.0 * DYN_NOTCH_BIN_HZ;
    let x: Vec<f64> = unf.row(0)[clamp(sel, unf.n)].iter().map(|&v| v as f64).collect();
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let t = crate::motors::dyn_notch_targets(
        &x,
        mhz.as_ref(),
        log.fs,
        (lo, hi),
        mhz.as_ref().map(|_| base),
        300.0,
        DYN_NOTCH_BIN_HZ,
        Some(&br),
    )?;
    Ok(t.freqs)
}

/// `_gyro_h2` : |H|² des chaînes gyro actuelle et modifiée pour une fenêtre (gaz `thr`, moteurs et centres arrondis). `peaks` :
/// pics prévus pour les notchs dynamiques ajoutés.
fn gyro_h2(
    f: &[f64],
    base: &Settings,
    modf: &Settings,
    thr: f64,
    motors: Option<&[f64]>,
    centres: Option<&[f64]>,
    peaks: &[f64],
) -> Result<Response2, String> {
    let h2 = |s: &Settings| -> Result<Vec<f64>, String> {
        let dynamic: Option<Vec<f64>> = centres.map(|cs| {
            let mut c: Vec<f64> = cs.iter().copied().filter(|&v| v > 0.0).collect();
            if s.dyn_notch_count != base.dyn_notch_count
                || s.dyn_notch_min_hz != base.dyn_notch_min_hz
                || s.dyn_notch_max_hz != base.dyn_notch_max_hz
            {
                // réglage du notch dynamique modifié : centres enregistrés ramenés dans la nouvelle plage, puis notchs
                // ajoutés sur les pics prévus, sinon à leur centre initial (D-041)
                c = complete_dyn_centres(&c, s, peaks);
            }
            c
        });
        let chain = gyro_chain(s, thr, motors, dynamic.as_deref())?;
        let (re, im) = chain_response(&chain, f)?;
        Ok(re
            .iter()
            .zip(&im)
            .map(|(r, i)| {
                let a = r.hypot(*i);
                a * a
            })
            .collect())
    };
    Ok((h2(base)?, h2(modf)?))
}

/// Intervalle `sel` ramené à une longueur `len` (une tranche Python ne sort jamais du tableau).
fn clamp(sel: &Range<usize>, len: usize) -> Range<usize> {
    let s = sel.start.min(len);
    s..sel.end.min(len).max(s)
}

/// `np.round(x, decimals)` : `rint(x·10^d) / 10^d`.
fn np_round(x: f64, scale: f64) -> f64 {
    (x * scale).round_ties_even() / scale
}

/// `whatif_spectrum` : spectre qu'aurait eu `x_logged` avec les filtres `modf` au lieu de `base`. Plutôt que de rejouer les
/// filtres à la cadence du log (trop basse, repliement), on part du spectre enregistré et, fenêtre par fenêtre (`seg_ms`), on le
/// multiplie par |H_mod|² / |H_base|², les deux chaînes évaluées à la vraie cadence PID avec le gaz, la vitesse des moteurs et
/// les centres des notchs dynamiques de la fenêtre. Réglages inchangés : on retrouve exactement le log. Là où les filtres actuels
/// écrasent tout (|H_base|² < 1 %), on repart du signal avant filtres × |H_mod|².
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
pub fn whatif_spectrum(
    x_logged: &[f64],
    x_prefilt: Option<&[f64]>,
    log: &FlightLog,
    sel: Range<usize>,
    base: &Settings,
    modf: &Settings,
    target: Target,
    seg_ms: f64,
    cache: &mut WhatIfCache,
) -> Result<WhatIfSpectrum, String> {
    let fs = log.fs;
    let l = 32.max(py_round(seg_ms * fs / 1000.0)?) as usize;
    let hop = l / 2;
    let x = &x_logged[clamp(&sel, x_logged.len())];
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let segs = segments(x, l, hop, Some(&br));
    let empty = || WhatIfSpectrum {
        freqs: Vec::new(),
        current_db: Vec::new(),
        modified_db: Vec::new(),
    };
    if segs.nseg == 0 {
        return Ok(empty());
    }
    let (f, p) = seg_spectra(&segs, fs, Mode::Psd)?;
    let nf = f.len();
    let ppf = match x_prefilt {
        Some(xp) => {
            let s2 = segments(&xp[clamp(&sel, xp.len())], l, hop, Some(&br));
            if s2.nseg < segs.nseg {
                return Err("x_prefilt : nombre de fenêtres différent".into());
            }
            Some(seg_spectra(&s2, fs, Mode::Psd)?.1)
        }
        None => None,
    };
    let col = |v: &[f64]| v[clamp(&sel, v.len())].to_vec();
    let thr: Vec<f64> = log.throttle[clamp(&sel, log.throttle.len())]
        .iter()
        .map(|&t| t as f64 / 100.0)
        .collect();
    let rows_sel = |m: &Matrix64| -> Vec<Vec<f64>> { (0..m.k).map(|r| col(m.row(r))).collect() };
    let mhz: Option<Vec<Vec<f64>>> = log.motor_hz.as_ref().map(|m| rows_sel(&Matrix64::from_rows(m)));
    let centres: Option<Vec<Vec<f64>>> = fft_freq_centres(log, None).map(|m| rows_sel(&m));

    let dyn_changed = modf.dyn_notch_count != base.dyn_notch_count
        || modf.dyn_notch_min_hz != base.dyn_notch_min_hz
        || modf.dyn_notch_max_hz != base.dyn_notch_max_hz;
    let mut peaks: Vec<f64> = Vec::new();
    if centres.is_some() && dyn_changed && modf.dyn_notch_count > 0 {
        let pkey = (sel.start, sel.end, modf.dyn_notch_min_hz, modf.dyn_notch_max_hz);
        if let Entry::Vacant(slot) = cache.peaks.entry(pkey) {
            slot.insert(dyn_peaks(log, &sel, base, modf)?);
        }
        peaks = cache.peaks[&pkey].clone();
    }
    // moyennes de chaque fenêtre, calculées en une fois (gaz au centième, moteurs et centres à 5 Hz près)
    let window_means = |v: &[f64]| -> Result<Vec<f64>, String> {
        segs.starts
            .iter()
            .map(|&i| {
                v.get(i..i + l)
                    .map(row_mean)
                    .ok_or("fenêtre hors du signal".to_string())
            })
            .collect()
    };
    let thr_w: Vec<f64> = window_means(&thr)?.into_iter().map(|v| np_round(v, 100.0)).collect();
    let by_five = |rows: &Vec<Vec<f64>>| -> Result<Vec<Vec<f64>>, String> {
        rows.iter()
            .map(|r| {
                Ok(window_means(r)?
                    .into_iter()
                    .map(|v| (v / 5.0).round_ties_even() * 5.0)
                    .collect())
            })
            .collect()
    };
    let mhz_w = mhz.as_ref().map(by_five).transpose()?;
    let cen_w = centres.as_ref().map(by_five).transpose()?;

    let mut out_mod = vec![0.0; segs.nseg * nf];
    for k in 0..segs.nseg {
        let t = thr_w[k];
        let m: Option<Vec<f64>> = mhz_w.as_ref().map(|rows| rows.iter().map(|r| r[k]).collect());
        let c: Option<Vec<f64>> = cen_w.as_ref().map(|rows| rows.iter().map(|r| r[k]).collect());
        let key: GyroKey = (nf, t.to_bits(), m.as_deref().map(bits), c.as_deref().map(bits));
        if let Entry::Vacant(slot) = cache.gyro.entry(key.clone()) {
            slot.insert(gyro_h2(&f, base, modf, t, m.as_deref(), c.as_deref(), &peaks)?);
        }
        let (mut r_cur, mut r_mod) = cache.gyro[&key].clone();
        if target == Target::Dterm {
            let dkey = (nf, t.to_bits());
            if let Entry::Vacant(slot) = cache.dterm.entry(dkey) {
                let d2 = |s: &Settings| -> Result<Vec<f64>, String> {
                    let (re, im) = chain_response(&dterm_chain(s, t), &f)?;
                    Ok(re
                        .iter()
                        .zip(&im)
                        .map(|(r, i)| {
                            let a = r.hypot(*i);
                            a * a
                        })
                        .collect())
                };
                slot.insert((d2(base)?, d2(modf)?));
            }
            let (d_cur, d_mod) = &cache.dterm[&dkey];
            r_cur.iter_mut().zip(d_cur).for_each(|(a, b)| *a *= b);
            r_mod.iter_mut().zip(d_mod).for_each(|(a, b)| *a *= b);
        }
        for j in 0..nf {
            // quotient direct : au creux d'un notch les deux |H|² valent ~1e-30 et un plancher sur le seul dénominateur
            // effondrerait le spectre (−90 dB) même à réglages inchangés (L-024)
            let ratio = if r_cur[j] > 0.0 {
                np_min(r_mod[j] / r_cur[j], 1e3)
            } else {
                1.0
            };
            let mut est = p[k * nf + j] * ratio;
            if let Some(pf) = &ppf {
                // là où les filtres actuels écrasaient le signal et que les nouveaux sont plus faibles, on repart du
                // signal avant filtres
                if r_cur[j] < 1e-2 && r_mod[j] > r_cur[j] {
                    est = np_max(est, pf[k * nf + j] * r_mod[j]);
                }
            }
            out_mod[k * nf + j] = est;
        }
    }
    let mean0 = |m: &[f64]| -> Vec<f64> {
        let mut v = vec![0.0; nf];
        for r in 0..segs.nseg {
            for (acc, val) in v.iter_mut().zip(&m[r * nf..(r + 1) * nf]) {
                *acc += val;
            }
        }
        v.iter().map(|a| a / segs.nseg as f64).collect()
    };
    Ok(WhatIfSpectrum {
        freqs: f,
        current_db: to_db(&mean0(&p)),
        modified_db: to_db(&mean0(&out_mod)),
    })
}

#[cfg(test)]
mod tests {
    use super::{clip, scale_lowpass, unwrap3};
    use otb_log::settings::Lowpass;

    #[test]
    fn clip_garde_x_a_egalite_comme_numpy() {
        // numpy : np.clip(-0.0, 0.0, 1.0) = -0.0 ; np.clip(0.0, -1.0, -0.0) = 0.0 ; NaN reste NaN ; lo > hi → hi
        assert_eq!(clip(-0.0, 0.0, 1.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(clip(0.0, -1.0, -0.0).to_bits(), 0.0f64.to_bits());
        assert!(clip(f64::NAN, 0.0, 1.0).is_nan());
        assert_eq!(clip(5.0, 3.0, 2.0), 2.0);
        assert_eq!(clip(-1.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn scale_lowpass_plafonne_un_facteur_enorme_comme_python() {
        // Python : scale_lowpass(PT1 100 Hz, 1e20) → static_hz = 1000
        let lp = Lowpass {
            kind: "PT1",
            static_hz: 100,
            dyn_min_hz: 0,
            dyn_max_hz: 0,
            expo: 5,
        };
        assert_eq!(scale_lowpass(&lp, 1e20).unwrap().static_hz, 1000);
        assert!(scale_lowpass(&lp, f64::INFINITY).is_err());
    }

    /// Flottant écrit en hexadécimal, octets dans l'ordre de la mémoire (petit-boutiste).
    fn h(s: &str) -> f64 {
        f64::from_bits(u64::from_str_radix(s, 16).unwrap().swap_bytes())
    }

    /// `np.unwrap` (numpy 2.5.4) sur trois valeurs : entrées et sorties relevées chez Python. Les suites « exactement π »
    /// et « π + 1 ulp » ne sortent jamais d'un vrai retard de filtre : sans ce test, la frontière de `unwrap3` ne serait
    /// couverte par rien.
    #[test]
    fn unwrap3_comme_numpy() {
        let pi = "182d4454fb210940";
        let cases: [(&str, [&str; 3], [&str; 3]); 11] = [
            (
                "π exact montant",
                ["0000000000000000", pi, "0000000000000000"],
                ["0000000000000000", pi, "0000000000000000"],
            ),
            (
                "π exact descendant",
                ["0000000000000000", "182d4454fb2109c0", "0000000000000000"],
                ["0000000000000000", "182d4454fb2109c0", "0000000000000000"],
            ),
            (
                "π exact deux fois",
                ["0000000000000000", pi, "182d4454fb211940"],
                ["0000000000000000", pi, "182d4454fb211940"],
            ),
            (
                "saut de 4",
                ["0000000000000000", "0000000000001040", "0000000000000000"],
                ["0000000000000000", "305a88a8f64302c0", "0000000000000000"],
            ),
            (
                "saut de 6",
                ["0000000000000840", "00000000000008c0", "0000000000000840"],
                ["0000000000000840", "305a88a8f6430a40", "0000000000000840"],
            ),
            (
                "variation lente",
                ["9a9999999999b93f", "9a9999999999c93f", "333333333333d33f"],
                ["9a9999999999b93f", "9a9999999999c93f", "333333333333d33f"],
            ),
            (
                "multiples de 2π",
                ["0000000000000000", "182d4454fb211940", "182d4454fb212940"],
                ["0000000000000000", "0000000000000000", "0000000000000000"],
            ),
            (
                "π − 1 ulp",
                ["0000000000000000", "172d4454fb210940", "0000000000000000"],
                ["0000000000000000", "172d4454fb210940", "0000000000000000"],
            ),
            (
                "π + 1 ulp",
                ["0000000000000000", "192d4454fb210940", "0000000000000000"],
                ["0000000000000000", pi, "182d4454fb211940"],
            ),
            (
                "NaN",
                ["0000000000000000", "000000000000f87f", "000000000000f03f"],
                ["0000000000000000", "000000000000f87f", "000000000000f87f"],
            ),
            (
                "zéros négatifs",
                ["0000000000000080", "0000000000000080", "0000000000000080"],
                ["0000000000000080", "0000000000000000", "0000000000000000"],
            ),
        ];
        for (name, input, want) in cases {
            let got = unwrap3([h(input[0]), h(input[1]), h(input[2])]);
            for i in 0..3 {
                let w = h(want[i]);
                assert!(
                    got[i].to_bits() == w.to_bits() || (got[i].is_nan() && w.is_nan()),
                    "{name} : élément {i} : Rust {:016x} / numpy {:016x}",
                    got[i].to_bits(),
                    w.to_bits()
                );
            }
        }
    }
}
