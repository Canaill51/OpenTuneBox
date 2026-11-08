//! Les comparateurs d'`otb-parity` comparent-ils vraiment ?
//!
//! `check_decoder` et `check_header` sont le filet qui garantit que le Rust décode comme l'étalon. Ils ne tournaient
//! jusqu'ici que lancés à la main sur le corpus (`otb-parity check-decoder`), et **aucun test ne vérifiait qu'ils
//! savent dire non** : un comparateur qui ne compare rien rend « 0 écart » sur tout, et son silence passe pour une
//! preuve.
//!
//! Ces tests fabriquent une référence à partir du Rust lui-même, puis la **sabotent**. Ils ne prouvent donc pas que le
//! Rust décode juste (c'est le rôle du corpus et de `decode_fixtures.rs`) : ils prouvent que si le Rust s'écartait de
//! l'étalon, le comparateur le **signalerait** au lieu de se taire. Ils tournent en CI, sans corpus (W-054).

use otb_decoder::header::{find_log_starts, parse_header};
use otb_parity::{decoded, layers};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Ce qu'on abîme dans une ligne de référence, et son nom pour le message d'échec.
type Sabotage = (&'static str, fn(&mut Value));

fn log_essai() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

/// Dossier de travail propre à ce test (le nom évite toute collision entre deux tests du même fichier).
fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-parity-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `decoder.json` qui décrit exactement ce que le Rust décode aujourd'hui.
fn reference_decodeur(dir: &Path, nom_relatif: &str) -> Value {
    let data = std::fs::read(log_essai()).unwrap();
    let sessions = otb_decoder::file::decode_bytes(&data, None).unwrap();
    assert!(!sessions.is_empty(), "le log d'essai doit donner au moins une session");
    let digests: Vec<Value> = sessions.iter().map(decoded::session_digest).collect();
    let reference = json!({ nom_relatif: digests });
    std::fs::write(dir.join("decoder.json"), reference.to_string()).unwrap();
    reference
}

/// `header.jsonl` qui décrit exactement ce que le Rust lit dans l'en-tête aujourd'hui.
fn reference_entete(dir: &Path, nom_relatif: &str) -> Vec<Value> {
    let data = std::fs::read(log_essai()).unwrap();
    let starts = find_log_starts(&data);
    let mut lignes = Vec::new();
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(data.len());
        let h = parse_header(&data, start, end);
        let frames: serde_json::Map<String, Value> = h
            .frames
            .iter()
            .map(|(t, d)| {
                (
                    t.to_string(),
                    json!({
                        "names": d.names, "signed": d.signed,
                        "predictor": d.predictor, "encoding": d.encoding,
                    }),
                )
            })
            .collect();
        lignes.push(json!({
            "file": nom_relatif,
            "session": i + 1, // les références Python numérotent les sessions à partir de 1
            "start": start,
            "end": end,
            "raw": h.raw,
            "frames": frames,
            "data_start": h.data_start,
            "data_end": h.data_end,
            "props": {
                "data_version": h.data_version(),
                "i_interval": h.i_interval(),
                "p_interval": [h.p_interval().0, h.p_interval().1],
                "minthrottle": h.minthrottle(),
                "motor_output": [h.motor_output().0, h.motor_output().1],
                "vbatref": h.vbatref(),
            },
        }));
    }
    ecrire_jsonl(dir, &lignes);
    lignes
}

fn ecrire_jsonl(dir: &Path, lignes: &[Value]) {
    let texte: String = lignes.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join("header.jsonl"), texte).unwrap();
}

/// Le log d'essai est posé sous `logs_root` sous le nom que la référence annonce.
fn poser_le_log(racine: &Path, nom_relatif: &str) {
    let cible = racine.join(nom_relatif);
    std::fs::create_dir_all(cible.parent().unwrap()).unwrap();
    std::fs::copy(log_essai(), cible).unwrap();
}

#[test]
fn check_decoder_accepte_une_reference_fidele_et_refuse_une_reference_sabotee() {
    let dir = dossier("decodeur");
    let racine = dir.join("logs");
    let nom = "essai/jeno5_5s.bbl";
    poser_le_log(&racine, nom);
    let reference = reference_decodeur(&dir, nom);

    // 1. Référence fidèle : aucun écart, et le comparateur a réellement regardé quelque chose.
    let rep = layers::check_decoder(&dir, &racine, 1).unwrap();
    assert!(rep.failures.is_empty(), "référence fidèle refusée : {:?}", rep.failures);
    assert!(
        rep.checked > 0,
        "aucune session comparée : le comparateur n'a rien regardé"
    );

    // 2. Une seule valeur changée dans la référence : l'écart doit être signalé.
    let mut sabotee = reference.clone();
    sabotee[nom][0]["stats"]["i_frames"] = json!(999_999);
    std::fs::write(dir.join("decoder.json"), sabotee.to_string()).unwrap();
    let rep = layers::check_decoder(&dir, &racine, 1).unwrap();
    assert!(
        !rep.failures.is_empty(),
        "une empreinte fausse n'a pas été vue : le comparateur ne compare pas"
    );

    // 3. Un nombre de sessions différent doit aussi être vu.
    let mut tronquee = reference.clone();
    let mut sessions = tronquee[nom].as_array().unwrap().clone();
    sessions.push(sessions[0].clone());
    tronquee[nom] = Value::Array(sessions);
    std::fs::write(dir.join("decoder.json"), tronquee.to_string()).unwrap();
    let rep = layers::check_decoder(&dir, &racine, 1).unwrap();
    assert!(!rep.failures.is_empty(), "un nombre de sessions faux n'a pas été vu");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_decoder_signale_un_log_introuvable_au_lieu_de_le_passer_sous_silence() {
    let dir = dossier("manquant");
    let racine = dir.join("logs");
    std::fs::create_dir_all(&racine).unwrap();
    reference_decodeur(&dir, "essai/jeno5_5s.bbl"); // la référence existe, le log non

    let rep = layers::check_decoder(&dir, &racine, 1).unwrap();
    assert!(
        !rep.failures.is_empty(),
        "un log absent doit être un échec, pas un silence"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_header_accepte_une_reference_fidele_et_refuse_chaque_sabotage() {
    let dir = dossier("entete");
    let racine = dir.join("logs");
    let nom = "essai/jeno5_5s.bbl";
    poser_le_log(&racine, nom);
    let lignes = reference_entete(&dir, nom);

    let rep = layers::check_header(&dir, &racine).unwrap();
    assert!(rep.failures.is_empty(), "référence fidèle refusée : {:?}", rep.failures);
    assert!(rep.checked > 0, "aucun en-tête comparé");

    // Chaque sabotage vise une partie différente de la comparaison : si l'une d'elles ne comparait rien,
    // le test le dirait ici et non six mois plus tard sur le corpus.
    let sabotages: [Sabotage; 5] = [
        ("une valeur de l'en-tête", |l| {
            let cle = l["raw"].as_object().unwrap().keys().next().unwrap().clone();
            l["raw"][cle] = json!("valeur inventée");
        }),
        ("une clé d'en-tête en trop", |l| {
            l["raw"]["cle_inexistante"] = json!("x")
        }),
        ("la fin des données", |l| l["data_end"] = json!(1)),
        ("une propriété du log", |l| l["props"]["i_interval"] = json!(12_345)),
        ("le nom d'un champ de trame", |l| {
            let t = l["frames"].as_object().unwrap().keys().next().unwrap().clone();
            l["frames"][t]["names"][0] = json!("champ_inexistant");
        }),
    ];
    for (quoi, saboter) in sabotages {
        let mut abimees = lignes.clone();
        saboter(&mut abimees[0]);
        ecrire_jsonl(&dir, &abimees);
        let rep = layers::check_header(&dir, &racine).unwrap();
        assert!(!rep.failures.is_empty(), "sabotage non vu : {quoi}");
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_header_compte_les_sessions_que_python_na_pas_pu_lire() {
    let dir = dossier("saut");
    let racine = dir.join("logs");
    let nom = "essai/jeno5_5s.bbl";
    poser_le_log(&racine, nom);
    let mut lignes = reference_entete(&dir, nom);
    // Une ligne « error » veut dire : Python a échoué sur cette session, il n'y a rien à comparer.
    lignes.push(json!({ "file": nom, "session": 99, "error": "Python a planté" }));
    ecrire_jsonl(&dir, &lignes);

    let rep = layers::check_header(&dir, &racine).unwrap();
    assert_eq!(rep.skipped, 1, "la session en erreur doit être comptée comme sautée");
    assert!(rep.failures.is_empty(), "une session sautée n'est pas un échec");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_header_refuse_un_numero_de_session_invalide_sans_paniquer() {
    let dir = dossier("session-zero");
    let racine = dir.join("logs");
    let nom = "essai/jeno5_5s.bbl";
    poser_le_log(&racine, nom);
    let mut lignes = reference_entete(&dir, nom);
    // Les références Python numérotent à partir de 1 : un 0 faisait déborder `session - 1` (panique en debug).
    lignes[0]["session"] = json!(0);
    ecrire_jsonl(&dir, &lignes);

    let rep = layers::check_header(&dir, &racine).unwrap();
    assert!(
        rep.failures.iter().any(|f| f.contains("à partir de 1")),
        "un numéro de session invalide doit être une erreur nommée, pas une panique : {:?}",
        rep.failures
    );
    std::fs::remove_dir_all(&dir).ok();
}
