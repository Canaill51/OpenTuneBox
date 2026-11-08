//! Cohérence de la documentation avec le code (remplace `tests/test_docs.py`, phase 7).
//!
//! Ce que ces tests ne peuvent pas juger (le sens du texte) est décrit dans la compétence `.claude/skills/docs-sync` :
//! quels documents relire selon le type de modification. Les contrôles eux-mêmes sont dans `src/docs.rs` ; `tests/checks.rs`
//! prouve que chacun échoue sur un cas fabriqué.

use otb_advisor::drone_card::DroneCard;
use otb_advisor::profiles::PROFILES;
use otb_advisor::tuning_path::phases;
use otb_dev::docs::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Racine du dépôt ; `OTB_DOCS_ROOT` en désigne une autre (copie sabotée, pour prouver que chaque contrôle échoue).
fn root() -> PathBuf {
    let dir = std::env::var_os("OTB_DOCS_ROOT").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".."),
        PathBuf::from,
    );
    dir.canonicalize().expect("racine du dépôt")
}

fn text(root: &Path, file: &str) -> String {
    read(&root.join(file))
}

fn md_files(root: &Path) -> Vec<PathBuf> {
    let files = tracked(root, &[".md"]);
    assert!(!files.is_empty(), "aucun fichier .md trouvé");
    files
}

/// Appelle `check(fichier, texte)` sur chaque fichier et réunit les problèmes, préfixés du nom du fichier.
fn each(root: &Path, files: &[PathBuf], check: impl Fn(&Path, &str) -> Vec<String>) -> Vec<String> {
    let mut all = Vec::new();
    for f in files {
        for p in check(f, &read(f)) {
            all.push(format!("{} : {p}", rel(root, f)));
        }
    }
    all
}

#[test]
fn relative_links_exist() {
    let root = root();
    let bad = each(&root, &md_files(&root), broken_links);
    assert!(bad.is_empty(), "liens cassés : {bad:#?}");
}

#[test]
fn code_paths_and_symbols_exist() {
    let root = root();
    let bad = each(&root, &md_files(&root), |f, t| {
        let rel = rel(&root, f);
        missing_paths(&root, t, HISTORICAL_DOCS.contains(&rel.as_str()))
    });
    assert!(bad.is_empty(), "chemins ou symboles introuvables : {bad:#?}");
}

fn tool_keys() -> Vec<&'static str> {
    otb_gui::guide::TOOLS.iter().map(|t| t.0).collect()
}

#[test]
fn every_tool_documented() {
    let root = root();
    let pages = otb_gui::tools::all_pages();
    let titles: Vec<&str> = pages.iter().map(|p| p.title()).collect();
    assert!(titles.len() >= 8, "pages d'outils trouvées : {titles:?}");
    let missing = tools_documented(
        &titles,
        &text(&root, "README.md"),
        &text(&root, "docs/guide-utilisateur.md"),
    );
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn guide_links_lead_somewhere() {
    let mut phases: Vec<&str> = otb_gui::guide::PHASE_KEYS.to_vec();
    phases.push(otb_gui::guide::HOME);
    let broken = guide_links(otb_gui::guide::HOME_TEXT, &phases, &tool_keys());
    assert!(broken.is_empty(), "{broken:?}");
}

#[test]
fn guide_steps_reference_real_tools_and_checks() {
    let root = root();
    let keys = log_check_keys(&text(&root, "rust/otb-advisor/src/log_check.rs"));
    // 22 clés distinctes au 2026-11 (autant que la lecture du Python) : un renommage du receveur de `add` ferait tomber ce nombre en silence.
    assert!(
        keys.len() >= 22 && keys.contains("version") && keys.contains("series"),
        "clés de log_check lues : {keys:?}"
    );
    let steps: Vec<StepInfo> = phases(&DroneCard::new("x"))
        .iter()
        .flat_map(|ph| ph.steps.iter())
        .map(|s| StepInfo {
            key: s.key.clone(),
            tool: s.tool.map(String::from),
            part: s.part.map(String::from),
            checks: s.checks.iter().map(|c| c.to_string()).collect(),
            usages: s.usages.iter().map(|c| c.to_string()).collect(),
        })
        .collect();
    assert!(steps.len() > 10, "cases du parcours : {}", steps.len());
    let bad = guide_steps(&steps, &tool_keys(), &keys);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn every_module_documented() {
    let root = root();
    let modules = rust_modules(&root);
    assert!(modules.len() > 50, "modules trouvés : {}", modules.len());
    let texts = text(&root, "AGENTS.md") + &text(&root, "docs/architecture.md");
    let missing = modules_documented(&root, &modules, &texts);
    assert!(
        missing.is_empty(),
        "modules absents de AGENTS.md et docs/architecture.md : {missing:#?}"
    );
}

#[test]
fn docs_index_complete() {
    let root = root();
    let names: Vec<String> = fs::read_dir(root.join("docs"))
        .expect("docs/")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    let missing = docs_index_missing(&names, &text(&root, "docs/README.md"));
    assert!(missing.is_empty(), "documents absents de docs/README.md : {missing:?}");
}

#[test]
fn download_files_documented_in_user_docs() {
    let root = root();
    let published = release_files(&text(&root, ".github/workflows/construire.yml"));
    let (readme, guide) = (text(&root, "README.md"), text(&root, "docs/guide-utilisateur.md"));
    let problems = download_files_documented(
        &published,
        &[("README.md", &readme), ("docs/guide-utilisateur.md", &guide)],
    );
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn cli_commands_documented_in_readme() {
    let root = root();
    let missing = cli_commands_documented(&otb_cli::COMMANDS, &text(&root, "README.md"));
    assert!(missing.is_empty(), "commandes absentes du README : {missing:?}");
}

#[test]
fn documentation_in_french() {
    let root = root();
    let files: Vec<PathBuf> = md_files(&root)
        .into_iter()
        .filter(|p| !rel(&root, p).starts_with(".github"))
        .collect();
    let bad = each(&root, &files, |_, t| not_french(t).into_iter().collect());
    assert!(bad.is_empty(), "{bad:#?}");
}

fn register_text(root: &Path, prefix: &str) -> String {
    let file = REGISTERS.iter().find(|r| r.0 == prefix).expect("registre").1;
    text(root, &format!("docs/{file}"))
}

#[test]
fn register_ids_unique() {
    let root = root();
    for (prefix, ..) in REGISTERS {
        let problems = register_problems(prefix, &register_text(&root, prefix));
        assert!(problems.is_empty(), "registre {prefix} : {problems:#?}");
    }
}

#[test]
fn register_ids_cited_exist() {
    let root = root();
    let known: BTreeSet<String> = REGISTERS
        .iter()
        .flat_map(|(p, ..)| register_entries(p, &register_text(&root, p)))
        .collect();
    let files = tracked(&root, &[".md", ".py", ".yml", ".rs"]);
    let mut unknown = Vec::new();
    for f in files {
        let t = read(&f);
        unknown.extend(
            unknown_ids(&t, &known)
                .into_iter()
                .map(|id| format!("{} : {id}", rel(&root, &f))),
        );
    }
    assert!(unknown.is_empty(), "numéros cités qui n'existent pas : {unknown:#?}");
}

#[test]
fn assistant_doc_matches_profiles() {
    let root = root();
    let views: Vec<ProfileView> = PROFILES
        .iter()
        .map(|p| ProfileView {
            key: p.key.to_string(),
            gyro_noise_high_db: p.gyro_noise_high_db,
            dterm_noise_high_db: p.dterm_noise_high_db,
            low_band_high_db: p.low_band_high_db,
            motor_left_high_db: p.motor_left_high_db,
            latency_ms: p.latency_ms,
            gyro_delay_max_ms: p.gyro_delay_max_ms,
            dterm_delay_max_ms: p.dterm_delay_max_ms,
            imbalance_high_pct: p.imbalance_high_pct,
            iterm_offset_high: p.iterm_offset_high,
            saturation_high_pct: p.saturation_high_pct,
            roll_pitch_gap_high: p.roll_pitch_gap_high,
            resonance_high_db: p.resonance_high_db,
        })
        .collect();
    assert_eq!(views.len(), 5);
    let problems = assistant_doc_problems(&text(&root, "docs/assistant.md"), &views);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Les seuils que le guide et `assistant.md` recopient doivent porter la valeur de leur constante Rust.
#[test]
fn les_seuils_recopies_portent_la_valeur_du_code() {
    let root = root();
    let seuils = [
        SeuilCite {
            constante: "SENSITIVITY_HIGH_DB",
            format: |v| format!("{v:.0} dB"),
            quoi: "pic de sensibilité",
        },
        SeuilCite {
            constante: "PHASE_MARGIN_LOW_DEG",
            format: |v| format!("{v:.0}°"),
            quoi: "marge de phase",
        },
    ];
    let guide = text(&root, "docs/guide-utilisateur.md");
    let assistant = text(&root, "docs/assistant.md");
    let rules = text(&root, "rust/otb-advisor/src/rules.rs");
    let problems = seuils_cites_concordent(
        &[("docs/guide-utilisateur.md", &guide), ("docs/assistant.md", &assistant)],
        &rules,
        &seuils,
    );
    assert!(problems.is_empty(), "{problems:#?}");
}
