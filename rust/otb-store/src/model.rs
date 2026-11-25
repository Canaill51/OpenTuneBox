//! Les sept tables de la bibliothèque et leur écriture en JSON.
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

/// Un drone. Sa clé `id` est choisie à sa création (le nom de machine de son premier log, ou le nom donné au
/// branchement USB) et ne change plus ; le nom reste un libellé modifiable. Une fois branché par USB, le drone porte
/// aussi l'**identifiant gravé de sa carte** (`uid`, lot 32b, D-074) : c'est lui qui le reconnaît ensuite, même
/// renommé, et même si deux drones portent le même nom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drone {
    pub id: String,
    pub name: String,
    pub firmware: String,
    /// Première et dernière fois que ce drone a été vu (dates ISO fournies par l'appelant).
    pub first_seen: String,
    pub last_seen: String,
    /// Carte de vol (`STM32F405`…), lue au branchement USB ; vide sinon.
    pub board: String,
    /// Identifiant gravé de la carte de vol (`MSP_UID`, 24 chiffres hexadécimaux), lu au branchement USB ; vide tant
    /// que le drone n'a pas été branché. ⚠️ Donnée de l'utilisateur : ne jamais la recopier dans le dépôt (public).
    pub uid: String,
    /// La fiche du drone (D-068). `None` : jamais remplie — le drone n'est connu que par ses logs, et le Guide
    /// n'a pas de fiche pour lui. ⚠️ Ne pas la remplacer par des valeurs par défaut : une fiche inventée
    /// donnerait des conseils faux (taille, version) sans que personne ne l'ait choisie.
    pub sheet: Option<DroneSheet>,
    /// Ce que le **dernier log** du drone (par sa date) dit de la machine ; `None` tant qu'aucun log ne l'a dit.
    /// Absent des fichiers écrits avant : lu `None`.
    pub from_log: Option<LogMachine>,
}

/// Pôles moteur, KV et boucle PID lus dans l'en-tête d'un log (`motor_poles`, `motor_kv`, `looptime` et
/// `pid_process_denom`), écrits comme dans la fiche (`14`, `1950`, `3.2`) ; vide : l'en-tête ne le disait pas.
/// ⚠️ C'est ce que **Betaflight** croit : un mauvais `motor_poles` y est recopié tel quel. La fiche le compare à
/// ce que le pilote a saisi, elle ne l'écrase pas (D-077).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogMachine {
    /// Date du log (ISO) : un log plus ancien rouvert ne remplace pas celui-ci.
    pub date: String,
    pub poles: String,
    pub kv: String,
    pub pid_khz: String,
}

impl LogMachine {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("le", self.date.clone().into()),
            ("poles", self.poles.clone().into()),
            ("kv", self.kv.clone().into()),
            ("boucle_pid_khz", self.pid_khz.clone().into()),
        ])
    }

    pub fn from_value(v: &Value) -> Self {
        LogMachine {
            date: text(v, "le"),
            poles: text(v, "poles"),
            kv: text(v, "kv"),
            pid_khz: text(v, "boucle_pid_khz"),
        }
    }
}

/// Ce que le pilote dit de son drone : la partie « Machine » de l'écran de la fiche, et les cases du Guide
/// cochées à la main. Les valeurs sont gardées **telles qu'affichées** (`5″`, `6S`, `3.2`) ; c'est l'interface
/// qui les traduit pour l'assistant, la bibliothèque ne les interprète pas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DroneSheet {
    /// Taille (`65 mm`, `5″`…) : elle choisit le profil des seuils de l'assistant.
    pub size: String,
    pub cells: String,
    /// Poids en grammes, taille des moteurs (`2207`), moteurs en KV, pôles, hélices : saisis librement, aucun
    /// calcul ne s'en sert encore. Taille des moteurs absente des fichiers écrits avant : lue vide.
    pub weight: String,
    pub motor_size: String,
    pub kv: String,
    pub poles: String,
    pub props: String,
    pub esc: String,
    /// Version de Betaflight choisie (`4.5`, `2026.6`…).
    pub betaflight: String,
    /// Fréquence de la boucle PID en kHz, écrite avec un point (`3.2`).
    pub pid_khz: String,
    /// Clés des cases du Guide cochées à la main, dans l'ordre où elles l'ont été.
    pub done: Vec<String>,
    /// Clés des cases que le Guide a vues **vérifiées par un log** à son dernier relevé (D-071) : la carte du
    /// tableau de bord les compte sans refaire une analyse par log. Le Guide, lui, les recalcule toujours.
    /// Absent des fichiers écrits avant le lot 30g : lu vide.
    pub verified: Vec<String>,
    /// La fiche cible (D-072), case par case : (clé de `otb_advisor::target::ROWS`, texte saisi ; vide = pas de
    /// cible). `None` : le pilote n'a rien changé, la fiche montre les **valeurs proposées** pour la taille, qui
    /// suivent la taille et les seuils de l'assistant. Absente des fichiers écrits avant le lot 31b : lue `None`.
    pub target: Option<Vec<(String, String)>>,
}

/// Une liste de clés de cases ; vide si le champ manque ou n'est pas une liste.
fn keys(v: &Value, field: &str) -> Vec<String> {
    v.get(field)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

impl DroneSheet {
    pub fn to_value(&self) -> Value {
        let mut v = object(vec![
            ("taille", self.size.clone().into()),
            ("batterie", self.cells.clone().into()),
            ("poids", self.weight.clone().into()),
            ("taille_moteurs", self.motor_size.clone().into()),
            ("kv", self.kv.clone().into()),
            ("poles", self.poles.clone().into()),
            ("helices", self.props.clone().into()),
            ("esc", self.esc.clone().into()),
            ("betaflight", self.betaflight.clone().into()),
            ("boucle_pid_khz", self.pid_khz.clone().into()),
            (
                "cases",
                Value::Array(self.done.iter().map(|k| Value::from(k.as_str())).collect()),
            ),
            (
                "cases_verifiees",
                Value::Array(self.verified.iter().map(|k| Value::from(k.as_str())).collect()),
            ),
        ]);
        // écrite seulement si le pilote l'a changée : une fiche sans cible garde le texte d'avant
        if let (Some(t), Value::Object(m)) = (&self.target, &mut v) {
            let cible: Map<String, Value> = t.iter().map(|(k, x)| (k.clone(), Value::from(x.as_str()))).collect();
            m.insert("cible".to_string(), Value::Object(cible));
        }
        v
    }

    pub fn from_value(v: &Value) -> Self {
        DroneSheet {
            size: text(v, "taille"),
            cells: text(v, "batterie"),
            weight: text(v, "poids"),
            motor_size: text(v, "taille_moteurs"),
            kv: text(v, "kv"),
            poles: text(v, "poles"),
            props: text(v, "helices"),
            esc: text(v, "esc"),
            betaflight: text(v, "betaflight"),
            pid_khz: text(v, "boucle_pid_khz"),
            done: keys(v, "cases"),
            verified: keys(v, "cases_verifiees"),
            target: v.get("cible").and_then(Value::as_object).map(|m| {
                m.iter()
                    .map(|(k, x)| (k.clone(), x.as_str().unwrap_or("").to_string()))
                    .collect()
            }),
        }
    }
}

impl Drone {
    pub fn new(id: &str, name: &str) -> Self {
        Drone {
            id: id.to_string(),
            name: name.to_string(),
            firmware: String::new(),
            first_seen: String::new(),
            last_seen: String::new(),
            board: String::new(),
            uid: String::new(),
            sheet: None,
            from_log: None,
        }
    }

    pub fn to_value(&self) -> Value {
        let mut pairs = vec![
            ("id", self.id.clone().into()),
            ("nom", self.name.clone().into()),
            ("firmware", self.firmware.clone().into()),
            ("vu_le", self.first_seen.clone().into()),
            ("revu_le", self.last_seen.clone().into()),
        ];
        // champs écrits seulement s'ils existent : un drone connu par ses seuls logs garde le texte d'avant
        if !self.board.is_empty() {
            pairs.push(("carte", self.board.clone().into()));
        }
        if !self.uid.is_empty() {
            pairs.push(("carte_uid", self.uid.clone().into()));
        }
        if let Some(sheet) = &self.sheet {
            pairs.push(("fiche", sheet.to_value()));
        }
        if let Some(m) = &self.from_log {
            pairs.push(("lu_dans_le_log", m.to_value()));
        }
        object(pairs)
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        Ok(Drone {
            id: required(v, "id")?,
            name: text(v, "nom"),
            firmware: text(v, "firmware"),
            first_seen: text(v, "vu_le"),
            last_seen: text(v, "revu_le"),
            board: text(v, "carte"),
            uid: text(v, "carte_uid"),
            sheet: v.get("fiche").filter(|f| f.is_object()).map(DroneSheet::from_value),
            from_log: v
                .get("lu_dans_le_log")
                .filter(|f| f.is_object())
                .map(LogMachine::from_value),
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

/// Un réglage qui change d'un `diff all` au suivant : (nom, avant, après). Le nom porte la section où il se
/// trouve (`profile 1 · p_roll`) quand ce n'est pas la première : chaque profil a ses propres PID.
pub type ConfigChange = (String, String, String);

/// Les réglages qui changent entre deux `diff all`, dans l'ordre du plus récent. Un réglage absent d'un des deux
/// vaut sa valeur par défaut (c'est ce que dit un `diff all`), écrite ici comme une chaîne vide. Seules les lignes
/// `set nom = valeur` comptent : les autres (ressources, modes, `feature`) ne sont pas lues.
pub fn config_changes(before: &str, after: &str) -> Vec<ConfigChange> {
    let b = config_values(before);
    let a = config_values(after);
    let mut out: Vec<ConfigChange> = Vec::new();
    for (k, v) in &a {
        let old = b.iter().find(|(kb, _)| kb == k).map_or("", |(_, vb)| vb.as_str());
        if old != v {
            out.push((k.clone(), old.to_string(), v.clone()));
        }
    }
    for (k, v) in &b {
        if !a.iter().any(|(ka, _)| ka == k) {
            out.push((k.clone(), v.clone(), String::new()));
        }
    }
    out
}

/// Les lignes `set` d'un `diff all`, nommées par leur section (`profile N`, `rateprofile N`, `battery_profile N` :
/// `cli/cli.c`, `cliDumpPidProfile` et suivantes).
fn config_values(text: &str) -> Vec<(String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        match words.next() {
            Some(w @ ("profile" | "rateprofile" | "battery_profile")) => {
                section = format!("{w} {}", words.next().unwrap_or(""));
            }
            Some("set") => {
                let rest = line["set".len()..].trim();
                if let Some((name, value)) = rest.split_once('=') {
                    let name = name.trim();
                    let key = if section.is_empty() {
                        name.to_string()
                    } else {
                        format!("{section} · {name}")
                    };
                    out.push((key, value.trim().to_string()));
                }
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------- mesures

/// Une mesure calculée sur un fragment : une ligne par fragment, axe et grandeur (latence, pic, temps de montée,
/// marges, bruit du gyro et du terme D, retard des filtres…).
///
/// ⚠️ Trois champs décident de sa validité, et **sans eux la bibliothèque garderait des chiffres périmés sans le
/// savoir** : `start` et `end` (les bornes du fragment au moment du calcul), `settings` (les réglages de calcul qui
/// l'ont produite) et `calc_version` (version du calcul). Changer les bornes du fragment ou la version jette la
/// mesure (`Library::put_fragment`, `Library::drop_measures_before_version`). Les bornes sont **dans la mesure**
/// et pas seulement vérifiées au moment où le fragment change : la table des mesures se lit à la demande (D-064),
/// et un fragment déplacé avant cette lecture doit quand même perdre ses mesures (`Library::attach_measures`).
///
/// `value` peut être NaN : la grandeur n'a pas pu se mesurer sur ce fragment (pas de réponse indicielle, pas de
/// terme D…). La ligne est gardée quand même, pour que le fragment compte comme mesuré et ne se recalcule pas à
/// chaque ouverture ; elle s'écrit `null` dans le fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    pub fragment_id: String,
    pub axis: u64,
    pub key: String,
    pub value: f64,
    pub start: u64,
    pub end: u64,
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
            ("debut", self.start.into()),
            ("fin", self.end.into()),
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
            start: number(v, "debut"),
            end: number(v, "fin"),
            settings: text(v, "reglages"),
            calc_version: number(v, "version_calcul"),
        })
    }
}

// ---------------------------------------------------------------- choix sur les conseils

/// Le choix du pilote sur un conseil de l'assistant (lot 31d, D-073) : **appliqué** (il a changé le réglage à la
/// main, dans le configurateur), ou **écarté** avec sa raison. Il s'inscrit au clic ; un choix par carte et par
/// fragment.
///
/// Il reste attaché au fragment même si ses bornes changent ensuite : c'est le souvenir d'un geste fait sur le
/// drone, pas une mesure qui deviendrait fausse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advice {
    pub fragment_id: String,
    /// La carte : clé du constat et son axe (`card_id` de la page Assistant), jamais son titre (D-028).
    pub card: String,
    /// Titre du constat au moment du choix : l'historique le montre même si le constat a disparu depuis.
    pub title: String,
    /// `true` : appliqué ; `false` : écarté.
    pub applied: bool,
    /// Raison d'un conseil écarté (vide pour un conseil appliqué).
    pub reason: String,
    /// Date et heure du choix, ISO 8601 en temps universel (`AAAA-MM-JJTHH:MM:SSZ`), fournies par l'appelant.
    pub when: String,
}

impl Advice {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("fragment", self.fragment_id.clone().into()),
            ("carte", self.card.clone().into()),
            ("titre", self.title.clone().into()),
            ("choix", if self.applied { "applique" } else { "ecarte" }.into()),
            ("raison", self.reason.clone().into()),
            ("le", self.when.clone().into()),
        ])
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        let applied = match text(v, "choix").as_str() {
            "applique" => true,
            "ecarte" => false,
            autre => return Err(format!("choix inconnu « {autre} »")),
        };
        Ok(Advice {
            fragment_id: required(v, "fragment")?,
            card: required(v, "carte")?,
            title: text(v, "titre"),
            applied,
            reason: text(v, "raison"),
            when: text(v, "le"),
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
