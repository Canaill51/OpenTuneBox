//! Les six tables de la bibliothèque et leur écriture en JSON.
//!
//! Conversion **à la main** (`to_value` / `from_value`), comme les fiches drone de
//! `rust/otb-gui/src/guide.rs` : aucune dérive, donc aucune dépendance de plus, et une lecture **tolérante** —
//! un champ absent prend sa valeur par défaut, seul un identifiant manquant rend une erreur. Un fichier venu
//! d'une version plus récente peut ainsi porter des champs que cette version ignore sans rien casser.
//!
//! Les dates sont des **chaînes** fournies par l'appelant (ISO 8601) : la bibliothèque n'a pas d'horloge, ce qui
//! la rend testable sans dépendance au temps.

use serde_json::{Map, Value};

/// Version du format des fichiers. ⚠️ Un fichier qui annonce une version **plus grande** n'est jamais écrasé :
/// `Library::open` le met de côté et le dit dans `problems` (lot 28, même règle que D-039 pour les fiches drone).
pub const FORMAT_VERSION: u64 = 1;

// ---------------------------------------------------------------- lecture tolérante

/// Chaîne d'un champ, ou `""` s'il manque.
fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Chaîne d'un champ exigé : `Err` s'il manque ou s'il est vide (un enregistrement sans identifiant ne se
/// retrouverait plus).
fn required(v: &Value, k: &str) -> Result<String, String> {
    let s = text(v, k);
    if s.is_empty() {
        return Err(format!("champ « {k} » manquant"));
    }
    Ok(s)
}

/// Entier d'un champ, ou 0 s'il manque.
fn number(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

/// Flottant d'un champ, ou NaN s'il manque (une mesure absente n'est pas une mesure à zéro).
fn float(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// Booléen d'un champ, ou `default` s'il manque.
fn flag(v: &Value, k: &str, default: bool) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(default)
}

/// `{"clé": valeur, …}` dans l'ordre d'écriture (`preserve_order`) : un fichier relu puis réécrit sans
/// modification garde le même texte, donc le même diff.
fn object(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

// ---------------------------------------------------------------- drones

/// Un drone. La clé est l'**identifiant de la carte de vol** (`MSP_UID` quand il vient de l'USB, sinon un
/// identifiant choisi par l'appelant) : le nom reste un libellé modifiable, parce que deux drones peuvent porter
/// le même et qu'il se change en une commande.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drone {
    pub id: String,
    pub name: String,
    pub firmware: String,
    /// Première et dernière fois que ce drone a été vu (dates ISO fournies par l'appelant).
    pub first_seen: String,
    pub last_seen: String,
}

impl Drone {
    pub fn new(id: &str, name: &str) -> Self {
        Drone {
            id: id.to_string(),
            name: name.to_string(),
            firmware: String::new(),
            first_seen: String::new(),
            last_seen: String::new(),
        }
    }

    pub fn to_value(&self) -> Value {
        object(vec![
            ("id", self.id.clone().into()),
            ("nom", self.name.clone().into()),
            ("firmware", self.firmware.clone().into()),
            ("vu_le", self.first_seen.clone().into()),
            ("revu_le", self.last_seen.clone().into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(Drone {
            id: required(v, "id")?,
            name: text(v, "nom"),
            firmware: text(v, "firmware"),
            first_seen: text(v, "vu_le"),
            last_seen: text(v, "revu_le"),
        })
    }
}

// ---------------------------------------------------------------- logs

/// Un fichier de log connu de la bibliothèque. `copied` dit si le fichier vit **dans** la bibliothèque (récupéré
/// par USB : la carte est vidée ensuite, donc la copie est la seule trace) ou s'il reste là où l'utilisateur l'a
/// rangé (⚠️ dans ce cas la bibliothèque n'y écrit jamais).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// L'empreinte du contenu sert d'identifiant : deux fois le même fichier, un seul enregistrement.
    pub id: String,
    pub drone_id: String,
    pub name: String,
    pub path: String,
    pub copied: bool,
    pub bytes: u64,
    pub added: String,
}

impl LogEntry {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("id", self.id.clone().into()),
            ("drone", self.drone_id.clone().into()),
            ("nom", self.name.clone().into()),
            ("chemin", self.path.clone().into()),
            ("copie", self.copied.into()),
            ("octets", self.bytes.into()),
            ("ajoute_le", self.added.clone().into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(LogEntry {
            id: required(v, "id")?,
            drone_id: text(v, "drone"),
            name: text(v, "nom"),
            path: text(v, "chemin"),
            copied: flag(v, "copie", false),
            bytes: number(v, "octets"),
            added: text(v, "ajoute_le"),
        })
    }
}

// ---------------------------------------------------------------- fragments

/// Un morceau de vol nommé : le log d'origine, la session dans ce fichier, et les bornes en **numéros
/// d'échantillon** (réglables à l'échantillon près dans l'écran d'extraction). `role` est le rôle au sens du
/// Guide (« filtres », « série PID ») : la chaîne est celle des constantes de `otb-advisor`, pas un nouveau
/// vocabulaire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub id: String,
    pub log_id: String,
    pub session: u64,
    pub start: u64,
    pub end: u64,
    pub name: String,
    pub role: String,
    /// Date du vol (ISO), utilisée pour ranger l'historique d'une mesure.
    pub date: String,
    /// Les bornes ont été **choisies** dans l'écran d'extraction. Tant que ce n'est pas le cas, elles suivent les
    /// limites automatiques du vol (décollage → atterrissage, `otb_analysis::period::auto_period`) à chaque
    /// ouverture : un fragment rangé d'office n'a rien décidé, il ne doit pas figer un calcul qui peut s'améliorer.
    /// Absent des fichiers écrits avant le lot 29b : lu comme `false`, ce qui est juste (personne n'avait pu choisir).
    pub chosen: bool,
}

impl Fragment {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("id", self.id.clone().into()),
            ("log", self.log_id.clone().into()),
            ("session", self.session.into()),
            ("debut", self.start.into()),
            ("fin", self.end.into()),
            ("nom", self.name.clone().into()),
            ("role", self.role.clone().into()),
            ("date", self.date.clone().into()),
            ("bornes_choisies", self.chosen.into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(Fragment {
            id: required(v, "id")?,
            log_id: text(v, "log"),
            session: number(v, "session"),
            start: number(v, "debut"),
            end: number(v, "fin"),
            name: text(v, "nom"),
            role: text(v, "role"),
            date: text(v, "date"),
            chosen: flag(v, "bornes_choisies", false),
        })
    }
}

// ---------------------------------------------------------------- configurations

/// Un `diff all` daté. Un nouveau n'est gardé que s'il **diffère** du dernier de ce drone : on obtient
/// l'historique des réglages, et non une copie par branchement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub id: String,
    pub drone_id: String,
    pub recorded: String,
    pub text: String,
}

impl Config {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("id", self.id.clone().into()),
            ("drone", self.drone_id.clone().into()),
            ("le", self.recorded.clone().into()),
            ("texte", self.text.clone().into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(Config {
            id: required(v, "id")?,
            drone_id: text(v, "drone"),
            recorded: text(v, "le"),
            text: text(v, "texte"),
        })
    }
}

// ---------------------------------------------------------------- mesures

/// Une mesure calculée sur un fragment : une ligne par fragment, axe et grandeur (latence, pic, temps de montée,
/// marges, bruit du gyro et du terme D, retard des filtres…).
///
/// ⚠️ Deux champs décident de sa validité, et **sans eux la bibliothèque garderait des chiffres périmés sans le
/// savoir** : `settings` (empreinte des réglages de calcul qui l'ont produite) et `calc_version` (version du
/// calcul). Changer les bornes du fragment ou la version jette la mesure (`Library::drop_measures_of_fragment`,
/// `Library::drop_measures_before_version`).
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    pub fragment_id: String,
    pub axis: u64,
    pub key: String,
    pub value: f64,
    pub settings: String,
    pub calc_version: u64,
}

impl Measure {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("fragment", self.fragment_id.clone().into()),
            ("axe", self.axis.into()),
            ("grandeur", self.key.clone().into()),
            ("valeur", self.value.into()),
            ("reglages", self.settings.clone().into()),
            ("version_calcul", self.calc_version.into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(Measure {
            fragment_id: required(v, "fragment")?,
            axis: number(v, "axe"),
            key: required(v, "grandeur")?,
            value: float(v, "valeur"),
            settings: text(v, "reglages"),
            calc_version: number(v, "version_calcul"),
        })
    }
}

// ---------------------------------------------------------------- liste d'analyse

/// Un fragment de la liste d'analyse : sa place dans l'ordre vient de sa position dans la liste. `colour` est un
/// **indice** de couleur, jamais une couleur : les couleurs ne s'écrivent que dans `rust/otb-gui/src/theme.rs`
/// (L-016).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisItem {
    pub fragment_id: String,
    pub colour: u64,
    pub active: bool,
}

/// La liste d'analyse : l'ordre, la couleur de chaque fragment, lesquels sont actifs, et le fragment de travail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Analysis {
    pub items: Vec<AnalysisItem>,
    pub working: Option<String>,
}

impl AnalysisItem {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("fragment", self.fragment_id.clone().into()),
            ("couleur", self.colour.into()),
            ("actif", self.active.into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(AnalysisItem {
            fragment_id: required(v, "fragment")?,
            colour: number(v, "couleur"),
            active: flag(v, "actif", true),
        })
    }
}
