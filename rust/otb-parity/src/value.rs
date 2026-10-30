//! Valeurs lues dans un fichier de référence (`MODULE.FONCTION.K.npz`).

use serde_json::Value as Json;
use std::io::Read;
use std::path::Path;

/// Tableau numpy en ordre C, octets petit-boutistes tels que dans le fichier.
#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    pub shape: Vec<usize>,
    /// Lettre du type numpy : `f` flottant, `i` entier, `u` non signé, `b` booléen, `c` complexe (paires re, im).
    pub kind: char,
    pub size: usize,
    pub bytes: Vec<u8>,
}

impl Array {
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }
    /// Nombre de flottants à comparer : 2 par élément pour un complexe.
    pub fn n_floats(&self) -> usize {
        self.len() * if self.kind == 'c' { 2 } else { 1 }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Texte d'un tableau `<U…` à un élément (`np.array("None")`).
    pub fn as_string(&self) -> Option<String> {
        (self.kind == 'U' && self.len() == 1).then(|| {
            self.bytes
                .chunks(4)
                .map(|c| char::from_u32(u32::from_le_bytes(c.try_into().unwrap())).unwrap_or('\u{fffd}'))
                .take_while(|&c| c != '\0')
                .collect()
        })
    }
    /// Élément `i` converti en f64 (entiers exacts jusqu'à 2^53).
    pub fn get_f64(&self, i: usize) -> f64 {
        let (kind, size) = if self.kind == 'c' {
            ('f', self.size / 2)
        } else {
            (self.kind, self.size)
        };
        let b = &self.bytes[i * size..(i + 1) * size];
        match (kind, size) {
            ('f', 8) => f64::from_le_bytes(b.try_into().unwrap()),
            ('f', 4) => f32::from_le_bytes(b.try_into().unwrap()) as f64,
            ('i', 8) => i64::from_le_bytes(b.try_into().unwrap()) as f64,
            ('i', 4) => i32::from_le_bytes(b.try_into().unwrap()) as f64,
            ('i', 2) => i16::from_le_bytes(b.try_into().unwrap()) as f64,
            ('i', 1) => b[0] as i8 as f64,
            ('u', 8) => u64::from_le_bytes(b.try_into().unwrap()) as f64,
            ('u', 4) => u32::from_le_bytes(b.try_into().unwrap()) as f64,
            ('u', 2) => u16::from_le_bytes(b.try_into().unwrap()) as f64,
            ('u' | 'b', 1) => b[0] as f64,
            _ => panic!("type de tableau non géré : {}{}", self.kind, self.size),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    F64(f64),
    /// Scalaire numpy : type et valeur.
    NpScalar(String, Box<Value>),
    Str(String),
    Array(Array),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Dict(Vec<(Value, Value)>),
    Slice(Box<[Value; 3]>),
    /// Dataclass ou NamedTuple : nom de la classe, champs dans l'ordre.
    Obj(String, Vec<(String, Value)>),
    /// Journal à rouvrir par le décodeur : chemin relatif au corpus, session.
    FlightLog(String, i64),
    /// Objet sans valeur de comparaison (voir l'en-tête de `golden_modules.py`).
    Repr(String),
}

/// Un appel enregistré.
#[derive(Debug)]
pub struct Case {
    pub function: String,
    pub args: Vec<Value>,
    pub kwargs: Vec<(String, Value)>,
    /// `Ok(résultat)` ou `Err(exception levée par Python)`.
    pub result: Result<Value, String>,
}

impl Case {
    /// Argument par position ou par nom.
    pub fn arg(&self, pos: usize, name: &str) -> Option<&Value> {
        self.args
            .get(pos)
            .or_else(|| self.kwargs.iter().find(|(k, _)| k == name).map(|(_, v)| v))
    }
}

fn parse_npy(raw: &[u8]) -> Result<Array, String> {
    if raw.len() < 10 || &raw[..6] != b"\x93NUMPY" {
        return Err("pas un fichier .npy".into());
    }
    let (hlen, start) = if raw[6] == 1 {
        (u16::from_le_bytes([raw[8], raw[9]]) as usize, 10)
    } else {
        (u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize, 12)
    };
    let header = std::str::from_utf8(&raw[start..start + hlen]).map_err(|e| e.to_string())?;
    let field = |key: &str| -> Result<&str, String> {
        let i = header.find(&format!("'{key}':")).ok_or(format!("clé {key} absente"))? + key.len() + 3;
        Ok(header[i..].trim_start())
    };
    let descr = field("descr")?;
    let descr = descr.trim_start_matches('\'').split('\'').next().unwrap();
    let fortran = field("fortran_order")?.starts_with("True");
    let shape_txt = field("shape")?;
    let inner = &shape_txt[1..shape_txt.find(')').unwrap()];
    let shape: Vec<usize> = inner
        .split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().parse().unwrap())
        .collect();
    let (order, rest) = descr.split_at(1);
    let kind = rest.chars().next().unwrap();
    let size: usize = rest[1..].parse().map_err(|_| format!("type {descr}"))?;
    if order == ">" && size > 1 {
        return Err(format!("grand-boutiste non géré : {descr}"));
    }
    if !matches!(kind, 'f' | 'i' | 'u' | 'b' | 'c' | 'U') {
        return Err(format!("type non géré : {descr}"));
    }
    let n: usize = shape.iter().product();
    let data = &raw[start + hlen..];
    // `<U4` : texte UCS4, `size` caractères de 4 octets par élément
    let elem = if kind == 'U' { 4 * size } else { size };
    if data.len() != n * elem {
        return Err(format!("{} octets pour {n} éléments de {elem}", data.len()));
    }
    let bytes = if fortran && shape.len() > 1 {
        to_c_order(data, &shape, elem)
    } else {
        data.to_vec()
    };
    Ok(Array {
        shape,
        kind,
        size,
        bytes,
    })
}

/// Réordonne des octets écrits en ordre Fortran (première dimension la plus rapide) vers l'ordre C.
fn to_c_order(data: &[u8], shape: &[usize], size: usize) -> Vec<u8> {
    let n: usize = shape.iter().product();
    let mut out = vec![0u8; data.len()];
    let mut idx = vec![0usize; shape.len()];
    for c in 0..n {
        // c : position en ordre C ; idx : indices multi-dimensionnels correspondants
        let mut f = 0;
        let mut stride = 1;
        for d in 0..shape.len() {
            f += idx[d] * stride;
            stride *= shape[d];
        }
        out[c * size..(c + 1) * size].copy_from_slice(&data[f * size..(f + 1) * size]);
        for d in (0..shape.len()).rev() {
            idx[d] += 1;
            if idx[d] < shape[d] {
                break;
            }
            idx[d] = 0;
        }
    }
    out
}

fn decode(j: &Json, arrays: &std::collections::HashMap<String, Array>) -> Result<Value, String> {
    let get = |k: &str| j.get(k);
    Ok(match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Int(n.as_i64().ok_or("entier hors i64")?),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(_) => return Err("liste JSON nue".into()),
        Json::Object(_) => {
            if let Some(h) = get("__f64__") {
                Value::F64(f64::from_bits(
                    u64::from_str_radix(h.as_str().ok_or("__f64__")?, 16).map_err(|e| e.to_string())?,
                ))
            } else if let Some(k) = get("__npy__") {
                Value::Array(
                    arrays
                        .get(k.as_str().ok_or("__npy__")?)
                        .ok_or("tableau absent")?
                        .clone(),
                )
            } else if let Some(t) = get("__np__") {
                Value::NpScalar(
                    t.as_str().unwrap().to_string(),
                    Box::new(decode(get("v").ok_or("v")?, arrays)?),
                )
            } else if let Some(l) = get("__list__") {
                Value::List(seq(l, arrays)?)
            } else if let Some(l) = get("__tuple__") {
                Value::Tuple(seq(l, arrays)?)
            } else if let Some(d) = get("__dict__") {
                let mut out = Vec::new();
                for kv in d.as_array().ok_or("__dict__")? {
                    out.push((decode(&kv[0], arrays)?, decode(&kv[1], arrays)?));
                }
                Value::Dict(out)
            } else if let Some(s) = get("__slice__") {
                let v = seq(s, arrays)?;
                Value::Slice(Box::new([v[0].clone(), v[1].clone(), v[2].clone()]))
            } else if let Some(name) = get("__obj__") {
                let mut fields = Vec::new();
                for (k, v) in get("fields").and_then(Json::as_object).ok_or("fields")? {
                    fields.push((k.clone(), decode(v, arrays)?));
                }
                Value::Obj(name.as_str().unwrap().to_string(), fields)
            } else if let Some(f) = get("__flightlog__") {
                Value::FlightLog(f["path"].as_str().unwrap().to_string(), f["session"].as_i64().unwrap())
            } else if let Some(r) = get("__repr__") {
                Value::Repr(r.as_str().unwrap().to_string())
            } else {
                return Err(format!("objet JSON inconnu : {j}"));
            }
        }
    })
}

fn seq(j: &Json, arrays: &std::collections::HashMap<String, Array>) -> Result<Vec<Value>, String> {
    j.as_array()
        .ok_or("liste attendue")?
        .iter()
        .map(|x| decode(x, arrays))
        .collect()
}

/// Tous les tableaux d'un `.npz` sous leur nom (sans `.npy`), par exemple les références `analysis/*.npz` (4c-0).
pub fn load_arrays(path: &Path) -> Result<std::collections::HashMap<String, Array>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut arrays = std::collections::HashMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f
            .name()
            .map_err(|e| e.to_string())?
            .trim_end_matches(".npy")
            .to_string();
        let mut raw = Vec::new();
        f.read_to_end(&mut raw).map_err(|e| e.to_string())?;
        arrays.insert(name, parse_npy(&raw)?);
    }
    Ok(arrays)
}

/// Lit un fichier `MODULE.FONCTION.K.npz`.
pub fn load_case(path: &Path) -> Result<Case, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut arrays = std::collections::HashMap::new();
    let mut meta = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f
            .name()
            .map_err(|e| e.to_string())?
            .trim_end_matches(".npy")
            .to_string();
        let mut raw = Vec::new();
        f.read_to_end(&mut raw).map_err(|e| e.to_string())?;
        if name == "__meta__" {
            meta = parse_npy(&raw)?.bytes;
        } else {
            arrays.insert(name, parse_npy(&raw)?);
        }
    }
    let m: Json = serde_json::from_slice(&meta).map_err(|e| format!("{}: JSON : {e}", path.display()))?;
    let list = |k: &str| -> Result<Vec<Value>, String> {
        m[k].as_array()
            .map_or(Ok(vec![]), |a| a.iter().map(|x| decode(x, &arrays)).collect())
    };
    let mut kwargs = Vec::new();
    if let Some(o) = m["kwargs"].as_object() {
        for (k, v) in o {
            kwargs.push((k.clone(), decode(v, &arrays)?));
        }
    }
    let result = match m.get("raised") {
        Some(r) => Err(r.as_str().unwrap_or("").to_string()),
        None => Ok(decode(&m["result"], &arrays)?),
    };
    Ok(Case {
        function: m["fn"].as_str().ok_or("fn")?.to_string(),
        args: list("args")?,
        kwargs,
        result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npy(descr: &str, fortran: bool, shape: &str, data: &[u8]) -> Vec<u8> {
        let mut h = format!(
            "{{'descr': '{descr}', 'fortran_order': {}, 'shape': {shape}, }}",
            if fortran { "True" } else { "False" }
        );
        while (10 + h.len() + 1) % 64 != 0 {
            h.push(' ');
        }
        h.push('\n');
        let mut v = b"\x93NUMPY\x01\x00".to_vec();
        v.extend((h.len() as u16).to_le_bytes());
        v.extend(h.as_bytes());
        v.extend(data);
        v
    }

    #[test]
    fn lit_un_tableau_2d_en_ordre_fortran() {
        // matrice 2×3 [[1,2,3],[4,5,6]] écrite colonne par colonne : 1,4,2,5,3,6
        let data: Vec<u8> = [1i32, 4, 2, 5, 3, 6].iter().flat_map(|v| v.to_le_bytes()).collect();
        let a = parse_npy(&npy("<i4", true, "(2, 3)", &data)).unwrap();
        assert_eq!(a.shape, vec![2, 3]);
        assert_eq!(
            (0..6).map(|i| a.get_f64(i)).collect::<Vec<_>>(),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
    }

    #[test]
    fn lit_un_scalaire_zero_dimension_et_un_complexe() {
        let a = parse_npy(&npy("<f8", false, "()", &2.5f64.to_le_bytes())).unwrap();
        assert_eq!((a.shape.len(), a.get_f64(0)), (0, 2.5));
        let data: Vec<u8> = [1.0f64, -2.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let c = parse_npy(&npy("<c16", false, "(1,)", &data)).unwrap();
        assert_eq!((c.n_floats(), c.get_f64(1)), (2, -2.0));
    }

    #[test]
    fn refuse_un_fichier_tronque() {
        assert!(parse_npy(&npy("<f8", false, "(2,)", &[0u8; 8])).is_err());
    }
}
