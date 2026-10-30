//! Journal de vol en unités physiques (port de `log/flightlog.py`).
//!
//! Les courbes sont en simple précision (`f32`, 7 chiffres significatifs, bien plus que la résolution des capteurs
//! loggés : moitié moins de mémoire qu'en `f64`) ; le temps `t` reste en `f64` (microseconde sur des vols longs).
//! ⚠️ Une somme cumulée sur tout un log en `f32` perd des chiffres : la faire en `f64` (D-029).
//!
//! Chaque opération reproduit celle de numpy (NEP 50) : un nombre Python mélangé à un tableau `float32` reste en
//! `float32` (converti d'abord, puis opération en `f32`) ; un `float32` mélangé à un `float64` passe en `f64`. Voir L-041.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};

use otb_decoder::file::{DecodedSession, Matrix, Stats, decode_file};
use otb_dsp::pairwise_sum;

use crate::settings::{Settings, parse_settings};
use crate::{KIND_GYRO, KIND_PID, KIND_SKIP, KIND_WRAP, drop_implausible, rows_f32_scaled};

pub const AXES: [&str; 3] = ["Roll", "Pitch", "Yaw"];
/// Boîtes ANGLE (bit 1) et HORIZON (bit 2), mêmes rangs dans toutes les versions.
pub const ANGLE_HORIZON_MASK: i32 = 0b110;
/// ANGLE seul : le feedforward de la boucle de vitesse y est à 0 (pas en HORIZON).
pub const ANGLE_MODE_MASK: i32 = 0b010;
/// Au-delà, consigne calculée par l'autolevel : instrument = commande radio.
pub const MAX_LEVEL_SHARE: f64 = 0.2;

/// `k` canaux de `n` échantillons, rangés canal après canal (le `(k, n)` de numpy).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rows {
    pub k: usize,
    pub n: usize,
    pub data: Vec<f32>,
}

impl Rows {
    pub fn zeros(k: usize, n: usize) -> Rows {
        Rows {
            k,
            n,
            data: vec![0.0; k * n],
        }
    }

    pub fn row(&self, i: usize) -> &[f32] {
        &self.data[i * self.n..(i + 1) * self.n]
    }

    pub fn row_mut(&mut self, i: usize) -> &mut [f32] {
        &mut self.data[i * self.n..(i + 1) * self.n]
    }

    /// Les `k` premiers canaux (`a[:k]`).
    pub fn first(&self, k: usize) -> Rows {
        Rows {
            k,
            n: self.n,
            data: self.data[..k * self.n].to_vec(),
        }
    }

    fn map(&self, f: impl Fn(f32) -> f32) -> Rows {
        Rows {
            k: self.k,
            n: self.n,
            data: self.data.iter().map(|&v| f(v)).collect(),
        }
    }

    fn zip(&self, other: &Rows, f: impl Fn(f32, f32) -> f32) -> Rows {
        Rows {
            k: self.k,
            n: self.n,
            data: self.data.iter().zip(&other.data).map(|(&a, &b)| f(a, b)).collect(),
        }
    }
}

/// Termes PID d'un axe sur une période (`log/pid_terms.py`).
#[derive(Debug, Clone, PartialEq)]
pub struct PidTerms {
    pub p: Vec<f32>,
    pub i: Vec<f32>,
    pub d: Vec<f32>,
    pub f: Vec<f32>,
    /// Non vide : boucle ouverte non mesurable (raison).
    pub note: String,
    /// Signal extérieur à la boucle pour estimer G ; `None` : la consigne.
    pub instrument: Option<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct FlightLog {
    pub path: PathBuf,
    pub session: usize,
    pub settings: Settings,
    pub t: Vec<f64>,
    pub fs: f64,
    pub gyro: Rows,
    pub gyro_unfilt: Option<Rows>,
    pub setpoint: Rows,
    pub rc_command: Rows,
    pub p_term: Rows,
    pub i_term: Rows,
    pub d_term: Rows,
    pub f_term: Rows,
    /// % (0–100)
    pub throttle: Vec<f32>,
    /// % (nombre de moteurs × n)
    pub motors: Rows,
    /// Fréquence de rotation en Hz, si eRPM.
    pub motor_hz: Option<Rows>,
    /// Valeurs brutes (nombre de champs debug × n).
    pub debug: Rows,
    pub vbat: Option<Vec<f32>>,
    pub current: Option<Vec<f32>>,
    pub acc: Option<Rows>,
    /// `flightModeFlags` (masque des boîtes actives).
    pub mode_flags: Option<Vec<i32>>,
    /// (e, 4) : type, ligne, donnée 1, donnée 2.
    pub events: Matrix,
    pub stats: Stats,
    /// Trames écartées parce qu'impossibles (`stats["implausible_frames"]` de Python).
    pub implausible_frames: i64,
    /// Champs `axisP[0]`… présents dans le log.
    pub pid_columns: BTreeSet<String>,
}

// ---------------------------------------------------------------- construction

/// Rôle de chaque colonne principale pour `drop_implausible` : 0 ignorée (`loopIteration`, `time`), 1 gyro ou consigne
/// (rejet au-delà de 4000 °/s), 2 terme PID (rejet au-delà de ±65535), 3 autre (ramenée aux 32 bits seulement).
/// `setpoint[3]` (gaz du mixeur) n'est jamais contrôlée.
fn column_kinds(names: &[String]) -> Vec<u8> {
    names
        .iter()
        .map(|n| {
            if n == "loopIteration" || n == "time" {
                KIND_SKIP
            } else if n.starts_with("gyroADC[")
                || n.starts_with("gyroUnfilt[")
                || (n.starts_with("setpoint[") && n != "setpoint[3]")
            {
                KIND_GYRO
            } else if ["axisP[", "axisI[", "axisD[", "axisF["]
                .iter()
                .any(|p| n.starts_with(p))
            {
                KIND_PID
            } else {
                KIND_WRAP
            }
        })
        .collect()
}

/// Garde les lignes où `keep` est vrai.
fn keep_rows(m: &mut Matrix, keep: &[bool]) {
    let cols = m.cols;
    let mut data = Vec::with_capacity(m.data.len());
    let mut rows = 0;
    for (r, &k) in keep.iter().enumerate() {
        if k {
            data.extend_from_slice(&m.data[r * cols..(r + 1) * cols]);
            rows += 1;
        }
    }
    *m = Matrix { rows, cols, data };
}

/// Accès aux champs de la session, par lots (une passe sur les lignes quel que soit le nombre de colonnes).
struct Fields {
    main: Matrix,
    slow: Matrix,
    main_idx: HashMap<String, usize>,
    slow_names: Vec<String>,
    /// champ → échelle (division float32)
    plan: HashMap<String, f64>,
    /// champ → (lot, ligne dans le lot)
    lots: HashMap<String, (usize, usize)>,
    lot_data: Vec<Vec<f32>>,
}

impl Fields {
    /// (tableau, indice de colonne) d'un champ : trames principales d'abord, puis lentes.
    fn locate(&self, name: &str) -> Option<(&Matrix, usize)> {
        if let Some(&i) = self.main_idx.get(name) {
            return Some((&self.main, i));
        }
        self.slow_names.iter().position(|n| n == name).map(|i| (&self.slow, i))
    }

    fn present(&self, name: &str) -> bool {
        self.locate(name).is_some()
    }

    /// Champ en `f32` divisé par `scale`, ou `None` s'il est absent.
    fn field(&self, name: &str, scale: f64) -> Option<Vec<f32>> {
        if let (Some(&(lot, row)), Some(&s)) = (self.lots.get(name), self.plan.get(name))
            && s == scale
        {
            let n = self.main.rows;
            return Some(self.lot_data[lot][row * n..(row + 1) * n].to_vec());
        }
        let (m, c) = self.locate(name)?;
        Some(rows_f32_scaled(&m.data, m.rows, m.cols, &[c as i64], &[scale]).expect("colonne valide"))
    }

    /// `prefix[0..count]` en `f32` : `None` si aucune voie n'existe, zéros pour une voie absente.
    fn axes(&self, prefix: &str, scale: f64, count: usize) -> Option<Rows> {
        let rows: Vec<Option<Vec<f32>>> = (0..count)
            .map(|i| self.field(&format!("{prefix}[{i}]"), scale))
            .collect();
        if rows.iter().all(Option::is_none) {
            return None;
        }
        let n = self.main.rows;
        let mut out = Rows::zeros(count, n);
        for (i, r) in rows.into_iter().enumerate() {
            if let Some(r) = r {
                out.row_mut(i).copy_from_slice(&r);
            }
        }
        Some(out)
    }

    fn column_i64(&self, name: &str) -> Option<Vec<i64>> {
        let (m, c) = self.locate(name)?;
        Some((0..m.rows).map(|r| m.data[r * m.cols + c]).collect())
    }
}

/// `np.median` d'un tableau de `f64` sans NaN (la moyenne des deux valeurs du milieu si le nombre est pair).
fn median(v: &mut [f64]) -> f64 {
    let n = v.len();
    let cmp = |a: &f64, b: &f64| a.partial_cmp(b).unwrap();
    let (left, mid, _) = v.select_nth_unstable_by(n / 2, cmp);
    let hi = *mid;
    if n % 2 == 1 {
        return hi;
    }
    let lo = left.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lo + hi) / 2.0
}

/// Voie divisée par un nombre Python : le nombre est converti en `f32` d'abord (NEP 50), la division se fait en `f32`.
fn div32(v: f32, d: f64) -> f32 {
    v / d as f32
}

/// FlightLog d'une session, ou `None` si elle ne contient aucune trame exploitable.
pub fn from_session(path: &Path, mut sess: DecodedSession) -> Option<FlightLog> {
    let st = parse_settings(&sess.header.raw);
    let hr: f64 = if st.high_resolution { 10.0 } else { 1.0 };
    let n0 = sess.main.rows;

    // 1. trames impossibles
    let mut n_bad = 0usize;
    if n0 > 0 {
        let kinds = column_kinds(&sess.main_names);
        let bad = drop_implausible(&mut sess.main.data, n0, sess.main.cols, &kinds, hr).expect("noyau");
        n_bad = bad.iter().filter(|&&b| b).count();
        if n_bad > 0 {
            let keep: Vec<bool> = bad.iter().map(|b| !b).collect();
            keep_rows(&mut sess.main, &keep);
            keep_rows(&mut sess.slow, &keep);
            sess.main_kind = sess
                .main_kind
                .iter()
                .zip(&keep)
                .filter(|(_, k)| **k)
                .map(|(v, _)| *v)
                .collect();
        }
    }
    if sess.main.rows < 2 || n_bad as f64 > n0 as f64 / 2.0 {
        return None; // session essentiellement corrompue
    }
    let n = sess.main.rows;

    // 2. champs, convertis en f32 par lots : une passe sur la matrice par lot (D-034)
    let mut main_idx = HashMap::new();
    for (i, name) in sess.main_names.iter().enumerate() {
        main_idx.entry(name.clone()).or_insert(i); // premier indice si un nom est répété, comme `list.index`
    }
    let mut plan_order: Vec<(String, f64)> = Vec::new();
    let mut kept: BTreeSet<String> = BTreeSet::new();
    let mut want = |prefix: &str, count: usize, scale: f64, keep: bool| {
        for i in 0..count {
            let name = format!("{prefix}[{i}]");
            plan_order.push((name.clone(), scale));
            if keep {
                kept.insert(name);
            }
        }
    };
    want("gyroADC", 3, hr, true);
    want("gyroUnfilt", 3, hr, true);
    want("debug", 8, 1.0, true);
    want("rcCommand", 4, 1.0, true);
    for term in ["axisP", "axisI", "axisD", "axisF"] {
        want(term, 3, 1.0, true);
    }
    want("accSmooth", 3, st.acc_1g as f64, true);
    want("setpoint", 4, 1.0, false);
    want("motor", 16, 1.0, false);
    want("eRPM", 16, 1.0, false);
    plan_order.push(("vbatLatest".into(), 1.0));
    plan_order.push(("amperageLatest".into(), 1.0));

    let mut f = Fields {
        main: std::mem::take(&mut sess.main),
        slow: std::mem::take(&mut sess.slow),
        main_idx,
        slow_names: sess.slow_names.clone(),
        plan: plan_order.iter().cloned().collect(),
        lots: HashMap::new(),
        lot_data: Vec::new(),
    };
    for keep_lot in [true, false] {
        let wanted: Vec<&(String, f64)> = plan_order
            .iter()
            .filter(|(name, _)| f.main_idx.contains_key(name) && kept.contains(name) == keep_lot)
            .collect();
        if wanted.is_empty() {
            continue;
        }
        let cols: Vec<i64> = wanted.iter().map(|(name, _)| f.main_idx[name] as i64).collect();
        let scales: Vec<f64> = wanted.iter().map(|(_, s)| *s).collect();
        let lot = rows_f32_scaled(&f.main.data, f.main.rows, f.main.cols, &cols, &scales).expect("colonnes valides");
        let id = f.lot_data.len();
        f.lot_data.push(lot);
        for (row, (name, _)) in wanted.iter().enumerate() {
            f.lots.insert(name.clone(), (id, row));
        }
    }

    // 3. temps et fréquence d'enregistrement
    let time_us: Vec<f64> = (0..n).map(|r| f.main.data[r * f.main.cols + 1] as f64).collect();
    let t: Vec<f64> = time_us.iter().map(|&u| (u - time_us[0]) * 1e-6).collect();
    let mut dt: Vec<f64> = time_us.windows(2).map(|w| w[1] - w[0]).filter(|&d| d > 0.0).collect();
    let fs = if dt.is_empty() {
        st.log_rate_hz()
    } else {
        1e6 / median(&mut dt)
    };
    let zeros3 = Rows::zeros(3, n);

    let gyro = f.axes("gyroADC", hr, 3);
    let mut gyro_unfilt = f.axes("gyroUnfilt", hr, 3);
    let debug = match f.axes("debug", 1.0, 8) {
        None => Rows::zeros(0, n),
        Some(d) => {
            let present = (0..8).filter(|i| f.present(&format!("debug[{i}]"))).max();
            d.first(present.map_or(0, |m| m + 1))
        }
    };
    if gyro_unfilt.is_none() && st.debug_mode == "GYRO_SCALED" && debug.k >= 3 {
        gyro_unfilt = Some(debug.first(3).map(|v| div32(v, hr)));
    }

    let mut throttle: Option<Vec<f32>> = None;
    let setpoint = match f.axes("setpoint", 1.0, 4) {
        Some(sp) => {
            // seuls les axes sont en haute résolution ; setpoint[3] = gaz du mixeur × 1000, jamais × hr (spec §7)
            if f.present("setpoint[3]") {
                throttle = Some(sp.row(3).iter().map(|&v| div32(v, 10.0)).collect());
            }
            sp.first(3).map(|v| div32(v, hr))
        }
        None => zeros3.clone(),
    };
    let mut rc = f.axes("rcCommand", 1.0, 4).unwrap_or_else(|| Rows::zeros(4, n));
    // les quatre voies rcCommand sont en haute résolution aussi, gaz compris (blackbox.c, spec §7)
    rc = rc.map(|v| div32(v, hr));
    let throttle = throttle.unwrap_or_else(|| {
        rc.row(3)
            .iter()
            .map(|&v| (div32(v - 1000.0, 10.0)).clamp(0.0, 100.0))
            .collect()
    });

    // moteurs
    let mut motor_cols: Vec<Vec<f32>> = Vec::new();
    while f.present(&format!("motor[{}]", motor_cols.len())) {
        motor_cols.push(f.field(&format!("motor[{}]", motor_cols.len()), 1.0).expect("présent"));
    }
    let nm = motor_cols.len();
    let motors = if nm > 0 {
        let (lo, hi) = if st.is_dshot() || st.motor_output.1 > 2000 {
            (48.0, 2047.0)
        } else {
            (st.motor_output.0 as f64, st.motor_output.1 as f64)
        };
        let (lo32, span32, hundred) = (lo as f32, (1.0f64).max(hi - lo) as f32, 100.0f64 as f32);
        let mut m = Rows {
            k: nm,
            n,
            data: motor_cols.concat(),
        };
        m.data
            .iter_mut()
            .for_each(|v| *v = (((*v - lo32) / span32) * hundred).clamp(-5.0, 105.0));
        m
    } else {
        Rows::zeros(0, n)
    };

    let mut erpm: Vec<Vec<f32>> = Vec::new();
    for k in 0..nm {
        match f.field(&format!("eRPM[{k}]"), 1.0) {
            Some(c) => erpm.push(c),
            None => {
                erpm.clear();
                break;
            }
        }
    }
    let to_hz = |r: &Rows| {
        let poles2 = st.motor_poles as f64 / 2.0;
        r.map(|v| div32(div32(v * 100.0f64 as f32, 60.0), poles2))
    };
    let motor_hz = if !erpm.is_empty() {
        Some(to_hz(&Rows {
            k: nm,
            n,
            data: erpm.concat(),
        }))
    } else if st.debug_mode == "DSHOT_RPM_TELEMETRY" && debug.k >= nm && nm > 0 {
        Some(to_hz(&debug.first(nm)))
    } else {
        None
    };

    let vbat = f
        .field("vbatLatest", 1.0)
        .map(|v| v.into_iter().map(|x| div32(x, 100.0)).collect::<Vec<_>>());
    let current = f
        .field("amperageLatest", 1.0)
        .map(|v| v.into_iter().map(|x| div32(x, 100.0)).collect::<Vec<_>>());
    let acc = f.axes("accSmooth", st.acc_1g as f64, 3);
    let mode_flags = f
        .column_i64("flightModeFlags")
        .map(|v| v.into_iter().map(|x| x as i32).collect::<Vec<_>>());

    let pid_or_zeros = |prefix: &str| {
        if f.present(&format!("{prefix}[0]")) {
            f.axes(prefix, 1.0, 3)
        } else {
            Some(zeros3.clone())
        }
    };
    let yaw_d = f.field("axisD[2]", 1.0);
    let mut d_term = pid_or_zeros("axisD");
    if let (None, Some(d)) = (&yaw_d, d_term.as_mut()) {
        d.row_mut(2).fill(0.0);
    }

    let stats = sess.stats.clone();
    let pid_columns: BTreeSet<String> = sess
        .main_names
        .iter()
        .filter(|n| {
            ["axisP[", "axisI[", "axisD[", "axisF["]
                .iter()
                .any(|p| n.starts_with(p))
        })
        .cloned()
        .collect();
    Some(FlightLog {
        path: path.to_path_buf(),
        session: sess.index,
        settings: st,
        t,
        fs,
        gyro: gyro.unwrap_or_else(|| zeros3.clone()),
        gyro_unfilt,
        setpoint,
        rc_command: rc,
        p_term: pid_or_zeros("axisP").unwrap_or_else(|| zeros3.clone()),
        i_term: pid_or_zeros("axisI").unwrap_or_else(|| zeros3.clone()),
        d_term: d_term.unwrap_or_else(|| zeros3.clone()),
        f_term: pid_or_zeros("axisF").unwrap_or_else(|| zeros3.clone()),
        throttle,
        motors,
        motor_hz,
        debug,
        vbat,
        current,
        acc,
        mode_flags,
        events: sess.events,
        stats,
        implausible_frames: n_bad as i64,
        pid_columns,
    })
}

/// Charge toutes les sessions d'un fichier `.bbl`/`.bfl`.
pub fn load(path: &Path) -> std::io::Result<Vec<FlightLog>> {
    Ok(decode_file(path, None)?
        .into_iter()
        .filter(|s| s.main.rows > 1)
        .filter_map(|s| from_session(path, s))
        .collect())
}

// ---------------------------------------------------------------- propriétés et méthodes

/// `slice(start, stop)` de Python appliqué à une longueur `n` (bornes ramenées dans `0..=n`).
pub fn clamp_range(start: usize, stop: usize, n: usize) -> Range<usize> {
    let stop = stop.min(n);
    start.min(stop)..stop
}

fn gradient(f: &[f32]) -> Vec<f32> {
    // `np.gradient` d'un tableau 1D `float32` (pas 1, ordre 1 aux bords) : opérations en f32, comme numpy
    let n = f.len();
    let mut out = vec![0.0f32; n];
    out[0] = f[1] - f[0];
    out[n - 1] = f[n - 1] - f[n - 2];
    for i in 1..n - 1 {
        out[i] = (f[i + 1] - f[i - 1]) / 2.0;
    }
    out
}

/// `np.convolve(a, kern, mode="same")` avec un noyau constant `1/w` (somme séquentielle ; numpy passe par BLAS : l'ordre
/// des sommes diffère, donc des écarts de l'ordre de 1e-16 relatif).
fn smooth_same(a: &[f64], w: usize) -> Vec<f64> {
    let kern = 1.0 / w as f64;
    let n = a.len() as isize;
    let left = (w / 2) as isize;
    (0..n)
        .map(|i| {
            let mut s = 0.0;
            for k in 0..w as isize {
                let j = i - left + k;
                if (0..n).contains(&j) {
                    s += a[j as usize] * kern;
                }
            }
            s
        })
        .collect()
}

/// Gain k minimisant |d − k·raw| sur les composantes < ~30 Hz (moyenne glissante).
fn slow_gain(raw: &[f32], d: &[f32], fs: f64) -> f64 {
    let w = 3.max((fs / 60.0) as usize);
    let a = smooth_same(&raw.iter().map(|&v| v as f64).collect::<Vec<_>>(), w);
    let b = smooth_same(&d.iter().map(|&v| v as f64).collect::<Vec<_>>(), w);
    let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).fold(0.0, |s, (p, q)| s + p * q);
    let den = dot(&a, &a);
    if den <= 0.0 {
        return 0.0;
    }
    dot(&a, &b) / den
}

/// Nombre de cellules par la formule du firmware (`sensors/battery.c` : tension / vbatmaxcellvoltage + 1), avec la tension
/// au branchement (`vbatref`), sinon la première tension loggée.
pub fn cell_count(settings: &Settings, vbat: Option<&[f32]>) -> Option<i64> {
    let mut v = settings.get_int("vbatref", 0) as f64 / 100.0;
    if v <= 0.0
        && let Some(&first) = vbat.and_then(|b| b.first())
    {
        v = first as f64;
    }
    if v <= 0.5 {
        return None;
    }
    let cv = settings.get_ints("vbatcellvoltage"); // min, alerte, max (centièmes de volt)
    let vmax = if cv.len() == 3 && cv[2] > 0 {
        cv[2] as f64 / 100.0
    } else {
        4.30
    };
    Some(((v / vmax) as i64 + 1).min(24))
}

impl FlightLog {
    pub fn n(&self) -> usize {
        self.t.len()
    }

    pub fn duration(&self) -> f64 {
        if self.n() > 1 {
            self.t[self.n() - 1] - self.t[0]
        } else {
            0.0
        }
    }

    /// Nom du fichier sans extension, suivi de `.NN` si la session n'est pas la première.
    pub fn name(&self) -> String {
        let stem = self
            .path
            .file_stem()
            .map_or(String::new(), |s| s.to_string_lossy().to_string());
        if self.session > 1 {
            format!("{stem}.{:02}", self.session)
        } else {
            stem
        }
    }

    pub fn n_motors(&self) -> usize {
        self.motors.k
    }

    pub fn pid_sum(&self) -> Rows {
        self.p_term
            .zip(&self.i_term, |a, b| a + b)
            .zip(&self.d_term, |a, b| a + b)
            .zip(&self.f_term, |a, b| a + b)
    }

    /// Erreur PID, convention PIDtoolbox : gyro − consigne (°/s).
    pub fn pid_error(&self) -> Rows {
        self.gyro.zip(&self.setpoint, |a, b| a - b)
    }

    /// Terme D avant ses filtres passe-bas, reconstruit à partir du gyro filtré : D = Kd · (−dgyro/dt), gain ajusté par
    /// moindres carrés sur les composantes lentes. Comparé à Python avec une tolérance, pas au bit près (BLAS).
    pub fn d_term_prefilter(&self) -> Rows {
        let mut out = Rows::zeros(self.d_term.k, self.d_term.n);
        for ax in 0..3 {
            let d = self.d_term.row(ax);
            if !d.iter().any(|&v| v != 0.0) {
                continue;
            }
            let fs32 = self.fs as f32;
            let raw: Vec<f32> = gradient(self.gyro.row(ax)).into_iter().map(|g| -g * fs32).collect();
            let k = slow_gain(&raw, d, self.fs) as f32;
            for (o, r) in out.row_mut(ax).iter_mut().zip(&raw) {
                *o = r * k;
            }
        }
        out
    }

    pub fn cells(&self) -> Option<i64> {
        cell_count(&self.settings, self.vbat.as_deref())
    }

    /// Vrai quand le drone est armé (bit ARM des modes), ou partout si inconnu.
    pub fn armed(&self) -> Vec<bool> {
        match &self.mode_flags {
            None => vec![true; self.n()],
            Some(f) => f.iter().map(|&v| v & 1 != 0).collect(),
        }
    }

    /// Le terme « P », « I », « D » ou « F » de l'axe est-il enregistré (sinon il vaut 0 partout) ?
    pub fn term_present(&self, term: char, ax: usize) -> bool {
        self.pid_columns.contains(&format!("axis{term}[{ax}]"))
    }

    pub fn debug_name(&self) -> &str {
        &self.settings.debug_mode
    }

    /// Termes PID de l'axe sur `sel`, pour mesurer la boucle ouverte (`analysis/loop.py`).
    pub fn pid_terms(&self, ax: usize, sel: Range<usize>) -> PidTerms {
        let pid = self.settings.pids.get(ax);
        let need = |t: char| match t {
            'D' => pid.is_none_or(|p| p.d.max(p.dmax) > 0),
            'F' => pid.is_none_or(|p| p.f > 0),
            _ => true,
        };
        let missing: Vec<String> = ['P', 'I', 'D', 'F']
            .into_iter()
            .filter(|&t| need(t) && !self.term_present(t, ax))
            .map(String::from)
            .collect();
        let note = if missing.is_empty() {
            String::new()
        } else {
            let s = if missing.len() > 1 { "s" } else { "" };
            format!("terme{s} {} non enregistré{s}", missing.join(", "))
        };
        let mut instrument = None;
        if let Some(flags) = &self.mode_flags {
            let sel_flags = &flags[clamp_range(sel.start, sel.end, flags.len())];
            let level = sel_flags.iter().filter(|&&v| v & ANGLE_HORIZON_MASK != 0).count();
            if !sel_flags.is_empty() && level as f64 / sel_flags.len() as f64 > MAX_LEVEL_SHARE {
                let rc = self.rc_command.row(ax);
                let r = clamp_range(sel.start, sel.end, rc.len());
                let mut inst = rc[r.clone()].to_vec();
                if let Some(chirp) = self.chirp_excitation(ax, sel.clone()) {
                    for (a, b) in inst.iter_mut().zip(chirp) {
                        *a += b;
                    }
                }
                instrument = Some(inst);
            }
        }
        let pick = |r: &Rows| {
            let row = r.row(ax);
            row[clamp_range(sel.start, sel.end, row.len())].to_vec()
        };
        PidTerms {
            p: pick(&self.p_term),
            i: pick(&self.i_term),
            d: pick(&self.d_term),
            f: pick(&self.f_term),
            note,
            instrument,
        }
    }

    /// Excitation du chirp de Betaflight sur l'axe `ax` (°/s, avant son filtre d'avance de phase), ou `None` sans le debug
    /// CHIRP. Debug 1 = axe excité (−1 : aucun), debug 3 = excitation × 1000 ; amplitude `chirp_amplitude_<axe>` de l'en-tête.
    pub fn chirp_excitation(&self, ax: usize, sel: Range<usize>) -> Option<Vec<f32>> {
        if self.debug_name() != "CHIRP" || self.debug.k < 4 || ax > 2 {
            return None;
        }
        let key = format!("chirp_amplitude_{}", AXES[ax].to_lowercase());
        let amp = self.settings.raw.get(&key).map_or(0.0, |v| python_float(v));
        let r = clamp_range(sel.start, sel.end, self.n());
        let active: Vec<bool> = self.debug.row(1)[r.clone()].iter().map(|&v| v == ax as f32).collect();
        if amp <= 0.0 || !active.iter().any(|&a| a) {
            return None;
        }
        let (k, amp32) = (1000.0f64 as f32, amp as f32);
        Some(
            self.debug.row(3)[r]
                .iter()
                .zip(&active)
                .map(|(&v, &a)| if a { v / k * amp32 } else { 0.0 })
                .collect(),
        )
    }

    /// Morceaux contigus (sans trou d'enregistrement) du log.
    pub fn segments(&self, max_gap_factor: f64) -> Vec<Range<usize>> {
        let n = self.n();
        if n < 2 {
            #[allow(clippy::single_range_in_vec_init)] // une liste d'un seul morceau, comme `[slice(0, n)]`
            return vec![0..n];
        }
        let thr = max_gap_factor / self.fs;
        let mut bounds = vec![0];
        bounds.extend((0..n - 1).filter(|&i| self.t[i + 1] - self.t[i] > thr).map(|i| i + 1));
        bounds.push(n);
        bounds
            .windows(2)
            .filter(|w| w[1] - w[0] > 1)
            .map(|w| w[0]..w[1])
            .collect()
    }

    /// Raccords de `sel` : indices (comptés depuis le début de `sel`) des échantillons qui suivent un trou.
    pub fn breaks(&self, sel: Range<usize>, max_gap_factor: f64) -> Vec<usize> {
        let t = &self.t[clamp_range(sel.start, sel.end, self.n())];
        let thr = max_gap_factor / self.fs;
        (0..t.len().saturating_sub(1))
            .filter(|&i| t[i + 1] - t[i] > thr)
            .map(|i| i + 1)
            .collect()
    }

    /// (nombre de trous, secondes perdues) dans `sel` : mêmes trous que `segments`.
    pub fn recording_gaps(&self, sel: Range<usize>, max_gap_factor: f64) -> (usize, f64) {
        let t = &self.t[clamp_range(sel.start, sel.end, self.n())];
        let thr = max_gap_factor / self.fs;
        let lost: Vec<f64> = t
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|&d| d > thr)
            .map(|d| d - 1.0 / self.fs)
            .collect();
        (lost.len(), 0.0 + pairwise_sum(&lost)) // `np.sum` : +0,0 plus la somme par paires de tous les éléments
    }

    pub fn index_of(&self, time_s: f64) -> usize {
        let i = self.t.partition_point(|&x| x < time_s); // `np.searchsorted(t, time_s)` (côté gauche)
        i.min(self.n().saturating_sub(1))
    }

    pub fn slice_time(&self, t0: f64, t1: f64) -> Range<usize> {
        self.index_of(t0)..self.index_of(t1) + 1
    }
}

/// `float(v)` de Python (0 si illisible : `float(x or 0)` d'une valeur d'en-tête, qui lèverait une exception sinon).
fn python_float(v: &str) -> f64 {
    otb_decoder::header::py_float(v).unwrap_or(0.0)
}
