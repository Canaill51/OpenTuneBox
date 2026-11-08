//! Chaque contrôle de `src/docs.rs` échoue sur un cas fabriqué (et passe sur le cas sain voisin) : un test de documentation
//! qui ne peut pas échouer ne prouve rien (piège transverse « test qui passe toujours »).

use otb_dev::docs::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Dossier de travail propre à un test, vidé au départ.
fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("otb-dev-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("dossier de travail");
    dir
}

fn write(root: &Path, file: &str, text: &str) -> PathBuf {
    let p = root.join(file);
    fs::create_dir_all(p.parent().expect("parent")).expect("dossier");
    fs::write(&p, text).expect("écriture");
    p
}

#[test]
fn liens_cassés() {
    let root = sandbox("liens");
    write(
        &root,
        "docs/autre.md",
        "# Un titre\n\n## Sous-titre, avec ponctuation !\n",
    );
    let md = write(&root, "docs/a.md", "");
    assert!(broken_links(&md, "[ok](autre.md) [ancre](autre.md#un-titre) [ancre2](autre.md#sous-titre-avec-ponctuation-) [web](https://x.y/z) [guide](otb:phase/prep)").is_empty());
    assert!(broken_links(&md, "[x](absent.md)")[0].contains("fichier absent"));
    assert!(broken_links(&md, "[x](autre.md#titre-inexistant)")[0].contains("titre absent"));
    assert!(
        broken_links(&md, "[x](#nulle-part)")[0].contains("titre absent"),
        "ancre dans le même fichier"
    );
    assert!(
        broken_links(&md, "```\n[x](absent.md)\n```").is_empty(),
        "un bloc de code n'est pas un lien"
    );
}

#[test]
fn chemins_et_symboles() {
    let root = sandbox("chemins");
    write(
        &root,
        "rust/otb-x/src/lib.rs",
        "pub fn calcule() {}\npub struct Page;\npub const MAX: usize = 3;\n",
    );
    write(&root, "docs/a.md", "");
    let ok = "`rust/otb-x/src/lib.rs:calcule` `rust/otb-x/src/lib.rs:Page` `rust/otb-x/src/lib.rs:MAX` `docs/a.md` `refs/heads/main` `docs/*.md`";
    assert_eq!(missing_paths(&root, ok, false), Vec::<String>::new());
    assert_eq!(missing_paths(&root, "`docs/absent.md`", false), ["docs/absent.md"]);
    assert_eq!(
        missing_paths(&root, "`rust/otb-y/src/lib.rs`", false),
        ["rust/otb-y/src/lib.rs"]
    );
    assert!(missing_paths(&root, "`rust/otb-x/src/lib.rs:disparue`", false)[0].contains("symbole absent"));
    assert!(
        missing_paths(&root, "`rust/otb-spike-egui/src/main.rs`", false).is_empty(),
        "crate supprimé, cité par l'historique"
    );
    assert!(
        missing_paths(&root, "`rust/otb-py/src/lib.rs`", false).is_empty(),
        "pont PyO3 supprimé en 7-5, cité par l'historique"
    );
    assert!(missing_paths(&root, "```\n`docs/absent.md`\n```", false).is_empty());
}

#[test]
fn chemins_python_disparus_acceptes_seulement_dans_l_historique() {
    let root = sandbox("chemins-python");
    write(&root, "docs/a.md", "");
    write(&root, "rust/otb-x/src/lib.rs", "pub fn calcule() {}\n");
    let anciens = "`gui/app.py:hard_exit` `src/opentunebox/log/flightlog.py` `scripts/release.py` `tests/test_docs.py` `analysis/loop.py` `packaging/opentunebox.spec` `packaging/macos/build_dmg.sh`";
    // document vivant : chaque chemin Python disparu est signalé
    assert_eq!(missing_paths(&root, anciens, false).len(), 7);
    // registre historique : accepté
    assert!(missing_paths(&root, anciens, true).is_empty());
    // l'histoire n'excuse pas un chemin Rust ou un document qui n'existe pas
    assert_eq!(
        missing_paths(&root, "`rust/otb-y/src/lib.rs` `docs/absent.md`", true),
        ["rust/otb-y/src/lib.rs", "docs/absent.md"]
    );
    assert!(missing_paths(&root, "`rust/otb-x/src/lib.rs:disparue`", true)[0].contains("symbole absent"));
    // un chemin au nom « Python » qui existe encore est vérifié comme les autres
    write(&root, "scripts/reste.py", "");
    assert!(missing_paths(&root, "`scripts/reste.py`", false).is_empty());
}

#[test]
fn outils_documentés() {
    let readme = "| Bouton | Outil |\n|---|---|\n| Spectre | Analyse spectrale | x |\n| Configuration | Infos de configuration | x |\n";
    let guide = "### Analyse spectrale (Spectre)\n### Infos de configuration (Configuration)\n";
    assert!(tools_documented(&["Analyse spectrale", "Configuration"], readme, guide).is_empty());
    let m = tools_documented(&["Analyse spectrale", "Nouvel outil"], readme, guide);
    assert_eq!(m.len(), 2, "{m:?}");
    assert!(m[0].contains("README") && m[1].contains("guide"));
    assert_eq!(
        tools_documented(&["Configuration"], readme, "### Configuration").len(),
        0
    );
    assert_eq!(
        tools_documented(&["Configuration"], readme, "### Configurations").len(),
        1,
        "titre voisin ≠ titre"
    );
}

#[test]
fn liens_du_guide() {
    let home = "[a](otb:phase/prep) [b](otb:tool/spectral) [c](otb:tool/viewer)";
    assert!(guide_links(home, &["prep"], &["spectral", "viewer"]).is_empty());
    assert_eq!(
        guide_links("[a](otb:phase/inconnue)", &["prep"], &[]),
        ["otb:phase/inconnue"]
    );
    assert_eq!(
        guide_links("[a](otb:tool/inconnu)", &[], &["spectral"]),
        ["otb:tool/inconnu"]
    );
    assert_eq!(
        guide_links("[a](otb:autre/prep)", &["prep"], &["prep"]),
        ["otb:autre/prep"]
    );
}

#[test]
fn cases_du_parcours() {
    let keys: BTreeSet<String> = ["rpm".to_string(), "noise".to_string()].into();
    let step = |tool: Option<&str>, part: Option<&str>, check: &str| StepInfo {
        key: "s".into(),
        tool: tool.map(String::from),
        part: part.map(String::from),
        checks: vec![check.into()],
        usages: vec![],
    };
    let tools = ["spectral", "viewer"];
    assert!(guide_steps(&[step(Some("spectral"), Some("hover"), "rpm")], &tools, &keys).is_empty());
    assert!(guide_steps(&[step(None, Some("moving"), "noise")], &tools, &keys).is_empty());
    assert!(guide_steps(&[step(Some("fantôme"), None, "rpm")], &tools, &keys)[0].contains("outil fantôme"));
    assert!(guide_steps(&[step(None, Some("vol"), "rpm")], &tools, &keys)[0].contains("partie vol"));
    assert!(guide_steps(&[step(None, None, "inconnu")], &tools, &keys)[0].contains("contrôle inconnu"));
    let mut usage = step(None, None, "rpm");
    usage.usages = vec!["série".into()];
    assert!(guide_steps(&[usage], &tools, &keys)[0].contains("contrôle série"));
}

#[test]
fn clés_de_log_check() {
    let src = "c.add(\n        \"version\",\n  x);\n    usages.push(Usage {\n        key: \"series\".into(),\n";
    let keys = log_check_keys(src);
    assert_eq!(keys, BTreeSet::from(["version".to_string(), "series".to_string()]));
}

#[test]
fn modules_cités() {
    let root = sandbox("modules");
    let a = write(&root, "rust/otb-x/src/calcul.rs", "");
    write(&root, "rust/otb-x/src/lib.rs", "");
    write(&root, "rust/otb-x/src/_prive.rs", "");
    write(&root, "rust/otb-x/src/sous/profond.rs", "");
    let found = rust_modules(&root);
    assert_eq!(found.len(), 2, "lib et _prive exclus : {found:?}");
    assert!(modules_documented(&root, &found, "calcul profond").is_empty());
    assert_eq!(
        modules_documented(&root, &found, "calcul"),
        ["rust/otb-x/src/sous/profond.rs"]
    );
    assert_eq!(modules_documented(&root, &[a], ""), ["rust/otb-x/src/calcul.rs"]);
}

#[test]
fn index_des_documents() {
    let names = vec!["README.md".to_string(), "a.md".to_string(), "b.md".to_string()];
    assert!(docs_index_missing(&names, "[A](a.md) [B](b.md)").is_empty());
    assert_eq!(docs_index_missing(&names, "[A](a.md)"), ["b.md"]);
}

#[test]
fn fichiers_publiés() {
    let wf = "          - os: x\n            package: OpenTuneBox-Windows-Setup.exe\n            package: OpenTuneBox-Linux.AppImage\n";
    let files = release_files(wf);
    assert_eq!(files, ["OpenTuneBox-Windows-Setup.exe", "OpenTuneBox-Linux.AppImage"]);
    let both = "OpenTuneBox-Windows-Setup.exe et OpenTuneBox-Linux.AppImage";
    assert!(download_files_documented(&files, &[("a", both)]).is_empty());
    assert!(download_files_documented(&files, &[("a", "OpenTuneBox-Windows-Setup.exe")])[0].contains("non décrits"));
    let extra = format!("{both} OpenTuneBox-macOS.dmg");
    assert!(download_files_documented(&files, &[("a", &extra)])[0].contains("ne sont plus publiés"));
    assert!(download_files_documented(&[], &[])[0].contains("aucun fichier"));
}

#[test]
fn commandes_du_readme() {
    assert!(cli_commands_documented(&["info", "report"], "`opentunebox info` et `opentunebox report`").is_empty());
    assert_eq!(
        cli_commands_documented(&["info", "report"], "`opentunebox info`"),
        ["report"]
    );
}

#[test]
fn langue() {
    let francais = "Le diagnostic est fait pour le drone dans la boucle et par les filtres, avec une mesure sur le vol et des conseils.";
    assert!(not_french(francais).is_none());
    let anglais = "The tool is only used when the log is from a drone and the filters are set by the user for the first time, with the help of the guide and the notes.";
    assert!(not_french(anglais).is_some());
    let code = "Le texte est en français, pour la `the and with from when which only are is of in to by for` ligne.";
    assert!(not_french(code).is_none(), "le code entre accents graves ne compte pas");
}

const REG: &str = "| ID | x |\n|---|---|\n| D-002 | a |\n| D-001 | b |\n\n### D-002 : deux\n\n### D-001 : un\n";

#[test]
fn registres() {
    assert!(register_problems("D", REG).is_empty());
    assert_eq!(register_entries("D", REG), ["D-002", "D-001"]);
    let double = format!("{REG}\n### D-001 : encore un\n");
    assert!(register_problems("D", &double)[0].contains("deux fois"));
    let sans_index = REG.replace("| D-001 | b |\n", "");
    assert!(register_problems("D", &sans_index)[0].contains("absents de l'index"));
    let index_en_trop = REG.replace("| D-001 | b |\n", "| D-001 | b |\n| D-009 | c |\n");
    assert!(register_problems("D", &index_en_trop)[0].contains("en trop"));
    assert!(register_problems("D", "rien")[0].contains("aucune entrée"));
    let w = "- ~~**W-003**~~ barré\n- **W-004** actif\n| W-003 | x |\n| W-004 | y |\n";
    assert_eq!(register_entries("W", w), ["W-003", "W-004"]);
    assert!(register_problems("W", w).is_empty());
}

#[test]
fn numéros_cités() {
    let known = BTreeSet::from(["D-001".to_string(), "L-002".to_string()]);
    assert!(unknown_ids("voir D-001 et L-002, pas AD-123 ni D-12", &known).is_empty());
    // Le faux numéro est construit : écrit en toutes lettres, `register_ids_cited_exist` le lirait dans ce fichier.
    let faux = format!("D-{}", 999);
    assert_eq!(
        unknown_ids(&format!("voir {faux} puis W-001"), &known),
        [faux.as_str(), "W-001"]
    );
}

fn profil(key: &str) -> ProfileView {
    ProfileView {
        key: key.into(),
        gyro_noise_high_db: -28.5,
        dterm_noise_high_db: -13.5,
        low_band_high_db: -15.5,
        motor_left_high_db: -29.5,
        latency_ms: (10.0, 21.0),
        gyro_delay_max_ms: 1.8,
        dterm_delay_max_ms: 2.6,
        imbalance_high_pct: 13.5,
        iterm_offset_high: 33.0,
        saturation_high_pct: 1.5,
        roll_pitch_gap_high: 0.12,
        resonance_high_db: 7.9,
    }
}

const ASSISTANT: &str = "Seuils retenus dans `rust/otb-advisor/src/profiles.rs` :\n- −28,5 dB, −13,5, −15,5, −29,5 dB, latence 10–21 ms, retards 1,8 / 2,6 ms.\n\nPour un profil provisoire\n\n| Seuil mécanique | Commun | 5\" | 7\" |\n|---|---|---|---|\n| Déséquilibre des moteurs (%) | 13,5 | 13,5 | 13,5 |\n| Terme I moyen (valeur absolue) | 33 | 33 | 33 |\n| Saturation (% du temps ≥ 98 %) | 1,5 | 1,5 | 1,5 |\n| Écart de pic roll / pitch | 0,12 | 0,12 | 0,12 |\n| Résonance (dB au-dessus du bruit voisin) | 7,9 | 7,9 | 7,9 |\n";

#[test]
fn seuils_de_l_assistant() {
    let profils: Vec<ProfileView> = ["whoop", "micro", "five", "seven", "large"]
        .iter()
        .map(|k| profil(k))
        .collect();
    assert_eq!(assistant_doc_problems(ASSISTANT, &profils), Vec::<String>::new());
    let change = |f: &dyn Fn(&mut ProfileView)| {
        let mut p = profils.clone();
        f(&mut p[2]);
        assistant_doc_problems(ASSISTANT, &p)
    };
    assert!(change(&|p| p.gyro_noise_high_db = -30.1)[0].contains("absent de « Seuils retenus »"));
    assert!(change(&|p| p.latency_ms = (11.0, 21.0))[0].contains("latence"));
    assert!(change(&|p| p.dterm_delay_max_ms = 2.7)[0].contains("retards"));
    assert!(change(&|p| p.imbalance_high_pct = 14.0)[0].contains("Déséquilibre"));
    assert!(change(&|p| p.resonance_high_db = 8.5)[0].contains("5\""));
    let mut seven = profils.clone();
    seven[3].saturation_high_pct = 2.5;
    assert!(assistant_doc_problems(ASSISTANT, &seven)[0].contains("7\""));
    let mut micro = profils.clone();
    micro[1].roll_pitch_gap_high = 0.2;
    assert!(assistant_doc_problems(ASSISTANT, &micro)[0].contains("micro ≠ colonne commune"));
    assert!(assistant_doc_problems("rien", &profils)[0].contains("introuvables"));
    assert!(
        assistant_doc_problems(&ASSISTANT.replace("| Écart de pic roll / pitch", "| Autre"), &profils)[0]
            .contains("lignes du tableau")
    );
}

/// Preuve du contrôle : une valeur changée dans le code et pas dans la documentation doit être signalée.
#[test]
fn seuils_cites_par_la_documentation() {
    let seuils = [SeuilCite {
        constante: "SENSITIVITY_HIGH_DB",
        format: |v| format!("{v:.0} dB"),
        quoi: "pic de sensibilité",
    }];
    let rules = "pub const SENSITIVITY_HIGH_DB: f64 = 7.0;";
    let doc = "le pic de sensibilité doit rester sous 7 dB";

    assert_eq!(
        seuils_cites_concordent(&[("guide", doc)], rules, &seuils),
        Vec::<String>::new()
    );

    // Le code change, la documentation garde l'ancienne valeur : c'est le cas à attraper.
    let rules_change = "pub const SENSITIVITY_HIGH_DB: f64 = 6.0;";
    let p = seuils_cites_concordent(&[("guide", doc)], rules_change, &seuils);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(p[0].contains("6 dB"), "{p:?}");

    // Un document qui ne parle pas du seuil n'est pas en faute.
    assert_eq!(
        seuils_cites_concordent(&[("guide", "rien à dire ici")], rules_change, &seuils),
        Vec::<String>::new()
    );

    // Une constante absente du code est signalée au lieu d'être ignorée en silence.
    let p = seuils_cites_concordent(&[("guide", doc)], "", &seuils);
    assert!(p[0].contains("introuvable"), "{p:?}");

    // La valeur se lit bien, signe compris.
    assert_eq!(constante_f64("pub const X: f64 = -30.0;", "X"), Some(-30.0));
    assert_eq!(constante_f64("pub const X: f64 = -30.0;", "Y"), None);
}
