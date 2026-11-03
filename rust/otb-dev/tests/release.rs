//! Publication : préparation du CHANGELOG, numéros de version, notes de version (cas de `tests/test_release.py` repris, plus les fichiers).

use otb_dev::release::*;
use std::fs;
use std::path::{Path, PathBuf};

const CHANGELOG: &str = "# Journal des versions

## [Non publié]

### Ajouté
- Page de téléchargement.

## [0.1.0] — 2026-10-01

### Ajouté
- Première version.

[Non publié]: ../../compare/v0.1.0...HEAD
[0.1.0]: ../../releases/tag/v0.1.0
";

#[test]
fn prepare_changelog_dates_section_and_links() {
    let text = prepare_changelog(CHANGELOG, "0.2.0", "2026-10-05").expect("préparation");
    assert!(text.contains("## [Non publié]\n\n## [0.2.0] — 2026-10-05\n\n### Ajouté\n- Page de téléchargement."));
    assert!(text.contains("[Non publié]: ../../compare/v0.2.0...HEAD\n[0.2.0]: ../../releases/tag/v0.2.0\n[0.1.0]:"));
    assert_eq!(
        changelog_section(&text, "0.2.0").as_deref(),
        Some("### Ajouté\n- Page de téléchargement.")
    );
    assert_eq!(
        changelog_section(&text, "0.1.0").as_deref(),
        Some("### Ajouté\n- Première version.")
    );
    // La nouvelle section « Non publié » est vide.
    assert_eq!(changelog_section(&text, "Non publié").as_deref(), Some(""));
}

#[test]
fn prepare_changelog_refuses_existing_version_and_missing_section() {
    assert!(
        prepare_changelog(CHANGELOG, "0.1.0", "2026-10-05")
            .unwrap_err()
            .contains("déjà une section")
    );
    assert!(
        prepare_changelog("# rien\n", "0.2.0", "2026-10-05")
            .unwrap_err()
            .contains("introuvable")
    );
}

#[test]
fn changelog_section_missing_and_last_section() {
    assert_eq!(changelog_section(CHANGELOG, "9.9.9"), None);
    // Dernière section sans lien de bas de page : elle va jusqu'à la fin du texte.
    assert_eq!(
        changelog_section("## [1.0.0] — x\n\nfin\n", "1.0.0").as_deref(),
        Some("fin")
    );
    // « 0.1.0 » ne correspond pas à « 0.1.01 » ni à « 10.1.0 ».
    assert_eq!(changelog_section("## [10.1.0] — x\n\na\n", "0.1.0"), None);
}

#[test]
fn dates_must_be_real() {
    for ok in ["2026-11-03", "2024-02-29", "2000-02-29", "2026-12-31"] {
        assert!(is_valid_date(ok), "{ok}");
    }
    for bad in [
        "2026-13-45",
        "0000-00-00",
        "2026-00-10",
        "2026-02-29",
        "2100-02-29",
        "2026-04-31",
        "2026-1-3",
        "hier",
        "2026-11-03x",
        "",
    ] {
        assert!(!is_valid_date(bad), "{bad}");
    }
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
    let init = "\"\"\"Doc.\"\"\"\n__version__ = \"0.8.0\"\n";
    assert_eq!(
        write_version(&SOURCES[2], init, "0.9.0").as_deref(),
        Some("\"\"\"Doc.\"\"\"\n__version__ = \"0.9.0\"\n")
    );
    assert_eq!(write_version(&SOURCES[1], "rien", "0.9.0"), None);
}

#[test]
fn iso_dates() {
    assert_eq!(iso_date(0), "1970-01-01");
    assert_eq!(iso_date(86_399), "1970-01-01");
    assert_eq!(iso_date(86_400), "1970-01-02");
    assert_eq!(iso_date(951_782_400), "2000-02-29", "année bissextile");
    assert_eq!(iso_date(1_782_000_000), "2026-06-21");
    assert_eq!(iso_date(1_793_664_000), "2026-11-03");
    // Cas limites recoupés avec `date -u -d` : fin de siècle non bissextile, bissextile ordinaire, dernière seconde d'un i32.
    assert_eq!(iso_date(4_107_456_000), "2100-02-28");
    assert_eq!(iso_date(4_107_542_400), "2100-03-01", "2100 n'est pas bissextile");
    assert_eq!(iso_date(1_709_164_800), "2024-02-29");
    assert_eq!(iso_date(2_147_483_647), "2038-01-19");
}

fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("otb-release-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("rust")).expect("dossier");
    fs::write(dir.join("CHANGELOG.md"), CHANGELOG).expect("changelog");
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
    fs::write(root.join("pyproject.toml"), "[project]\nversion = \"0.1.0\"\n").expect("pyproject");
    fs::create_dir_all(root.join("src/opentunebox")).expect("src");
    fs::write(root.join("src/opentunebox/__init__.py"), "__version__ = \"0.1.0\"\n").expect("init");
    // Avant : l'étiquette 0.2.0 est incohérente.
    assert!(
        notes(&root, "v0.2.0")
            .unwrap_err()
            .contains("rust/Cargo.toml indique 0.1.0")
    );
    let msg = prepare(&root, "0.2.0", Some("2026-10-05")).expect("prepare");
    assert!(
        msg.contains("0.1.0 → 0.2.0 (2026-10-05)") && msg.contains("git tag v0.2.0"),
        "{msg}"
    );
    assert!(text(&root, "rust/Cargo.toml").contains("version = \"0.2.0\""));
    assert!(text(&root, "pyproject.toml").contains("version = \"0.2.0\""));
    assert!(text(&root, "src/opentunebox/__init__.py").contains("__version__ = \"0.2.0\""));
    assert!(text(&root, "CHANGELOG.md").contains("## [0.2.0] — 2026-10-05"));
    assert_eq!(
        notes(&root, "v0.2.0").as_deref(),
        Ok("### Ajouté\n- Page de téléchargement.")
    );
    // Une étiquette qui n'est pas le numéro du projet est refusée, avec la liste des fichiers en retard.
    let err = notes(&root, "v0.3.0").unwrap_err();
    assert!(
        err.contains("rust/Cargo.toml indique 0.2.0")
            && err.contains("pas de section [0.3.0]")
            && err.contains("otb-dev release prepare 0.3.0"),
        "{err}"
    );
}

#[test]
fn prepare_without_python_files_and_refusals() {
    let root = sandbox("sans-python");
    assert!(prepare(&root, "v0.2.0", None).unwrap_err().contains("Numéro invalide"));
    assert!(
        prepare(&root, "0.2.0", Some("hier"))
            .unwrap_err()
            .contains("Date invalide")
    );
    assert!(
        prepare(&root, "0.2.0", Some("2026-13-45"))
            .unwrap_err()
            .contains("Date invalide")
    );
    prepare(&root, "0.2.0", Some("2026-10-05")).expect("sans fichiers Python");
    assert_eq!(
        notes(&root, "0.2.0").as_deref(),
        Ok("### Ajouté\n- Page de téléchargement."),
        "l'étiquette sans « v » est acceptée"
    );
    // Une seconde préparation du même numéro échoue sans rien modifier.
    let before = (text(&root, "CHANGELOG.md"), text(&root, "rust/Cargo.toml"));
    assert!(
        prepare(&root, "0.2.0", Some("2026-10-06"))
            .unwrap_err()
            .contains("déjà une section")
    );
    assert_eq!(before, (text(&root, "CHANGELOG.md"), text(&root, "rust/Cargo.toml")));
}

#[test]
fn prepare_changes_nothing_when_a_version_file_is_unreadable() {
    let root = sandbox("incomplet");
    fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"\n").expect("pyproject sans numéro");
    let before = (text(&root, "CHANGELOG.md"), text(&root, "rust/Cargo.toml"));
    assert!(
        prepare(&root, "0.2.0", Some("2026-10-05"))
            .unwrap_err()
            .contains("pyproject.toml")
    );
    assert_eq!(
        before,
        (text(&root, "CHANGELOG.md"), text(&root, "rust/Cargo.toml")),
        "aucun fichier à moitié modifié"
    );
}

#[test]
fn the_real_repository_has_one_version_and_a_changelog_section() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let versions = project_versions(&root).expect("versions");
    assert!(!versions.is_empty());
    assert!(
        versions.iter().all(|(_, v)| *v == versions[0].1),
        "numéros différents : {versions:?}"
    );
    let body = changelog_section(&text(&root, "CHANGELOG.md"), &versions[0].1);
    assert!(
        body.is_some_and(|b| !b.is_empty()),
        "pas de section [{}] dans le CHANGELOG",
        versions[0].1
    );
}
