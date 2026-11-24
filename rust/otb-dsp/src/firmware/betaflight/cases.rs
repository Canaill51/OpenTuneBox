//! Les cas du banc de référence (`otb-dev firmware-bench`), joués avec les briques Rust : le pendant exact de
//! `rust/otb-dev/bench/betaflight_filters.c`, qui les joue avec les fichiers du firmware. Le test
//! `tests/firmware_vectors.rs` compare les deux au bit près.
//!
//! ⚠️ Un cas changé ici doit l'être aussi dans le programme C, puis les vecteurs refaits par le banc ; sinon le test compare
//! deux expériences différentes.

use super::{Arith, Biquad, BiquadKind, Decimator, Pt1, Pt2, Pt3, RpmNotch, SvfLowpass, SvfNotch, approx, notch_q};

/// Paramètres d'un cas, lus par leur nom (valeurs float32 exactes, portées en f64 par le manifeste).
pub type Params<'a> = &'a dyn Fn(&str) -> Option<f64>;

fn pf(p: Params, name: &str) -> Result<f32, String> {
    p(name)
        .map(|v| v as f32)
        .ok_or_else(|| format!("paramètre manquant : {name}"))
}

fn pu(p: Params, name: &str) -> Result<u32, String> {
    p(name)
        .map(|v| v as u32)
        .ok_or_else(|| format!("paramètre manquant : {name}"))
}

fn input<'a>(inputs: &[&'a [f32]], i: usize) -> Result<&'a [f32], String> {
    inputs.get(i).copied().ok_or_else(|| format!("entrée {i} manquante"))
}

/// `looptime * 1e-6f` du firmware (entier de microsecondes converti en float32, puis produit).
fn dt_of(looptime_us: u32) -> f32 {
    looptime_us as f32 * 1e-6
}

/// Les types de cas qu'une arithmétique sait jouer.
pub fn kinds(arith: Arith) -> &'static [&'static str] {
    if arith.uses_svf() {
        &[
            "sincos",
            "sincosf",
            "gains",
            "notch_q",
            "pt",
            "pt_dyn",
            "svf_lpf",
            "svf_lpf_dyn",
            "svf_notch",
            "svf_notch_dyn",
            "rpm_notch",
            "two_rate",
        ]
    } else {
        &[
            "sincos",
            "gains",
            "notch_q",
            "pt",
            "pt_dyn",
            "biquad_lpf",
            "biquad_lpf_dyn",
            "biquad_notch",
            "biquad_notch_dyn",
            "biquad_rpm",
            "two_rate",
        ]
    }
}

/// Passe-bas d'un type du firmware (1 PT1, 2 biquad ou SVF, 3 PT2, 4 PT3 ; 0 aucun) à `looptime_us`.
enum Lowpass {
    None,
    Pt1(Pt1),
    Pt2(Pt2),
    Pt3(Pt3),
    Biquad(Biquad),
    Svf(SvfLowpass),
}

impl Lowpass {
    fn new(arith: Arith, kind: u32, hz: f32, looptime_us: u32) -> Lowpass {
        let dt = dt_of(looptime_us);
        match kind {
            1 => Lowpass::Pt1(Pt1::new(arith.pt1_gain(hz, dt))),
            2 if arith.uses_svf() => Lowpass::Svf(SvfLowpass::new(hz, dt)),
            2 => Lowpass::Biquad(Biquad::lowpass(arith, hz, looptime_us)),
            3 => Lowpass::Pt2(Pt2::new(arith.pt2_gain(hz, dt))),
            4 => Lowpass::Pt3(Pt3::new(arith.pt3_gain(hz, dt))),
            _ => Lowpass::None,
        }
    }
    fn apply(&mut self, x: f32) -> f32 {
        match self {
            Lowpass::None => x,
            Lowpass::Pt1(f) => f.apply(x),
            Lowpass::Pt2(f) => f.apply(x),
            Lowpass::Pt3(f) => f.apply(x),
            Lowpass::Biquad(f) => f.apply_df2(x),
            Lowpass::Svf(f) => f.apply(x),
        }
    }
}

/// Notch statique (biquad en forme directe II, ou SVF) ; aucun quand le centre vaut 0.
enum StaticNotch {
    None,
    Biquad(Biquad),
    Svf(SvfNotch),
}

impl StaticNotch {
    fn new(arith: Arith, center: f32, cutoff: f32, looptime_us: u32) -> StaticNotch {
        if center <= 0.0 {
            return StaticNotch::None;
        }
        let q = notch_q(center, cutoff);
        if arith.uses_svf() {
            StaticNotch::Svf(SvfNotch::new(center, dt_of(looptime_us), q))
        } else {
            StaticNotch::Biquad(Biquad::new(arith, center, looptime_us, q, BiquadKind::Notch, 1.0))
        }
    }
    fn apply(&mut self, x: f32) -> f32 {
        match self {
            StaticNotch::None => x,
            StaticNotch::Biquad(f) => f.apply_df2(x),
            StaticNotch::Svf(f) => f.apply(x),
        }
    }
}

fn run_pt(arith: Arith, p: Params, inputs: &[&[f32]], dynamic: bool) -> Result<Vec<f32>, String> {
    let x = input(inputs, 0)?;
    let track = if dynamic { Some(input(inputs, 1)?) } else { None };
    let order = pu(p, "order")?;
    let dt = dt_of(pu(p, "looptime_us")?);
    let hz = match track {
        Some(t) => t[0],
        None => pf(p, "hz")?,
    };
    let mut p1 = Pt1::new(arith.pt1_gain(hz, dt));
    let mut p2 = Pt2::new(arith.pt2_gain(hz, dt));
    let mut p3 = Pt3::new(arith.pt3_gain(hz, dt));
    let mut out = Vec::with_capacity(x.len());
    for (i, &v) in x.iter().enumerate() {
        if let Some(t) = track {
            p1.set_gain(arith.pt1_gain(t[i], dt));
            p2.set_gain(arith.pt2_gain(t[i], dt));
            p3.set_gain(arith.pt3_gain(t[i], dt));
        }
        out.push(match order {
            1 => p1.apply(v),
            2 => p2.apply(v),
            _ => p3.apply(v),
        });
    }
    Ok(out)
}

fn run_two_rate(arith: Arith, p: Params, inputs: &[&[f32]]) -> Result<Vec<f32>, String> {
    let x = input(inputs, 0)?;
    let gyro_us = pu(p, "gyro_us")?;
    let denom = pu(p, "denom")?.max(1) as usize;
    let pid_us = gyro_us * denom as u32;
    let lpf2_type = pu(p, "lpf2_type")?;
    let mut lpf2 = Lowpass::new(arith, lpf2_type, pf(p, "lpf2_hz")?, gyro_us);
    let mut notch = StaticNotch::new(arith, pf(p, "notch_center_hz")?, pf(p, "notch_cutoff_hz")?, pid_us);
    let mut lpf1 = Lowpass::new(arith, pu(p, "lpf1_type")?, pf(p, "lpf1_hz")?, pid_us);
    let mut dec = Decimator::new(!matches!(lpf2, Lowpass::None));
    let mut out = Vec::with_capacity(x.len() / denom);
    for (i, &v) in x.iter().enumerate() {
        dec.push(lpf2.apply(v));
        if (i + 1) % denom == 0 {
            let y = notch.apply(dec.take());
            out.push(lpf1.apply(y));
        }
    }
    Ok(out)
}

/// Joue un cas avec les briques Rust de `arith` ; même sortie, dans le même ordre, que le programme C du banc.
pub fn run(arith: Arith, kind: &str, p: Params, inputs: &[&[f32]]) -> Result<Vec<f32>, String> {
    let svf = arith.uses_svf();
    let mut out = Vec::new();
    match kind {
        "sincos" => {
            let a = input(inputs, 0)?;
            out.extend(a.iter().map(|&x| arith.sin(x)));
            out.extend(a.iter().map(|&x| arith.cos(x)));
        }
        "sincosf" if svf => {
            let a = input(inputs, 0)?;
            let sc: Vec<(f32, f32)> = a.iter().map(|&x| approx::sincos_quadrant(x)).collect();
            out.extend(sc.iter().map(|v| v.0));
            out.extend(sc.iter().map(|v| v.1));
        }
        "gains" => {
            let (fc, dt) = (input(inputs, 0)?, input(inputs, 1)?);
            let pairs = || fc.iter().zip(dt);
            out.extend(pairs().map(|(&f, &d)| arith.pt1_gain(f, d)));
            out.extend(pairs().map(|(&f, &d)| arith.pt2_gain(f, d)));
            out.extend(pairs().map(|(&f, &d)| arith.pt3_gain(f, d)));
        }
        "notch_q" => {
            let (c, f) = (input(inputs, 0)?, input(inputs, 1)?);
            out.extend(c.iter().zip(f).map(|(&c, &f)| notch_q(c, f)));
        }
        "pt" => out = run_pt(arith, p, inputs, false)?,
        "pt_dyn" => out = run_pt(arith, p, inputs, true)?,
        "two_rate" => out = run_two_rate(arith, p, inputs)?,
        "svf_lpf" | "svf_lpf_dyn" if svf => {
            let x = input(inputs, 0)?;
            let dt = dt_of(pu(p, "looptime_us")?);
            let track = if kind == "svf_lpf_dyn" {
                Some(input(inputs, 1)?)
            } else {
                None
            };
            let mut f = SvfLowpass::new(track.map_or_else(|| pf(p, "hz"), |t| Ok(t[0]))?, dt);
            for (i, &v) in x.iter().enumerate() {
                if let Some(t) = track {
                    f.update(t[i], dt);
                }
                out.push(f.apply(v));
            }
        }
        "svf_notch" | "svf_notch_dyn" if svf => {
            let x = input(inputs, 0)?;
            let dt = dt_of(pu(p, "looptime_us")?);
            if kind == "svf_notch_dyn" {
                let track = input(inputs, 1)?;
                let q = pf(p, "q")?;
                let mut f = SvfNotch::new(track[0], dt, q);
                for (i, &v) in x.iter().enumerate() {
                    f.update(track[i], dt, q);
                    out.push(f.apply(v));
                }
            } else {
                let center = pf(p, "center_hz")?;
                let mut f = SvfNotch::new(center, dt, notch_q(center, pf(p, "cutoff_hz")?));
                out.extend(x.iter().map(|&v| f.apply(v)));
            }
        }
        "rpm_notch" if svf => {
            let (x0, x1, x2) = (input(inputs, 0)?, input(inputs, 1)?, input(inputs, 2)?);
            let (track, weight) = (input(inputs, 3)?, input(inputs, 4)?);
            let dt = dt_of(pu(p, "looptime_us")?);
            let q = pf(p, "q")?;
            let mut f = RpmNotch::new(pf(p, "init_hz")?, dt, q, 0.0);
            let n = x0.len();
            out = vec![0.0; 3 * n];
            for i in 0..n {
                f.update(track[i], dt, q, weight[i]);
                let mut v = [x0[i], x1[i], x2[i]];
                f.apply(&mut v);
                out[i] = v[0];
                out[n + i] = v[1];
                out[2 * n + i] = v[2];
            }
        }
        "biquad_lpf" | "biquad_lpf_dyn" if !svf => {
            let x = input(inputs, 0)?;
            let looptime = pu(p, "looptime_us")?;
            let track = if kind == "biquad_lpf_dyn" {
                Some(input(inputs, 1)?)
            } else {
                None
            };
            let mut f = Biquad::lowpass(arith, track.map_or_else(|| pf(p, "hz"), |t| Ok(t[0]))?, looptime);
            let df1 = track.is_some() || pu(p, "df1")? != 0;
            for (i, &v) in x.iter().enumerate() {
                if let Some(t) = track {
                    f.update_lowpass(t[i], looptime);
                }
                out.push(if df1 { f.apply_df1(v) } else { f.apply_df2(v) });
            }
        }
        "biquad_notch" | "biquad_notch_dyn" if !svf => {
            let x = input(inputs, 0)?;
            let looptime = pu(p, "looptime_us")?;
            if kind == "biquad_notch_dyn" {
                let track = input(inputs, 1)?;
                let q = pf(p, "q")?;
                let mut f = Biquad::new(arith, track[0], looptime, q, BiquadKind::Notch, 1.0);
                for (i, &v) in x.iter().enumerate() {
                    f.update(track[i], looptime, q, BiquadKind::Notch, 1.0);
                    out.push(f.apply_df1(v));
                }
            } else {
                let center = pf(p, "center_hz")?;
                let q = notch_q(center, pf(p, "cutoff_hz")?);
                let mut f = Biquad::new(arith, center, looptime, q, BiquadKind::Notch, 1.0);
                out.extend(x.iter().map(|&v| f.apply_df2(v)));
            }
        }
        "biquad_rpm" if !svf => {
            let (x, track, weight) = (input(inputs, 0)?, input(inputs, 1)?, input(inputs, 2)?);
            let looptime = pu(p, "looptime_us")?;
            let q = pf(p, "q")?;
            let mut f = Biquad::new(arith, pf(p, "init_hz")?, looptime, q, BiquadKind::Notch, 0.0);
            for (i, &v) in x.iter().enumerate() {
                f.update(track[i], looptime, q, BiquadKind::Notch, weight[i]);
                out.push(f.apply_df1_weighted(v));
            }
        }
        _ => return Err(format!("type de cas inconnu pour {arith:?} : {kind}")),
    }
    Ok(out)
}
