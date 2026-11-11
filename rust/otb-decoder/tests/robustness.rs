//! Un fichier malformé ou hostile donne une erreur propre (ou zéro session) en moins d'une seconde, sans emporter le
//! programme (D-044). Les variantes sont fabriquées en mémoire à partir de `tests/data/jeno5_5s.bbl` : aucun fichier
//! binaire de plus dans le dépôt.
//!
//! ⚠️ Une allocation ratée n'est pas une panique : `catch_unwind` ne la rattrape pas, le processus est abandonné (L-080).
//! Un test de ce fichier qui échoue peut donc tuer toute la série : le lancer seul, sous `timeout`.

use std::time::{Duration, Instant};

use otb_decoder::file::decode_bytes;

#[path = "support/mutations.rs"]
mod mutations;
use mutations::{muter, nombre_de_mutations};

fn original() -> Vec<u8> {
    let p = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(p).expect("tests/data/jeno5_5s.bbl")
}

/// Remplace la valeur de la ligne d'en-tête `H <clé>:` par `valeur(ancienne)` (toutes les sessions).
fn set_header(data: &[u8], key: &str, valeur: &dyn Fn(&str) -> String) -> Vec<u8> {
    let needle = format!("H {key}:").into_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut found = false;
    while i < data.len() {
        if data[i..].starts_with(&needle) {
            let nl = data[i..].iter().position(|&b| b == b'\n').map_or(data.len(), |n| n + i);
            let old = String::from_utf8_lossy(&data[i + needle.len()..nl]).to_string();
            out.extend_from_slice(&needle);
            out.extend_from_slice(valeur(&old).as_bytes());
            i = nl;
            found = true;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    if !found {
        out.clear(); // signal : ligne absente (voir `set_header_opt`)
    }
    out
}

/// Comme `set_header`, mais rend les données inchangées si la ligne n'existe pas.
fn set_header_opt(data: &[u8], key: &str, valeur: &dyn Fn(&str) -> String) -> Vec<u8> {
    let r = set_header(data, key, valeur);
    if r.is_empty() { data.to_vec() } else { r }
}

/// Déclare `n` champs dans les trames I et P : les quatre listes (noms, signes, prédicteurs, encodages) sont répétées.
fn with_fields(data: &[u8], n: usize) -> Vec<u8> {
    let mut d = data.to_vec();
    for kind in ["I", "P"] {
        for what in ["name", "signed", "predictor", "encoding"] {
            d = set_header_opt(&d, &format!("Field {kind} {what}"), &|old| {
                let items: Vec<&str> = old.split(',').collect();
                (0..n).map(|k| items[k % items.len()]).collect::<Vec<_>>().join(",")
            });
        }
    }
    d
}

/// Plafond de durée d'un décodage hostile.
///
/// ⚠️ Ce test vise une **boucle sans fin** (D-044), pas une performance : un plafond serré (1 s au départ) échoue au
/// hasard sur une machine chargée sans rien prouver de plus — une boucle sans fin dépasse n'importe quel plafond (W-054)
const LIMITE: Duration = Duration::from_secs(30);

/// Le décodage rend une erreur ou zéro session, vite ; un résultat avec des sessions n'est pas admis pour un cas hostile.
fn refuse(nom: &str, data: &[u8]) {
    let t = Instant::now();
    let r = decode_bytes(data, None);
    let dt = t.elapsed();
    assert!(dt < LIMITE, "{nom} : {dt:?} (plus de {LIMITE:?})");
    match r {
        Err(e) => assert!(!e.to_string().is_empty(), "{nom} : message vide"),
        Ok(s) => assert!(
            s.is_empty(),
            "{nom} : {} session(s) décodée(s) au lieu d'un refus",
            s.len()
        ),
    }
}

/// Le décodage rend n'importe quoi de propre (erreur ou sessions), vite : jamais de panique ni de blocage.
fn survit(nom: &str, data: &[u8]) {
    let t = Instant::now();
    let r = std::panic::catch_unwind(|| decode_bytes(data, None));
    let dt = t.elapsed();
    assert!(r.is_ok(), "{nom} : panique");
    assert!(dt < LIMITE, "{nom} : {dt:?} (plus de {LIMITE:?})");
}

#[test]
fn le_fichier_source_se_decode() {
    let s = decode_bytes(&original(), None).unwrap();
    assert!(!s.is_empty() && s[0].main.rows > 100);
}

#[test]
fn i_interval_astronomique_est_refuse() {
    refuse(
        "I interval 9e18",
        &set_header(&original(), "I interval", &|_| "9000000000000000000".into()),
    );
}

#[test]
fn i_interval_juste_au_dessus_du_plafond_est_refuse() {
    refuse(
        "I interval 65536",
        &set_header(&original(), "I interval", &|_| "65536".into()),
    );
}

#[test]
fn p_interval_denominateur_astronomique_est_refuse() {
    refuse(
        "P interval 1/9e18",
        &set_header(&original(), "P interval", &|_| "1/9000000000000000000".into()),
    );
}

#[test]
fn p_interval_numerateur_astronomique_est_refuse() {
    refuse(
        "P interval 9e18/2",
        &set_header(&original(), "P interval", &|_| "9000000000000000000/2".into()),
    );
}

#[test]
fn vingt_mille_champs_sont_refuses() {
    refuse("20 000 champs", &with_fields(&original(), 20_000));
}

#[test]
fn juste_au_dessus_de_512_champs_est_refuse() {
    refuse("513 champs", &with_fields(&original(), 513));
}

#[test]
fn les_bornes_elles_memes_sont_acceptees() {
    // 512 champs et intervalles 65 535 : au plafond, pas au-dessus ; le fichier peut ne rien donner mais pas être « refusé »
    let d = with_fields(&original(), 512);
    assert!(decode_bytes(&d, None).is_ok(), "512 champs refusés");
    let d = set_header(&original(), "I interval", &|_| "65535".into());
    assert!(decode_bytes(&d, None).is_ok(), "I interval 65535 refusé");
    let d = set_header(&original(), "P interval", &|_| "65535/65535".into());
    assert!(decode_bytes(&d, None).is_ok(), "P interval 65535/65535 refusé");
}

#[test]
fn tous_les_reglages_a_zero_ne_font_ni_panique_ni_blocage() {
    let mut d = original();
    for k in [
        "I interval",
        "P interval",
        "minthrottle",
        "maxthrottle",
        "vbatref",
        "motorOutput",
    ] {
        d = set_header_opt(&d, k, &|_| "0".into());
    }
    survit("réglages à zéro", &d);
}

#[test]
fn fichiers_abimes_restent_propres() {
    let o = original();
    survit("vide", &[]);
    survit("tronqué 50 %", &o[..o.len() / 2]);
    survit("en-tête coupé", &o[..300]);
    survit("corps à 0xFF", &{
        let mut d = o.clone();
        let start = d.windows(3).position(|w| w == b"\nI\x00").unwrap_or(2000).min(d.len());
        d[start.max(2000)..].fill(0xFF);
        d
    });
    // dix mutations à 1 % (graine fixe : refaisable)
    let mut etat: u64 = 0x2545_F491_4F6C_DD1D;
    for m in 0..10 {
        let mut d = o.clone();
        for _ in 0..d.len() / 100 {
            etat ^= etat << 13;
            etat ^= etat >> 7;
            etat ^= etat << 17;
            let i = (etat % d.len() as u64) as usize;
            d[i] = (etat >> 32) as u8;
        }
        survit(&format!("mutation {m}"), &d);
    }
}

/// Mutations variées (octets isolés, en-tête, coupure, trou, morceau doublé, 1 %) : 24 cas en CI. `OTB_MUTATIONS=N` en
/// lance N : essai prolongé, en `--release` et seul (voir docs/validation.md, « Robustesse »).
#[test]
fn mutations_variees_restent_propres() {
    let o = original();
    let total = nombre_de_mutations(24);
    for m in 0..total {
        let (nom, d) = muter(&o, m);
        survit(&nom, &d);
    }
}
