//! Mentions de licences des composants livrés dans l'exécutable (`THIRD-PARTY-LICENSES.txt`, W-043).
//!
//! `otb-dev licenses` lit, pour chaque crate tierce que `otb-gui` embarque, les fichiers de licence de son paquet (dans les sources
//! téléchargées par cargo), ajoute les polices et les icônes embarquées, et écrit un seul fichier que l'installateur Windows et
//! l'AppImage livrent. Les textes identiques (MIT, Apache…) sont écrits une fois, avec la liste des composants qui les portent ;
//! un composant sans fichier de licence dans son paquet prend les textes copiés de son dépôt d'origine
//! (`rust/otb-dev/upstream-licenses/`, table [`UPSTREAM`]) ; s'il n'y en a pas non plus, il est listé à part, avec la licence
//! qu'il déclare.
//!
//! `otb-dev licenses --check` refait le même texte en mémoire et le compare au fichier du dépôt sans jamais l'écrire : c'est ce que lancent
//! `otb-dev verify` et la CI, pour qu'une dépendance ajoutée ou changée ne passe pas sans mettre les mentions à jour.
//!
//! Les composants viennent de `cargo metadata` pour Linux et pour Windows (les crates propres à un système en font partie) :
//! seules les dépendances **normales** comptent, pas celles de construction ni de test, ni les macros procédurales (jamais liées).

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Cibles livrées (D-042 : ni macOS ni ARM).
pub const TARGETS: [&str; 2] = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"];
/// Le programme livré.
pub const ROOT_PACKAGE: &str = "otb-gui";

/// Une crate tierce livrée.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Component {
    pub name: String,
    pub version: String,
    /// Licence déclarée (SPDX), vide si absente.
    pub license: String,
    /// Auteurs déclarés dans le manifeste, séparés par des virgules (vide si absents).
    pub authors: String,
    pub dir: PathBuf,
}

/// Les crates tierces atteintes depuis `root_name` par des dépendances normales d'un résultat de `cargo metadata`.
pub fn shipped(metadata: &Value, root_name: &str) -> Result<Vec<Component>, String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("metadata : pas de « packages »")?;
    let by_id: BTreeMap<&str, &Value> = packages.iter().filter_map(|p| Some((p["id"].as_str()?, p))).collect();
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("metadata : pas de « resolve »")?;
    let deps_of: BTreeMap<&str, &Vec<Value>> = nodes
        .iter()
        .filter_map(|n| Some((n["id"].as_str()?, n["deps"].as_array()?)))
        .collect();
    let root = packages
        .iter()
        .find(|p| p["name"] == root_name && p["source"].is_null())
        .and_then(|p| p["id"].as_str())
        .ok_or_else(|| format!("paquet {root_name} introuvable"))?;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut todo = vec![root];
    while let Some(id) = todo.pop() {
        if !seen.insert(id) {
            continue;
        }
        for dep in deps_of.get(id).map_or(&[][..], |d| d.as_slice()) {
            // `kind` vaut null pour une dépendance normale, « build » ou « dev » sinon
            let normal = dep["dep_kinds"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k["kind"].is_null()));
            if let (true, Some(pkg)) = (normal, dep["pkg"].as_str()) {
                todo.push(pkg);
            }
        }
    }
    let mut out = Vec::new();
    for id in seen {
        let Some(p) = by_id.get(id) else { continue };
        let external = !p["source"].is_null();
        let proc_macro = p["targets"].as_array().is_some_and(|t| {
            t.iter().any(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|k| k == "proc-macro"))
            })
        });
        if !external || proc_macro {
            continue;
        }
        let manifest = p["manifest_path"].as_str().ok_or("metadata : pas de manifest_path")?;
        out.push(Component {
            name: p["name"].as_str().unwrap_or_default().to_string(),
            version: p["version"].as_str().unwrap_or_default().to_string(),
            license: p["license"].as_str().unwrap_or_default().to_string(),
            authors: p["authors"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
                .unwrap_or_default(),
            dir: Path::new(manifest).parent().map(Path::to_path_buf).unwrap_or_default(),
        });
    }
    out.sort();
    Ok(out)
}

/// Dossier des textes copiés des dépôts d'origine, relatif à la racine du dépôt (provenance : `PROVENANCE.txt`, non livré).
pub const UPSTREAM_DIR: &str = "rust/otb-dev/upstream-licenses";

/// Composants dont le paquet ne contient aucun fichier de licence → dossier de [`UPSTREAM_DIR`] qui porte les textes de leur
/// dépôt d'origine (W-043, lot 21).
/// ⚠️ Sans cette table, ces 22 composants partaient sans ligne de copyright, et le code d'AccessKit issu de Chromium
/// (licence BSD, exigée dans toute distribution binaire) sans son texte.
pub const UPSTREAM: &[(&str, &str)] = &[
    ("accesskit", "accesskit"),
    ("accesskit_atspi_common", "accesskit"),
    ("accesskit_consumer", "accesskit"),
    ("accesskit_unix", "accesskit"),
    ("accesskit_windows", "accesskit"),
    ("accesskit_winit", "accesskit"),
    ("clipboard-win", "clipboard-win"),
    ("ecolor", "egui"),
    ("eframe", "egui"),
    ("egui", "egui"),
    ("egui-wgpu", "egui"),
    ("egui-winit", "egui"),
    ("egui_extras", "egui"),
    ("egui_glow", "egui"),
    ("egui_system_font_provider", "egui"),
    ("emath", "egui"),
    ("epaint", "egui"),
    ("epaint_default_fonts", "egui"),
    ("egui_plot", "egui_plot"),
    ("enum-map", "enum-map"),
    ("profiling", "profiling"),
    ("spirv", "rspirv"),
];

/// Tous les fichiers d'un dossier, triés par nom : (nom, texte normalisé comme [`license_files`]). Vide si le dossier manque.
pub fn all_files(dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return found };
    for e in rd.flatten() {
        if e.path().is_file()
            && let Ok(bytes) = fs::read(e.path())
        {
            let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
            found.push((e.file_name().to_string_lossy().to_string(), text.trim_end().to_string()));
        }
    }
    found.sort();
    found
}

/// Les textes de licence d'un composant : ceux de son paquet, sinon ceux de son dépôt d'origine copiés sous `upstream`
/// (dossier donné par [`UPSTREAM`]), sinon rien.
pub fn component_licenses(c: &Component, upstream: &Path) -> Vec<(String, String)> {
    let own = license_files(&c.dir);
    if !own.is_empty() {
        return own;
    }
    UPSTREAM
        .iter()
        .find(|(name, _)| *name == c.name)
        .map(|(_, dir)| all_files(&upstream.join(dir)))
        .unwrap_or_default()
}

/// Les fichiers de licence à la racine d'un paquet : `LICENSE*`, `LICENCE*`, `COPYING*`, `NOTICE*`, `UNLICENSE*`, `COPYRIGHT*`.
/// Rend (nom, texte) trié par nom ; textes avec fins de ligne `\n`, sans espace final.
pub fn license_files(dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return found };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        let wanted = ["license", "licence", "copying", "notice", "unlicense", "copyright"]
            .iter()
            .any(|p| lower.starts_with(p));
        if wanted
            && e.path().is_file()
            && let Ok(bytes) = fs::read(e.path())
        {
            let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
            found.push((name, text.trim_end().to_string()));
        }
    }
    found.sort();
    found
}

/// `cargo metadata` pour une cible, lu comme JSON.
fn metadata_for(root: &Path, target: &str) -> Result<Value, String> {
    let out = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            target,
            "--manifest-path",
        ])
        .arg(root.join("rust").join("Cargo.toml"))
        .output()
        .map_err(|e| format!("cargo metadata : {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata ({target}) : {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata ({target}) illisible : {e}"))
}

/// Polices et icônes embarquées hors des crates : (titre, texte). Les polices viennent de `epaint_default_fonts` (celles que compile
/// `egui` par défaut : Hack, Ubuntu-Light, egui-icons ; NotoEmoji (OFL) et emoji-icon-font ne le sont qu'avec l'option
/// `monochrome_emoji_fonts`, que nous n'activons pas).
pub fn embedded_assets(root: &Path, metadata: &Value) -> Result<Vec<(String, String)>, String> {
    let fonts = metadata["packages"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["name"] == "epaint_default_fonts"))
        .and_then(|p| p["manifest_path"].as_str())
        .and_then(|m| Path::new(m).parent())
        .map(|d| d.join("fonts"))
        .ok_or("epaint_default_fonts introuvable dans cargo metadata")?;
    let read = |p: &Path| -> Result<String, String> {
        fs::read_to_string(p)
            .map(|t| t.replace("\r\n", "\n").trim_end().to_string())
            .map_err(|e| format!("{} : {e}", p.display()))
    };
    let mut out = vec![
        (
            "Police Hack (texte à chasse fixe et signes de repli ; copyright dans la police : « Copyright (c) 2018 Source Foundry Authors / Copyright (c) 2003 by Bitstream, Inc. »)"
                .to_string(),
            read(&fonts.join("Hack-Regular.txt"))?,
        ),
        (
            "Police Ubuntu Light (texte de l'interface ; copyright dans la police : « Copyright 2011 Canonical Ltd. Licensed under the Ubuntu Font Licence 1.0 »)"
                .to_string(),
            read(&fonts.join("UFL.txt"))?,
        ),
        (
            "Logos Android, Windows, GitHub, git et Apple de egui (egui-icons, sous-ensemble de emoji-icon-font)".to_string(),
            read(&fonts.join("emoji-icon-font-mit-license.txt"))?,
        ),
    ];
    out.push((
        "Icônes Lucide (boutons de l'interface)".to_string(),
        read(&root.join("rust/otb-gui/assets/icons/LICENSE-lucide.txt"))?,
    ));
    Ok(out)
}

/// Le fichier complet.
/// `upstream` : le dossier [`UPSTREAM_DIR`] (textes des composants dont le paquet n'en a pas).
pub fn render(assets: &[(String, String)], components: &[Component], upstream: &Path) -> String {
    let mut s = String::new();
    s.push_str("OpenTuneBox — mentions de licences des composants tiers\n");
    s.push_str("=======================================================\n\n");
    s.push_str(
        "OpenTuneBox embarque les composants ci-dessous. Chacun reste sous sa propre licence, reproduite ici telle que\n\
         son auteur la distribue. Fichier produit par `otb-dev licenses` (ne pas modifier à la main).\n\n",
    );
    s.push_str("1. Polices et icônes\n--------------------\n");
    for (title, text) in assets {
        s.push_str(&format!("\n### {title}\n\n{text}\n"));
    }
    // textes identiques regroupés : texte → composants
    let mut by_text: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut without: Vec<&Component> = Vec::new();
    for c in components {
        let files = component_licenses(c, upstream);
        if files.is_empty() {
            without.push(c);
        }
        for (_, text) in files {
            by_text
                .entry(text)
                .or_default()
                .insert(format!("{} {}", c.name, c.version));
        }
    }
    s.push_str(&format!(
        "\n2. Bibliothèques Rust ({} composants)\n-------------------------------------\n\n\
         Textes des paquets publiés ; pour un composant dont le paquet n'en contient aucun, textes de son dépôt d'origine, avec\n\
         les lignes de copyright relevées dans ses sources quand son texte MIT n'en porte pas.\n",
        components.len()
    ));
    for (i, (text, users)) in by_text.iter().enumerate() {
        let list = users.iter().cloned().collect::<Vec<_>>().join(", ");
        s.push_str(&format!("\n### Texte {} — utilisé par : {list}\n\n{text}\n", i + 1));
    }
    if !without.is_empty() {
        s.push_str("\n3. Composants sans texte de licence (ni dans leur paquet ni copié de leur dépôt)\n---------------------------------------------------------------------------------\n\n");
        s.push_str(
            "Licence et auteurs déclarés dans le manifeste du paquet (champs `license` et `authors`). Les textes standard de ces licences (MIT,\n\
             Apache 2.0…) figurent dans la partie 2 ; le copyright est celui des auteurs nommés.\n\n",
        );
        for c in without {
            let lic = if c.license.is_empty() {
                "non déclarée"
            } else {
                &c.license
            };
            let authors = if c.authors.is_empty() {
                "non déclarés"
            } else {
                &c.authors
            };
            s.push_str(&format!("- {} {} : {lic} ; auteurs : {authors}\n", c.name, c.version));
        }
    }
    s
}

/// Le fichier tel qu'il devrait être, avec le compte des composants et de ceux sans fichier de licence.
pub struct Rendered {
    pub text: String,
    pub components: usize,
    pub missing: usize,
}

/// Produit le texte en mémoire, sans rien écrire (les deux cibles de [`TARGETS`] sont interrogées).
pub fn rendered(root: &Path) -> Result<Rendered, String> {
    let mut merged: BTreeSet<Component> = BTreeSet::new();
    let mut first: Option<Value> = None;
    for target in TARGETS {
        let md = metadata_for(root, target)?;
        merged.extend(shipped(&md, ROOT_PACKAGE)?);
        first.get_or_insert(md);
    }
    let md = first.ok_or("aucune cible")?;
    let components: Vec<Component> = merged.into_iter().collect();
    let upstream = root.join(UPSTREAM_DIR);
    let text = render(&embedded_assets(root, &md)?, &components, &upstream);
    let missing = components
        .iter()
        .filter(|c| component_licenses(c, &upstream).is_empty())
        .count();
    Ok(Rendered {
        text,
        components: components.len(),
        missing,
    })
}

/// Le chemin du fichier livré.
pub fn path_in(root: &Path) -> PathBuf {
    root.join("THIRD-PARTY-LICENSES.txt")
}

/// `otb-dev licenses` : écrit `THIRD-PARTY-LICENSES.txt` à la racine du dépôt ; rend le message à afficher.
pub fn generate(root: &Path) -> Result<String, String> {
    let r = rendered(root)?;
    let path = path_in(root);
    fs::write(&path, &r.text).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(format!(
        "{} : {} composants, {} octets, dont {} sans texte de licence (ni paquet ni dépôt d'origine)",
        path.display(),
        r.components,
        r.text.len(),
        r.missing
    ))
}

/// Ce qu'il faut faire quand le fichier ne correspond plus aux dépendances.
pub const RERUN: &str = "relancer `cargo run -p otb-dev -- licenses`";

/// Compare le texte attendu au contenu du fichier du dépôt (`None` : fichier absent ou illisible).
/// `Ok` : identique ; `Err` : l'écart et le message de réparation. Fonction pure : aucune lecture, aucun `cargo`.
pub fn compare(expected: &str, on_disk: Option<&str>) -> Result<String, String> {
    let Some(found) = on_disk else {
        return Err(format!("THIRD-PARTY-LICENSES.txt est absent ou illisible : {RERUN}"));
    };
    if found == expected {
        return Ok(format!(
            "THIRD-PARTY-LICENSES.txt est à jour ({} octets)",
            expected.len()
        ));
    }
    let line = expected
        .lines()
        .zip(found.lines())
        .position(|(a, b)| a != b)
        .map_or_else(
            || format!("longueurs différentes ({} lignes attendues)", expected.lines().count()),
            |i| format!("première différence à la ligne {}", i + 1),
        );
    Err(format!(
        "THIRD-PARTY-LICENSES.txt ne correspond plus aux dépendances livrées \
         ({} octets attendus, {} trouvés ; {line}) : {RERUN}",
        expected.len(),
        found.len()
    ))
}

/// `otb-dev licenses --check` : compare sans jamais écrire ; `Err` si le fichier diffère.
pub fn check(root: &Path) -> Result<String, String> {
    let r = rendered(root)?;
    let path = path_in(root);
    let on_disk = fs::read(&path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).replace("\r\n", "\n"));
    compare(&r.text, on_disk.as_deref())
}
