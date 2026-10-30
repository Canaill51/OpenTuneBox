//! Curseurs du réglage simplifié Betaflight ⇄ valeurs PID, de `analysis/sliders.py` (D-035, phase 4b-5).
//!
//! `pids_from_sliders` calcule **en flottant 32 bits dans l'ordre du C** (`-ffast-math` : `x / 100` devient `x * 0.01f`, produits
//! invariants réassociés) : chaque opération est arrondie en `f32`, sans FMA, exactement comme les scalaires `np.float32` de
//! Python ; la troncature vers zéro est celle de `constrain(int)`. Résultat identique au bit près.
//!
//! `sliders_from_pids` résout un système de moindres carrés (au plus 12 équations, 6 inconnues) que Python confie à
//! `np.linalg.lstsq` (LAPACK `gelsd`, SVD). Ici une SVD de Jacobi à un côté donne la même solution de norme minimale (même seuil
//! de rang que `lstsq` : `eps × max(lignes, colonnes)` fois la plus grande valeur singulière) ; elle s'écarte de LAPACK de
//! quelques derniers bits, d'où une tolérance (mesurée dans `tests/sliders.rs` et sur le corpus).
//!
//! Les erreurs de Python deviennent des `Err` : `int(NaN)` et `int(inf)` lèvent, un `base` plus court que les axes calculés
//! lève `IndexError`.

use otb_dsp::pairwise_sum;
use otb_log::settings::{AxisPid, Settings};

pub const PID_GAIN_MAX: i64 = 250;
pub const F_GAIN_MAX: i64 = 1000;
pub const LPF_MAX_HZ: i64 = 1000;
pub const DYN_LPF_MAX_HZ: i64 = 1000;
pub const SLIDER_MAX: i64 = 200;
pub const FILTER_SLIDER_MIN: i64 = 10;

/// (P, I, D, F) par défaut par axe, par famille de version (`flight/pid.h`).
const PID_DEFAULTS_4X: [[i64; 4]; 3] = [[45, 80, 40, 120], [47, 84, 46, 125], [45, 80, 0, 120]];
const PID_DEFAULTS_NEW: [[i64; 4]; 3] = [[45, 80, 30, 120], [47, 84, 34, 125], [45, 80, 0, 120]];
/// 4.4 / 4.5 : D de base (`D_MIN_DEFAULT`).
const D_MIN_DEFAULT_4X: [i64; 3] = [30, 34, 0];
/// 2025.12+ : D maximum (`D_MAX_DEFAULT`).
const D_MAX_DEFAULT_NEW: [i64; 3] = [40, 46, 0];

/// Valeur par défaut d'un curseur (identique dans toutes les versions).
pub fn slider_default(key: &str) -> Option<f64> {
    Some(match key {
        "mode" => 2.0,
        "master" | "i_gain" | "d_gain" | "pi_gain" | "ff_gain" | "pitch_d" | "pitch_pi" | "dmax_gain" => 100.0,
        "dterm_filter" | "gyro_filter" => 1.0,
        "dterm_mult" | "gyro_mult" => 100.0,
        _ => return None,
    })
}

/// Les curseurs de PID, dans l'ordre de `PID_SLIDER_KEYS`.
pub const PID_SLIDER_KEYS: [&str; 8] = [
    "master",
    "pi_gain",
    "i_gain",
    "d_gain",
    "ff_gain",
    "pitch_d",
    "pitch_pi",
    "dmax_gain",
];

/// Catégories de l'écran PIDtoolbox → curseur, dans l'ordre de `PTB_CATEGORIES`.
pub const PTB_CATEGORIES: [(&str, &str); 8] = [
    ("Damping", "d_gain"),
    ("Tracking", "pi_gain"),
    ("Stick Resp.", "ff_gain"),
    ("Dynamic D.", "dmax_gain"),
    ("Drift-Wobble", "i_gain"),
    ("Pitch D.", "pitch_d"),
    ("Pitch T.", "pitch_pi"),
    ("Master", "master"),
];

const MODE_NAMES: [&str; 3] = ["OFF", "RP", "RPY"];
const OFF_ON: [&str; 2] = ["OFF", "ON"];

fn is_new(family: &str) -> bool {
    family != "4.4" && family != "4.5"
}

/// PID par défaut du firmware (forme normalisée : D = D de base, Dmax = D maximum).
pub fn default_pids(family: &str) -> Vec<AxisPid> {
    (0..3)
        .map(|ax| {
            if is_new(family) {
                let [p, i, d, f] = PID_DEFAULTS_NEW[ax];
                AxisPid {
                    p,
                    i,
                    d,
                    dmax: d.max(D_MAX_DEFAULT_NEW[ax]),
                    f,
                }
            } else {
                let [p, i, d, f] = PID_DEFAULTS_4X[ax];
                let dmin = D_MIN_DEFAULT_4X[ax];
                AxisPid {
                    p,
                    i,
                    d: if 0 < dmin && dmin < d { dmin } else { d },
                    dmax: d,
                    f,
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------- curseurs → PID

/// `constrain(int)` du firmware : conversion `float → int` par troncature, puis bornes `[0, hi]`. Python : `int(v)` lève pour
/// NaN et l'infini.
fn trunc(v: f32, hi: i64) -> Result<i64, String> {
    if !v.is_finite() {
        return Err(format!("int({v}) : valeur non finie"));
    }
    Ok((v as f64).trunc().clamp(0.0, hi as f64) as i64)
}

/// PID calculés par le firmware à partir des curseurs (`simplified_tuning apply`).
///
/// `sliders` : paires (clé, valeur) ; les valeurs absentes (`None` de Python) sont déjà retirées par l'appelant. Une valeur
/// négative ou NaN est ignorée (curseur à 100 %). Les axes non couverts par le mode gardent les valeurs de `base` (vide ou
/// absent : PID par défaut de la famille).
pub fn pids_from_sliders(
    sliders: &[(&str, f64)],
    family: &str,
    base: Option<&[AxisPid]>,
) -> Result<Vec<AxisPid>, String> {
    // `{**SLIDER_DEFAULTS, **{k: v … si v >= 0}}` : la dernière valeur valide d'une clé l'emporte
    let get = |key: &str| -> f64 {
        sliders
            .iter()
            .rev()
            .find(|(k, v)| *k == key && *v >= 0.0)
            .map(|(_, v)| *v)
            .or_else(|| slider_default(key))
            .unwrap_or(f64::NAN)
    };
    let mut out: Vec<AxisPid> = match base {
        Some(b) if !b.is_empty() => b.to_vec(),
        _ => default_pids(family),
    };
    // -ffast-math : « x / 100.0f » est compilé en « x * 0.01f » (0.01f < 0,01 → 54 devient 53,99…)
    let pct = 0.01f32;
    let scaled = |key: &str| get(key) as f32 * pct;
    let m = scaled("master");
    let pi = scaled("pi_gain");
    let dg = scaled("d_gain");
    let ff = scaled("ff_gain");
    let ig = scaled("i_gain");
    let s = scaled("dmax_gain");
    let new = is_new(family);
    let defaults = if new { &PID_DEFAULTS_NEW } else { &PID_DEFAULTS_4X };
    let mode = get("mode");
    if !mode.is_finite() {
        return Err(format!("int({mode}) : valeur non finie"));
    }
    let mode = mode.trunc();
    // -ffast-math autorise aussi le réassociement : les produits invariants de la boucle sont calculés une fois (M·PI, M·Dg,
    // M·FF), puis × défaut, puis × facteur de tangage. Cet ordre reproduit les arrondis du firmware sur tout le corpus.
    let m_pi = m * pi;
    let m_pi_ig = m_pi * ig;
    let m_dg = m * dg;
    let m_ff = m * ff;
    let axes = if mode > 0.0 { mode.min(2.0) as usize + 1 } else { 0 };
    for ax in 0..axes {
        let (pr, pp) = if ax == 1 {
            (scaled("pitch_d"), scaled("pitch_pi"))
        } else {
            (1.0f32, 1.0f32)
        };
        let [pd, id, dd, fd] = defaults[ax].map(|v| v as f32);
        let p = trunc(pd * m_pi * pp, PID_GAIN_MAX)?;
        let i = trunc(id * m_pi_ig * pp, PID_GAIN_MAX)?;
        let f = trunc(fd * m_ff * pp, F_GAIN_MAX)?;
        let slot = out
            .get(ax)
            .copied()
            .ok_or("IndexError : base plus courte que les axes calculés")?;
        out[ax] = if new {
            let dmax_def = D_MAX_DEFAULT_NEW[ax];
            let d = trunc(dd * m_dg * pr, PID_GAIN_MAX)?;
            let dmax = if dmax_def > 0 {
                let gain = s + (1.0f32 - s) * dd / dmax_def as f32;
                trunc(dmax_def as f32 * m_dg * pr * gain, PID_GAIN_MAX)?
            } else if slot.dmax > slot.d {
                slot.dmax
            } else {
                0
            };
            AxisPid {
                p,
                i,
                d,
                dmax: if dmax > d { dmax } else { d },
                f,
            }
        } else {
            let dmin_def = D_MIN_DEFAULT_4X[ax];
            let dmn = dmin_def as f32;
            let ratio = if dmin_def > 0 {
                1.0f32 + ((dd - dmn) / dmn) * s
            } else {
                1.0f32
            };
            let d_lo_f = dmn * m_dg * pr;
            let d_hi = trunc(d_lo_f * ratio, PID_GAIN_MAX)?;
            let d_lo = trunc(d_lo_f, PID_GAIN_MAX)?;
            AxisPid {
                p,
                i,
                d: if 0 < d_lo && d_lo < d_hi { d_lo } else { d_hi },
                dmax: d_hi,
                f,
            }
        };
    }
    Ok(out)
}

/// Fréquences des filtres imposées par les curseurs de filtres (arithmétique entière du firmware), dans l'ordre de Python.
///
/// Le firmware ne modifie que les filtres déjà actifs (valeur ≠ 0) : avec `settings`, les filtres désactivés restent à 0.
/// ⚠️ Comme Python, les deux fréquences dynamiques « max » lisent `dyn_min_hz` du réglage actuel (seul le test « actif » en est
/// tiré). Le paramètre `family` de Python est ignoré (mêmes valeurs de 4.4 à 2026.6) : il n'existe pas ici.
pub fn filters_from_sliders(gyro_mult: i64, dterm_mult: i64, settings: Option<&Settings>) -> Vec<(&'static str, i64)> {
    let scaled = |default: i64, mult: i64, hi: i64| -> i64 {
        (i128::from(default) * i128::from(mult))
            .div_euclid(100)
            .clamp(0, i128::from(hi)) as i64
    };
    let mut out = vec![
        ("gyro_lpf1_dyn_min_hz", scaled(250, gyro_mult, DYN_LPF_MAX_HZ)),
        ("gyro_lpf1_dyn_max_hz", scaled(500, gyro_mult, DYN_LPF_MAX_HZ)),
        ("gyro_lpf1_static_hz", scaled(250, gyro_mult, DYN_LPF_MAX_HZ)),
        ("gyro_lpf2_static_hz", scaled(500, gyro_mult, LPF_MAX_HZ)),
        ("dterm_lpf1_dyn_min_hz", scaled(75, dterm_mult, DYN_LPF_MAX_HZ)),
        ("dterm_lpf1_dyn_max_hz", scaled(150, dterm_mult, DYN_LPF_MAX_HZ)),
        ("dterm_lpf1_static_hz", scaled(75, dterm_mult, DYN_LPF_MAX_HZ)),
        ("dterm_lpf2_static_hz", scaled(150, dterm_mult, LPF_MAX_HZ)),
    ];
    if let Some(s) = settings {
        let current = [
            s.gyro_lpf1.dyn_min_hz,
            s.gyro_lpf1.dyn_min_hz,
            s.gyro_lpf1.static_hz,
            s.gyro_lpf2.static_hz,
            s.dterm_lpf1.dyn_min_hz,
            s.dterm_lpf1.dyn_min_hz,
            s.dterm_lpf1.static_hz,
            s.dterm_lpf2.static_hz,
        ];
        for ((_, v), cur) in out.iter_mut().zip(current) {
            if cur == 0 {
                *v = 0;
            }
        }
    }
    out
}

// ---------------------------------------------------------------- PID → curseurs (estimation)

/// Solution de norme minimale de `min ‖A·x − b‖` (équivalent de `np.linalg.lstsq(A, b, rcond=None)`) pour six inconnues.
///
/// SVD de Jacobi à un côté : les colonnes de `A` sont tournées deux à deux jusqu'à être orthogonales (`A·V = U·Σ`), puis
/// `x = Σ_j (u_j·b / σ_j) v_j` sur les valeurs singulières au-dessus du seuil de rang de `lstsq`.
fn lstsq6(rows: &[[f64; 6]], b: &[f64]) -> [f64; 6] {
    const N: usize = 6;
    let m = rows.len().max(N); // des lignes nulles en plus ne changent pas la solution
    let mut a = vec![[0.0f64; N]; N.max(m)];
    a[..rows.len()].copy_from_slice(rows);
    let mut v = [[0.0f64; N]; N];
    for (j, row) in v.iter_mut().enumerate() {
        row[j] = 1.0;
    }
    let col = |a: &[[f64; N]], j: usize| a.iter().map(|r| r[j]).collect::<Vec<f64>>();
    for _sweep in 0..60 {
        let mut rotated = false;
        for p in 0..N - 1 {
            for q in p + 1..N {
                let (cp, cq) = (col(&a, p), col(&a, q));
                let alpha: f64 = cp.iter().map(|x| x * x).sum();
                let beta: f64 = cq.iter().map(|x| x * x).sum();
                let gamma: f64 = cp.iter().zip(&cq).map(|(x, y)| x * y).sum();
                if alpha == 0.0 || beta == 0.0 || gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for r in a.iter_mut() {
                    let (x, y) = (r[p], r[q]);
                    r[p] = c * x - s * y;
                    r[q] = s * x + c * y;
                }
                for r in v.iter_mut() {
                    let (x, y) = (r[p], r[q]);
                    r[p] = c * x - s * y;
                    r[q] = s * x + c * y;
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let sigma: Vec<f64> = (0..N)
        .map(|j| col(&a, j).iter().map(|x| x * x).sum::<f64>().sqrt())
        .collect();
    let top = sigma.iter().cloned().fold(0.0f64, f64::max);
    let cutoff = f64::EPSILON * rows.len().max(N) as f64 * top;
    let mut x = [0.0f64; N];
    for j in 0..N {
        if sigma[j] > cutoff {
            let c: f64 = col(&a, j).iter().zip(b).map(|(u, y)| u * y).sum::<f64>() / (sigma[j] * sigma[j]);
            for (k, xk) in x.iter_mut().enumerate() {
                *xk += c * v[k][j];
            }
        }
    }
    x
}

/// Estime les positions des curseurs (en %, flottants) qui reproduiraient ces PID : moindres carrés en logarithme sur P, I, D, F
/// de roulis / tangage / lacet. Le maître multiplie tout : par convention il vaut la moyenne géométrique des échelles PI et D.
pub fn sliders_from_pids(pids: &[AxisPid], family: &str) -> Vec<(&'static str, f64)> {
    let new = is_new(family);
    let defaults = if new { &PID_DEFAULTS_NEW } else { &PID_DEFAULTS_4X };
    // inconnues (log) : 0 mPI, 1 Ig, 2 mDg, 3 mFF, 4 pp, 5 pr
    let mut rows: Vec<[f64; 6]> = Vec::new();
    let mut rhs: Vec<f64> = Vec::new();
    let mut eq = |cols: &[(usize, f64)], value: i64, reference: i64| {
        if value > 0 && reference > 0 {
            let mut r = [0.0f64; 6];
            for &(c, v) in cols {
                r[c] = v;
            }
            rows.push(r);
            rhs.push((value as f64 / reference as f64).ln());
        }
    };
    for (ax, p) in pids.iter().take(3).enumerate() {
        let pitch = if ax == 1 { 1.0 } else { 0.0 };
        let [pd, id, _, fd] = defaults[ax];
        eq(&[(0, 1.0), (4, pitch)], p.p, pd);
        eq(&[(0, 1.0), (1, 1.0), (4, pitch)], p.i, id);
        eq(&[(3, 1.0), (4, pitch)], p.f, fd);
        let d_ref = if new {
            PID_DEFAULTS_NEW[ax][2]
        } else {
            D_MIN_DEFAULT_4X[ax]
        };
        eq(&[(2, 1.0), (5, pitch)], p.d, d_ref);
    }
    let mut est = [0.0f64; 6];
    if !rows.is_empty() {
        let sol = lstsq6(&rows, &rhs);
        // colonnes non contraintes (aucune équation) → 0 (curseur 100 %)
        for (j, e) in est.iter_mut().enumerate() {
            if rows.iter().any(|r| r[j] != 0.0) {
                *e = sol[j];
            }
        }
    }
    let [m_pi, ig, m_dg, m_ff, pp, pr] = est.map(f64::exp);
    let master = (m_pi * m_dg).sqrt();

    // Curseur D max : rapport Dmax/D = 1 + s·(Dref_max/Dref_min − 1) (même forme dans les deux familles)
    let mut ss: Vec<f64> = Vec::new();
    for ax in 0..2 {
        let Some(p) = pids.get(ax) else { continue };
        if p.d <= 0 {
            continue;
        }
        let span = if new {
            D_MAX_DEFAULT_NEW[ax] as f64 / PID_DEFAULTS_NEW[ax][2] as f64 - 1.0
        } else {
            (PID_DEFAULTS_4X[ax][2] - D_MIN_DEFAULT_4X[ax]) as f64 / D_MIN_DEFAULT_4X[ax] as f64
        };
        ss.push(((p.dmax.max(p.d)) as f64 / p.d as f64 - 1.0) / span);
    }
    let dmax = if ss.is_empty() {
        1.0
    } else {
        pairwise_sum(&ss) / ss.len() as f64 // np.mean
    };
    vec![
        ("master", 100.0 * master),
        ("pi_gain", 100.0 * m_pi / master),
        ("i_gain", 100.0 * ig),
        ("d_gain", 100.0 * m_dg / master),
        ("ff_gain", 100.0 * m_ff / master),
        ("pitch_d", 100.0 * pr),
        ("pitch_pi", 100.0 * pp),
        ("dmax_gain", 100.0 * if dmax > 0.0 { dmax } else { 0.0 }), // max(0.0, dmax) : NaN → 0.0
    ]
}

// ---------------------------------------------------------------- vue PIDtoolbox

#[derive(Debug, Clone, PartialEq)]
pub struct SliderView {
    /// Catégorie PTB → fraction (1,0 = 100 %), dans l'ordre de `PTB_CATEGORIES`.
    pub values: Vec<(&'static str, f64)>,
    /// Vrai : curseurs de l'en-tête ; faux : estimation depuis les PID.
    pub from_header: bool,
    /// Les PID de l'en-tête correspondent aux curseurs (ou estimation exacte).
    pub consistent: bool,
}

fn same(a: &AxisPid, b: &AxisPid) -> bool {
    (a.p, a.i, a.d, a.dmax, a.f) == (b.p, b.i, b.d, b.dmax, b.f)
}

/// Les 8 curseurs affichés par PIDtoolbox, en fractions (1,0 = 100).
pub fn slider_view(settings: &Settings) -> Result<SliderView, String> {
    let fam = settings.family();
    let sl: Vec<(&str, f64)> = settings.sliders().into_iter().map(|(k, v)| (k, v as f64)).collect();
    let lookup = |key: &str| sl.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    let (src, calc, from_header): (Vec<(&str, f64)>, Vec<AxisPid>, bool) = if lookup("mode").unwrap_or(0.0) > 0.0 {
        // `float(sl.get(k, défaut))` : une valeur illisible (-1) est gardée telle quelle ici, pas dans `pids_from_sliders`
        let src = PID_SLIDER_KEYS
            .iter()
            .map(|&k| (k, lookup(k).unwrap_or_else(|| slider_default(k).unwrap())))
            .collect();
        let calc = pids_from_sliders(&sl, fam, Some(&settings.pids))?;
        (src, calc, true)
    } else {
        let src = sliders_from_pids(&settings.pids, fam);
        let rounded: Vec<(&str, f64)> = src
            .iter()
            .map(|&(k, v)| {
                if v.is_finite() {
                    Ok((k, v.round_ties_even()))
                } else {
                    Err(format!("round({v}) : valeur non finie"))
                }
            })
            .chain([Ok(("mode", 2.0))])
            .collect::<Result<_, String>>()?;
        let calc = pids_from_sliders(&rounded, fam, None)?;
        (src, calc, false)
    };
    let consistent = calc.iter().zip(&settings.pids).all(|(a, b)| same(a, b));
    let values = PTB_CATEGORIES
        .iter()
        .map(|&(cat, key)| (cat, src.iter().find(|(k, _)| *k == key).unwrap().1 / 100.0))
        .collect();
    Ok(SliderView {
        values,
        from_header,
        consistent,
    })
}

// ---------------------------------------------------------------- commandes CLI

/// Clé interne, nom CLI, minimum, maximum, table de noms des choix.
type CliSlider<'a> = (&'a str, &'a str, i64, i64, Option<&'a [&'a str]>);

/// Lignes CLI Betaflight pour appliquer ces curseurs, suivies de `simplified_tuning apply` et `save`.
///
/// Une valeur absente ou négative est sautée ; NaN et l'infini font lever `round()` en Python (`Err`).
pub fn cli_commands(sliders: &[(&str, f64)], family: &str) -> Result<Vec<String>, String> {
    let dmax_key = if is_new(family) {
        "simplified_d_max_gain"
    } else {
        "simplified_dmax_gain"
    };
    // clé interne, nom CLI, min, max, table de choix
    let spec: [CliSlider; 13] = [
        ("mode", "simplified_pids_mode", 0, 2, Some(&MODE_NAMES)),
        ("master", "simplified_master_multiplier", 0, SLIDER_MAX, None),
        ("i_gain", "simplified_i_gain", 0, SLIDER_MAX, None),
        ("d_gain", "simplified_d_gain", 0, SLIDER_MAX, None),
        ("pi_gain", "simplified_pi_gain", 0, SLIDER_MAX, None),
        ("dmax_gain", dmax_key, 0, SLIDER_MAX, None),
        ("ff_gain", "simplified_feedforward_gain", 0, SLIDER_MAX, None),
        ("pitch_d", "simplified_pitch_d_gain", 0, SLIDER_MAX, None),
        ("pitch_pi", "simplified_pitch_pi_gain", 0, SLIDER_MAX, None),
        ("dterm_filter", "simplified_dterm_filter", 0, 1, Some(&OFF_ON)),
        (
            "dterm_mult",
            "simplified_dterm_filter_multiplier",
            FILTER_SLIDER_MIN,
            SLIDER_MAX,
            None,
        ),
        ("gyro_filter", "simplified_gyro_filter", 0, 1, Some(&OFF_ON)),
        (
            "gyro_mult",
            "simplified_gyro_filter_multiplier",
            FILTER_SLIDER_MIN,
            SLIDER_MAX,
            None,
        ),
    ];
    let mut lines = Vec::new();
    for (key, name, lo, hi, table) in spec {
        let Some(&(_, v)) = sliders.iter().rev().find(|(k, _)| *k == key) else {
            continue;
        };
        if v < 0.0 {
            continue;
        }
        if !v.is_finite() {
            return Err(format!("round({v}) : valeur non finie"));
        }
        let iv = v.round_ties_even().clamp(lo as f64, hi as f64) as usize;
        lines.push(match table {
            Some(t) => format!("set {name} = {}", t[iv]),
            None => format!("set {name} = {iv}"),
        });
    }
    lines.push("simplified_tuning apply".into());
    lines.push("save".into());
    Ok(lines)
}
