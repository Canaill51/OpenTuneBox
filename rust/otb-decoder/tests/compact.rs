//! Le rangement compact des trames principales (`decode_bytes_compact`, L-081) rend les mêmes valeurs que le rangement
//! complet, aux 32 bits près pour les colonnes étroites, et les mêmes positions pour les trames lentes, GPS et événements.

use otb_decoder::file::{decode_bytes, decode_bytes_compact};
use otb_decoder::wrap32;

#[test]
fn compact_et_complet_concordent() {
    let data = std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let full = decode_bytes(&data, None).unwrap();
    let compact = decode_bytes_compact(&data, None).unwrap();
    assert_eq!(full.len(), compact.len());
    for (f, c) in full.iter().zip(&compact) {
        let t = c.compact.as_ref().expect("log ordinaire : rangement compact");
        assert!(c.main.data.is_empty());
        assert_eq!((f.main.rows, f.main.cols), (t.rows, t.cols));
        assert_eq!(t.wide_cols, [0, 1]);
        for r in 0..f.main.rows {
            for col in 0..f.main.cols {
                let v = f.main.data[r * f.main.cols + col];
                let attendu = if col < 2 { v } else { i64::from(wrap32(v)) };
                assert_eq!(t.get(r, col), attendu, "ligne {r}, colonne {col}");
            }
        }
        assert_eq!(f.main_kind, c.main_kind);
        assert_eq!(f.slow, c.slow, "trames lentes");
        assert_eq!(f.gps, c.gps);
        assert_eq!(f.gps_at, c.gps_at);
        assert_eq!(f.events, c.events);
        assert_eq!(f.stats, c.stats);
    }
}

#[test]
fn wrap32_ramene_aux_32_bits() {
    assert_eq!(wrap32(2_147_483_648), -2_147_483_648);
    assert_eq!(wrap32(-2_147_483_649), 2_147_483_647);
    assert_eq!(wrap32(123), 123);
}
