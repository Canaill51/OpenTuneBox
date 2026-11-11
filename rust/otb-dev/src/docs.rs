//! Les contrôles de la documentation (voir `lib.rs`). Les noms suivent ceux de `tests/test_docs.py`.

use otb_format::fixed;
use regex::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Une expression régulière écrite dans ce fichier : une faute de frappe est une erreur de programmation.
fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| panic!("expression invalide {pattern} : {e}"))
}

/// Le texte sans ses blocs ```…``` (Python : `re.sub(r"```.*?```", "", text, flags=re.S)`).
pub fn strip_fences(text: &str) -> String {
    re(r"(?s)```.*?```").replace_all(text, "").into_owned()
}

/// Lit un fichier en UTF-8, fins de ligne ramenées à « \n » (comme la lecture de texte de Python) ; panique s'il est illisible.
pub fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("lecture de {} impossible : {e}", path.display()))
        .replace("\r\n", "\n")
}

/// Chemin relatif au dépôt, avec des « / » (même texte sous Windows).
pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Fichiers suivis par git qui se terminent par `suffixes`. Sans dépôt git (copie de travail sans `.git`), les fichiers de la racine et de `docs/` ;
/// si `root/.git` existe et que git échoue, panique plutôt que de contrôler moins de fichiers sans le dire.
pub fn tracked(root: &Path, suffixes: &[&str]) -> Vec<PathBuf> {
    let out = Command::new("git").args(["ls-files", "-z"]).current_dir(root).output();
    let mut files: Vec<PathBuf> = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .split('\0')
            .filter(|f| !f.is_empty() && suffixes.iter().any(|s| f.ends_with(s)))
            .map(|f| root.join(f))
            .collect(),
        other => {
            assert!(
                !root.join(".git").exists(),
                "git ls-files a échoué dans {} ({other:?}) : les contrôles ne verraient qu'une partie des fichiers",
                root.display()
            );
            Vec::new()
        }
    };
    if files.is_empty() {
        for dir in [root.to_path_buf(), root.join("docs")] {
            if let Ok(rd) = fs::read_dir(dir) {
                files.extend(rd.flatten().map(|e| e.path()).filter(|p| {
                    let name = p.to_string_lossy();
                    suffixes.iter().any(|s| name.ends_with(s))
                }));
            }
        }
    }
    files.sort();
    files.retain(|p| p.exists());
    files
}

// ------------------------------------------------------------------------------------------------ liens

/// Ancre GitHub d'un titre : minuscules, ponctuation retirée, espaces → tirets.
pub fn slug(heading: &str) -> String {
    re(r"[^\w\- ]")
        .replace_all(&heading.trim().to_lowercase(), "")
        .replace(' ', "-")
}

pub fn anchors(text: &str) -> BTreeSet<String> {
    let text = strip_fences(text);
    re(r"(?m)^#{1,6}\s+(.+)$")
        .captures_iter(&text)
        .map(|c| slug(&c[1]))
        .collect()
}

/// Liens `[texte](chemin#ancre)` vers des fichiers du dépôt : le fichier et l'ancre existent.
pub fn broken_links(md: &Path, text: &str) -> Vec<String> {
    let text = strip_fences(text);
    let mut broken = Vec::new();
    for c in re(r"\]\(([^)\s]+)\)").captures_iter(&text) {
        let target = &c[1];
        // Pages GitHub (Releases…), liens externes, liens internes du guide (voir `guide_links`).
        if ["http://", "https://", "mailto:", "../../", "otb:"]
            .iter()
            .any(|p| target.starts_with(p))
        {
            continue;
        }
        let (file_part, anchor) = target.split_once('#').unwrap_or((target, ""));
        let dest = if file_part.is_empty() {
            md.to_path_buf()
        } else {
            md.parent().unwrap_or(Path::new("")).join(file_part)
        };
        if !dest.exists() {
            broken.push(format!("{target} (fichier absent)"));
        } else if !anchor.is_empty()
            && dest.extension().is_some_and(|e| e == "md")
            && !anchors(&read(&dest)).contains(anchor)
        {
            broken.push(format!("{target} (titre absent)"));
        }
    }
    broken
}

// ------------------------------------------------------------------------------------------------ chemins et symboles

/// Préfixes des chemins du dépôt cités entre `…`. Ceux de `gui/`, `analysis/`… (relatifs à l'ancien `src/opentunebox/`, Python) ne
/// sont plus acceptés que dans les documents historiques (`HISTORICAL_DOCS`).
pub const REPO_PREFIXES: [&str; 8] = [
    "src/opentunebox/",
    "tests/",
    "scripts/",
    "packaging/",
    "docs/",
    ".github/",
    ".claude/",
    "rust/",
];
/// Chemins supprimés du dépôt mais cités par les registres d'histoire (l'historique les garde) : jamais signalés.
pub const REMOVED_PATHS: [&str; 2] = ["rust/otb-spike-egui", "rust/otb-py"];
pub const PKG_DIRS: [&str; 5] = ["decoder/", "log/", "analysis/", "advisor/", "gui/"];
/// Documents qui racontent l'histoire du projet : ils peuvent citer des fichiers Python disparus à la phase 7-5 (D-035). Les autres
/// documents décrivent le dépôt tel qu'il est et ne citent que des chemins qui existent.
pub const HISTORICAL_DOCS: [&str; 4] = [
    "docs/decisions.md",
    "docs/lecons.md",
    "docs/migration-rust.md",
    "docs/travaux-reportes.md",
];

/// Le chemin désigne un fichier de l'ancienne version Python (disparue en 7-5) : module sous `src/opentunebox/`, script, test, emballage.
pub fn is_removed_python_path(path: &str) -> bool {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    path.starts_with("src/opentunebox/")
        || PKG_DIRS.iter().any(|p| path.starts_with(p))
        || path.starts_with("tools/")
        || path.starts_with("widgets/")
        || path.starts_with("scripts/")
        || path.starts_with("packaging/macos")
        || matches!(ext, "py" | "spec" | "icns")
}

/// Dossiers produits par la compilation : leur contenu existe sur un poste où l'on a construit, jamais dans un dépôt
/// fraîchement cloné.
pub const BUILD_DIRS: [&str; 3] = ["target/", "dist/", "build/"];

/// Le chemin désigne un fichier **produit par la compilation** *et* porte le préfixe du dépôt (`rust/target/release/…`).
/// La forme admise par L-079 — le même chemin **sans** ce préfixe (« le fichier `target/release/…` du dossier `rust` ») —
/// rend `false` : c'est justement celle qu'il faut écrire.
/// ⚠️ Un tel chemin est signalé qu'il existe ou non : sinon le contrôle passe chez celui qui a compilé et échoue sur la CI (L-079)
pub fn is_build_output_path(path: &str) -> bool {
    REPO_PREFIXES.iter().any(|p| path.starts_with(p)) && BUILD_DIRS.iter().any(|d| path.contains(&format!("/{d}")))
}

/// Le symbole est déclaré dans un fichier Rust : `fn`, `struct`, `enum`, `trait`, `const`, `static`, `type`, `mod`, `macro_rules!`.
pub fn rs_symbol_defined(text: &str, name: &str) -> bool {
    let n = regex::escape(name);
    re(&format!(
        r"(?m)\b(?:fn|struct|enum|trait|const|static|type|mod|union)\s+{n}\b|macro_rules!\s*{n}\b"
    ))
    .is_match(text)
}

/// Chemins cités entre `…` (`rust/otb-gui/src/app.rs:App`, `packaging/…`) : le fichier existe, et le symbole s'il est donné.
/// `historical` : le document est un registre d'histoire, un chemin Python disparu n'y est pas signalé.
pub fn missing_paths(root: &Path, text: &str, historical: bool) -> Vec<String> {
    let src = root.join("src").join("opentunebox");
    let span_re = re(r"^([\w./-]+?)(?::(\w+))?$");
    let mut missing = Vec::new();
    for c in re(r"`([^`\n]+)`").captures_iter(&strip_fences(text)) {
        let span = &c[1];
        let Some(m) = span_re.captures(span.trim()) else {
            continue;
        };
        if !m[1].contains('/') || span.contains('*') {
            continue;
        }
        let path = m[1].trim_end_matches('/');
        let symbol = m.get(2).map(|s| s.as_str());
        if REMOVED_PATHS.iter().any(|r| path.starts_with(r)) {
            continue;
        }
        if historical && is_removed_python_path(path) && !root.join(path).exists() {
            continue;
        }
        // ⚠️ signalé même s'il existe : sinon le contrôle passe sur un poste qui a compilé et échoue sur la CI (L-079)
        if is_build_output_path(path) {
            missing.push(format!(
                "{span} (fichier de compilation : le citer sans le préfixe du dépôt)"
            ));
            continue;
        }
        let candidates: Vec<PathBuf> = if REPO_PREFIXES.iter().any(|p| path.starts_with(p)) {
            vec![root.join(path)]
        } else if PKG_DIRS.iter().any(|p| path.starts_with(p))
            || path.starts_with("tools/")
            || path.starts_with("widgets/")
        {
            vec![src.join(path), src.join("gui").join(path)]
        } else {
            continue; // chemins du firmware Betaflight, noms de branches…
        };
        match candidates.iter().find(|c| c.exists()) {
            None => missing.push(span.to_string()),
            Some(found) => {
                if let Some(sym) = symbol {
                    let ext = found.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let ok = match ext {
                        "rs" => rs_symbol_defined(&read(found), sym),
                        _ => true,
                    };
                    if !ok {
                        missing.push(format!("{span} (symbole absent)"));
                    }
                }
            }
        }
    }
    missing
}

// ------------------------------------------------------------------------------------------------ outils et guide

/// Chaque outil (titre de sa page) figure dans une ligne du tableau du README (colonne « Bouton » ou « Outil ») et dans un titre `###` du guide
/// (`Titre`, `Titre (Bouton)` ou `Nom (Titre)`).
pub fn tools_documented(titles: &[&str], readme: &str, guide: &str) -> Vec<String> {
    let guide_titles: Vec<String> = re(r"(?m)^### (.+)$")
        .captures_iter(guide)
        .map(|c| c[1].to_string())
        .collect();
    let rows: Vec<Vec<String>> = readme
        .lines()
        .filter(|l| l.starts_with("| "))
        .map(|l| l.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect())
        .collect();
    let mut missing = Vec::new();
    for t in titles {
        if !rows.iter().any(|r| r.iter().take(2).any(|c| c == t)) {
            missing.push(format!("README : ligne « | … {t} … | » du tableau des outils"));
        }
        let ok = guide_titles
            .iter()
            .any(|g| g == t || g.starts_with(&format!("{t} (")) || g.ends_with(&format!("({t})")));
        if !ok {
            missing.push(format!("guide : titre « ### {t} » (ou « … ({t}) »)"));
        }
    }
    missing
}

/// Guide intégré : chaque lien `otb:phase/…` et `otb:tool/…` de l'accueil mène quelque part.
pub fn guide_links(home: &str, phase_keys: &[&str], tool_keys: &[&str]) -> Vec<String> {
    let mut broken = Vec::new();
    for c in re(r"\]\(otb:(\w+)/(\w+)\)").captures_iter(home) {
        let (kind, key) = (&c[1], &c[2]);
        let ok = match kind {
            "phase" => phase_keys.contains(&key),
            "tool" => tool_keys.contains(&key),
            _ => false,
        };
        if !ok {
            broken.push(format!("otb:{kind}/{key}"));
        }
    }
    broken
}

/// Une case du parcours de réglage, vue par le contrôle (`otb_advisor::tuning_path::Step`).
#[derive(Debug, Clone)]
pub struct StepInfo {
    pub key: String,
    pub tool: Option<String>,
    pub part: Option<String>,
    pub checks: Vec<String>,
    pub usages: Vec<String>,
}

/// Clés des points de contrôle et des usages déclarés dans le code de `log_check.rs`.
pub fn log_check_keys(source: &str) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = re(r#"c\.add\(\s*"(\w+)""#)
        .captures_iter(source)
        .map(|c| c[1].to_string())
        .collect();
    keys.extend(
        re(r#"key:\s*"(\w+)"\s*\.into\(\)"#)
            .captures_iter(source)
            .map(|c| c[1].to_string()),
    );
    keys
}

/// Parcours du guide : outils, parties du vol et points de contrôle cités existent.
pub fn guide_steps(steps: &[StepInfo], tool_keys: &[&str], check_keys: &BTreeSet<String>) -> Vec<String> {
    let mut bad = Vec::new();
    for s in steps {
        if let Some(t) = &s.tool
            && !tool_keys.contains(&t.as_str())
        {
            bad.push(format!("{} : outil {t}", s.key));
        }
        if let Some(p) = &s.part
            && p != "hover"
            && p != "moving"
        {
            bad.push(format!("{} : partie {p}", s.key));
        }
        for k in s.checks.iter().chain(&s.usages) {
            if !check_keys.contains(k) {
                bad.push(format!("{} : contrôle {k}", s.key));
            }
        }
    }
    bad
}

// ------------------------------------------------------------------------------------------------ modules, index, publication, CLI

/// Fichiers source Rust du programme (`rust/*/src/**/*.rs`, hors `lib`, `main`, `mod` et noms commençant par « _ »), relatifs au dépôt.
pub fn rust_modules(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(root.join("rust")) {
        for e in rd.flatten() {
            walk(&e.path().join("src"), &mut out);
        }
    }
    out.retain(|p| {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        !stem.starts_with('_') && !["lib", "main", "mod"].contains(&stem)
    });
    out.sort();
    out
}

/// Chaque module cité par son nom dans `texts` (AGENTS.md et docs/architecture.md réunis).
pub fn modules_documented(root: &Path, modules: &[PathBuf], texts: &str) -> Vec<String> {
    modules
        .iter()
        .filter(|p| !texts.contains(p.file_stem().and_then(|s| s.to_str()).unwrap_or("")))
        .map(|p| rel(root, p))
        .collect()
}

/// `docs/README.md` liste chaque document de `docs/` (noms de fichiers, `README.md` exclu).
pub fn docs_index_missing(doc_names: &[String], index: &str) -> Vec<String> {
    doc_names
        .iter()
        .filter(|n| *n != "README.md" && !index.contains(&format!("({n})")))
        .cloned()
        .collect()
}

/// Fichiers publiés par `construire.yml` (lignes `package: …`).
pub fn release_files(workflow: &str) -> Vec<String> {
    re(r"(?m)^\s+package: (\S+)$")
        .captures_iter(workflow)
        .map(|c| c[1].to_string())
        .collect()
}

/// Les fichiers publiés sont ceux que décrit chaque document utilisateur, ni plus ni moins.
pub fn download_files_documented(published: &[String], docs: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    if published.is_empty() {
        problems.push("aucun fichier « package: » dans construire.yml".to_string());
    }
    let name_re = re(r"OpenTuneBox-[\w.-]+?\.(?:exe|dmg|AppImage|zip|tar\.gz)");
    for (name, text) in docs {
        let cited: BTreeSet<String> = name_re.find_iter(text).map(|m| m.as_str().to_string()).collect();
        let published: BTreeSet<String> = published.iter().cloned().collect();
        let absent: Vec<_> = published.difference(&cited).collect();
        let extra: Vec<_> = cited.difference(&published).collect();
        if !absent.is_empty() {
            problems.push(format!("{name} : fichiers publiés non décrits : {absent:?}"));
        }
        if !extra.is_empty() {
            problems.push(format!("{name} : fichiers qui ne sont plus publiés : {extra:?}"));
        }
    }
    problems
}

/// Chaque commande de `opentunebox` figure dans le README.
pub fn cli_commands_documented(commands: &[&str], readme: &str) -> Vec<String> {
    commands
        .iter()
        .filter(|c| !readme.contains(&format!("opentunebox {c}")))
        .map(|c| c.to_string())
        .collect()
}

// ------------------------------------------------------------------------------------------------ langue

/// (mots anglais courants, mots français courants) d'un document, code et liens mis à part.
pub fn language_counts(text: &str) -> (usize, usize) {
    let text = re(r"(?s)```.*?```|`[^`\n]+`|\([^)]*\)|https?://\S+")
        .replace_all(text, " ")
        .into_owned();
    let english = re(r"(?i)\b(the|and|with|from|when|which|only|are|is|of|in|to|by|for)\b");
    let french = re(r"(?i)\b(le|la|les|des|du|et|avec|pour|dans|sur|est|une|un|par|en)\b");
    (english.find_iter(&text).count(), french.find_iter(&text).count())
}

/// Règle du projet : documentation en français (le code, les formules et les noms restent tels quels).
pub fn not_french(text: &str) -> Option<String> {
    let (en, fr) = language_counts(text);
    (en > 5.max(fr / 3)).then(|| format!("semble rédigé en anglais ({en} mots anglais courants, {fr} français)"))
}

// ------------------------------------------------------------------------------------------------ registres D, L, W

/// Registre numéroté (décision D-018) : son préfixe, son motif d'entrée.
pub const REGISTERS: [(&str, &str, &str); 3] = [
    ("D", "decisions.md", r"(?m)^### D-(\d{3}) "),
    ("L", "lecons.md", r"(?m)^### L-(\d{3}) "),
    ("W", "travaux-reportes.md", r"(?m)^- (?:~~)?\*\*W-(\d{3})\*\*"),
];

pub fn register_entries(prefix: &str, text: &str) -> Vec<String> {
    let pattern = REGISTERS
        .iter()
        .find(|r| r.0 == prefix)
        .unwrap_or_else(|| panic!("registre inconnu : {prefix}"))
        .2;
    re(pattern)
        .captures_iter(&strip_fences(text))
        .map(|c| format!("{prefix}-{}", &c[1]))
        .collect()
}

/// Chaque numéro n'est attribué qu'une fois, et l'index du registre liste exactement les entrées.
pub fn register_problems(prefix: &str, text: &str) -> Vec<String> {
    let entries = register_entries(prefix, text);
    let mut problems = Vec::new();
    if entries.is_empty() {
        return vec![format!("aucune entrée {prefix}- dans le registre")];
    }
    let doubles: BTreeSet<&String> = entries
        .iter()
        .filter(|e| entries.iter().filter(|x| x == e).count() > 1)
        .collect();
    if !doubles.is_empty() {
        problems.push(format!("numéros attribués deux fois : {doubles:?}"));
    }
    let index: Vec<String> = re(&format!(r"(?m)^\| ({prefix}-\d{{3}}) \|"))
        .captures_iter(text)
        .map(|c| c[1].to_string())
        .collect();
    let (e, i): (BTreeSet<_>, BTreeSet<_>) = (entries.iter().cloned().collect(), index.iter().cloned().collect());
    let mut sorted_entries = entries.clone();
    let mut sorted_index = index.clone();
    sorted_entries.sort();
    sorted_index.sort();
    if sorted_entries != sorted_index {
        problems.push(format!(
            "index ≠ entrées (absents de l'index : {:?}, en trop : {:?})",
            e.difference(&i).collect::<Vec<_>>(),
            i.difference(&e).collect::<Vec<_>>()
        ));
    }
    problems
}

/// Numéros D-xxx, L-xxx, W-xxx cités dans `text` qui ne sont pas dans `known`.
pub fn unknown_ids(text: &str, known: &BTreeSet<String>) -> Vec<String> {
    let cited: BTreeSet<String> = re(r"\b[DLW]-\d{3}\b")
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect();
    cited.into_iter().filter(|r| !known.contains(r)).collect()
}

// ------------------------------------------------------------------------------------------------ seuils de l'assistant

/// Ce que `docs/assistant.md` doit retrouver d'un profil (`otb_advisor::profiles::CraftProfile`).
#[derive(Debug, Clone)]
pub struct ProfileView {
    pub key: String,
    pub gyro_noise_high_db: f64,
    pub dterm_noise_high_db: f64,
    pub low_band_high_db: f64,
    pub motor_left_high_db: f64,
    pub latency_ms: (f64, f64),
    pub gyro_delay_max_ms: f64,
    pub dterm_delay_max_ms: f64,
    pub imbalance_high_pct: f64,
    pub iterm_offset_high: f64,
    pub saturation_high_pct: f64,
    pub roll_pitch_gap_high: f64,
    pub resonance_high_db: f64,
}

/// Nombre à la française : virgule décimale et vrai signe moins.
fn fr(x: f64, digits: usize) -> String {
    fixed(x, digits, false).replace('.', ",").replace('-', "−")
}

/// Entier ou décimal tel qu'on l'écrit (`10`, `1.5`), comme `str()` d'un nombre Python.
fn plain(x: f64) -> String {
    format!("{x}")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(a.abs()) + 1e-12
}

/// Seuils que la documentation recopie, et la **constante du code** qui fait foi.
///
/// ⚠️ Le guide s'adresse aux pilotes : lui faire dire « voir assistant.md » serait moins utile que de donner la
/// valeur. Elle y est donc recopiée. Ce contrôle la compare à la **constante Rust**, et non à un autre document :
/// comparer deux textes entre eux laissait passer un changement, puisque la même valeur apparaît à plusieurs
/// endroits pour d'autres raisons (D-018, un maître par sujet)
pub struct SeuilCite {
    /// Nom de la constante dans `rust/otb-advisor/src/rules.rs`.
    pub constante: &'static str,
    /// Comment la valeur s'écrit dans les documents, à partir du nombre (ex. 7 → « 7 dB »).
    pub format: fn(f64) -> String,
    /// Ce que le seuil décide, pour le message d'erreur.
    pub quoi: &'static str,
}

/// Valeur d'une constante `pub const NOM: f64 = …;` lue dans le source Rust.
pub fn constante_f64(source: &str, nom: &str) -> Option<f64> {
    let motif = format!(r"pub const {nom}: f64 = (-?[0-9.]+);");
    re(&motif).captures(source)?.get(1)?.as_str().parse().ok()
}

/// Chaque seuil recopié dans un document doit porter la valeur de sa constante.
///
/// Un document qui ne cite pas le seuil n'est pas en faute : il renvoie alors à son maître. Ce qui est refusé, c'est
/// de citer une valeur **différente** de celle du code.
pub fn seuils_cites_concordent(documents: &[(&str, &str)], rules_rs: &str, seuils: &[SeuilCite]) -> Vec<String> {
    let mut problems = Vec::new();
    for s in seuils {
        let Some(valeur) = constante_f64(rules_rs, s.constante) else {
            problems.push(format!("constante {} introuvable dans rules.rs", s.constante));
            continue;
        };
        let attendu = (s.format)(valeur);
        for (nom, texte) in documents {
            // Le document parle-t-il de ce seuil ? On le repère par son unité et son rôle, pas par la valeur.
            if !texte.contains(&attendu) && texte.contains(s.quoi) {
                problems.push(format!(
                    "{nom} parle de « {} » sans citer « {attendu} » ({} = {valeur}) : la valeur a changé d'un seul côté",
                    s.quoi, s.constante
                ));
            }
        }
    }
    problems
}

/// `docs/assistant.md` est la référence des seuils (compétence `assistant-seuils`) : chaque valeur des profils y figure, dans « Seuils retenus »
/// (bruit, latence, retards) et dans le tableau des seuils mécaniques (colonne commune pour whoop, 2–3,5" et 8"+).
pub fn assistant_doc_problems(text: &str, profiles: &[ProfileView]) -> Vec<String> {
    let mut problems = Vec::new();
    let (Some(a), Some(b)) = (
        text.find("Seuils retenus dans `rust/otb-advisor/src/profiles.rs`"),
        text.find("Pour un profil provisoire"),
    ) else {
        return vec!["sections « Seuils retenus » ou « Pour un profil provisoire » introuvables".into()];
    };
    let Some(kept) = text.get(a..b) else {
        return vec!["« Pour un profil provisoire » précède « Seuils retenus »".into()];
    };
    for p in profiles {
        for v in [
            p.gyro_noise_high_db,
            p.dterm_noise_high_db,
            p.low_band_high_db,
            p.motor_left_high_db,
        ] {
            if !kept.contains(&fr(v, 1)) && !kept.contains(&fr(v, 0)) {
                problems.push(format!("{} : {v} dB absent de « Seuils retenus »", p.key));
            }
        }
        if !kept.contains(&format!("{}–{} ms", plain(p.latency_ms.0), plain(p.latency_ms.1))) {
            problems.push(format!("{} : latence {:?} absente", p.key, p.latency_ms));
        }
        if !kept.contains(&format!(
            "{} / {}",
            fr(p.gyro_delay_max_ms, 1),
            fr(p.dterm_delay_max_ms, 1)
        )) {
            problems.push(format!("{} : retards absents", p.key));
        }
    }
    let Some(t) = text.find("| Seuil mécanique |") else {
        return vec!["tableau « Seuil mécanique » introuvable".into()];
    };
    let mut rows: Vec<(String, Vec<f64>)> = Vec::new();
    for line in text[t..].lines().skip(2) {
        if !line.starts_with('|') {
            break;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let nums: Result<Vec<f64>, _> = cells[1..].iter().map(|c| c.replace(',', ".").parse::<f64>()).collect();
        match nums {
            Ok(n) => rows.push((cells[0].to_string(), n)),
            Err(_) => problems.push(format!("tableau des seuils mécaniques : ligne illisible « {line} »")),
        }
    }
    type Getter = fn(&ProfileView) -> f64;
    let fields: [(&str, Getter); 5] = [
        ("Déséquilibre des moteurs (%)", |p| p.imbalance_high_pct),
        ("Terme I moyen (valeur absolue)", |p| p.iterm_offset_high),
        ("Saturation (% du temps ≥ 98 %)", |p| p.saturation_high_pct),
        ("Écart de pic roll / pitch", |p| p.roll_pitch_gap_high),
        ("Résonance (dB au-dessus du bruit voisin)", |p| p.resonance_high_db),
    ];
    let labels: BTreeSet<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    let expected: BTreeSet<&str> = fields.iter().map(|f| f.0).collect();
    if labels != expected {
        problems.push(format!(
            "lignes du tableau des seuils mécaniques : {labels:?} au lieu de {expected:?}"
        ));
        return problems;
    }
    for (label, get) in fields {
        let values = &rows.iter().find(|r| r.0 == label).expect("ligne présente").1;
        let [common, five, seven] = values[..] else {
            problems.push(format!("{label} : trois colonnes attendues"));
            continue;
        };
        for key in ["whoop", "micro", "large"] {
            if let Some(p) = profiles.iter().find(|p| p.key == key)
                && !close(get(p), common)
            {
                problems.push(format!("{label} : {key} ≠ colonne commune"));
            }
        }
        for (key, col, name) in [("five", five, "5\""), ("seven", seven, "7\"")] {
            if let Some(p) = profiles.iter().find(|p| p.key == key)
                && !close(get(p), col)
            {
                problems.push(format!("{label} : {name}"));
            }
        }
    }
    problems
}
