//! Décodage d'un fichier Blackbox de bout en bout (port de `decoder/reader.py`) : sessions, paramètres tirés de l'en-tête,
//! décodage des trames (`decode_stream`), remplissage des trames lentes, statistiques.

use std::path::Path;

use crate::header::{FrameDef as HeaderFrameDef, LogHeader, find_log_starts, parse_header};
use crate::{
    CTX_DATA_VERSION, CTX_HOME0_IDX, CTX_HOME1_IDX, CTX_MINTHROTTLE, CTX_MOTOR_MIN, CTX_MOTOR0_IDX, CTX_SIZE,
    CTX_VBATREF, DecodeError, FrameDef, OutOfMemory, Params, Table, decode_stream,
};

/// Tableau d'entiers rangé ligne après ligne.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<i64>,
}

impl Matrix {
    pub fn row(&self, i: usize) -> &[i64] {
        &self.data[i * self.cols..(i + 1) * self.cols]
    }

    /// Les `cols` premières colonnes d'une table plus large (`a[:, :cols]` de numpy).
    fn from_table(t: &Table, rows: usize, cols: usize) -> Matrix {
        let mut data = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            data.extend_from_slice(&t.data[r * t.cols..r * t.cols + cols]);
        }
        Matrix { rows, cols, data }
    }
}

/// Statistiques du décodage (`stats` de `DecodedSession`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub corrupt_frames: i64,
    pub desync_frames: i64,
    pub skipped_iterations: i64,
    pub log_end_seen: bool,
    pub bytes: i64,
    pub i_frames: i64,
    pub p_frames: i64,
}

/// Une session décodée : valeurs brutes (unités du firmware), sans conversion.
#[derive(Debug, Clone)]
pub struct DecodedSession {
    /// Numéro de session dans le fichier (1, 2, …).
    pub index: usize,
    pub header: LogHeader,
    pub main_names: Vec<String>,
    /// (n, champs) : trames I/P valides, dans l'ordre.
    pub main: Matrix,
    /// 0 = I, 1 = P.
    pub main_kind: Vec<i8>,
    pub slow_names: Vec<String>,
    /// (n, champs lents) : dernière trame S connue à chaque ligne principale.
    pub slow: Matrix,
    pub gps_names: Vec<String>,
    pub gps: Matrix,
    pub gps_at: Vec<i64>,
    /// (e, 4) : type, ligne principale, donnée 1, donnée 2.
    pub events: Matrix,
    pub stats: Stats,
}

impl DecodedSession {
    /// Colonne `name` des trames principales, sinon des trames lentes (`column` de Python).
    pub fn column(&self, name: &str) -> Option<Vec<i64>> {
        let pick = |names: &[String], m: &Matrix| {
            names
                .iter()
                .position(|n| n == name)
                .map(|c| (0..m.rows).map(|r| m.data[r * m.cols + c]).collect())
        };
        pick(&self.main_names, &self.main).or_else(|| pick(&self.slow_names, &self.slow))
    }
}

/// Prédicteurs et encodages d'une trame, complétés de zéros jusqu'à `max(n, 1)` (`_defs` + `as_array`) ;
/// `n = 0` et `ok = false` si la définition est absente ou incomplète.
fn defs(fd: Option<&HeaderFrameDef>) -> (Vec<i64>, Vec<i64>, usize, bool) {
    match fd.filter(|f| f.complete()) {
        None => (vec![0], vec![0], 0, false),
        Some(f) => {
            let n = f.count();
            let pad = |v: &[i64]| {
                let mut a = vec![0i64; n.max(1)];
                let k = v.len().min(n);
                a[..k].copy_from_slice(&v[..k]);
                a
            };
            (pad(&f.predictor), pad(&f.encoding), n, true)
        }
    }
}

/// Pour chaque ligne principale, la dernière trame S reçue avant elle (zéros avant la première).
fn forward_fill(slow: &Table, at: &[i64], n: usize, width: usize) -> Vec<i64> {
    let mut out = vec![0i64; n * width];
    let mut j = 0; // nombre de trames S dont la position `at` est ≤ ligne
    for r in 0..n {
        while j < at.len() && at[j] <= r as i64 {
            j += 1;
        }
        if j > 0 {
            out[r * width..(r + 1) * width]
                .copy_from_slice(&slow.data[(j - 1) * slow.cols..(j - 1) * slow.cols + width]);
        }
    }
    out
}

fn decode_session(buf: &[u8], header: LogHeader, index: usize) -> Result<Option<DecodedSession>, DecodeError> {
    header.check_bounds(index)?;
    let Some(fi) = header.frames.get(&'I').filter(|f| f.complete() && f.count() >= 2) else {
        return Ok(None);
    }; // itération et temps indispensables (D-031)
    let fp = header.frames.get(&'P');
    let (i_pred, i_enc, n_main, _) = defs(Some(fi));
    let (p_pred, p_enc, _, has_p) = defs(fp);
    let (s_pred, s_enc, n_slow, has_s) = defs(header.frames.get(&'S'));
    let (g_pred, g_enc, n_gps, has_g) = defs(header.frames.get(&'G'));
    let (h_pred, h_enc, n_home, has_h) = defs(header.frames.get(&'H'));
    let fh = header.frames.get(&'H');

    let mut ctx = [0i64; CTX_SIZE];
    ctx[CTX_MINTHROTTLE] = header.minthrottle();
    ctx[CTX_MOTOR0_IDX] = fi.index("motor[0]").max(0);
    ctx[CTX_VBATREF] = header.vbatref();
    ctx[CTX_MOTOR_MIN] = header.motor_output().0;
    ctx[CTX_HOME0_IDX] = fh.map_or(0, |f| f.index("GPS_home[0]").max(0));
    ctx[CTX_HOME1_IDX] = fh.map_or(0, |f| f.index("GPS_home[1]").max(0));
    ctx[CTX_DATA_VERSION] = header.data_version();

    let (p_num, p_denom) = header.p_interval();
    let nbytes = header.data_end as i64 - header.data_start as i64;
    let est_rows = (nbytes.div_euclid(20)).max(1024) as usize;
    let prm = Params {
        start: header.data_start as i64,
        end: header.data_end as i64,
        i_def: FrameDef {
            pred: &i_pred,
            enc: &i_enc,
            n: n_main,
        },
        p_def: FrameDef {
            pred: &p_pred,
            enc: &p_enc,
            n: if has_p { n_main } else { 0 },
        },
        s_def: FrameDef {
            pred: &s_pred,
            enc: &s_enc,
            n: n_slow,
        },
        g_def: FrameDef {
            pred: &g_pred,
            enc: &g_enc,
            n: n_gps,
        },
        h_def: FrameDef {
            pred: &h_pred,
            enc: &h_enc,
            n: n_home,
        },
        has_p,
        has_s,
        has_g,
        has_h,
        i_interval: header.i_interval(),
        p_num,
        p_denom,
        ctx,
        est_rows,
    };
    let d = decode_stream(buf, &prm).map_err(|OutOfMemory| DecodeError::OutOfMemory { session: index })?;

    let slow_names: Vec<String> = if has_s {
        header.frames[&'S'].names.clone()
    } else {
        vec![]
    };
    let gps_names: Vec<String> = if has_g {
        header.frames[&'G'].names.clone()
    } else {
        vec![]
    };
    let rows = d.main.rows();
    let width = d.slow.cols.max(1);
    let filled = Table {
        cols: width,
        data: forward_fill(&d.slow, &d.slow_at, rows, width),
    };
    let main_names = fi.names.clone();
    let stats = Stats {
        corrupt_frames: d.stats[0],
        desync_frames: d.stats[1],
        skipped_iterations: d.stats[2],
        log_end_seen: d.stats[3] != 0,
        bytes: d.stats[4],
        i_frames: d.stats[5],
        p_frames: d.stats[6],
    };
    Ok(Some(DecodedSession {
        index,
        main: Matrix {
            rows,
            cols: d.main.cols,
            data: d.main.data,
        },
        main_kind: d.kind,
        slow: Matrix::from_table(&filled, rows, slow_names.len()),
        gps: Matrix::from_table(&d.gps, d.gps.rows(), gps_names.len()),
        gps_at: d.gps_at,
        events: Matrix {
            rows: d.events.len(),
            cols: 4,
            data: d.events.iter().flatten().copied().collect(),
        },
        stats,
        main_names,
        slow_names,
        gps_names,
        header,
    }))
}

/// En-têtes de toutes les sessions d'un fichier (rapide, sans décoder les trames).
pub fn list_sessions(data: &[u8]) -> Vec<LogHeader> {
    let starts = find_log_starts(data);
    (0..starts.len())
        .map(|k| parse_header(data, starts[k], starts.get(k + 1).copied().unwrap_or(data.len())))
        .collect()
}

/// Décode toutes les sessions (ou seulement la n° `only`, à partir de 1) ; une session sans définition I complète est écartée.
pub fn decode_bytes(data: &[u8], only: Option<usize>) -> Result<Vec<DecodedSession>, DecodeError> {
    let starts = find_log_starts(data);
    let mut sessions = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        if only.is_some_and(|o| o != k + 1) {
            continue;
        }
        let end = starts.get(k + 1).copied().unwrap_or(data.len());
        if let Some(sess) = decode_session(data, parse_header(data, s, end), k + 1)? {
            sessions.push(sess);
        }
    }
    Ok(sessions)
}

pub fn decode_file(path: &Path, only: Option<usize>) -> std::io::Result<Vec<DecodedSession>> {
    decode_bytes(&std::fs::read(path)?, only).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
