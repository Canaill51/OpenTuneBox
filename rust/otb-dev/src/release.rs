//! Préparation et publication d'une version (remplace `scripts/release.py`, phase 7-2).
//!
//! `otb-dev release prepare 0.9.0` met le numéro dans `rust/Cargo.toml` (et dans `pyproject.toml` / `__init__.py` tant qu'ils existent),
//! transforme la section « Non publié » du CHANGELOG en « [0.9.0] — date » et en rouvre une vide.
//! `otb-dev release notes v0.9.0` vérifie que l'étiquette correspond aux numéros du projet et au CHANGELOG, puis affiche la section
//! de cette version (texte de la page Releases ; workflow `construire.yml`).

use regex::Regex;
use std::fs;
use std::path::Path;

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| panic!("expression invalide {pattern} : {e}"))
}

/// Un fichier qui porte le numéro de version : chemin relatif au dépôt, motif (groupe 1 = numéro), modèle d'écriture, repère après lequel chercher.
pub struct VersionSource {
    pub path: &'static str,
    pub pattern: &'static str,
    pub template: &'static str,
    pub after: Option<&'static str>,
    /// Absent du dépôt : on l'ignore (les fichiers Python disparaissent en 7-5) ; sinon une erreur.
    pub optional: bool,
}

pub const SOURCES: [VersionSource; 3] = [
    VersionSource {
        path: "rust/Cargo.toml",
        pattern: r#"(?m)^version = "([^"]+)""#,
        template: "version = \"{}\"",
        after: Some("[workspace.package]"),
        optional: false,
    },
    VersionSource {
        path: "pyproject.toml",
        pattern: r#"(?m)^version = "([^"]+)""#,
        template: "version = \"{}\"",
        after: None,
        optional: true,
    },
    VersionSource {
        path: "src/opentunebox/__init__.py",
        pattern: r#"(?m)^__version__ = "([^"]+)""#,
        template: "__version__ = \"{}\"",
        after: None,
        optional: true,
    },
];

/// `0.2.0`, sans « v ».
pub fn is_version(s: &str) -> bool {
    re(r"^\d+\.\d+\.\d+$").is_match(s)
}

/// Le numéro lu dans le texte d'un fichier de version (après le repère s'il y en a un).
pub fn read_version(src: &VersionSource, text: &str) -> Option<String> {
    let from = match src.after {
        Some(marker) => text.find(marker)?,
        None => 0,
    };
    re(src.pattern).captures(&text[from..]).map(|c| c[1].to_string())
}

/// Le texte avec le nouveau numéro ; `None` si le numéro n'a pas été trouvé.
pub fn write_version(src: &VersionSource, text: &str, version: &str) -> Option<String> {
    let from = match src.after {
        Some(marker) => text.find(marker)?,
        None => 0,
    };
    let m = re(src.pattern).find(&text[from..])?;
    let new = src.template.replace("{}", version);
    Some(format!(
        "{}{}{}",
        &text[..from + m.start()],
        new,
        &text[from + m.end()..]
    ))
}

/// Corps de la section « ## [version] » du CHANGELOG (sans le titre), ou `None` si elle manque.
pub fn changelog_section(text: &str, version: &str) -> Option<String> {
    let head = re(&format!(r"(?m)^## \[{}\][^\n]*\n", regex::escape(version))).find(text)?;
    let rest = &text[head.end()..];
    // La section s'arrête au titre suivant ou aux liens de bas de page.
    let end = re(r"(?m)^## \[|^\[[^\]]+\]: ")
        .find(rest)
        .map_or(rest.len(), |m| m.start());
    Some(rest[..end].trim().to_string())
}

/// Renomme « Non publié » en version datée, rouvre une section vide et met à jour les liens de bas de page.
pub fn prepare_changelog(text: &str, version: &str, date: &str) -> Result<String, String> {
    if text.contains(&format!("## [{version}]")) {
        return Err(format!("Le CHANGELOG contient déjà une section [{version}]."));
    }
    if !text.contains("## [Non publié]") {
        return Err("Section « ## [Non publié] » introuvable dans le CHANGELOG.".into());
    }
    let text = text.replacen(
        "## [Non publié]",
        &format!("## [Non publié]\n\n## [{version}] — {date}"),
        1,
    );
    // Liens de bas de page : « Non publié » compare désormais à la nouvelle version.
    let link = re(r"(?m)^\[Non publié\]: \.\./\.\./compare/v\d+\.\d+\.\d+\.\.\.HEAD$");
    let new = format!("[Non publié]: ../../compare/v{version}...HEAD\n[{version}]: ../../releases/tag/v{version}");
    Ok(link.replace(&text, regex::NoExpand(&new)).into_owned())
}

/// Date civile (UTC) d'un nombre de secondes depuis 1970, au format `AAAA-MM-JJ` (algorithme de H. Hinnant, jours depuis 1970-01-01).
pub fn iso_date(unix_seconds: u64) -> String {
    let z = (unix_seconds / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn today_utc() -> Result<String, String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "horloge antérieure à 1970 : donner la date avec --date AAAA-MM-JJ".to_string())?
        .as_secs();
    Ok(iso_date(secs))
}

/// `AAAA-MM-JJ` d'une vraie date : mois de 1 à 12, jour dans le mois (années bissextiles comprises).
pub fn is_valid_date(s: &str) -> bool {
    let Some(c) = re(r"^(\d{4})-(\d{2})-(\d{2})$").captures(s) else {
        return false;
    };
    let (y, m, d): (u32, u32, u32) = (
        c[1].parse().unwrap_or(0),
        c[2].parse().unwrap_or(0),
        c[3].parse().unwrap_or(0),
    );
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 28 + u32::from(leap),
        _ => return false,
    };
    y >= 1970 && (1..=days).contains(&d)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|t| t.replace("\r\n", "\n"))
        .map_err(|e| format!("lecture de {} impossible : {e}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("écriture de {} impossible : {e}", path.display()))
}

/// Les numéros des fichiers de version présents : (chemin, numéro lu).
pub fn project_versions(root: &Path) -> Result<Vec<(&'static str, String)>, String> {
    let mut found = Vec::new();
    for src in &SOURCES {
        let path = root.join(src.path);
        if !path.exists() && src.optional {
            continue;
        }
        let text = read(&path)?;
        let v = read_version(src, &text).ok_or_else(|| format!("{} : numéro de version introuvable", src.path))?;
        found.push((src.path, v));
    }
    Ok(found)
}

/// `otb-dev release prepare X.Y.Z [--date AAAA-MM-JJ]` : rend le message à afficher. Sans `--date`, la date est celle d'UTC.
pub fn prepare(root: &Path, version: &str, date: Option<&str>) -> Result<String, String> {
    if !is_version(version) {
        return Err(format!("Numéro invalide : {version:?} (attendu : 0.2.0, sans « v »)."));
    }
    if let Some(d) = date
        && !is_valid_date(d)
    {
        return Err(format!("Date invalide : {d:?} (attendu : 2026-11-03)."));
    }
    let old = project_versions(root)?.first().map(|v| v.1.clone()).unwrap_or_default();
    let date = date.map_or_else(today_utc, |d| Ok(d.to_string()))?;
    let changelog = root.join("CHANGELOG.md");
    let new_changelog = prepare_changelog(&read(&changelog)?, version, &date)?;
    // Tout est calculé avant la première écriture : une erreur ne laisse pas des fichiers à moitié modifiés.
    let mut writes = vec![(changelog, new_changelog)];
    let mut names = vec!["CHANGELOG.md"];
    for src in &SOURCES {
        let path = root.join(src.path);
        if !path.exists() && src.optional {
            continue;
        }
        let new = write_version(src, &read(&path)?, version)
            .ok_or_else(|| format!("{} : numéro de version introuvable", src.path))?;
        writes.push((path, new));
        names.push(src.path);
    }
    for (path, text) in &writes {
        write(path, text)?;
    }
    Ok(format!(
        "Version {old} → {version} ({date}) : {} modifiés.\nLancer `cd rust && cargo check` (met Cargo.lock à jour), relire le CHANGELOG, faire valider la demande de fusion, puis sur main :\n  git tag v{version} && git push origin v{version}",
        names.join(", ")
    ))
}

/// `otb-dev release notes vX.Y.Z` : le texte de la page Releases, ou l'incohérence trouvée.
pub fn notes(root: &Path, tag: &str) -> Result<String, String> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let mut errors: Vec<String> = project_versions(root)?
        .into_iter()
        .filter(|(_, found)| found != version)
        .map(|(name, found)| format!("{name} indique {found}"))
        .collect();
    let body = changelog_section(&read(&root.join("CHANGELOG.md"))?, version).filter(|b| !b.is_empty());
    if body.is_none() {
        errors.push(format!(
            "le CHANGELOG n'a pas de section [{version}] (ou elle est vide)"
        ));
    }
    match body {
        Some(b) if errors.is_empty() => Ok(b),
        _ => Err(format!(
            "Étiquette {tag} incohérente : {}.\nLancer d'abord : otb-dev release prepare {version}",
            errors.join(" ; ")
        )),
    }
}
