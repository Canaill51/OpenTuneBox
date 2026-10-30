//! `header::parse_header` rend ce que Python rend, sur les cas de `scripts/header_cases.py`
//! (régénérer `tests/data/header_cases.json` avec ce script si `decoder/header.py` change).

use otb_decoder::header::{find_log_starts, parse_header};
use serde_json::Value;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}

#[test]
fn parse_header_suit_python() {
    let p = format!("{}/tests/data/header_cases.json", env!("CARGO_MANIFEST_DIR"));
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let (mut compared, mut skipped) = (0, 0);
    for (n, c) in cases.iter().enumerate() {
        let data = hex(c["hex"].as_str().unwrap());
        assert_eq!(
            find_log_starts(&data),
            c["starts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect::<Vec<_>>(),
            "cas {n} : débuts de session"
        );
        if c.get("error").is_some() {
            skipped += 1; // Python plante (ex. int(float("inf"))) : le Rust rend la valeur par défaut, non comparé
            continue;
        }
        let h = parse_header(
            &data,
            c["start"].as_u64().unwrap() as usize,
            c["end"].as_u64().unwrap() as usize,
        );
        let ctx = format!("cas {n} ({})", String::from_utf8_lossy(&data).escape_debug());
        let raw = c["raw"].as_object().unwrap();
        assert_eq!(h.raw.len(), raw.len(), "{ctx} : nombre de clés");
        for (k, v) in raw {
            assert_eq!(h.raw.get(k).map(String::as_str), v.as_str(), "{ctx} : clé {k:?}");
        }
        let frames = c["frames"].as_object().unwrap();
        assert_eq!(h.frames.len(), frames.len(), "{ctx} : types de trames");
        for (t, f) in frames {
            let d = &h.frames[&t.chars().next().unwrap()];
            let names: Vec<&str> = f["names"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect();
            assert_eq!(d.names, names, "{ctx} : noms {t}");
            assert_eq!(d.signed, ints(&f["signed"]), "{ctx} : signed {t}");
            assert_eq!(d.predictor, ints(&f["predictor"]), "{ctx} : predictor {t}");
            assert_eq!(d.encoding, ints(&f["encoding"]), "{ctx} : encoding {t}");
        }
        assert_eq!(
            (h.data_start, h.data_end),
            (
                c["data_start"].as_u64().unwrap() as usize,
                c["end"].as_u64().unwrap() as usize
            ),
            "{ctx}"
        );
        let pr = &c["props"];
        assert_eq!(
            h.data_version(),
            pr["data_version"].as_i64().unwrap(),
            "{ctx} : data_version"
        );
        assert_eq!(h.i_interval(), pr["i_interval"].as_i64().unwrap(), "{ctx} : i_interval");
        assert_eq!(
            h.p_interval(),
            (ints(&pr["p_interval"])[0], ints(&pr["p_interval"])[1]),
            "{ctx} : p_interval"
        );
        assert_eq!(
            h.minthrottle(),
            pr["minthrottle"].as_i64().unwrap(),
            "{ctx} : minthrottle"
        );
        assert_eq!(
            h.motor_output(),
            (ints(&pr["motor_output"])[0], ints(&pr["motor_output"])[1]),
            "{ctx} : motor_output"
        );
        assert_eq!(h.vbatref(), pr["vbatref"].as_i64().unwrap(), "{ctx} : vbatref");
        compared += 1;
    }
    assert!(compared > 400 && skipped <= 5, "{compared} comparés, {skipped} ignorés");
}
