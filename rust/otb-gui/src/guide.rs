//! Guide intégré, partie sans fenêtre (`gui/guide.py`) : fichier des fiches drone, liens internes, avancement, outils cités.
//!
//! Rien ici ne dépend d'egui : tout se teste sans interface (`tests/guide.rs`). La page est dans `guide_page.rs`.

use otb_advisor::drone_card::DroneCard;
use otb_advisor::log_check::{FILTERS, FilterTargets, LogCheck, PID, ROLE_NAMES};
use otb_advisor::tuning_path::{AUTO_OK, auto_status, phases};
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::{PrettyFormatter, Serializer};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Accueil du guide : le **même** fichier que la version Python (une seule source tant qu'elle existe).
pub const HOME_TEXT: &str = include_str!("../../../src/opentunebox/gui/assets/guide/accueil.md");
/// Clé de l'accueil dans les liens `otb:phase/home` et dans la mémoire de la page.
pub const HOME: &str = "home";
pub const PHASE_KEYS: [&str; 4] = ["prep", "filters", "pid", "finish"];
pub const PHASE_TITLES: [&str; 4] = ["Préparer le drone", "Filtres", "PID par séries", "Finitions"];
/// Partie du vol → libellé du bouton « Ouvrir <outil> (<partie>) ».
pub const PART_NAMES: [(&str, &str); 2] = [("hover", "partie stationnaire"), ("moving", "partie avec mouvements")];

/// Où mène la clé d'outil d'une case (les clés sont celles du `REGISTRY` de Python, `Step::tool`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolTarget {
    Viewer,
    /// Titre de la page (`Page::title`).
    Page(&'static str),
}

/// (clé de Python, libellé du bouton, destination).
pub const TOOLS: [(&str, &str, ToolTarget); 9] = [
    ("spectral", "Spectre", ToolTarget::Page("Analyse spectrale")),
    ("freq_throttle", "Fréq. × gaz", ToolTarget::Page("Fréquence × gaz")),
    ("freq_time", "Fréq. × temps", ToolTarget::Page("Fréquence × temps")),
    ("step", "Réponse indicielle", ToolTarget::Page("Réponse indicielle")),
    ("sliders", "Curseurs PID", ToolTarget::Page("Curseurs PID")),
    ("filter_sim", "Filtres", ToolTarget::Page("Simulateur de filtres")),
    ("setup", "Configuration", ToolTarget::Page("Configuration")),
    ("advisor", "Assistant", ToolTarget::Page("Assistant")),
    ("viewer", "Visualiseur", ToolTarget::Viewer),
];

pub fn tool_target(key: &str) -> Option<ToolTarget> {
    TOOLS.iter().find(|(k, ..)| *k == key).map(|t| t.2)
}

/// Libellé du bouton « Ouvrir … » d'une case (la clé elle-même si elle est inconnue, comme Python).
pub fn tool_label(key: &str, part: Option<&str>) -> String {
    let name = TOOLS.iter().find(|(k, ..)| *k == key).map_or(key, |t| t.1);
    match part.and_then(|p| PART_NAMES.iter().find(|(k, _)| *k == p)) {
        Some((_, label)) => format!("{name} ({label})"),
        None => name.to_string(),
    }
}

/// Libellé du bouton d'ouverture des logs d'un rôle.
pub fn open_logs_label(role: &str) -> &'static str {
    if role == PID {
        "Ouvrir les logs de la série…"
    } else {
        "Ouvrir les logs des filtres…"
    }
}

/// Nom du rôle pour les phrases (« filtres », « séries PID »).
pub fn role_name(role: &str) -> &'static str {
    ROLE_NAMES.iter().find(|(k, _)| *k == role).map_or("", |r| r.1)
}

/// Le rôle d'un texte, sous forme de constante de `log_check` (`LogCheck::role` est un `&'static str`).
pub fn role_static(role: &str) -> Option<&'static str> {
    ROLE_NAMES.iter().find(|(k, _)| *k == role).map(|r| r.0)
}

/// Le rôle choisi par l'utilisateur remplace le rôle deviné (`dataclasses.replace` de Python).
pub fn with_role(check: &LogCheck, role: Option<&str>) -> LogCheck {
    let mut c = check.clone();
    if let Some(r) = role.and_then(role_static) {
        c.role = r;
    }
    c
}

/// L'autre rôle : celui vers lequel « classer dans les … » déplace un log.
pub fn other_role(role: &str) -> &'static str {
    if role == FILTERS { PID } else { FILTERS }
}

// ------------------------------------------------------------------ liens internes
/// Un lien `otb:…` d'un texte du guide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuideLink {
    /// `otb:phase/<clé>` : une phase, ou `HOME`.
    Phase(&'static str),
    /// `otb:tool/<clé>`.
    Tool(ToolTarget),
    /// `otb:open/<rôle>` : choisir des logs pour ce rôle.
    Open(&'static str),
    /// `otb:role/<numéro du log>/<rôle>` : classer le log.
    Role(usize, &'static str),
}

/// Lit un lien `otb:…` ; `None` pour un autre lien (page web : le navigateur) ou un lien invalide.
pub fn parse_link(dest: &str) -> Option<GuideLink> {
    let path = dest.strip_prefix("otb:")?;
    let (kind, key) = path.split_once('/')?;
    match kind {
        "phase" => PHASE_KEYS
            .iter()
            .find(|k| **k == key)
            .map(|k| GuideLink::Phase(k))
            .or((key == HOME).then_some(GuideLink::Phase(HOME))),
        "tool" => tool_target(key).map(GuideLink::Tool),
        "open" => role_static(key).map(GuideLink::Open),
        "role" => {
            let (i, role) = key.split_once('/')?;
            if i.is_empty() || !i.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some(GuideLink::Role(i.parse().ok()?, role_static(role)?))
        }
        _ => None,
    }
}

// ------------------------------------------------------------------ avancement
/// La case est cochée à la main.
pub fn is_done(card: &DroneCard, key: &str) -> bool {
    card.done.iter().any(|(k, v)| k == key && *v)
}

/// Cases faites / total par phase : cochées à la main, ou vérifiées par un log (`AUTO_OK`). Une erreur de `auto_status` (usage absent
/// d'un log, W-029 point 2) ne vérifie rien.
pub fn progress(card: &DroneCard, checks: &[LogCheck], targets: Option<&FilterTargets>) -> Vec<(String, usize, usize)> {
    phases(card)
        .iter()
        .map(|ph| {
            let n = ph
                .steps
                .iter()
                .filter(|s| is_done(card, &s.key) || matches!(auto_status(s, checks, targets), Ok((Some(AUTO_OK), _))))
                .count();
            (ph.key.to_string(), n, ph.steps.len())
        })
        .collect()
}

// ------------------------------------------------------------------ fichier des fiches
/// Dossier de données de Qt (`QStandardPaths.AppDataLocation`, avec le nom de l'organisation puis celui de l'application, tous deux
/// « OpenTuneBox » dans `gui/app.py`) : le même fichier que la version Python.
/// Linux : mesuré sur le poste (`~/.local/share/OpenTuneBox/OpenTuneBox/drones.json`). Windows et macOS : d'après la documentation de Qt
/// (`%APPDATA%` = AppData/Roaming ; `~/Library/Application Support`) ; **non essayés** sur ces systèmes.
pub fn cards_path_in(os: &str, var: &dyn Fn(&str) -> Option<OsString>) -> PathBuf {
    let get = |k: &str| var(k).filter(|v| !v.is_empty());
    let home = get("HOME").or_else(|| get("USERPROFILE")).map(PathBuf::from);
    let base = match os {
        "windows" => get("APPDATA").map(PathBuf::from),
        "macos" => home.as_ref().map(|h| h.join("Library").join("Application Support")),
        // XDG : seul un chemin absolu compte
        _ => get("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.as_ref().map(|h| h.join(".local").join("share"))),
    };
    match base {
        Some(b) => b.join("OpenTuneBox").join("OpenTuneBox").join("drones.json"),
        // repli de Python quand Qt ne sait pas : `~/.opentunebox`
        None => home.unwrap_or_default().join(".opentunebox").join("drones.json"),
    }
}

pub fn cards_path() -> PathBuf {
    cards_path_in(std::env::consts::OS, &|k| std::env::var_os(k))
}

/// Le texte du fichier des fiches, au format de Python (`json.dumps(ensure_ascii=False, indent=1)`, sans saut de ligne final).
pub fn cards_to_json(cards: &[DroneCard]) -> String {
    let v = Value::Array(cards.iter().map(DroneCard::to_value).collect());
    let mut out = Vec::new();
    let mut ser = Serializer::with_formatter(&mut out, PrettyFormatter::with_indent(b" "));
    v.serialize(&mut ser).expect("écriture en mémoire");
    String::from_utf8(out).expect("JSON en UTF-8")
}

/// Les fiches lues dans le texte du fichier. `Err` : le texte n'est pas une liste de fiches lisibles.
pub fn cards_from_json(text: &str) -> Result<Vec<DroneCard>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("JSON invalide : {e}"))?;
    let list = v.as_array().ok_or("le fichier n'est pas une liste de fiches")?;
    list.iter()
        .enumerate()
        .map(|(i, d)| DroneCard::from_value(d).map_err(|e| format!("fiche {} : {e}", i + 1)))
        .collect()
}

/// Les fiches drone et leur fichier. Un fichier illisible n'est jamais écrasé en silence (D-039) : il est mis de côté sous le nom
/// `drones.json.illisible` avant la première écriture, et `problem` dit pourquoi les fiches sont vides.
pub struct Cards {
    /// `None` : fiches seulement en mémoire (bancs de test : jamais le vrai fichier de l'utilisateur).
    pub path: Option<PathBuf>,
    pub list: Vec<DroneCard>,
    problem: Option<String>,
}

impl Cards {
    /// Un fichier absent donne une liste vide sans problème.
    pub fn load(path: PathBuf) -> Cards {
        let (list, problem) = match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes)
                .map_err(|e| e.to_string())
                .and_then(|t| cards_from_json(&t))
            {
                Ok(list) => (list, None),
                Err(e) => (Vec::new(), Some(e)),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => (Vec::new(), None),
            Err(e) => (Vec::new(), Some(format!("lecture impossible : {e}"))),
        };
        Cards {
            path: Some(path),
            list,
            problem,
        }
    }

    /// Fiches sans fichier : rien n'est lu ni écrit.
    pub fn memory() -> Cards {
        Cards {
            path: None,
            list: Vec::new(),
            problem: None,
        }
    }

    /// Pourquoi le fichier n'a pas pu être lu (tant qu'il n'a pas été mis de côté).
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// Écrit le fichier, par un fichier temporaire puis un renommage : une coupure ne laisse jamais un fichier à moitié écrit.
    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = self.path.clone() else { return Ok(()) };
        if self.problem.is_some() && path.exists() {
            std::fs::rename(&path, free_name(&path, "illisible"))?;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, cards_to_json(&self.list))?;
        std::fs::rename(&tmp, &path)?;
        self.problem = None;
        Ok(())
    }

    /// Ajoute une fiche (elle remplace celle qui porte exactement le même nom, comme Python) ; rend son numéro.
    pub fn add(&mut self, card: DroneCard) -> io::Result<usize> {
        self.list.retain(|c| c.name != card.name);
        self.list.push(card);
        self.save()?;
        Ok(self.list.len() - 1)
    }

    /// Remplace la fiche `i` (modification : l'avancement est repris par l'appelant).
    pub fn replace(&mut self, i: usize, card: DroneCard) -> io::Result<()> {
        if let Some(slot) = self.list.get_mut(i) {
            *slot = card;
        }
        self.save()
    }

    pub fn delete(&mut self, i: usize) -> io::Result<()> {
        if i < self.list.len() {
            self.list.remove(i);
        }
        self.save()
    }

    /// Coche ou décoche une case : une case décochée est retirée de `done`, jamais écrite à `false` (comme Python).
    pub fn set_done(&mut self, i: usize, key: &str, on: bool) -> io::Result<()> {
        let Some(card) = self.list.get_mut(i) else {
            return Ok(());
        };
        if on {
            match card.done.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = true,
                None => card.done.push((key.to_string(), true)),
            }
        } else {
            card.done.retain(|(k, _)| k != key);
        }
        self.save()
    }

    /// Décoche toutes les cases de la fiche `i`.
    pub fn reset(&mut self, i: usize) -> io::Result<()> {
        if let Some(card) = self.list.get_mut(i) {
            card.done.clear();
        }
        self.save()
    }
}

/// `drones.json.illisible`, puis `.illisible.1`… : un fichier déjà mis de côté n'est jamais écrasé.
fn free_name(path: &Path, suffix: &str) -> PathBuf {
    let base = path.as_os_str().to_owned();
    let with = |n: Option<u32>| {
        let mut s = base.clone();
        s.push(format!(".{suffix}"));
        if let Some(n) = n {
            s.push(format!(".{n}"));
        }
        PathBuf::from(s)
    };
    let first = with(None);
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| with(Some(n)))
        .find(|p| !p.exists())
        .expect("un nom libre existe")
}

// ------------------------------------------------------------------ textes affichés
/// « 1. Préparer le drone (3/11) » : la liste des phases et la frise (`GuidePage._update_progress`). Sans case, pas de compte.
pub fn phase_text(index: usize, done: usize, total: usize) -> String {
    let title = PHASE_TITLES.get(index).copied().unwrap_or("");
    let count = if total > 0 {
        format!(" ({done}/{total})")
    } else {
        String::new()
    };
    format!("{}. {title}{count}", index + 1)
}

/// La phase est finie : toutes ses cases sont faites (la coche s'affiche devant son titre).
pub fn phase_finished(done: usize, total: usize) -> bool {
    total > 0 && done == total
}

/// Les polices d'egui n'ont pas « ✔ » (L-068) : un texte du guide qui le contient (intro de la phase 1) est affiché sans lui.
pub fn display_text(s: &str) -> String {
    s.replace("✔ ", "").replace('✔', "")
}

/// Le texte copié par « Copier les commandes CLI » : les commandes, puis `save` (`GuidePage._copy_cli`).
pub fn cli_text(commands: &[String]) -> String {
    commands
        .iter()
        .map(String::as_str)
        .chain(["save"])
        .collect::<Vec<_>>()
        .join("\n")
}

/// Période d'une partie du vol (« hover » ou « moving ») trouvée dans le log, pour régler l'outil ouvert depuis une case.
pub fn part_period(check: &LogCheck, part: &str) -> Option<(f64, f64)> {
    match part {
        "hover" => check.hover,
        "moving" => check.moving_part,
        _ => None,
    }
}

/// Numéro du dernier log de ce rôle dans la liste des rôles des logs du drone (le plus récent compte, comme `_last_card_log`).
pub fn last_index_with_role(roles: &[&str], role: &str) -> Option<usize> {
    roles.iter().rposition(|r| *r == role)
}

/// Pour une case qui ouvre un outil sur une partie du vol : (numéro du log dans la liste, période à lui donner) : le dernier log des
/// filtres, s'il a cette partie.
pub fn part_target(checks: &[&LogCheck], part: &str) -> Option<(usize, (f64, f64))> {
    let roles: Vec<&str> = checks.iter().map(|c| c.role).collect();
    let i = last_index_with_role(&roles, FILTERS)?;
    part_period(checks[i], part).map(|p| (i, p))
}

/// Numéros des logs « d'un autre drone » : ni vérifiés pour la fiche choisie, ni en échec (un log du drone dont la vérification a échoué
/// reste au drone).
pub fn other_drone_logs(count: usize, mine: &[usize], failed: &[usize]) -> Vec<usize> {
    (0..count)
        .filter(|i| !mine.contains(i) && !failed.contains(i))
        .collect()
}
