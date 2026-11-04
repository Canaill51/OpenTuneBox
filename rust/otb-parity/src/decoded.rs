//! Empreintes des sessions décodées, au format de `scripts/golden.py:decode_digest` (`decoder.json`) :
//! `"int64|(n, m)|sha256"` par tableau, noms de champs, statistiques. Sans l'empreinte d'en-tête (propre à Python).

use otb_decoder::file::{DecodedSession, Matrix};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn digest(dtype: &str, shape: &str, bytes: impl Iterator<Item = u8>) -> String {
    let mut h = Sha256::new();
    let buf: Vec<u8> = bytes.collect();
    h.update(&buf);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!("{dtype}|{shape}|{hex}")
}

fn matrix(m: &Matrix) -> String {
    digest(
        "int64",
        &format!("({}, {})", m.rows, m.cols),
        m.data.iter().flat_map(|v| v.to_le_bytes()),
    )
}

fn vector_i64(v: &[i64]) -> String {
    digest(
        "int64",
        &format!("({},)", v.len()),
        v.iter().flat_map(|x| x.to_le_bytes()),
    )
}

/// Empreintes d'une session, mêmes clés que `decode_digest` de `golden.py` (sauf `header`).
pub fn session_digest(s: &DecodedSession) -> Value {
    json!({
        "main": matrix(&s.main),
        "main_kind": digest("int8", &format!("({},)", s.main_kind.len()), s.main_kind.iter().map(|k| *k as u8)),
        "slow": matrix(&s.slow),
        "gps": matrix(&s.gps),
        "gps_at": vector_i64(&s.gps_at),
        "events": matrix(&s.events),
        "names": [s.main_names, s.slow_names, s.gps_names],
        "stats": {
            "bytes": s.stats.bytes, "corrupt_frames": s.stats.corrupt_frames, "desync_frames": s.stats.desync_frames,
            "i_frames": s.stats.i_frames, "log_end_seen": s.stats.log_end_seen, "p_frames": s.stats.p_frames,
            "skipped_iterations": s.stats.skipped_iterations,
        },
    })
}

/// Différences entre les empreintes attendues (Python) et celles de `s`, clé par clé ; vide si identiques.
pub fn diff(want: &Value, s: &DecodedSession) -> Vec<String> {
    let got = session_digest(s);
    let mut out = Vec::new();
    for key in ["main", "main_kind", "slow", "gps", "gps_at", "events", "names", "stats"] {
        if want[key] != got[key] {
            out.push(format!("{key} : Python {}, Rust {}", want[key], got[key]));
        }
    }
    out
}
