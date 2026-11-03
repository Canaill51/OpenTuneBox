//! `otb-dev corpus labels` : tableau d'étiquettes du corpus, une ligne par drone, à compléter dans un tableur (remplace
//! `scripts/corpus_labels.py`, docs/assistant.md).
//!
//! ```text
//! otb-dev corpus labels [--limit N] [-j FILS] [--csv CHEMIN]   met à jour OTB_LOGS/corpus.csv (ou CHEMIN)
//! ```
//!
//! Colonnes remplies à chaque passage : drone, dossiers, sessions, origine, taille_detectee, freq_moteur_hz, motor_kv (vide si absent
//! ou égal au défaut 1960), cellules. Colonnes de Cédric, pré-remplies seulement à la création de la ligne et **jamais écrasées
//! ensuite** : etat (aide, origine, regle, inconnu), taille_reelle (pouces, ou « whoop »), remarque. Les drones absents d'un passage
//! gardent leur ligne (0 session après un passage complet). Le fichier est écrit comme Python l'écrit (BOM, `;`, fins de ligne
//! `\r\n`) : Excel et les deux outils le relisent.
//!
//! Différence assumée : Python parcourt des `set` (dossiers et noms d'un drone) dans un ordre qui change d'un lancement à l'autre ;
//! Rust les parcourt triés. Cela ne change `taille_reelle` que si les noms d'un même drone citent deux tailles différentes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use otb_advisor::profiles::{detect_profile, digit_value, profile_key};
use otb_analysis::motors::hover_motor_hz;
use otb_analysis::npstat::median_f64;
use otb_log::flightlog::load;
use regex::Regex;

use super::{Args, MIN_SESSION_S, Output, log_files, logs_root, map_files, rel};

pub const COLUMNS: [&str; 11] = [
    "drone",
    "dossiers",
    "sessions",
    "origine",
    "taille_detectee",
    "freq_moteur_hz",
    "motor_kv",
    "cellules",
    "etat",
    "taille_reelle",
    "remarque",
];
/// Colonnes de Cédric : jamais réécrites une fois la ligne créée.
pub const USER_COLUMNS: [&str; 3] = ["etat", "taille_reelle", "remarque"];
/// Demandes d'aide (forums, groupes).
const HELP_FOLDERS: [&str; 2] = ["1. PTB", "3. BBLR FB"];
/// `motor_kv` par défaut du firmware (pg/motor.c) : laissé vide, réglage probablement absent.
const DEFAULT_KV: i64 = 1960;
pub const CSV_NAME: &str = "corpus.csv";
/// Noms trouvés dans plusieurs dossiers, mais d'un seul drone (confirmé par Cédric, 2026-10-05) : pas d'alerte.
const SAME_DRONE_NAMES: [&str; 5] = ["JeNo Ced", "JeNo CED", "M75 O4 CED", "AIR65 CED", "BABYDRONE"];

/// Une ligne du tableau : nom de colonne → texte.
pub type Row = BTreeMap<String, String>;
/// Les lignes dans l'ordre du fichier, par drone (un `dict` de Python : un drone en double garde sa première place).
pub type Labels = Vec<(String, Row)>;

fn is_az(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// État pré-rempli : dossier de demandes d'aide, puis mots des noms de fichiers et dossiers.
pub fn guess_state(origins: &[&str], texts: &[&str]) -> &'static str {
    if origins.iter().any(|o| HELP_FOLDERS.contains(o)) {
        return "aide";
    }
    let low = texts.join(" ").to_lowercase();
    // `(?<![a-z])last(?![a-z])` : le mot « last », pas « blast » ni « lasting »
    let last = low.match_indices("last").any(|(i, w)| {
        !low[..i].chars().next_back().is_some_and(is_az) && !low[i + w.len()..].chars().next().is_some_and(is_az)
    });
    if low.contains("final") || low.contains("tuned") || last {
        "regle"
    } else if low.contains("stock") {
        "origine"
    } else {
        "inconnu"
    }
}

static WHOOP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"whoop|air ?65|meteor ?(?:65|75)").unwrap());
static MM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:65|75) ?mm").unwrap());
// `in(?![a-z])` de Python devient `in` suivi d'un caractère hors a-z ou de la fin : seul le groupe 1 compte
static INCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(\d{1,2}(?:[.,]\d)?) ?(?:"|”|''|in(?:[^a-z]|$)|inch|pouces?)"#).unwrap());

/// Première occurrence de `re` dont le caractère précédent est accepté (le `(?<!…)` de Python, que `regex` ne connaît pas).
fn find_after<'t>(re: &Regex, text: &'t str, ok_before: impl Fn(char) -> bool) -> Option<regex::Captures<'t>> {
    let mut pos = 0;
    while pos <= text.len() {
        let c = re.captures_at(text, pos)?;
        let start = c.get(0)?.start();
        if !text[..start].chars().next_back().is_some_and(|p| !ok_before(p)) {
            return Some(c);
        }
        pos = start + text[start..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// Taille écrite dans un nom (drone, dossier, fichier : « 3.5 inch », « 5" », « Air65 »), sinon vide.
pub fn guess_size(texts: &[&str]) -> String {
    let low = texts.join(" ").to_lowercase();
    let digit = |c: char| digit_value(c).is_some();
    if WHOOP.is_match(&low) || find_after(&MM, &low, |p| !digit(p)).is_some() {
        return "whoop".into();
    }
    match find_after(&INCH, &low, |p| !(digit(p) || p == '.' || p == ',')) {
        Some(c) => c[1].replace('.', ","),
        None => String::new(),
    }
}

/// `str.splitlines()` de Python : coupe à `\n`, `\r\n`, `\r` et aux séparateurs Unicode de ligne.
fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut chars) = (0, text.char_indices().peekable());
    while let Some((i, c)) = chars.next() {
        if matches!(
            c,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        ) {
            out.push(&text[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|&(_, n)| n == '\n') {
                chars.next();
                end += 1;
            }
            start = end;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// `csv.reader(lines, delimiter=…)` de Python (dialecte par défaut) : les lignes sont données sans leur fin de ligne, donc un champ
/// entre guillemets sur plusieurs lignes est recollé sans saut de ligne.
fn read_records(lines: &[&str], delim: char) -> Vec<Vec<String>> {
    #[derive(PartialEq)]
    enum St {
        StartRecord,
        StartField,
        InField,
        InQuoted,
        QuoteInQuoted,
    }
    let (mut records, mut fields, mut field, mut st) = (Vec::new(), Vec::new(), String::new(), St::StartRecord);
    for line in lines {
        for c in line.chars() {
            st = match st {
                St::StartRecord | St::StartField => {
                    if c == delim {
                        fields.push(std::mem::take(&mut field));
                        St::StartField
                    } else if c == '"' {
                        St::InQuoted
                    } else {
                        field.push(c);
                        St::InField
                    }
                }
                St::InField => {
                    if c == delim {
                        fields.push(std::mem::take(&mut field));
                        St::StartField
                    } else {
                        field.push(c);
                        St::InField
                    }
                }
                St::InQuoted => {
                    if c == '"' {
                        St::QuoteInQuoted
                    } else {
                        field.push(c);
                        St::InQuoted
                    }
                }
                St::QuoteInQuoted => {
                    if c == '"' {
                        field.push('"');
                        St::InQuoted
                    } else if c == delim {
                        fields.push(std::mem::take(&mut field));
                        St::StartField
                    } else {
                        field.push(c); // mode non strict : le texte après le guillemet fermant est gardé
                        St::InField
                    }
                }
            };
        }
        match st {
            St::InQuoted => {} // le champ continue sur la ligne suivante
            St::StartRecord => records.push(Vec::new()),
            _ => {
                fields.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut fields));
                st = St::StartRecord;
            }
        }
    }
    if st == St::InQuoted {
        fields.push(field);
        records.push(fields);
    }
    records
}

/// `read_labels` : les lignes du tableau (séparateur « ; » ou « , », BOM accepté), vide si le texte est vide.
pub fn parse_labels(text: &str) -> Labels {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.split('\n').next().unwrap_or("");
    let delim = if first.matches(';').count() >= first.matches(',').count() {
        ';'
    } else {
        ','
    };
    let mut records = read_records(&splitlines(text), delim).into_iter();
    let Some(names) = records.next() else { return Vec::new() };
    let mut out: Labels = Vec::new();
    for rec in records.filter(|r| !r.is_empty()) {
        // `zip` : les colonnes en trop (clé `None`) et les colonnes manquantes sont ignorées, un nom en double garde la dernière valeur
        let raw: BTreeMap<&str, &str> = names
            .iter()
            .map(String::as_str)
            .zip(rec.iter().map(String::as_str))
            .collect();
        let Some(drone) = raw.get("drone").filter(|d| !d.is_empty()) else {
            continue;
        };
        let row: Row = raw
            .iter()
            .filter(|(k, _)| !k.is_empty())
            .map(|(k, v)| (k.to_string(), v.trim().to_string()))
            .collect();
        match out.iter().position(|(d, _)| d == drone) {
            Some(i) => out[i].1 = row,
            None => out.push((drone.to_string(), row)),
        }
    }
    out
}

pub fn read_labels(path: &Path) -> Result<Labels, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(parse_labels(&text))
}

/// Un champ comme `csv.writer` (citation minimale : guillemets s'il contient `;`, `"` ou un saut de ligne).
fn quote(field: &str) -> String {
    if field.contains([';', '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Le contenu du fichier : BOM (accents corrects dans Excel), en-tête, lignes ; `;` et `\r\n` comme Python.
pub fn labels_text(rows: &[Row]) -> String {
    let mut text = String::from('\u{feff}');
    text += &COLUMNS.join(";");
    text += "\r\n";
    for r in rows {
        let line: Vec<String> = COLUMNS
            .iter()
            .map(|c| quote(r.get(*c).map_or("", String::as_str)))
            .collect();
        text += &line.join(";");
        text += "\r\n";
    }
    text
}

fn get<'r>(r: &'r Row, k: &str) -> &'r str {
    r.get(k).map_or("", String::as_str)
}

/// Lignes à écrire : colonnes automatiques à jour, colonnes de Cédric conservées. Après un passage complet (`full`), un drone absent
/// garde sa ligne avec 0 session (à effacer à la main).
pub fn merge(old: &Labels, new: &Labels, full: bool) -> Vec<Row> {
    let mut out: Labels = old
        .iter()
        .map(|(d, row)| {
            let mut row = row.clone();
            if full && !new.iter().any(|(n, _)| n == d) {
                row.insert("sessions".into(), "0".into());
            }
            (d.clone(), row)
        })
        .collect();
    for (d, row) in new {
        let mut r = row.clone();
        let place = out.iter().position(|(o, _)| o == d);
        if let Some(i) = old.iter().position(|(o, _)| o == d) {
            for k in USER_COLUMNS {
                r.insert(k.to_string(), get(&old[i].1, k).to_string());
            }
        }
        match place {
            Some(i) => out[i].1 = r,
            None => out.push((d.clone(), r)),
        }
    }
    let mut rows: Vec<Row> = out.into_iter().map(|(_, r)| r).collect();
    rows.sort_by_cached_key(|r| (get(r, "origine").to_string(), get(r, "drone").to_lowercase()));
    rows
}

/// `Counter(v).most_common(1)[0][0]` : la valeur la plus fréquente, à égalité la première rencontrée.
fn most_common<T: PartialEq + Clone>(v: &[T]) -> Option<T> {
    let mut seen: Vec<(&T, usize)> = Vec::new();
    for x in v {
        match seen.iter_mut().find(|(y, _)| *y == x) {
            Some((_, n)) => *n += 1,
            None => seen.push((x, 1)),
        }
    }
    let mut best: Option<(&T, usize)> = None;
    for (x, n) in seen {
        if best.is_none_or(|(_, b)| n > b) {
            best = Some((x, n));
        }
    }
    best.map(|(x, _)| x.clone())
}

/// Une session d'au moins 8 s : (nom du drone, fréquence moteur, KV, cellules, taille détectée).
type Sess = (String, Option<f64>, i64, Option<i64>, &'static str);

fn scan_file(f: &Path) -> Result<Vec<Sess>, String> {
    Ok(load(f)
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|lg| lg.duration() >= MIN_SESSION_S)
        .map(|lg| {
            (
                lg.settings.craft_name.clone(),
                hover_motor_hz(lg),
                lg.settings.motor_kv,
                lg.cells(),
                detect_profile(lg).0.key,
            )
        })
        .collect())
}

#[derive(Default)]
struct Acc {
    dirs: BTreeSet<String>,
    origins: BTreeSet<String>,
    names: BTreeSet<String>,
    sessions: usize,
    hz: Vec<f64>,
    kv: Vec<i64>,
    cells: Vec<i64>,
    profiles: Vec<&'static str>,
}

/// Les lignes (une par drone, dans l'ordre de première rencontre) construites depuis les sessions de chaque fichier
/// (`(chemin relatif au corpus, sessions)`).
pub fn rows_from_sessions(files: &[(String, Vec<Sess>)]) -> Labels {
    let mut order: Vec<(String, Acc)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (rel, sessions) in files {
        let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel));
        let origin = rel.split_once('/').map_or("", |(o, _)| o);
        for (craft, hz, kv, cells, prof) in sessions {
            let id = super::drone_id(craft, rel);
            let i = *index.entry(id.clone()).or_insert_with(|| {
                order.push((id, Acc::default()));
                order.len() - 1
            });
            let a = &mut order[i].1;
            a.dirs.insert(dir.to_string());
            a.origins.insert(origin.to_string());
            a.names.insert(name.to_string());
            a.sessions += 1;
            a.hz.extend(hz);
            if *kv > 0 && *kv != DEFAULT_KV {
                a.kv.push(*kv);
            }
            a.cells.extend(cells.filter(|&n| n != 0));
            a.profiles.push(prof);
        }
    }
    order
        .into_iter()
        .map(|(d, a)| {
            let hz = (!a.hz.is_empty()).then(|| median_f64(&a.hz)).filter(|h| *h != 0.0);
            let cells = most_common(&a.cells);
            let named: Vec<&str> = std::iter::once(d.trim_start_matches('?').split(" · ").next().unwrap_or(""))
                .chain(a.dirs.iter().map(String::as_str))
                .chain(a.names.iter().map(String::as_str))
                .collect();
            let origins: Vec<&str> = a.origins.iter().map(String::as_str).collect();
            let row: Row = [
                ("drone", d.clone()),
                ("dossiers", a.dirs.iter().cloned().collect::<Vec<_>>().join(" | ")),
                ("sessions", a.sessions.to_string()),
                ("origine", a.origins.iter().cloned().collect::<Vec<_>>().join(" | ")),
                (
                    "taille_detectee",
                    match hz {
                        Some(h) => profile_key(h, cells).to_string(),
                        None => most_common(&a.profiles).unwrap_or("").to_string(),
                    },
                ),
                ("freq_moteur_hz", hz.map_or(String::new(), |h| format!("{h:.0}"))),
                ("motor_kv", most_common(&a.kv).map_or(String::new(), |k| k.to_string())),
                ("cellules", cells.map_or(String::new(), |c| c.to_string())),
                ("etat", guess_state(&origins, &named).to_string()),
                ("taille_reelle", guess_size(&named)),
                ("remarque", String::new()),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
            (d, row)
        })
        .collect()
}

/// Noms trouvés dans plusieurs dossiers sans lien de parenté : peut-être plusieurs machines.
pub fn shared_name_candidates(rows: &Labels) -> Vec<String> {
    let inside = |a: &str, b: &str| a == b || a.starts_with(&format!("{b}/"));
    rows.iter()
        .filter(|(d, row)| {
            let dirs: Vec<&str> = get(row, "dossiers").split(" | ").collect();
            !d.starts_with('?')
                && !SAME_DRONE_NAMES.contains(&d.as_str())
                && dirs.iter().any(|a| dirs.iter().any(|b| !inside(a, b) && !inside(b, a)))
        })
        .map(|(d, row)| format!("{d} : {}", get(row, "dossiers")))
        .collect()
}

/// `Counter(...).most_common()` : valeurs par fréquence décroissante, à égalité dans l'ordre de première rencontre.
fn counted(v: &[&str]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for x in v {
        match out.iter_mut().find(|(y, _)| y == x) {
            Some((_, n)) => *n += 1,
            None => out.push((x.to_string(), 1)),
        }
    }
    out.sort_by_key(|(_, n)| std::cmp::Reverse(*n)); // tri stable
    out
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["--limit", "-j", "--csv"], &[])?;
    if !a.positional.is_empty() {
        return Err(super::USAGE.into());
    }
    let (limit, jobs) = (a.number("--limit")?, a.number("-j")?);
    let root = logs_root();
    let path = a
        .text("--csv")
        .map_or_else(|| root.join(CSV_NAME), std::path::PathBuf::from);
    let old = read_labels(&path)?;
    let mut files = log_files(&root).map_err(|e| format!("{} : {e}", root.display()))?;
    files.truncate(limit.unwrap_or(files.len()));
    let scanned: Vec<(String, Vec<Sess>)> = map_files(scan_file, &files, jobs, &root)
        .into_iter()
        .map(|(f, s)| (rel(&f, &root), s))
        .collect();
    let new = rows_from_sessions(&scanned);
    let rows = merge(&old, &new, limit.is_none());
    std::fs::write(&path, labels_text(&rows)).map_err(|e| format!("{} : {e}", path.display()))?;
    let mut text = String::new();
    let shared = shared_name_candidates(&new);
    if !shared.is_empty() {
        text += &format!(
            "Noms trouvés dans plusieurs dossiers (machines différentes ? → SHARED_NAMES) :\n  {}\n",
            shared.join("\n  ")
        );
    }
    let states: Vec<&str> = rows
        .iter()
        .map(|r| Some(get(r, "etat")).filter(|e| !e.is_empty()).unwrap_or("(vide)"))
        .collect();
    text += &format!(
        "{} : {} drones ({} nouveaux) ; états : {}",
        path.display(),
        rows.len(),
        rows.len() as i64 - old.len() as i64,
        counted(&states)
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(Output::ok(text))
}
