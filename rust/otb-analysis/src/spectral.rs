//! Analyses spectrales de `analysis/spectral.py` (D-035, phase 4b-2) : spectre de Welch, carte fréquence × gaz,
//! spectrogramme. Mêmes opérations que Python dans le même ordre ; la FFT, `log10` et `hypot` s'écartent de numpy du dernier
//! bit (L-048), tout le reste (découpage, sommes, tranches de gaz) est identique.
//!
//! Les matrices sont à plat, rangées ligne par ligne.

use otb_dsp::numeric::{gaussian_filter, hann, rfft_rows, uniform_filter1d};
use otb_dsp::pairwise_sum;

pub const EPS: f64 = 1e-12;
/// Nos DSP en dB (densité physique) sont 6,5 dB au-dessus de celles de PIDtoolbox (`spectral.PTB_PSD_OFFSET_DB`).
pub const PTB_PSD_OFFSET_DB: f64 = 6.5;

/// `round()` de Python : au pair le plus proche sur les demi-entiers. Erreur pour NaN et infini (Python lève).
pub fn py_round(x: f64) -> Result<i64, String> {
    if !x.is_finite() || x.abs() >= 9.0e18 {
        return Err(format!("round({x}) : hors entiers"));
    }
    Ok(x.round_ties_even() as i64)
}

/// Fenêtres successives d'un signal : `nseg` lignes de `length` valeurs, et leur indice de début.
pub struct Segments {
    pub data: Vec<f64>,
    pub nseg: usize,
    pub length: usize,
    pub starts: Vec<usize>,
}

impl Segments {
    pub fn row(&self, i: usize) -> &[f64] {
        &self.data[i * self.length..(i + 1) * self.length]
    }
}

/// Masque des fenêtres [début, début + length) qui ne chevauchent aucun raccord (`clear_of_breaks`, D-020). `breaks` est
/// trié (comme `FlightLog.breaks`) : `searchsorted(side="right")` de numpy devient une recherche dichotomique.
pub fn clear_of_breaks(starts: &[i64], length: i64, breaks: Option<&[i64]>) -> Vec<bool> {
    let Some(b) = breaks.filter(|b| !b.is_empty() && !starts.is_empty()) else {
        return vec![true; starts.len()];
    };
    starts
        .iter()
        .map(|&s| {
            // premier raccord strictement après le début : la fenêtre le contient s'il tombe avant sa fin
            let k = b.partition_point(|&v| v <= s);
            let next = if k < b.len() { b[k] } else { i64::MAX };
            next >= s.wrapping_add(length)
        })
        .collect()
}

/// `_segments` : fenêtres de `length` valeurs tous les `hop` (≥ 1) ; celles qui chevauchent un raccord sont écartées.
/// Signal plus court que la fenêtre : aucune fenêtre.
pub fn segments(x: &[f64], length: usize, hop: usize, breaks: Option<&[i64]>) -> Segments {
    let n = x.len();
    if n < length {
        return Segments {
            data: Vec::new(),
            nseg: 0,
            length,
            starts: Vec::new(),
        };
    }
    let all: Vec<usize> = (0..=n - length).step_by(hop).collect();
    let keep = clear_of_breaks(
        &all.iter().map(|&s| s as i64).collect::<Vec<_>>(),
        length as i64,
        breaks,
    );
    let starts: Vec<usize> = all.iter().zip(&keep).filter(|(_, k)| **k).map(|(s, _)| *s).collect();
    let mut data = Vec::with_capacity(starts.len() * length);
    for &s in &starts {
        data.extend_from_slice(&x[s..s + length]);
    }
    Segments {
        nseg: starts.len(),
        data,
        length,
        starts,
    }
}

/// Facteur de recouvrement de PIDtoolbox selon la durée analysée (≤ 20 s, 20–60 s, > 60 s) ; un NaN va au dernier cas.
pub fn ptb_subsample(duration_s: f64, short: i64, mid: i64, long: i64) -> i64 {
    if duration_s <= 20.0 {
        short
    } else if duration_s <= 60.0 {
        mid
    } else {
        long
    }
}

/// Moyenne d'une ligne comme `np.mean(axis=1)` : +0,0 plus la somme par paires, divisée par la longueur (L-042).
fn row_mean(row: &[f64]) -> f64 {
    (0.0 + pairwise_sum(row)) / row.len() as f64
}

/// `np.fft.rfftfreq(n, 1/fs)`.
pub fn rfftfreq(n: usize, fs: f64) -> Vec<f64> {
    let d = 1.0 / fs;
    let val = 1.0 / (n as f64 * d);
    (0..n / 2 + 1).map(|k| k as f64 * val).collect()
}

/// Mode d'un spectre : `"amp"` (amplitude), `"psd"` (densité en dB) ; toute autre chaîne donne la densité sans dB, comme Python.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Amp,
    Psd,
    Other,
}

impl Mode {
    pub fn parse(s: &str) -> Mode {
        match s {
            "amp" => Mode::Amp,
            "psd" => Mode::Psd,
            _ => Mode::Other,
        }
    }
}

/// `_seg_spectra` : spectres unilatéraux de fenêtres déjà découpées (Hann périodique, moyenne retirée). Rend les
/// fréquences et la matrice `nseg × (L / 2 + 1)`. Longueur de fenêtre 0 : erreur (numpy ne sait pas faire une FFT vide).
pub fn seg_spectra(segs: &Segments, fs: f64, mode: Mode) -> Result<(Vec<f64>, Vec<f64>), String> {
    let l = segs.length;
    if l == 0 {
        return Err("Invalid number of FFT data points (0)".into());
    }
    let w = hann(l as i64, false);
    let mut windowed = Vec::with_capacity(segs.nseg * l);
    for i in 0..segs.nseg {
        let row = segs.row(i);
        let mean = row_mean(row);
        windowed.extend(row.iter().zip(&w).map(|(v, wk)| (v - mean) * wk));
    }
    let spec = rfft_rows(&windowed, segs.nseg, l);
    let nf = l / 2 + 1;
    let f = rfftfreq(l, fs);
    let abs = spec.iter().map(|c| c.re.hypot(c.im));
    if mode == Mode::Amp {
        // amplitude d'un sinus (correction de la fenêtre de Hann : somme(w) = L/2)
        let wsum = pairwise_sum(&w);
        return Ok((f, abs.map(|a| a * 2.0 / wsum).collect()));
    }
    let w2: Vec<f64> = w.iter().map(|v| v * v).collect();
    let norm = fs * pairwise_sum(&w2);
    let mut p: Vec<f64> = abs.map(|a| (a * a) / norm).collect();
    // spectre unilatéral : cases doublées sauf le continu et Nyquist (absent si L est impair), comme scipy
    let hi = if l.is_multiple_of(2) { nf.saturating_sub(1) } else { nf };
    if hi > 1 {
        for r in 0..segs.nseg {
            for v in &mut p[r * nf + 1..r * nf + hi] {
                *v *= 2.0;
            }
        }
    }
    Ok((f, p))
}

/// `to_db` : 10·log10(max(p, 1e-12)) ; un NaN reste NaN.
pub fn to_db(p: &[f64]) -> Vec<f64> {
    p.iter()
        .map(|&v| if v.is_nan() { v } else { 10.0 * v.max(EPS).log10() })
        .collect()
}

/// `to_ptb_db` : DSP en dB → échelle de PIDtoolbox.
pub fn to_ptb_db(db: &[f64]) -> Vec<f64> {
    db.iter().map(|v| v - PTB_PSD_OFFSET_DB).collect()
}

/// `smooth_freq` : lissage le long du dernier axe (0 ou moins = aucun ; 1…5 de plus en plus fort).
pub fn smooth_freq(y: &[f64], shape: &[usize], level: i64) -> Vec<f64> {
    if level <= 0 {
        return y.to_vec();
    }
    uniform_filter1d(y, shape, 1 + 2 * level, -1).expect("largeur ≥ 3")
}

/// Spectre 1D (`Spectrum`).
#[derive(Debug)]
pub struct Spectrum {
    pub freqs: Vec<f64>,
    /// dB si `psd`, sinon amplitude.
    pub values: Vec<f64>,
    pub resolution_hz: f64,
}

/// `spectrum` : moyenne de Welch (fenêtres de Hann, recouvrement 50 %). `resolution_hz` fixe la longueur des fenêtres.
pub fn spectrum(
    x: &[f64],
    fs: f64,
    mode: Mode,
    resolution_hz: f64,
    smoothing: i64,
    breaks: Option<&[i64]>,
) -> Result<Spectrum, String> {
    if resolution_hz == 0.0 {
        return Err("float division by zero".into());
    }
    let n = x.len() as i64;
    let l = n.min(64.max(py_round(fs / resolution_hz)?)) as usize;
    let hop = 1.max(l / 2);
    let segs = segments(x, l, hop, breaks);
    if segs.nseg == 0 {
        return Ok(Spectrum {
            freqs: Vec::new(),
            values: Vec::new(),
            resolution_hz,
        });
    }
    let (f, s) = seg_spectra(&segs, fs, mode)?;
    let nf = f.len();
    // moyenne sur l'axe 0 : somme séquentielle des lignes (en partant de +0,0), puis division
    let mut v = vec![0.0; nf];
    for r in 0..segs.nseg {
        for (acc, p) in v.iter_mut().zip(&s[r * nf..(r + 1) * nf]) {
            *acc += p;
        }
    }
    for acc in &mut v {
        *acc /= segs.nseg as f64;
    }
    if mode == Mode::Psd {
        v = to_db(&v);
    }
    let v = smooth_freq(&v, &[nf], smoothing);
    Ok(Spectrum {
        freqs: f,
        values: v,
        resolution_hz: fs / l as f64,
    })
}

/// Carte fréquence × gaz (`FreqThrottle`) : `z` est `nf × nbins`.
#[derive(Debug)]
pub struct FreqThrottle {
    pub freqs: Vec<f64>,
    /// Centre des tranches.
    pub bins: Vec<f64>,
    pub z: Vec<f64>,
    pub counts: Vec<i64>,
}

/// `np.linspace(lo, hi, num)` (extrémité comprise).
fn linspace(lo: f64, hi: f64, num: usize) -> Vec<f64> {
    if num == 0 {
        return Vec::new();
    }
    if num == 1 {
        return vec![lo];
    }
    let step = (hi - lo) / (num - 1) as f64;
    let mut y: Vec<f64> = (0..num).map(|i| i as f64 * step + lo).collect();
    y[num - 1] = hi;
    y
}

/// `astype(int)` de numpy sur x86 : NaN et valeurs hors de i64 donnent `i64::MIN` (Rust saturerait), ensuite `clip`.
fn to_bin(t: f64, nbins: usize) -> usize {
    let k = if t.is_nan() || t >= i64::MAX as f64 || t < i64::MIN as f64 {
        i64::MIN
    } else {
        t as i64
    };
    k.clamp(0, nbins as i64 - 1) as usize
}

/// `freq_throttle` : chaque fenêtre de `seg_ms` est rangée selon la moyenne de `xaxis` ; elle compte aussi pour les deux
/// tranches voisines (comme PIDtoolbox). Erreur si `xaxis` ne donne pas les mêmes fenêtres que `x`.
#[allow(clippy::too_many_arguments)]
pub fn freq_throttle(
    x: &[f64],
    xaxis: &[f64],
    fs: f64,
    mode: Mode,
    seg_ms: f64,
    nbins: usize,
    x_range: (f64, f64),
    breaks: Option<&[i64]>,
) -> Result<FreqThrottle, String> {
    let l = 16.max(py_round(fs * seg_ms / 1000.0)?) as usize;
    let duration = x.len() as f64 / fs;
    let hop = 1.max(l as i64 / ptb_subsample(duration, 5, 3, 1)) as usize;
    let segs = segments(x, l, hop, breaks);
    let (lo, hi) = x_range;
    let edges = linspace(lo, hi, nbins + 1);
    let bins: Vec<f64> = (0..nbins).map(|i| 0.5 * (edges[i] + edges[i + 1])).collect();
    if segs.nseg == 0 {
        let f = rfftfreq(l, fs);
        let nf = f.len();
        return Ok(FreqThrottle {
            z: vec![f64::NAN; nf * nbins],
            freqs: f,
            bins,
            counts: vec![0; nbins],
        });
    }
    let (f, s) = seg_spectra(&segs, fs, mode)?;
    let nf = f.len();
    let xs = segments(xaxis, l, hop, breaks);
    if xs.nseg != segs.nseg {
        return Err("boolean index did not match".into());
    }
    let k: Vec<usize> = (0..xs.nseg)
        .map(|i| to_bin(((row_mean(xs.row(i)) - lo) / (hi - lo) * nbins as f64).floor(), nbins))
        .collect();
    let mut z = vec![0.0; nf * nbins];
    let mut counts = vec![0i64; nbins];
    // accumulation de chaque fenêtre dans sa tranche et ses deux voisines (np.add.at : dans l'ordre des fenêtres)
    for off in [-1i64, 0, 1] {
        for (w, &kw) in k.iter().enumerate() {
            let kk = kw as i64 + off;
            if (0..nbins as i64).contains(&kk) {
                let kk = kk as usize;
                for fi in 0..nf {
                    z[fi * nbins + kk] += s[w * nf + fi];
                }
                counts[kk] += 1;
            }
        }
    }
    for fi in 0..nf {
        for b in 0..nbins {
            z[fi * nbins + b] = if counts[b] == 0 {
                f64::NAN
            } else {
                z[fi * nbins + b] / counts[b] as f64
            };
        }
    }
    if mode == Mode::Psd {
        z = to_db(&z);
    }
    Ok(FreqThrottle {
        freqs: f,
        bins,
        z,
        counts,
    })
}

/// `smooth_map` : lissage gaussien d'une carte `nf × nb` en ignorant les tranches vides (NaN).
pub fn smooth_map(z: &[f64], shape: &[usize], level: i64) -> Vec<f64> {
    if level <= 0 {
        return z.to_vec();
    }
    let mask: Vec<bool> = z.iter().map(|v| !v.is_nan()).collect();
    let zz: Vec<f64> = z.iter().zip(&mask).map(|(v, m)| if *m { *v } else { 0.0 }).collect();
    let mf: Vec<f64> = mask.iter().map(|m| if *m { 1.0 } else { 0.0 }).collect();
    let sig = [1.25 * level as f64, 0.5 * level as f64];
    let num = gaussian_filter(&zz, shape, &sig);
    let den = gaussian_filter(&mf, shape, &sig);
    num.iter()
        .zip(&den)
        .zip(&mask)
        .map(|((n, d), m)| if *m { n / d } else { f64::NAN })
        .collect()
}

/// Spectrogramme : `z` est `nf × nt` (fréquence × temps).
#[derive(Debug)]
pub struct Spectrogram {
    pub times: Vec<f64>,
    pub freqs: Vec<f64>,
    pub z: Vec<f64>,
    pub nf: usize,
    pub nt: usize,
}

/// `freq_time` : spectrogramme en fenêtres de `win_ms`, pas réglable. Colonnes vides (NaN) sur un trou du log (D-020).
pub fn freq_time(
    x: &[f64],
    t: &[f64],
    fs: f64,
    win_ms: f64,
    hop_div: i64,
    freq_smoothing: i64,
    breaks: Option<&[i64]>,
) -> Result<Spectrogram, String> {
    let l = 16.max(py_round(fs * win_ms / 1000.0)?) as usize;
    let hop = 1.max(l as i64 / 1.max(hop_div)) as usize;
    let segs = segments(x, l, hop, None);
    if segs.nseg == 0 {
        return Ok(Spectrogram {
            times: Vec::new(),
            freqs: Vec::new(),
            z: Vec::new(),
            nf: 0,
            nt: 0,
        });
    }
    let (f, p) = seg_spectra(&segs, fs, Mode::Psd)?;
    let (nf, nt) = (f.len(), segs.nseg);
    let db = smooth_freq(&to_db(&p), &[nt, nf], freq_smoothing);
    let starts: Vec<i64> = segs.starts.iter().map(|&s| s as i64).collect();
    let keep = clear_of_breaks(&starts, l as i64, breaks);
    let mut z = vec![0.0; nf * nt];
    for c in 0..nt {
        for r in 0..nf {
            z[r * nt + c] = if keep[c] { db[c * nf + r] } else { f64::NAN };
        }
    }
    if t.is_empty() {
        return Err("index 0 is out of bounds for axis 0 with size 0".into());
    }
    let times = segs.starts.iter().map(|&s| t[(s + l / 2).min(t.len() - 1)]).collect();
    Ok(Spectrogram {
        times,
        freqs: f,
        z,
        nf,
        nt,
    })
}
