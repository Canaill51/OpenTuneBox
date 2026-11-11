//! Préparation et publication d'une version (remplace `scripts/release.py`, phase 7-2, supprimé en 7-5).
//!
//! `otb-dev release prepare 0.9.0` met le numéro dans `rust/Cargo.toml`.
//! `otb-dev release notes v0.9.0 FICHIER` vérifie que l'étiquette correspond au numéro du projet, puis traduit la liste que
//! GitHub génère d'après les titres et les étiquettes `RN: …` des demandes de fusion (`.github/release.yml`, D-056) et
//! y ajoute en tête le bloc Téléchargements : c'est le texte de la page Releases (workflow `construire.yml`).

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
}

pub const SOURCES: [VersionSource; 1] = [VersionSource {
    path: "rust/Cargo.toml",
    pattern: r#"(?m)^version = "([^"]+)""#,
    template: "version = \"{}\"",
    after: Some("[workspace.package]"),
}];

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

/// Bloc placé en tête de chaque page Releases : les deux fichiers à télécharger, rangés par GitHub sous « Assets ».
pub const DOWNLOADS: &str = "## Téléchargements

En bas de cette page, rubrique « Assets » :

- **Windows** : `OpenTuneBox-Windows-Setup.exe`, l'installateur (menu Démarrer, Bureau, ouverture des `.bbl` par double-clic).
- **Linux** : `OpenTuneBox-Linux.AppImage`, un seul fichier : le rendre exécutable, puis double-clic.";

/// Mots fixes de GitHub qui ne doivent plus rester une fois la liste traduite.
const ENGLISH_LEFTOVERS: [&str; 5] = [
    "What's Changed",
    "New Contributors",
    "Full Changelog",
    "made their first contribution",
    " by @",
];

/// Traduit la liste que GitHub génère (`POST /releases/generate-notes`, champ `body`) : titres de section, lignes
/// « … by @auteur in lien », nouveaux contributeurs, lien de comparaison ; le commentaire HTML de tête est retiré.
/// Erreur si la liste ne contient aucune demande de fusion (une page sans notes ne doit pas partir) ou s'il reste un mot
/// fixe en anglais (GitHub a changé sa formulation : mieux vaut s'arrêter que publier un mélange).
pub fn translate_generated_notes(text: &str) -> Result<String, String> {
    let pr = re(r"^\* (.+) by (@\S+) in (https://\S+)$");
    let newcomer = re(r"^\* (@\S+) made their first contribution in (https://\S+)$");
    let full = re(r"^\*\*Full Changelog\*\*: (\S+)$");
    let mut lines = Vec::new();
    let mut prs = 0;
    for line in text.replace("\r\n", "\n").lines() {
        let line = line.trim_end();
        if line.starts_with("<!--") && line.ends_with("-->") {
            continue;
        }
        let out = if line == "## What's Changed" {
            "## Changements".to_string()
        } else if line == "## New Contributors" {
            "## Nouveaux contributeurs".to_string()
        } else if let Some(c) = newcomer.captures(line) {
            format!("* {} a contribué pour la première fois dans {}", &c[1], &c[2])
        } else if let Some(c) = pr.captures(line) {
            prs += 1;
            format!("* {} par {} dans {}", &c[1], &c[2], &c[3])
        } else if let Some(c) = full.captures(line) {
            format!("**Toutes les modifications** : {}", &c[1])
        } else {
            line.to_string()
        };
        lines.push(out);
    }
    if prs == 0 {
        return Err("la liste générée par GitHub ne contient aucune demande de fusion : vérifier `previous_tag_name` et les étiquettes `RN: …`".into());
    }
    let out = lines.join("\n").trim().to_string();
    if let Some(word) = ENGLISH_LEFTOVERS.iter().find(|w| out.contains(*w)) {
        return Err(format!(
            "mot fixe de GitHub non traduit : « {word} » (traduction à compléter dans release.rs)"
        ));
    }
    Ok(out)
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
        let text = read(&path)?;
        let v = read_version(src, &text).ok_or_else(|| format!("{} : numéro de version introuvable", src.path))?;
        found.push((src.path, v));
    }
    Ok(found)
}

/// `otb-dev release prepare X.Y.Z` : met le numéro dans les fichiers de version et rend le message à afficher.
pub fn prepare(root: &Path, version: &str) -> Result<String, String> {
    if !is_version(version) {
        return Err(format!("Numéro invalide : {version:?} (attendu : 0.2.0, sans « v »)."));
    }
    let old = project_versions(root)?.first().map(|v| v.1.clone()).unwrap_or_default();
    // Tout est calculé avant la première écriture : une erreur ne laisse pas des fichiers à moitié modifiés.
    let mut writes = Vec::new();
    for src in &SOURCES {
        let path = root.join(src.path);
        let new = write_version(src, &read(&path)?, version)
            .ok_or_else(|| format!("{} : numéro de version introuvable", src.path))?;
        writes.push((path, new));
    }
    for (path, text) in &writes {
        write(path, text)?;
    }
    let names: Vec<&str> = SOURCES.iter().map(|s| s.path).collect();
    Ok(format!(
        "Version {old} → {version} : {} modifié.\nLancer `cd rust && cargo check` (met Cargo.lock à jour), faire valider la demande de fusion, puis sur main :\n  git tag v{version} && git push origin v{version}\nLes notes de la page Releases sont générées par GitHub d'après les titres et les étiquettes `RN: …` des demandes de fusion.",
        names.join(", ")
    ))
}

/// `otb-dev release notes vX.Y.Z FICHIER` : vérifie que l'étiquette est le numéro du projet, puis rend le texte de la page
/// Releases (bloc Téléchargements + liste générée par GitHub, lue dans `generated` et traduite).
pub fn notes(root: &Path, tag: &str, generated: &str) -> Result<String, String> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let errors: Vec<String> = project_versions(root)?
        .into_iter()
        .filter(|(_, found)| found != version)
        .map(|(name, found)| format!("{name} indique {found}"))
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "Étiquette {tag} incohérente : {}.\nLancer d'abord : otb-dev release prepare {version}",
            errors.join(" ; ")
        ));
    }
    let list = translate_generated_notes(generated)?;
    Ok(format!("{DOWNLOADS}\n\n{list}"))
}
