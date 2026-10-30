//! Lecture des en-têtes d'un fichier Blackbox et découpage en sessions (port de `decoder/header.py`).
//!
//! Un fichier `.bbl`/`.bfl` contient une session par armement ; chacune commence par la ligne
//! « H Product:Blackbox flight data recorder by Nicholas Sherlock », suivie de lignes « H clé:valeur » puis des trames
//! binaires. Les textes sont lus en latin-1 (un octet = un caractère), comme en Python.
//!
//! ⚠️ Deux écarts de robustesse assumés, là où Python lève une exception : une valeur `inf` (`int(float("inf"))` plante
//! en Python) et un entier hors de `i64` (Python n'a pas de limite) rendent ici la valeur par défaut.

use std::collections::BTreeMap;

use crate::frame::{PRED_HOME_COORD, PRED_HOME_COORD_1};

pub const LOG_START_MARKER: &[u8] = b"H Product:Blackbox flight data recorder by Nicholas Sherlock";
const FRAME_MARKERS: &[u8] = b"IPGHSE";

/// Définition d'un type de trame : noms, signes, prédicteurs et encodages des champs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameDef {
    pub names: Vec<String>,
    pub signed: Vec<i64>,
    pub predictor: Vec<i64>,
    pub encoding: Vec<i64>,
}

impl FrameDef {
    pub fn count(&self) -> usize {
        self.names.len()
    }

    /// Indice du champ `name`, -1 s'il n'existe pas.
    pub fn index(&self, name: &str) -> i64 {
        self.names.iter().position(|n| n == name).map_or(-1, |i| i as i64)
    }

    pub fn complete(&self) -> bool {
        let n = self.count();
        n > 0 && self.predictor.len() == n && self.encoding.len() == n
    }
}

/// En-tête d'une session : toutes les lignes H brutes et les définitions de trames.
#[derive(Debug, Clone, PartialEq)]
pub struct LogHeader {
    pub raw: BTreeMap<String, String>,
    pub frames: BTreeMap<char, FrameDef>,
    /// Position (dans le fichier) de la première trame binaire. Peut valoir `data_end + 1` si l'en-tête n'a pas de trame.
    pub data_start: usize,
    /// Fin de la session (début de la suivante ou fin du fichier).
    pub data_end: usize,
}

/// Espace au sens de `str.strip()` de Python pour les caractères latin-1 : ni `trim` ni `is_whitespace` de Rust
/// (ils ignorent U+001C à U+001F).
fn py_space(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r' | '\x1c'..='\x1f' | '\u{85}' | '\u{a0}'
    )
}

fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}

/// `float(s)` de Python : espaces ignorés, soulignés permis entre deux chiffres.
fn py_float(s: &str) -> Option<f64> {
    let s = py_strip(s);
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == '_' && !(i > 0 && chars[i - 1].is_ascii_digit() && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            return None;
        }
    }
    s.replace('_', "").parse::<f64>().ok()
}

/// `int(s, 0)` d'un texte qui commence par `0x` (soulignés permis entre deux chiffres, ou juste après le préfixe).
fn py_hex(s: &str) -> Option<i64> {
    let digits = s.get(2..)?;
    let digits = digits.strip_prefix('_').unwrap_or(digits);
    let valid = digits.chars().all(|c| c.is_ascii_hexdigit() || c == '_'); // `from_str_radix` accepterait un signe
    if !valid || digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
        return None;
    }
    i64::from_str_radix(&digits.replace('_', ""), 16).ok()
}

/// Entier d'une valeur d'en-tête : `0x…`, ou flottant tronqué (`int(float(v))`) ; `default` si absent ou illisible.
pub fn int(v: Option<&str>, default: i64) -> i64 {
    let Some(v) = v else { return default };
    let v = py_strip(v);
    if v.to_ascii_lowercase().starts_with("0x") {
        return py_hex(v).unwrap_or(default);
    }
    match py_float(v) {
        Some(f) if f.is_finite() => f as i64,
        _ => default,
    }
}

/// Liste d'entiers séparés par des virgules ; les éléments illisibles sont ignorés.
pub fn ints(v: Option<&str>) -> Vec<i64> {
    let Some(v) = v.filter(|v| !v.is_empty()) else {
        return vec![];
    };
    v.split(',')
        .filter_map(|p| py_float(p).filter(|f| f.is_finite()).map(|f| f as i64))
        .collect()
}

impl LogHeader {
    fn get(&self, key: &str) -> Option<&str> {
        self.raw.get(key).map(String::as_str)
    }

    pub fn data_version(&self) -> i64 {
        int(self.get("Data version"), 2)
    }

    pub fn i_interval(&self) -> i64 {
        int(self.get("I interval"), 1).max(1)
    }

    /// (numérateur, dénominateur) de « P interval » (forme « N » ou « num/den »).
    pub fn p_interval(&self) -> (i64, i64) {
        let v = self.get("P interval").unwrap_or("1");
        match v.split_once('/') {
            Some((a, b)) => (int(Some(a), 1).max(1), int(Some(b), 1).max(1)),
            None => (1, int(Some(v), 1).max(1)),
        }
    }

    pub fn minthrottle(&self) -> i64 {
        int(self.get("minthrottle"), 0)
    }

    pub fn motor_output(&self) -> (i64, i64) {
        let vals = ints(self.get("motorOutput"));
        match vals.as_slice() {
            [a, b, ..] => (*a, *b),
            [a] => (*a, 2047),
            [] => (self.minthrottle(), int(self.get("maxthrottle"), 2047)),
        }
    }

    pub fn vbatref(&self) -> i64 {
        int(self.get("vbatref"), 0)
    }
}

/// Positions de début de chaque session dans le fichier.
pub fn find_log_starts(data: &[u8]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(i) = find(data, LOG_START_MARKER, from) {
        starts.push(i);
        from = i + LOG_START_MARKER.len();
    }
    starts
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Lit les lignes « H … » à partir de `start` jusqu'à la première trame binaire.
pub fn parse_header(data: &[u8], start: usize, end: usize) -> LogHeader {
    let mut raw = BTreeMap::new();
    let mut frames: BTreeMap<char, FrameDef> = BTreeMap::new();
    let mut pos = start;
    while pos < end {
        let c = data[pos];
        if c == b'H' && pos + 1 < end && data[pos + 1] == b' ' {
            let nl = data[pos..end].iter().position(|&b| b == b'\n').map_or(end, |i| i + pos);
            let line = latin1(&data[pos + 2..nl]);
            let line = line.trim_end_matches('\r');
            pos = nl + 1;
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            raw.insert(key.to_string(), value.to_string());
            apply_field_header(&mut frames, key, value);
        } else if FRAME_MARKERS.contains(&c) {
            break;
        } else {
            pos += 1; // octets parasites avant la première trame
        }
    }

    // Les trames P reprennent les noms et les signes des trames I
    if let (Some(fi), true) = (frames.get(&'I').cloned(), frames.contains_key(&'P')) {
        let fp = frames.get_mut(&'P').unwrap();
        fp.names = fi.names;
        fp.signed = fi.signed;
    }

    // Paires de coordonnées GPS : le second prédicteur HOME_COORD vise la longitude
    if let Some(fg) = frames.get_mut(&'G') {
        for i in 1..fg.count().min(fg.predictor.len()) {
            if fg.predictor[i - 1] == PRED_HOME_COORD && fg.predictor[i] == PRED_HOME_COORD {
                fg.predictor[i] = PRED_HOME_COORD_1;
            }
        }
    }
    LogHeader {
        raw,
        frames,
        data_start: pos,
        data_end: end,
    }
}

/// Clés de la forme « Field I name », « Field P predictor », …
fn apply_field_header(frames: &mut BTreeMap<char, FrameDef>, key: &str, value: &str) {
    let k: Vec<char> = key.chars().collect();
    if !key.starts_with("Field ") || k.len() < 8 {
        return;
    }
    let fd = frames.entry(k[6]).or_default();
    match k[8..].iter().collect::<String>().as_str() {
        "name" => fd.names = value.split(',').map(|n| py_strip(n).to_string()).collect(),
        "signed" => fd.signed = ints(Some(value)),
        "predictor" => fd.predictor = ints(Some(value)),
        "encoding" => fd.encoding = ints(Some(value)),
        _ => {}
    }
}
