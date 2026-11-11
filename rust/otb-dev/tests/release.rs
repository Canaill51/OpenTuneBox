//! Publication : numéros de version, traduction de la liste générée par GitHub, notes de version (D-056).

use otb_dev::release::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Liste telle que `POST /repos/…/releases/generate-notes` la rend avec `.github/release.yml` (relevée le 2026-11-11, format
/// réel ; titres et numéros inventés).
const GENERATED: &str = "<!-- Release notes generated using configuration in .github/release.yml at main -->

## What's Changed
### Nouveautés
* Ouverture de plusieurs logs à la fois by @Canaill51 in https://github.com/Canaill51/OpenTuneBox/pull/134
### Corrections
* L'export d'image n'attend plus indéfiniment by @Canaill51 in https://github.com/Canaill51/OpenTuneBox/pull/137
* Analyse du rapport par écrit (by design) by @pilote-x in https://github.com/Canaill51/OpenTuneBox/pull/150

## New Contributors
* @pilote-x made their first contribution in https://github.com/Canaill51/OpenTuneBox/pull/150

**Full Changelog**: https://github.com/Canaill51/OpenTuneBox/compare/v0.9.0...v0.10.0
";

const TRANSLATED: &str = "## Changements
### Nouveautés
* Ouverture de plusieurs logs à la fois par @Canaill51 dans https://github.com/Canaill51/OpenTuneBox/pull/134
### Corrections
* L'export d'image n'attend plus indéfiniment par @Canaill51 dans https://github.com/Canaill51/OpenTuneBox/pull/137
* Analyse du rapport par écrit (by design) par @pilote-x dans https://github.com/Canaill51/OpenTuneBox/pull/150

## Nouveaux contributeurs
* @pilote-x a contribué pour la première fois dans https://github.com/Canaill51/OpenTuneBox/pull/150

**Toutes les modifications** : https://github.com/Canaill51/OpenTuneBox/compare/v0.9.0...v0.10.0";

#[test]
fn generated_notes_are_translated() {
    assert_eq!(translate_generated_notes(GENERATED).as_deref(), Ok(TRANSLATED));
    // Fins de ligne Windows : même résultat.
    assert_eq!(
        translate_generated_notes(&GENERATED.replace('\n', "\r\n")).as_deref(),
        Ok(TRANSLATED)
    );
}

#[test]
fn empty_or_untranslated_lists_are_refused() {
    // Aucune demande de fusion (mauvaise version de départ, ou toutes exclues) : pas de page sans notes.
    let empty = "**Full Changelog**: https://github.com/Canaill51/OpenTuneBox/compare/v0.9.0...v0.10.0\n";
    assert!(
        translate_generated_notes(empty)
            .unwrap_err()
            .contains("aucune demande de fusion")
    );
    // GitHub change un mot fixe : on s'arrête plutôt que publier un mélange de français et d'anglais.
    let changed = GENERATED.replace("made their first contribution", "made their first contribution!");
    let err = translate_generated_notes(&changed).unwrap_err();
    assert!(err.contains("made their first contribution"), "{err}");
}

#[test]
fn version_numbers() {
    assert!(is_version("0.9.0") && is_version("10.20.300"));
    for bad in ["v0.9.0", "0.9", "0.9.0.1", "0.9.x", "", "0.9.0\n"] {
        assert!(!is_version(bad), "{bad:?}");
    }
}

#[test]
fn read_and_write_version_files() {
    let cargo = "[workspace]\nversion = \"9.9.9\"\n\n[workspace.package]\nversion = \"0.8.0\"\nedition = \"2024\"\n";
    let src = &SOURCES[0];
    assert_eq!(
        read_version(src, cargo).as_deref(),
        Some("0.8.0"),
        "le numéro est lu après [workspace.package]"
    );
    assert_eq!(
        write_version(src, cargo, "0.9.0").as_deref(),
        Some("[workspace]\nversion = \"9.9.9\"\n\n[workspace.package]\nversion = \"0.9.0\"\nedition = \"2024\"\n")
    );
    assert_eq!(
        read_version(src, "[package]\nversion = \"1.0.0\"\n"),
        None,
        "repère absent"
    );
    assert_eq!(write_version(src, "rien", "0.9.0"), None);
}

fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("otb-release-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("rust")).expect("dossier");
    fs::write(
        dir.join("rust/Cargo.toml"),
        "[workspace.package]\nversion = \"0.1.0\"\n",
    )
    .expect("cargo");
    dir
}

fn text(root: &Path, file: &str) -> String {
    fs::read_to_string(root.join(file)).expect(file)
}

#[test]
fn prepare_then_notes_on_a_repository() {
    let root = sandbox("complet");
    // Avant : l'étiquette 0.2.0 est incohérente, quelle que soit la liste.
    let err = notes(&root, "v0.2.0", GENERATED).unwrap_err();
    assert!(
        err.contains("rust/Cargo.toml indique 0.1.0") && err.contains("otb-dev release prepare 0.2.0"),
        "{err}"
    );
    let msg = prepare(&root, "0.2.0").expect("prepare");
    assert!(msg.contains("0.1.0 → 0.2.0") && msg.contains("git tag v0.2.0"), "{msg}");
    assert!(text(&root, "rust/Cargo.toml").contains("version = \"0.2.0\""));
    // Le texte de la page : Téléchargements en tête, puis la liste traduite.
    assert_eq!(
        notes(&root, "v0.2.0", GENERATED).as_deref(),
        Ok(format!("{DOWNLOADS}\n\n{TRANSLATED}").as_str())
    );
    assert!(DOWNLOADS.contains("OpenTuneBox-Windows-Setup.exe") && DOWNLOADS.contains("OpenTuneBox-Linux.AppImage"));
    assert_eq!(
        notes(&root, "0.2.0", GENERATED).as_deref(),
        Ok(format!("{DOWNLOADS}\n\n{TRANSLATED}").as_str()),
        "l'étiquette sans « v » est acceptée"
    );
    // Liste vide : refusée même quand le numéro est bon.
    assert!(
        notes(&root, "v0.2.0", "")
            .unwrap_err()
            .contains("aucune demande de fusion")
    );
}

#[test]
fn prepare_refuses_bad_numbers_and_changes_nothing_when_a_version_file_is_unreadable() {
    let root = sandbox("refus");
    assert!(prepare(&root, "v0.2.0").unwrap_err().contains("Numéro invalide"));
    assert_eq!(
        text(&root, "rust/Cargo.toml"),
        "[workspace.package]\nversion = \"0.1.0\"\n"
    );
    fs::write(root.join("rust/Cargo.toml"), "[workspace.package]\nname = \"x\"\n").expect("Cargo.toml sans numéro");
    assert!(prepare(&root, "0.2.0").unwrap_err().contains("rust/Cargo.toml"));
    assert_eq!(
        text(&root, "rust/Cargo.toml"),
        "[workspace.package]\nname = \"x\"\n",
        "aucun fichier à moitié modifié"
    );
}

#[test]
fn the_real_repository_has_one_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let versions = project_versions(&root).expect("versions");
    assert!(!versions.is_empty());
    assert!(
        versions.iter().all(|(_, v)| *v == versions[0].1),
        "numéros différents : {versions:?}"
    );
}

/// Étiquettes `RN: …` citées dans un fichier, entre guillemets.
fn rn_labels(text: &str) -> std::collections::BTreeSet<String> {
    regex::Regex::new(r#""(RN: [^"]+)""#)
        .expect("motif")
        .captures_iter(text)
        .map(|c| c[1].to_string())
        .collect()
}

/// ⚠️ Une étiquette que le contrôle accepte sans que `release.yml` la range ferait disparaître la demande des notes (D-056).
#[test]
fn label_check_and_release_sections_use_the_same_labels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let sections = rn_labels(&text(&root, ".github/release.yml"));
    let check = rn_labels(&text(&root, ".github/workflows/etiquette-pr.yml"));
    assert_eq!(sections.len(), 7, "{sections:?}");
    assert_eq!(
        sections, check,
        "release.yml et etiquette-pr.yml n'ont pas les mêmes étiquettes « RN: … »"
    );
}
