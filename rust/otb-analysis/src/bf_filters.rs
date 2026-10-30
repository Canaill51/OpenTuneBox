//! Chaînes de filtres Betaflight de `analysis/bf_filters.py` (D-035, phase 4b-3) : étages biquad d'un filtre, chaînes gyro et
//! D lues dans l'en-tête, réponse en fréquence, retard. La simulation temporelle et `whatif_spectrum` viennent en 4b-4.
//!
//! Les chaînes (liste de filtres avec leur cadence) ne font qu'arithmétique, arrondis et entiers : elles sont identiques à
//! Python au bit près sur toute plate-forme. La réponse, les coefficients et le retard passent par `sin`, `cos`, `atan2`
//! (bibliothèque mathématique du système) et par `np.convolve` (BLAS) : tolérance mesurée (L-048).

use crate::spectral::py_round;
use otb_dsp::{K_LPF, K_NOTCH, K_PT1, Sections, biquad, notch_q, sections_response};
use otb_log::settings::{Lowpass, Settings};
use std::f64::consts::PI;

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

/// `np.clip(x, lo, hi)` : `min(max(x, lo), hi)`, un NaN reste NaN (si `lo > hi`, le résultat est `hi`).
fn clip(x: f64, lo: f64, hi: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    let a = if x > lo { x } else { lo };
    if a < hi { a } else { hi }
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
/// recalcule que les coupures non nulles). Erreur si `v × factor` n'est pas un nombre fini non nul à arrondir.
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
    let kind = match lp.kind {
        "BIQUAD" => Kind::BiquadLpf,
        "PT2" => Kind::Pt2,
        "PT3" => Kind::Pt3,
        _ => Kind::Pt1,
    };
    Some(FilterSpec::new(kind, f))
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

#[cfg(test)]
mod tests {
    use super::unwrap3;

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
