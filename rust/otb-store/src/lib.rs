//! La bibliothèque de données d'OpenTuneBox : drones, logs connus (copiés ou non), fragments de vol, historique
//! des `diff all`, mesures suivies dans le temps et choix du pilote sur les conseils de l'assistant.
//!
//! **Un seul module d'accès** (lot 28 de `docs/plan.md`) : le reste du programme appelle des fonctions de lecture
//! et d'écriture, et ne voit **jamais** un fichier ni un chemin de la bibliothèque. C'est ce qui rendra le
//! passage à une vraie base, le jour où le déclencheur sera atteint, limité à ce seul fichier.
//!
//! **Pourquoi des fichiers JSON et pas une base** (tranché par Cédric le 2026-11-18) : le volume est petit —
//! quelques milliers de fragments et quelques dizaines de milliers de mesures — alors qu'une base embarquée
//! apporterait un compilateur C dans la construction (CI Windows comprise) et la première frontière FFI d'un
//! projet qui refuse `unsafe` (D-054). **Déclencheur mesuré du changement**, écrit d'avance : un fichier qui
//! dépasse quelques Mo, un lancement visiblement plus lent, ou une donnée perdue après une coupure. Chiffres de
//! départ dans `docs/validation.md`.
//!
//! **Ce qui ne s'écrase jamais en silence** (même règle que les fiches drone, D-039) : un fichier illisible, ou
//! qui annonce une version de format plus récente, est **mis de côté** avant la première écriture et la raison
//! est dans `problems()`. Une version plus récente d'OpenTuneBox a pu écrire des champs que celle-ci ignore :
//! les perdre sans le dire serait pire que de refuser de lire.

pub mod link;
pub mod model;

use model::{Advice, Analysis, AnalysisItem, Config, Drone, FORMAT_VERSION, Fragment, LogEntry, Measure};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Nom du dossier de la bibliothèque, dans le dossier de données de l'application. Il est **à part** pour que
/// rien n'y collisionne avec le fichier des fiches drone du Guide (`drones.json`), qui reste dans le dossier
/// parent et que la bibliothèque ne modifie jamais.
pub const FOLDER: &str = "bibliotheque";

/// Les fichiers de la bibliothèque, un par table. ⚠️ `drones.json` est **celui de la bibliothèque**, dans le
/// sous-dossier `bibliotheque/` : ne pas le confondre avec le fichier des fiches du Guide, un niveau au-dessus.
const DRONES: &str = "drones.json";
const LOGS: &str = "logs.json";
const FRAGMENTS: &str = "fragments.json";
const CONFIGS: &str = "configurations.json";
const MEASURES: &str = "mesures.json";
const ANALYSIS: &str = "analyse.json";
const ADVICE: &str = "conseils.json";

/// Sous-dossier des logs **copiés** (ceux récupérés par USB : la carte est vidée ensuite, donc la copie est la
/// seule trace). Les logs que l'utilisateur ouvre lui-même restent où ils sont, et la bibliothèque n'écrit jamais
/// dans leur dossier.
pub const LOGS_FOLDER: &str = "logs";

/// Empreinte d'un contenu : FNV-1a 64 bits mêlé à la longueur, en hexadécimal.
///
/// Elle sert à **refuser un doublon** et à reconnaître deux `diff all` identiques, pas à prouver une identité :
/// ce n'est pas une empreinte cryptographique et elle ne protège de rien de volontaire. Deux logs différents
/// tomberaient sur la même valeur avec une chance de l'ordre de 1 sur 2⁶⁴. Choisie pour ne pas ajouter de
/// dépendance livrée (D-045 : chaque dépendance passe par `cargo deny` et par les mentions de licences).
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}{:08x}", bytes.len() as u32)
}

/// Le dossier de la bibliothèque, calculé comme celui des fiches drone (`rust/otb-gui/src/guide.rs`) puis suivi
/// de `bibliotheque/`. Le système et la lecture des variables sont des arguments : les tests couvrent les trois
/// systèmes sans toucher au poste.
pub fn dir_in(os: &str, var: &dyn Fn(&str) -> Option<OsString>) -> PathBuf {
    let get = |k: &str| var(k).filter(|v| !v.is_empty());
    let home = get("HOME").or_else(|| get("USERPROFILE")).map(PathBuf::from);
    let base = match os {
        "windows" => get("APPDATA").map(PathBuf::from),
        "macos" => home.as_ref().map(|h| h.join("Library").join("Application Support")),
        // XDG : seul un chemin absolu au sens d'Unix compte (L-074, même raison que pour les fiches drone)
        _ => get("XDG_DATA_HOME")
            .filter(|v| v.to_string_lossy().starts_with('/'))
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local").join("share"))),
    };
    match base {
        Some(b) => b.join("OpenTuneBox").join("OpenTuneBox").join(FOLDER),
        None => home.unwrap_or_default().join(".opentunebox").join(FOLDER),
    }
}

/// Le dossier de la bibliothèque sur ce poste.
pub fn dir() -> PathBuf {
    dir_in(std::env::consts::OS, &|k| std::env::var_os(k))
}

/// `nom.json.suffixe`, puis `.suffixe.1`… : un fichier déjà mis de côté n'est jamais écrasé.
fn free_name(path: &Path, suffix: &str) -> PathBuf {
    let first = path.with_file_name(format!(
        "{}.{suffix}",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if !first.exists() {
        return first;
    }
    for n in 1..1000 {
        let p = first.with_file_name(format!(
            "{}.{n}",
            first.file_name().unwrap_or_default().to_string_lossy()
        ));
        if !p.exists() {
            return p;
        }
    }
    first
}

/// La bibliothèque : les sept tables en mémoire, et le dossier où elles s'écrivent.
///
/// `Library::in_memory()` n'a pas de dossier : `save` ne fait rien. Les bancs de test l'emploient pour ne
/// **jamais** écrire la vraie bibliothèque de l'utilisateur.
#[derive(Debug, Default)]
pub struct Library {
    dir: Option<PathBuf>,
    drones: Vec<Drone>,
    logs: Vec<LogEntry>,
    fragments: Vec<Fragment>,
    configs: Vec<Config>,
    /// ⚠️ Rangées par clé et non en liste : à 77 040 mesures, une recherche linéaire par insertion mettait
    /// **47 s** dans les tests (mesuré le 2026-11-18), contre 0,5 s ici. L'ordre des clés rend aussi le fichier
    /// écrit identique d'une fois sur l'autre, donc son diff lisible.
    measures: BTreeMap<MeasureKey, Measure>,
    /// La table des mesures a été lue (`attach_measures`). ⚠️ Tant qu'elle ne l'est pas, `save` **n'écrit pas**
    /// `mesures.json` : sinon le premier enregistrement après le lancement viderait le fichier (D-064).
    measures_loaded: bool,
    /// La table des mesures a changé depuis sa dernière écriture. ⚠️ Sans ce drapeau, l'enregistrement automatique
    /// de l'interface (toutes les 30 s) réécrirait les 8 Mo de la table au volume plein, sur le fil de l'écran.
    measures_dirty: bool,
    analysis: Analysis,
    /// Les choix du pilote sur les conseils (lot 31d).
    advice: Vec<Advice>,
    problems: Vec<String>,
    /// Changements de la table des drones depuis l'ouverture (`drones_revision`).
    drones_rev: u64,
    /// Fichiers à mettre de côté avant la première écriture, avec le suffixe qui dit pourquoi.
    aside: Vec<(PathBuf, &'static str)>,
}

/// La clé d'une mesure : le fragment, l'axe et la grandeur.
type MeasureKey = (String, u64, String);

fn measure_key(m: &Measure) -> MeasureKey {
    (m.fragment_id.clone(), m.axis, m.key.clone())
}

/// Une table lue : ses enregistrements lisibles, ce qui s'est mal passé, et le suffixe de mise de côté si le fichier
/// entier est à écarter avant la première écriture.
struct TableRead<T> {
    items: Vec<T>,
    problems: Vec<String>,
    aside: Option<&'static str>,
}

/// Lit le fichier de table `file` du dossier `dir`. Fonction libre (sans `Library`) pour que la table des mesures
/// puisse se lire **sur un autre fil** (`MeasuresSource::read`).
fn read_table<T>(dir: Option<&Path>, file: &str, parse: impl Fn(&Value) -> Result<T, String>) -> TableRead<T> {
    let mut out = TableRead {
        items: Vec::new(),
        problems: Vec::new(),
        aside: None,
    };
    let Some(text) = read_text(dir, file, &mut out.problems) else {
        return out;
    };
    match items_of(&text, file) {
        Err((why, suffix)) => {
            out.problems.push(why);
            out.aside = Some(suffix);
        }
        Ok(items) => {
            out.items.reserve(items.len());
            for (i, v) in items.iter().enumerate() {
                match parse(v) {
                    Ok(t) => out.items.push(t),
                    // un enregistrement abîmé ne fait pas perdre les autres : il est sauté et dit
                    Err(e) => out
                        .problems
                        .push(format!("{file} : enregistrement {} ignoré ({e})", i + 1)),
                }
            }
        }
    }
    out
}

/// Le texte d'un fichier, ou `None` s'il n'existe pas (première ouverture) ou s'il est illisible (consigné).
fn read_text(dir: Option<&Path>, file: &str, problems: &mut Vec<String>) -> Option<String> {
    let path = dir?.join(file);
    match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            problems.push(format!("{file} : lecture impossible ({e})"));
            None
        }
    }
}

/// Où lire la table des mesures, **sans** la bibliothèque : se déplace sur un autre fil (D-064 : au volume plein,
/// la lecture prend une seconde, l'écran ne doit pas geler). Le chemin reste caché : le reste du programme ne voit
/// jamais un fichier de la bibliothèque.
#[derive(Clone, Debug)]
pub struct MeasuresSource {
    dir: Option<PathBuf>,
}

/// La table des mesures lue par `MeasuresSource::read`, à rattacher par `Library::attach_measures`.
pub struct MeasuresFile {
    read: TableRead<Measure>,
}

impl MeasuresSource {
    /// Lit et décode `mesures.json`. Peut prendre une seconde : à appeler hors du fil de l'interface.
    pub fn read(&self) -> MeasuresFile {
        MeasuresFile {
            read: read_table(self.dir.as_deref(), MEASURES, Measure::from_value),
        }
    }
}

/// Le contenu d'un fichier de table : `{"version": 1, "items": [ … ]}`.
fn items_of(text: &str, file: &str) -> Result<Vec<Value>, (String, &'static str)> {
    let v: Value = serde_json::from_str(text).map_err(|e| (format!("{file} : JSON invalide ({e})"), "illisible"))?;
    let version = v.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > FORMAT_VERSION {
        return Err((
            format!(
                "{file} : version de format {version}, plus récente que {FORMAT_VERSION} — gardé de côté, rien n'est perdu"
            ),
            "plus-recente",
        ));
    }
    match v.get("items").and_then(Value::as_array) {
        Some(a) => Ok(a.clone()),
        None => Err((format!("{file} : liste « items » absente"), "illisible")),
    }
}

impl Library {
    /// Une bibliothèque qui ne s'écrit pas : pour les bancs de test et les essais.
    pub fn in_memory() -> Self {
        Library {
            // rien à lire : la table des mesures est d'office complète
            measures_loaded: true,
            ..Library::default()
        }
    }

    /// La bibliothèque du dossier `dir`. **Ne rend jamais d'erreur** : un dossier absent est une première
    /// ouverture, et un fichier illisible laisse sa table vide, consigne la raison dans `problems()` et sera mis
    /// de côté à la première écriture.
    ///
    /// ⚠️ **La table des mesures n'est pas lue ici** (D-064) : au volume plein elle met une seconde à se lire, à
    /// elle seule plus que les cinq autres réunies. Elle se lit à la première question, par `measures_source`
    /// (sur un autre fil) puis `attach_measures`, ou d'un coup par `load_measures`.
    pub fn open(dir: PathBuf) -> Self {
        let mut lib = Library {
            dir: Some(dir),
            ..Library::default()
        };
        lib.drones = lib.read_items(DRONES, Drone::from_value);
        lib.logs = lib.read_items(LOGS, LogEntry::from_value);
        lib.fragments = lib.read_items(FRAGMENTS, Fragment::from_value);
        lib.configs = lib.read_items(CONFIGS, Config::from_value);
        lib.advice = lib.read_items(ADVICE, Advice::from_value);
        let items = lib.read_items(ANALYSIS, AnalysisItem::from_value);
        lib.analysis = Analysis {
            items,
            working: lib.read_working(),
        };
        lib
    }

    /// Lit un fichier de table et rend ses enregistrements lisibles ; les autres sont comptés dans `problems`.
    fn read_items<T>(&mut self, file: &str, parse: impl Fn(&Value) -> Result<T, String>) -> Vec<T> {
        let read = read_table(self.dir.as_deref(), file, parse);
        self.take_read(file, read)
    }

    /// Consigne les problèmes d'une table lue et la met de côté s'il le faut ; rend ses enregistrements.
    fn take_read<T>(&mut self, file: &str, read: TableRead<T>) -> Vec<T> {
        self.problems.extend(read.problems);
        if let Some(suffix) = read.aside {
            self.set_aside(file, suffix);
        }
        read.items
    }

    /// Le fragment de travail, rangé à côté de la liste d'analyse.
    fn read_working(&mut self) -> Option<String> {
        let text = self.read_file(ANALYSIS)?;
        let v: Value = serde_json::from_str(&text).ok()?;
        v.get("travail")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Le texte d'un fichier, ou `None` s'il n'existe pas (première ouverture) ou s'il est illisible (consigné).
    fn read_file(&mut self, file: &str) -> Option<String> {
        read_text(self.dir.as_deref(), file, &mut self.problems)
    }

    fn set_aside(&mut self, file: &str, suffix: &'static str) {
        if let Some(d) = &self.dir {
            self.aside.push((d.join(file), suffix));
        }
    }

    /// Ce qui s'est mal passé à l'ouverture, en français, à montrer à l'utilisateur. Vide = tout va bien.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Le dossier des logs copiés, `None` pour une bibliothèque en mémoire.
    pub fn logs_dir(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(LOGS_FOLDER))
    }

    /// Place occupée par les logs **copiés**, à annoncer à l'utilisateur avant une récupération.
    pub fn copied_bytes(&self) -> u64 {
        self.logs.iter().filter(|l| l.copied).map(|l| l.bytes).sum()
    }

    // ------------------------------------------------------------ écriture

    /// Écrit les sept fichiers. Un fichier illisible ou plus récent est d'abord **mis de côté**, jamais écrasé.
    /// Ne fait rien pour une bibliothèque en mémoire.
    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = self.dir.clone() else {
            return Ok(());
        };
        for (path, suffix) in std::mem::take(&mut self.aside) {
            if path.exists() {
                std::fs::rename(&path, free_name(&path, suffix))?;
            }
        }
        std::fs::create_dir_all(&dir)?;
        let table = |items: Vec<Value>| -> Value {
            let mut m = Map::new();
            m.insert("version".into(), FORMAT_VERSION.into());
            m.insert("items".into(), Value::Array(items));
            Value::Object(m)
        };
        let mut analysis = Map::new();
        analysis.insert("version".into(), FORMAT_VERSION.into());
        analysis.insert(
            "items".into(),
            Value::Array(self.analysis.items.iter().map(AnalysisItem::to_value).collect()),
        );
        analysis.insert(
            "travail".into(),
            self.analysis.working.clone().unwrap_or_default().into(),
        );
        let files = [
            (DRONES, table(self.drones.iter().map(Drone::to_value).collect())),
            (LOGS, table(self.logs.iter().map(LogEntry::to_value).collect())),
            (
                FRAGMENTS,
                table(self.fragments.iter().map(Fragment::to_value).collect()),
            ),
            (CONFIGS, table(self.configs.iter().map(Config::to_value).collect())),
            (MEASURES, table(self.measures.values().map(Measure::to_value).collect())),
            (ANALYSIS, Value::Object(analysis)),
            (ADVICE, table(self.advice.iter().map(Advice::to_value).collect())),
        ];
        for (name, value) in files {
            if name == MEASURES && !(self.measures_loaded && self.measures_dirty) {
                continue; // jamais lue, ou inchangée : le fichier reste tel quel (D-064)
            }
            // fichier temporaire puis renommage : une coupure laisse l'ancien fichier entier, jamais un
            // fichier à moitié écrit (même procédé que les fiches drone, D-039)
            let path = dir.join(name);
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_string(&value).expect("écriture en mémoire"))?;
            std::fs::rename(&tmp, &path)?;
        }
        if self.measures_loaded {
            self.measures_dirty = false;
        }
        Ok(())
    }

    // ------------------------------------------------------------ drones

    pub fn drones(&self) -> &[Drone] {
        &self.drones
    }

    /// Ajoute ou remplace un drone, reconnu par l'identifiant de sa carte.
    pub fn put_drone(&mut self, drone: Drone) {
        self.drones_rev += 1;
        match self.drones.iter_mut().find(|d| d.id == drone.id) {
            Some(slot) => *slot = drone,
            None => self.drones.push(drone),
        }
    }

    pub fn drone(&self, id: &str) -> Option<&Drone> {
        self.drones.iter().find(|d| d.id == id)
    }

    /// Le drone qui porte ce nom (le `craft_name` des logs) : c'est le nom, et non l'identifiant, qui relie un
    /// log à sa fiche (écran de la fiche, D-068). Un drone créé à la main garde l'identifiant de sa création
    /// même renommé ; le chercher par identifiant raterait ses logs suivants. Le premier, si deux le portent.
    pub fn drone_by_name(&self, name: &str) -> Option<&Drone> {
        self.drones.iter().find(|d| d.name == name)
    }

    /// Compteur des changements de la table des drones : l'interface s'en sert pour savoir quand relire les
    /// fiches, sans les comparer à chaque image.
    pub fn drones_revision(&self) -> u64 {
        self.drones_rev
    }

    /// Supprime un drone et **tout ce qui le désigne** : ses logs connus, leurs fragments (avec leurs mesures, leur
    /// place dans la liste d'analyse et les choix faits sur leurs conseils) et ses configurations. Rend les identifiants des fragments retirés,
    /// pour que l'appelant ferme ce qui les affiche.
    ///
    /// ⚠️ Les fichiers de logs **copiés** dans la bibliothèque restent sur le disque : on n'efface jamais un vol
    /// sans un geste qui le dise (W-075).
    pub fn remove_drone(&mut self, id: &str) -> Vec<String> {
        let logs: Vec<String> = self
            .logs
            .iter()
            .filter(|l| l.drone_id == id)
            .map(|l| l.id.clone())
            .collect();
        let fragments: Vec<String> = self
            .fragments
            .iter()
            .filter(|f| logs.contains(&f.log_id))
            .map(|f| f.id.clone())
            .collect();
        for f in &fragments {
            self.remove_fragment(f);
        }
        self.logs.retain(|l| l.drone_id != id);
        self.configs.retain(|c| c.drone_id != id);
        self.drones.retain(|d| d.id != id);
        self.drones_rev += 1;
        fragments
    }

    // ------------------------------------------------------------ logs

    pub fn logs(&self) -> &[LogEntry] {
        &self.logs
    }

    /// Ajoute un log. Rend `false` **sans rien changer** si son empreinte est déjà connue : deux fois le même
    /// fichier ne fait pas deux entrées, et une récupération relancée ne duplique rien.
    pub fn add_log(&mut self, log: LogEntry) -> bool {
        if self.logs.iter().any(|l| l.id == log.id) {
            return false;
        }
        self.logs.push(log);
        true
    }

    pub fn log(&self, id: &str) -> Option<&LogEntry> {
        self.logs.iter().find(|l| l.id == id)
    }

    // ------------------------------------------------------------ fragments

    pub fn fragments(&self) -> &[Fragment] {
        &self.fragments
    }

    /// Ajoute ou remplace un fragment. ⚠️ Si ses **bornes changent**, ses mesures sont jetées : elles avaient été
    /// calculées sur un autre morceau de vol (c'est la règle qui empêche la bibliothèque d'afficher un chiffre
    /// périmé).
    pub fn put_fragment(&mut self, fragment: Fragment) {
        let moved =
            self.fragments.iter().find(|f| f.id == fragment.id).is_some_and(|old| {
                old.start != fragment.start || old.end != fragment.end || old.log_id != fragment.log_id
            });
        if moved {
            self.drop_measures_of_fragment(&fragment.id);
        }
        match self.fragments.iter_mut().find(|f| f.id == fragment.id) {
            Some(slot) => *slot = fragment,
            None => self.fragments.push(fragment),
        }
    }

    pub fn fragment(&self, id: &str) -> Option<&Fragment> {
        self.fragments.iter().find(|f| f.id == id)
    }

    /// Supprime un fragment, **avec ses mesures, sa place dans la liste d'analyse et les choix faits sur ses conseils** : rien ne doit plus désigner
    /// un fragment absent (une mesure orpheline ressortirait dans l'historique, une entrée de liste rouvrirait un
    /// fantôme au lancement). `false` s'il n'existait pas.
    pub fn remove_fragment(&mut self, id: &str) -> bool {
        let before = self.fragments.len();
        self.fragments.retain(|f| f.id != id);
        if before == self.fragments.len() {
            return false;
        }
        self.drop_measures_of_fragment(id);
        self.remove_from_analysis(id);
        self.advice.retain(|a| a.fragment_id != id);
        true
    }

    /// Les fragments d'un drone, par le log dont ils viennent.
    pub fn fragments_of_drone(&self, drone_id: &str) -> Vec<&Fragment> {
        self.fragments
            .iter()
            .filter(|f| self.log(&f.log_id).is_some_and(|l| l.drone_id == drone_id))
            .collect()
    }

    /// Les fragments d'un drone **dans l'ordre de ses vols** : par date, puis par nom de log, session et début (la
    /// date d'un fichier est au jour près). C'est l'ordre des numéros de vol du suivi et celui du « vol précédent »
    /// de l'assistant : les deux doivent dire la même chose.
    pub fn flights_of_drone(&self, drone_id: &str) -> Vec<&Fragment> {
        let mut fragments = self.fragments_of_drone(drone_id);
        let log_name = |f: &Fragment| self.log(&f.log_id).map(|l| l.name.as_str()).unwrap_or("");
        fragments.sort_by(|a, b| {
            (&a.date, log_name(a), a.session, a.start).cmp(&(&b.date, log_name(b), b.session, b.start))
        });
        fragments
    }

    // ------------------------------------------------------------ choix sur les conseils

    /// Les choix faits sur les conseils d'un fragment, dans l'ordre où ils ont été faits.
    pub fn advice_of(&self, fragment_id: &str) -> Vec<&Advice> {
        self.advice.iter().filter(|a| a.fragment_id == fragment_id).collect()
    }

    /// Le choix fait sur la carte `card` du fragment, s'il y en a un.
    pub fn advice(&self, fragment_id: &str, card: &str) -> Option<&Advice> {
        self.advice
            .iter()
            .find(|a| a.fragment_id == fragment_id && a.card == card)
    }

    /// Inscrit un choix ; il **remplace** celui de la même carte du même fragment (un seul choix par carte).
    pub fn set_advice(&mut self, advice: Advice) {
        match self
            .advice
            .iter_mut()
            .find(|a| a.fragment_id == advice.fragment_id && a.card == advice.card)
        {
            Some(slot) => *slot = advice,
            None => self.advice.push(advice),
        }
    }

    /// Annule le choix fait sur une carte. `false` s'il n'y en avait pas.
    pub fn remove_advice(&mut self, fragment_id: &str, card: &str) -> bool {
        let before = self.advice.len();
        self.advice
            .retain(|a| !(a.fragment_id == fragment_id && a.card == card));
        before != self.advice.len()
    }

    // ------------------------------------------------------------ configurations

    pub fn configs(&self) -> &[Config] {
        &self.configs
    }

    /// Enregistre un `diff all`. Rend `false` s'il est **identique au dernier** de ce drone : on garde
    /// l'historique des changements de réglages, pas une copie par branchement.
    pub fn record_config(&mut self, drone_id: &str, recorded: &str, text: &str) -> bool {
        let last = self.configs.iter().rev().find(|c| c.drone_id == drone_id);
        if last.is_some_and(|c| c.text == text) {
            return false;
        }
        self.configs.push(Config {
            id: format!("{drone_id}-{}", fingerprint(text.as_bytes())),
            drone_id: drone_id.to_string(),
            recorded: recorded.to_string(),
            text: text.to_string(),
        });
        true
    }

    /// Les configurations d'un drone, de la plus ancienne à la plus récente.
    pub fn configs_of_drone(&self, drone_id: &str) -> Vec<&Config> {
        self.configs.iter().filter(|c| c.drone_id == drone_id).collect()
    }

    // ------------------------------------------------------------ mesures

    /// Où lire la table des mesures sur un autre fil (D-064) ; le résultat se rattache par `attach_measures`.
    pub fn measures_source(&self) -> MeasuresSource {
        MeasuresSource { dir: self.dir.clone() }
    }

    /// La table des mesures a été lue : avant, les questions sur les mesures répondent « rien ».
    pub fn measures_loaded(&self) -> bool {
        self.measures_loaded
    }

    /// Rattache la table des mesures lue par `MeasuresSource::read`. Une mesure posée avant (`set_measure`) est plus
    /// récente que le fichier et l'emporte. ⚠️ Une mesure dont le fragment a disparu ou dont les bornes ne sont plus
    /// celles du fragment est **jetée ici** : elle a pu changer pendant que la table n'était pas lue. Sans effet si la
    /// table est déjà rattachée (une seconde lecture n'a rien à apprendre).
    pub fn attach_measures(&mut self, file: MeasuresFile) {
        if self.measures_loaded {
            return;
        }
        // une mesure posée avant la lecture, ou un fichier mis de côté : la table est à écrire
        let mut dirty = !self.measures.is_empty() || file.read.aside.is_some();
        for m in self.take_read(MEASURES, file.read) {
            self.measures.entry(measure_key(&m)).or_insert(m);
        }
        let bounds: BTreeMap<&str, (u64, u64)> = self
            .fragments
            .iter()
            .map(|f| (f.id.as_str(), (f.start, f.end)))
            .collect();
        let before = self.measures.len();
        self.measures
            .retain(|_, m| bounds.get(m.fragment_id.as_str()) == Some(&(m.start, m.end)));
        dirty |= self.measures.len() != before;
        self.measures_dirty |= dirty;
        self.measures_loaded = true;
    }

    /// Lit et rattache la table des mesures **sur ce fil** : pour la ligne de commande et les tests, jamais pour
    /// l'interface.
    pub fn load_measures(&mut self) {
        let file = self.measures_source().read();
        self.attach_measures(file);
    }

    /// Le fragment a des mesures de la version `calc_version` ou plus récente, calculées sur ses bornes actuelles.
    pub fn is_measured(&self, fragment: &Fragment, calc_version: u64) -> bool {
        self.measures
            .range((fragment.id.clone(), 0, String::new())..)
            .take_while(|((f, _, _), _)| *f == fragment.id)
            .any(|(_, m)| m.calc_version >= calc_version && m.start == fragment.start && m.end == fragment.end)
    }

    /// Les mesures, rangées par fragment, axe et grandeur.
    pub fn measures(&self) -> impl Iterator<Item = &Measure> {
        self.measures.values()
    }

    pub fn measures_len(&self) -> usize {
        self.measures.len()
    }

    /// Ajoute ou remplace la mesure d'un fragment, d'un axe et d'une grandeur.
    pub fn set_measure(&mut self, measure: Measure) {
        self.measures_dirty = true;
        self.measures.insert(measure_key(&measure), measure);
    }

    pub fn measure(&self, fragment_id: &str, axis: u64, key: &str) -> Option<&Measure> {
        self.measures.get(&(fragment_id.to_string(), axis, key.to_string()))
    }

    /// Jette les mesures d'un fragment (ses bornes ont changé).
    pub fn drop_measures_of_fragment(&mut self, fragment_id: &str) -> usize {
        let before = self.measures.len();
        self.measures.retain(|(f, _, _), _| f != fragment_id);
        self.measures_dirty |= self.measures.len() != before;
        before - self.measures.len()
    }

    /// Jette les mesures calculées par une version antérieure à `version` : le calcul a changé, les chiffres
    /// gardés ne veulent plus rien dire.
    pub fn drop_measures_before_version(&mut self, version: u64) -> usize {
        let before = self.measures.len();
        self.measures.retain(|_, m| m.calc_version >= version);
        self.measures_dirty |= self.measures.len() != before;
        before - self.measures.len()
    }

    /// L'historique d'une grandeur pour un drone : une valeur par fragment, **rangée par date de vol**. C'est la
    /// question à laquelle la bibliothèque doit répondre — « l'évolution de la latence roll sur tous les vols de
    /// ce drone » — et la raison d'être de la table des mesures.
    pub fn history(&self, drone_id: &str, key: &str, axis: u64) -> Vec<(String, f64)> {
        // les fragments du drone par identifiant : une recherche par mesure dans la liste des fragments coûterait
        // fragments × mesures comparaisons au volume plein
        let fragments: BTreeMap<&str, &Fragment> = self
            .fragments_of_drone(drone_id)
            .into_iter()
            .map(|f| (f.id.as_str(), f))
            .collect();
        let mut out: Vec<(String, String, f64)> = self
            .measures
            .values()
            .filter(|m| m.key == key && m.axis == axis)
            .filter_map(|m| {
                let f = fragments.get(m.fragment_id.as_str())?;
                // calculée sur d'autres bornes : périmée
                ((f.start, f.end) == (m.start, m.end)).then(|| (f.date.clone(), f.id.clone(), m.value))
            })
            .collect();
        // à date égale, l'identifiant du fragment range : deux fragments du même vol gardent un ordre stable
        out.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        out.into_iter().map(|(d, _, v)| (d, v)).collect()
    }

    // ------------------------------------------------------------ liste d'analyse

    pub fn analysis(&self) -> &Analysis {
        &self.analysis
    }

    pub fn set_analysis(&mut self, analysis: Analysis) {
        self.analysis = analysis;
    }

    /// Ajoute un fragment à la liste d'analyse, actif, avec le **plus petit indice de couleur libre**.
    ///
    /// Rend `false` s'il y est déjà : sa couleur ne change donc jamais tant qu'il reste dans la liste, et une
    /// courbe garde sa couleur d'un écran à l'autre. ⚠️ Un indice, pas une couleur : les couleurs ne s'écrivent
    /// que dans `rust/otb-gui/src/theme.rs` (L-016).
    pub fn add_to_analysis(&mut self, fragment_id: &str) -> bool {
        if self.analysis.items.iter().any(|i| i.fragment_id == fragment_id) {
            return false;
        }
        let used: Vec<u64> = self.analysis.items.iter().map(|i| i.colour).collect();
        let colour = (0..).find(|c| !used.contains(c)).unwrap_or(0);
        self.analysis.items.push(AnalysisItem {
            fragment_id: fragment_id.to_string(),
            colour,
            active: true,
        });
        true
    }

    /// Retire un fragment de la liste d'analyse. Son indice de couleur redevient libre, et s'il était le fragment
    /// de travail celui-ci est oublié — sinon l'interface désignerait un fragment absent de la liste.
    pub fn remove_from_analysis(&mut self, fragment_id: &str) -> bool {
        let before = self.analysis.items.len();
        self.analysis.items.retain(|i| i.fragment_id != fragment_id);
        if self.analysis.working.as_deref() == Some(fragment_id) {
            self.analysis.working = None;
        }
        before != self.analysis.items.len()
    }

    /// Déplace un fragment de la liste à la place `to` (son ordre décide de l'ordre des courbes).
    pub fn move_in_analysis(&mut self, fragment_id: &str, to: usize) -> bool {
        let Some(from) = self.analysis.items.iter().position(|i| i.fragment_id == fragment_id) else {
            return false;
        };
        let item = self.analysis.items.remove(from);
        let to = to.min(self.analysis.items.len());
        self.analysis.items.insert(to, item);
        true
    }

    /// Montre ou cache un fragment sans le retirer de la liste : il garde sa place et sa couleur.
    pub fn set_active(&mut self, fragment_id: &str, active: bool) -> bool {
        match self.analysis.items.iter_mut().find(|i| i.fragment_id == fragment_id) {
            Some(i) => {
                i.active = active;
                true
            }
            None => false,
        }
    }

    /// Désigne le fragment de travail. Il doit être dans la liste d'analyse : sinon l'interface afficherait des
    /// réglages qui ne correspondent à aucune courbe.
    pub fn set_working(&mut self, fragment_id: Option<&str>) -> bool {
        match fragment_id {
            None => {
                self.analysis.working = None;
                true
            }
            Some(id) if self.analysis.items.iter().any(|i| i.fragment_id == id) => {
                self.analysis.working = Some(id.to_string());
                true
            }
            Some(_) => false,
        }
    }

    /// Les fragments dont le log n'est plus connu de la bibliothèque : à signaler à l'utilisateur plutôt qu'à
    /// afficher comme s'ils étaient analysables (critère d'arrêt du lot 29).
    pub fn orphan_fragments(&self) -> Vec<&Fragment> {
        self.fragments
            .iter()
            .filter(|f| self.log(&f.log_id).is_none())
            .collect()
    }
}
