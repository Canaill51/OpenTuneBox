//! La fiche d'un drone : le second écran **hors rail** du prototype (D-060), ouvert par le crayon d'une carte du
//! tableau de bord, par « Nouveau drone », ou par le Guide. Une seule fiche par drone, rangée **dans la
//! bibliothèque** (D-068) : le Guide la lit, il n'a plus de fichier à lui.
//!
//! Le fichier se coupe en deux : ce qui n'a pas de fenêtre (choix proposés, traduction pour l'assistant, reprise de
//! l'ancien fichier des fiches, enregistrement), testé sans écran ; puis l'écran, au dessin du prototype
//! (`docs/maquettes/Application.dc.html`, écran `droneEdit`), dont les mesures sont des constantes nommées.
//!
//! Écarts au prototype, écrits ici et dans D-068 et D-072 (D-061 : un écart se justifie par écrit) :
//! - **sous-titre de la « Fiche cible »** : le prototype dit que « les jauges, le guide et le suivi » s'y comparent ;
//!   seul le suivi s'y compare, l'assistant garde ses seuils (D-072) ;
//! - **note de la fiche cible** : elle dit aussi quand les valeurs ont été changées, et quand les seuils de la
//!   taille sont encore provisoires (`CraftProfile::provisional`) ;
//! - **« Suivi des mesures »** sous la fiche cible, dans la colonne de droite (D-072 ; absent du prototype) : l'état
//!   du calcul, puis un graphique par grandeur de la fiche cible, au fil des vols, la zone visée teintée (lot 31c) ;
//! - **taille « 8″ et plus »** en plus des sept du prototype : l'assistant a un profil pour ces drones ;
//! - **boucle PID à 1,6 kHz** en plus des cinq du prototype : la moitié d'un gyro BMI270, déjà proposée avant ;
//! - **taille saisie librement** (la liste ne fait que proposer), le profil de l'Assistant qu'elle choisit écrit
//!   dessous ; **taille des moteurs** (`2207`) en plus du KV ; **pôles, boucle PID, version de Betaflight, batterie et
//!   carte lus dans le dernier log**, que la fiche suit, une note sous le champ quand le pilote l’a changée (D-077, D-086).

use egui::{Align, Layout, RichText, Ui, Vec2};
use otb_advisor::drone_card::{DEFAULT_SIZE, DroneCard};
use otb_advisor::profiles::{profile, size_profile_key};
use otb_advisor::target::{FILTERING, RESPONSE, ROWS, TargetRow, parse, proposed_values, shown};
use otb_log::settings::Settings;
use otb_store::Library;
use otb_store::model::{Config, Drone, DroneSheet, LogMachine, config_changes};

use crate::icons::icon;
use crate::plots::{integer_grid, nice_grid, swatch, tick_fr};
use crate::theme::{AXIS_NAMES, Theme, file_color, plot, with_alpha};
use crate::tracking::{Chart, Charts, Flight, Progress, TrackingView, charts_of};

// ---------------------------------------------------------------- choix proposés

/// Tailles proposées et profil de l'assistant qu'elles choisissent (`otb_advisor::profiles::PROFILES`). Une
/// proposition seulement : la taille se saisit librement, et `size_profile_key` en lit le profil.
pub const SIZES: [(&str, &str); 8] = [
    ("65 mm", "whoop"),
    ("2,5″", "micro"),
    ("3″", "micro"),
    ("4″", "five"),
    ("5″", "five"),
    ("6″", "seven"),
    ("7″", "seven"),
    ("8″ et plus", "large"),
];
pub const CELLS: [&str; 6] = ["1S", "2S", "3S", "4S", "5S", "6S"];
/// Dans l'ordre du prototype ; les valeurs sont celles de `otb_advisor::drone_card::ESCS`.
pub const ESC_CHOICES: [&str; 4] = ["BLHeli_S", "Bluejay", "BLHeli_32", "AM32"];
pub const BF_VERSIONS: [&str; 4] = ["4.4", "4.5", "2025.12", "2026.6"];
/// Fréquences de la boucle PID en kHz, écrites avec un point (affichées avec une virgule).
pub const PID_KHZ: [&str; 6] = ["8", "4", "3.2", "2", "1.6", "1"];

/// La fiche d'un drone neuf : les valeurs que le prototype propose.
pub fn new_sheet() -> DroneSheet {
    DroneSheet {
        size: "5″".into(),
        cells: "6S".into(),
        poles: "14".into(),
        esc: "BLHeli_32".into(),
        betaflight: "2026.6".into(),
        pid_khz: "8".into(),
        ..DroneSheet::default()
    }
}

/// Le profil de l'assistant d'une taille affichée ou saisie ; une taille illisible prend le profil par défaut.
pub fn profile_of_size(size: &str) -> &'static str {
    SIZES
        .iter()
        .find(|s| s.0 == size)
        .map(|s| s.1)
        .or_else(|| size_profile_key(size))
        .unwrap_or(DEFAULT_SIZE)
}

/// La note sous la taille : les seuils que l'Assistant prendra pour elle.
pub fn size_note(size: &str) -> String {
    let label = |k: &str| profile(k).map_or("", |p| p.label);
    match SIZES
        .iter()
        .find(|s| s.0 == size)
        .map(|s| s.1)
        .or_else(|| size_profile_key(size))
    {
        Some(k) => format!("Seuils de l’Assistant : {}.", label(k)),
        None => format!(
            "Taille non reconnue : seuils « {} » par défaut. Écrivez-la en pouces (5″) ou en mm (85 mm).",
            label(DEFAULT_SIZE)
        ),
    }
}

// ---------------------------------------------------------------- ce que le log dit de la machine (D-077)

/// Une fréquence en kHz écrite comme dans la fiche : au dixième, avec un point, sans `.0` (`8`, `3.2`).
pub fn khz_text(hz: f64) -> String {
    let t = format!("{:.1}", hz / 1000.0);
    t.strip_suffix(".0").map_or_else(|| t.clone(), str::to_string)
}

/// La carte de vol d'un log, écrite comme au branchement USB, processeur d'abord : `STM32G474 (BETAFPVG473_V2)`.
/// La cible vient de « Board information » (`BEFH BETAFPVG473_V2`, le code du fabricant retiré) ; vide si le log
/// ne dit ni l'un ni l'autre.
pub fn board_of_log(s: &Settings) -> String {
    let target = s
        .raw
        .get("Board information")
        .and_then(|b| b.split_whitespace().last())
        .unwrap_or("")
        .to_string();
    match (s.board.trim(), target.as_str()) {
        ("", t) => t.to_string(),
        (mcu, "") => mcu.to_string(),
        (mcu, t) => format!("{mcu} ({t})"),
    }
}

/// Ce qu'un log dit de la machine, daté de `date` (D-086) : pôles, boucle PID, version de Betaflight, carte de vol et
/// éléments de la batterie (`vbat` : la tension enregistrée, quand l'en-tête n'a pas `vbatref`). Jamais le KV : il
/// est saisi par le pilote (D-077). Une valeur que le log ne porte pas reste vide :
/// ⚠️ `Settings` remplace un `motor_poles` absent par 14, et un `looptime` absent par 125 µs — on regarde
/// donc l'en-tête brut, sinon la fiche recevrait une valeur inventée.
pub fn machine_of_log(s: &Settings, vbat: Option<&[f32]>, date: &str) -> LogMachine {
    let has = |k: &str| s.raw.contains_key(k);
    LogMachine {
        date: date.to_string(),
        poles: if has("motor_poles") {
            s.motor_poles.to_string()
        } else {
            String::new()
        },
        pid_khz: if has("looptime") && s.implausible().is_none() {
            khz_text(s.pid_rate_hz())
        } else {
            String::new()
        },
        betaflight: version_of_firmware(&s.firmware).unwrap_or("").to_string(),
        board: board_of_log(s),
        cells: otb_log::flightlog::cell_count(s, vbat).map_or_else(String::new, |n| format!("{n}S")),
    }
}

/// Recopie dans la fiche ce que le log en dit (une valeur vide ne change rien) : pôles, boucle PID, version de
/// Betaflight, batterie. La carte de vol n'est pas dans la fiche (`take_log`).
pub fn fill_from_log(s: &mut DroneSheet, m: &LogMachine) {
    for (field, v) in [
        (&mut s.poles, &m.poles),
        (&mut s.pid_khz, &m.pid_khz),
        (&mut s.betaflight, &m.betaflight),
        (&mut s.cells, &m.cells),
    ] {
        if !v.is_empty() {
            field.clone_from(v);
        }
    }
}

/// Range ce qu'un log dit de la machine dans son drone. Un log **plus ancien** que le dernier lu ne change rien.
/// La fiche, si elle existe, **suit** le dernier log (D-086, demandé par Cédric : une fiche à 14 pôles quand le log
/// dit 12 est fausse pour le filtre RPM) ; le pilote peut encore la corriger, une note sous le champ dit alors
/// l'écart (`log_note`), et le log suivant la remet à jour. La carte de vol suit le log tant que le drone n'a pas
/// été branché en USB : la carte lue au branchement (avec son identifiant) est plus sûre.
pub fn take_log(drone: &mut Drone, m: LogMachine) {
    if drone.from_log.as_ref().is_some_and(|old| m.date < old.date) {
        return;
    }
    if let Some(s) = drone.sheet.as_mut() {
        fill_from_log(s, &m);
    }
    if drone.uid.is_empty() && !m.board.is_empty() {
        drone.board = m.board.clone();
    }
    drone.from_log = Some(m);
}

/// Deux valeurs de la fiche égales une fois lues comme nombres (`3,2` et `3.2`, ` 14` et `14`).
fn same_number(a: &str, b: &str) -> bool {
    let n = |t: &str| t.trim().replace(',', ".").parse::<f64>().ok();
    match (n(a), n(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a.trim() == b.trim(),
    }
}

/// La note sous un champ que le log renseigne : `None` si le log n'en dit rien ; sinon `(texte, true)` quand la
/// fiche en diffère (le pilote l'a changée depuis). `what` : `poles`, `pid`, `betaflight` ou `cells`.
pub fn log_note(what: &str, sheet_value: &str, from_log: Option<&LogMachine>) -> Option<(String, bool)> {
    let m = from_log?;
    let v = match what {
        "poles" => &m.poles,
        "betaflight" => &m.betaflight,
        "cells" => &m.cells,
        _ => &m.pid_khz,
    };
    if v.is_empty() {
        return None;
    }
    if same_number(sheet_value, v) {
        return Some(("Lu dans le dernier log.".to_string(), false));
    }
    let text = match what {
        "poles" => format!("Le dernier log dit {v} : vérifiez motor_poles dans Betaflight, le filtre RPM s’en sert."),
        "pid" => format!("Le dernier log dit {} kHz.", khz(v)),
        _ => format!("Le dernier log dit {v}."),
    };
    Some((text, true))
}

/// La taille affichée pour un profil de l'assistant (reprise d'une ancienne fiche, fiche tirée d'un log). Un
/// profil couvre plusieurs tailles : on prend la plus courante.
pub fn size_of_profile(key: &str) -> &'static str {
    match key {
        "whoop" => "65 mm",
        "micro" => "3″",
        "seven" => "7″",
        "large" => "8″ et plus",
        _ => "5″",
    }
}

/// La version de Betaflight proposée qui correspond au firmware d'un log (« Betaflight 2026.6.0-rc3 … »).
pub fn version_of_firmware(firmware: &str) -> Option<&'static str> {
    let v = firmware.split_whitespace().nth(1)?;
    BF_VERSIONS
        .iter()
        .copied()
        .find(|b| v.starts_with(&format!("{b}.")) || v == *b)
}

/// La famille de version que l'assistant connaît : 4.4, 4.5, ou 2025.12 et plus récent.
fn family(version: &str) -> &'static str {
    match version {
        "4.4" => "4.4",
        "4.5" => "4.5",
        _ => "2025.12",
    }
}

// ---------------------------------------------------------------- traduction pour l'assistant

/// La fiche du Guide (`DroneCard`) d'un drone de la bibliothèque, `None` s'il n'a pas de fiche.
pub fn card_of(drone: &Drone) -> Option<DroneCard> {
    let s = drone.sheet.as_ref()?;
    let mut c = DroneCard::new(&drone.name);
    c.bf = family(&s.betaflight).to_string();
    c.size = profile_of_size(&s.size).to_string();
    c.pid_khz = s.pid_khz.parse().unwrap_or(c.pid_khz);
    if ESC_CHOICES.contains(&s.esc.as_str()) {
        c.esc = s.esc.clone();
    }
    c.done = s.done.iter().map(|k| (k.clone(), true)).collect();
    Some(c)
}

/// Les fiches du Guide de toute la bibliothèque, dans l'ordre des drones.
pub fn cards_of(lib: &Library) -> Vec<DroneCard> {
    lib.drones().iter().filter_map(card_of).collect()
}

/// Une fiche de bibliothèque d'après une fiche du Guide : reprise de l'ancien fichier, fiche tirée d'un log.
/// Ce que l'ancienne fiche ne disait pas (batterie, poids…) reste vide.
pub fn sheet_of_card(card: &DroneCard) -> DroneSheet {
    let pid = PID_KHZ
        .iter()
        .find(|k| k.parse::<f64>().is_ok_and(|v| v == card.pid_khz))
        .map_or_else(|| card.pid_khz.to_string(), |k| k.to_string());
    DroneSheet {
        size: size_of_profile(&card.size).to_string(),
        esc: card.esc.clone(),
        betaflight: card.bf.clone(),
        pid_khz: pid,
        done: card.done.iter().filter(|d| d.1).map(|d| d.0.clone()).collect(),
        ..DroneSheet::default()
    }
}

/// Reprend les fiches de l'ancien fichier du Guide (`drones.json`, D-039) dans la bibliothèque. Un drone qui a
/// déjà une fiche garde la sienne (la bibliothèque gagne) ; un drone connu par ses seuls logs reçoit celle-ci ;
/// sinon le drone est créé, son nom pour identifiant. Rend le nombre de fiches reprises.
pub fn import_cards(lib: &mut Library, cards: &[DroneCard]) -> usize {
    let mut n = 0;
    for card in cards {
        let mut d = lib
            .drone_by_name(&card.name)
            .cloned()
            .unwrap_or_else(|| Drone::new(&free_id(lib, &card.name), &card.name));
        if d.sheet.is_some() {
            continue;
        }
        d.sheet = Some(sheet_of_card(card));
        lib.put_drone(d);
        n += 1;
    }
    n
}

/// Un identifiant libre pour un drone créé à la main : son nom, suivi d'un numéro s'il est pris.
fn free_id(lib: &Library, name: &str) -> String {
    let mut id = name.to_string();
    let mut n = 2;
    while lib.drone(&id).is_some() {
        id = format!("{name} ({n})");
        n += 1;
    }
    id
}

// ---------------------------------------------------------------- formulaire et enregistrement

/// Ce que l'écran est en train de modifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetForm {
    /// Identifiant du drone modifié ; `None` : nouveau drone.
    pub id: Option<String>,
    pub name: String,
    pub sheet: DroneSheet,
    /// « Supprimer… » cliqué : on attend « Garder » ou « Supprimer définitivement ».
    pub ask_delete: bool,
    /// « Supprimer aussi les fichiers copiés » cochée (W-075) : décochée à chaque ouverture de la fiche.
    pub delete_copies: bool,
    /// Configuration dépliée : (identifiant, `true` pour « Comparer », `false` pour « Voir »).
    pub shown: Option<(String, bool)>,
    /// Pourquoi le dernier « Enregistrer » a été refusé.
    pub error: String,
}

impl SheetForm {
    /// La fiche d'un drone de la bibliothèque ; un drone sans fiche part des valeurs d'un drone neuf, avec ce que
    /// ses logs disent de la machine (version de Betaflight, pôles, boucle PID, batterie : D-086).
    pub fn edit(drone: &Drone) -> SheetForm {
        let sheet = drone.sheet.clone().unwrap_or_else(|| {
            let mut s = new_sheet();
            if let Some(v) = version_of_firmware(&drone.firmware) {
                s.betaflight = v.to_string();
            }
            if let Some(m) = &drone.from_log {
                fill_from_log(&mut s, m);
            }
            s
        });
        SheetForm {
            id: Some(drone.id.clone()),
            name: drone.name.clone(),
            sheet,
            ask_delete: false,
            delete_copies: false,
            shown: None,
            error: String::new(),
        }
    }

    /// Un nouveau drone, éventuellement prérempli (nom et fiche tirés du log ouvert, depuis le Guide).
    pub fn new(name: &str, sheet: DroneSheet) -> SheetForm {
        SheetForm {
            id: None,
            name: name.to_string(),
            sheet,
            ask_delete: false,
            delete_copies: false,
            shown: None,
            error: String::new(),
        }
    }

    pub fn is_new(&self) -> bool {
        self.id.is_none()
    }

    /// Le nom enregistré : celui saisi, ou « Nouveau drone » s'il est vide (comme le prototype).
    fn saved_name(&self) -> String {
        let n = self.name.trim();
        if n.is_empty() {
            "Nouveau drone".to_string()
        } else {
            n.to_string()
        }
    }
}

// ---------------------------------------------------------------- fiche cible (lot 31b, D-072)

/// Les textes des dix cases de la fiche cible, dans l'ordre de `ROWS` : ceux que le pilote a saisis, ou, s'il n'a
/// rien changé (`target` vide), les **valeurs proposées** pour la taille de la fiche — vides là où l'assistant n'a
/// pas de seuil. Une clé absente d'une cible enregistrée se lit vide.
pub fn target_texts(sheet: &DroneSheet) -> [String; 10] {
    match &sheet.target {
        Some(t) => ROWS.map(|r| {
            t.iter()
                .find(|(k, _)| k == r.key)
                .map_or_else(String::new, |(_, v)| v.clone())
        }),
        None => {
            let p = proposed_values(profile_of_size(&sheet.size));
            let mut out: [String; 10] = Default::default();
            for (i, r) in ROWS.iter().enumerate() {
                out[i] = p[i].map_or_else(String::new, |x| shown(r, x));
            }
            out
        }
    }
}

/// La note sous les cases : d'où viennent les valeurs.
pub fn target_note(sheet: &DroneSheet) -> String {
    let taille = if sheet.size.is_empty() {
        "par défaut"
    } else {
        sheet.size.as_str()
    };
    if sheet.target.is_some() {
        return format!("Valeurs choisies pour ce drone ; « Valeurs proposées » reprend celles de la taille {taille}.");
    }
    let provisoire = profile(profile_of_size(&sheet.size)).is_some_and(|p| p.provisional);
    format!(
        "Valeurs proposées pour la taille {taille}{}, modifiables.",
        if provisoire { " (seuils encore provisoires)" } else { "" }
    )
}

/// Les cibles des dix cases en nombres, dans l'ordre de `ROWS` : `None` pour une case vide **ou illisible** (le
/// suivi n'a pas de bande là ; l'enregistrement, lui, refuse une case illisible : `check_target`).
pub fn target_values(sheet: &DroneSheet) -> [Option<f64>; 10] {
    target_texts(sheet).map(|t| parse(&t).ok().flatten())
}

/// Refuse une case de la fiche cible qui n'est pas un nombre (une case vide est permise : pas de cible).
pub fn check_target(sheet: &DroneSheet) -> Result<(), String> {
    for (r, t) in ROWS.iter().zip(target_texts(sheet)) {
        if let Err(t) = parse(&t) {
            return Err(format!("Fiche cible : « {} » n'est pas un nombre ({t}).", r.label));
        }
    }
    Ok(())
}

/// Enregistre la fiche dans la bibliothèque ; rend l'identifiant du drone, ou pourquoi c'est refusé.
///
/// ⚠️ Le **nom** relie les logs à la fiche (`craft_name`) : deux drones ne peuvent pas le partager, sinon les logs
/// iraient au premier des deux. Un drone connu par ses seuls logs (sans fiche) **reçoit** la fiche d'un nouveau
/// drone du même nom, plutôt que d'en faire un second.
pub fn save_form(lib: &mut Library, form: &SheetForm) -> Result<String, String> {
    check_target(&form.sheet)?;
    let name = form.saved_name();
    let same_name = lib.drone_by_name(&name).cloned();
    match &form.id {
        Some(id) => {
            if same_name.as_ref().is_some_and(|d| &d.id != id) {
                return Err(format!("Un autre drone porte déjà le nom « {name} »."));
            }
            let mut d = lib
                .drone(id)
                .cloned()
                .ok_or("Ce drone n'est plus dans la bibliothèque.")?;
            d.name = name;
            d.sheet = Some(form.sheet.clone());
            lib.put_drone(d);
            Ok(id.clone())
        }
        None => {
            let mut d = match same_name {
                Some(d) if d.sheet.is_some() => {
                    return Err(format!(
                        "Le drone « {name} » a déjà une fiche : ouvrez-la par le crayon de sa carte."
                    ));
                }
                Some(d) => d,
                None => Drone::new(&free_id(lib, &name), &name),
            };
            d.sheet = Some(form.sheet.clone());
            let id = d.id.clone();
            lib.put_drone(d);
            Ok(id)
        }
    }
}

/// « 2026-11-13T18:15:02 » → « 13 nov. 2026, 18:15 » ; une date seule n'a pas d'heure ; un texte illisible
/// reste tel quel.
pub fn date_fr(iso: &str) -> String {
    const MOIS: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc.",
    ];
    let num = |a: usize, b: usize| iso.get(a..b).and_then(|s| s.parse::<usize>().ok());
    let (Some(an), Some(mois), Some(jour)) = (num(0, 4), num(5, 7), num(8, 10)) else {
        return iso.to_string();
    };
    let Some(m) = mois.checked_sub(1).and_then(|i| MOIS.get(i)) else {
        return iso.to_string();
    };
    match iso.get(11..16) {
        Some(h) if h.as_bytes().get(2) == Some(&b':') => format!("{jour} {m} {an}, {h}"),
        _ => format!("{jour} {m} {an}"),
    }
}

/// La ligne de résumé d'une configuration : ses deux premiers changements, ou « configuration initiale », puis
/// le premier log du drone rangé ce jour-là ou après (« avant FINAL »).
pub fn config_summary(lib: &Library, configs: &[&Config], i: usize) -> String {
    let c = configs[i];
    let mut s = match i.checked_sub(1).map(|p| configs[p]) {
        None => "configuration initiale".to_string(),
        Some(prev) => {
            let ch = config_changes(&prev.text, &c.text);
            let mut parts: Vec<String> = ch.iter().take(2).map(change_text).collect();
            if ch.len() > 2 {
                parts.push(format!("+ {}", ch.len() - 2));
            }
            if parts.is_empty() {
                "aucun réglage changé".to_string()
            } else {
                parts.join(", ")
            }
        }
    };
    let jour = c.recorded.get(..10).unwrap_or(&c.recorded);
    if let Some(l) = lib
        .logs()
        .iter()
        .filter(|l| l.drone_id == c.drone_id && l.added.as_str() >= jour)
        .min_by(|a, b| a.added.cmp(&b.added))
    {
        let nom = std::path::Path::new(&l.name)
            .file_stem()
            .map_or(l.name.clone(), |n| n.to_string_lossy().into_owned());
        s.push_str(&format!(" · avant {nom}"));
    }
    s
}

fn change_text(c: &(String, String, String)) -> String {
    let v = |x: &str| {
        if x.is_empty() {
            "défaut".to_string()
        } else {
            x.to_string()
        }
    };
    format!("{} {} → {}", c.0, v(&c.1), v(&c.2))
}

// ---------------------------------------------------------------- écran

/// Ce que l'utilisateur demande depuis la fiche ; `App` l'exécute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheetAction {
    /// Retour ou « Annuler » : rien n'est enregistré.
    Leave,
    /// « Enregistrer » / « Créer le drone ».
    Save,
    /// « Supprimer définitivement » : ce drone, et ses fichiers copiés si la case est cochée (W-075).
    Delete(String, bool),
}

// mesures relevées dans le prototype (écran `droneEdit`)
const PAGE_PAD_X: i8 = 26;
const PAGE_PAD_Y: i8 = 18;
const GAP: f32 = 14.0;
const BACK: f32 = 34.0;
const BADGE: f32 = 40.0;
const TITLE: f32 = 22.0;
const KICKER: f32 = 12.0;
/// Colonne de gauche : `minmax(0, 1.15fr) minmax(0, 1fr)`.
const LEFT_SHARE: f32 = 1.15 / 2.15;
const SECTION_RADIUS: u8 = 10;
const SECTION_PAD_X: i8 = 18;
const SECTION_PAD_Y: i8 = 16;
const H2: f32 = 15.0;
const LABEL: f32 = 12.5;
const HINT: f32 = 11.5;
const FIELD: f32 = 14.0;
const FIELD_RADIUS: u8 = 6;
/// Flèche des propositions d'un champ libre (la taille).
const CHEVRON: f32 = 16.0;
const MONO: f32 = 13.5;
/// Opacité du teint du badge (`couleur + '1f'`) et du trait du cadre « Supprimer » (prototype : `#f1c7c4`, le
/// rouge du thème à 25 % sur le blanc).
const TINT: u8 = 0x1f;
const DANGER_LINE: u8 = 64;

#[derive(Default)]
pub struct DroneSheetPage {
    /// `None` : aucune fiche ouverte (l'écran ne s'affiche pas).
    pub form: Option<SheetForm>,
}

impl DroneSheetPage {
    /// `suivi` : où en sont les mesures du drone (`None` pour un drone pas encore enregistré).
    pub fn ui(&mut self, ui: &mut Ui, lib: &Library, th: &Theme, suivi: Option<&TrackingView>) -> Vec<SheetAction> {
        let mut actions = Vec::new();
        let Some(form) = self.form.as_mut() else {
            actions.push(SheetAction::Leave);
            return actions;
        };
        let rang = form
            .id
            .as_ref()
            .and_then(|id| lib.drones().iter().position(|d| &d.id == id))
            .unwrap_or(lib.drones().len());
        let couleur = file_color(rang);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(th.ui.window).inner_margin(egui::Margin {
                left: PAGE_PAD_X,
                right: PAGE_PAD_X,
                top: PAGE_PAD_Y,
                bottom: PAGE_PAD_Y,
            }))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = GAP;
                header(ui, form, couleur, th, &mut actions);
                if !form.error.is_empty() {
                    ui.label(RichText::new(&form.error).color(th.accent("red")));
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let gauche = ((ui.available_width() - GAP) * LEFT_SHARE).floor();
                    let droite = (ui.available_width() - GAP - gauche).floor();
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = GAP;
                        ui.allocate_ui_with_layout(Vec2::new(gauche, 0.0), Layout::top_down(Align::LEFT), |ui| {
                            ui.set_width(gauche);
                            ui.spacing_mut().item_spacing.y = GAP;
                            identity(ui, form, lib, th);
                            machine(ui, form, lib, th);
                            configurations(ui, form, lib, th);
                            if let Some(id) = form.id.clone() {
                                delete(ui, form, lib, &id, th, &mut actions);
                            }
                        });
                        ui.allocate_ui_with_layout(Vec2::new(droite, 0.0), Layout::top_down(Align::LEFT), |ui| {
                            ui.set_width(droite);
                            ui.spacing_mut().item_spacing.y = GAP;
                            target(ui, form, th);
                            if let Some(v) = suivi {
                                // ⚠️ la bande suit les cases **du formulaire**, même pas encore enregistrées : le
                                // pilote voit tout de suite où tombe la cible qu'il tape
                                let charts = form
                                    .id
                                    .as_deref()
                                    .filter(|_| lib.measures_loaded())
                                    .map(|id| charts_of(lib, id, &target_values(&form.sheet)));
                                tracking(ui, th, v, charts.as_ref());
                            }
                        });
                    });
                });
            });
        actions
    }
}

/// Retour, badge, titre, « Annuler » et « Enregistrer ».
fn header(ui: &mut Ui, form: &SheetForm, couleur: egui::Color32, th: &Theme, actions: &mut Vec<SheetAction>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let retour = egui::Button::image(
            icon("arrow-left", 18.0)
                .tint(th.ui.text)
                .alt_text("Retour au tableau de bord"),
        )
        .min_size(Vec2::splat(BACK))
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, th.ui.mid))
        .corner_radius(8);
        if ui.add(retour).on_hover_text("Revenir sans enregistrer").clicked() {
            actions.push(SheetAction::Leave);
        }
        let (badge, _) = ui.allocate_exact_size(Vec2::splat(BADGE), egui::Sense::hover());
        ui.painter().rect_filled(badge, 10, with_alpha(couleur, TINT));
        icon("drone", 22.0)
            .tint(couleur)
            .paint_at(ui, egui::Rect::from_center_size(badge.center(), Vec2::splat(22.0)));
        let (kicker, titre) = if form.is_new() {
            let n = form.name.trim();
            (
                "Tableau de bord",
                if n.is_empty() {
                    "Nouveau drone".to_string()
                } else {
                    format!("Nouveau drone : {n}")
                },
            )
        } else {
            let n = form.name.trim();
            (
                "Tableau de bord · fiche du drone",
                if n.is_empty() {
                    "Sans nom".to_string()
                } else {
                    n.to_string()
                },
            )
        };
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(kicker).size(KICKER).color(th.ui.muted));
            ui.label(RichText::new(titre).size(TITLE).color(th.ui.text));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().button_padding = Vec2::new(14.0, 8.0);
            let on_accent = plot::ON_ACCENT;
            let libelle = if form.is_new() {
                "Créer le drone"
            } else {
                "Enregistrer"
            };
            let enregistrer = egui::Button::image_and_text(
                icon("check", 15.0).tint(on_accent),
                RichText::new(libelle).size(13.0).strong().color(on_accent),
            )
            .fill(th.ui.highlight)
            .corner_radius(6);
            if ui.add(enregistrer).clicked() {
                actions.push(SheetAction::Save);
            }
            let annuler = egui::Button::new(RichText::new("Annuler").size(13.0).color(th.ui.text))
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, th.ui.mid))
                .corner_radius(6);
            if ui.add(annuler).clicked() {
                actions.push(SheetAction::Leave);
            }
        });
    });
}

/// Un cadre de section : fond blanc, trait fin, coins de 10, titre de 15 et sous-titre facultatif.
fn section(ui: &mut Ui, th: &Theme, title: &str, subtitle: Option<&str>, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, th.ui.border))
        .corner_radius(SECTION_RADIUS)
        .inner_margin(egui::Margin {
            left: SECTION_PAD_X,
            right: SECTION_PAD_X,
            top: SECTION_PAD_Y,
            bottom: SECTION_PAD_Y,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = GAP;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(H2).strong().color(th.ui.text));
                if let Some(s) = subtitle {
                    ui.label(RichText::new(s).size(LABEL).color(th.ui.muted));
                }
            });
            body(ui);
        });
}

/// Libellé d'un champ (12,5, gras) ; rend sa réponse pour relier le champ à son libellé (lecteur d'écran, tests).
fn field_label(ui: &mut Ui, th: &Theme, text: &str) -> egui::Response {
    ui.label(RichText::new(text).size(LABEL).strong().color(th.ui.text))
}

fn hint(ui: &mut Ui, th: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(HINT).color(th.ui.muted)).wrap());
}

/// La note sous un champ : atténuée, ou couleur « attention » quand elle avertit (la fiche diffère du log).
type Note<'a> = Option<(&'a str, bool)>;

fn note(ui: &mut Ui, th: &Theme, n: Note) {
    match n {
        Some((t, false)) => hint(ui, th, t),
        Some((t, true)) => {
            ui.add(egui::Label::new(RichText::new(t).size(HINT).color(th.severity("attention"))).wrap());
        }
        None => {}
    }
}

/// Un champ de saisie encadré, avec son unité à droite (`g`, `KV`) s'il en a une, et sa note dessous.
fn text_field(ui: &mut Ui, th: &Theme, label: &str, value: &mut String, unit: Option<&str>, n: Note) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        let l = field_label(ui, th, label);
        egui::Frame::new()
            .fill(th.ui.base)
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(FIELD_RADIUS)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // ⚠️ l'unité d'abord, à droite, puis le texte dans la place qui reste : une place réservée à l'avance
                // (24 points) était trop courte pour « KV », le champ dépassait sa colonne et les lignes suivantes
                // de la section s'élargissaient
                let haut = ui.ctx().fonts_mut(|f| f.row_height(&egui::FontId::proportional(FIELD)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // la ligne a d'emblée la hauteur du texte : l'unité, posée la première, s'y centre
                    ui.set_min_height(haut);
                    if let Some(u) = unit {
                        ui.label(RichText::new(u).size(LABEL).color(th.ui.muted));
                    }
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(FIELD))
                            .text_color(th.ui.text)
                            .desired_width(ui.available_width()),
                    )
                    .labelled_by(l.id);
                });
            });
        note(ui, th, n);
    });
}

/// Un champ de saisie avec une flèche qui déroule des propositions : la taille se tape librement (« 3,5″ »,
/// « 10″ », « 85 mm ») ou se choisit dans la liste.
fn editable_select(ui: &mut Ui, th: &Theme, label: &str, choices: &[&str], value: &mut String, n: Note) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        let l = field_label(ui, th, label);
        let cadre = egui::Frame::new()
            .fill(th.ui.base)
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(FIELD_RADIUS)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    // ⚠️ la flèche sans marge, et sa place retirée du texte avec l'écart entre les deux : sinon le
                    // champ dépasse sa colonne, et toute la section s'élargit d'une ligne à l'autre
                    ui.spacing_mut().button_padding = Vec2::ZERO;
                    let reste = CHEVRON + ui.spacing().item_spacing.x;
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(FIELD))
                            .text_color(th.ui.text)
                            .desired_width(ui.available_width() - reste),
                    )
                    .labelled_by(l.id);
                    ui.add(
                        egui::Button::image(icon("chevron-down", CHEVRON).tint(th.ui.muted))
                            .frame(false)
                            .min_size(Vec2::splat(CHEVRON)),
                    )
                    .on_hover_text(format!("{label} : propositions"))
                })
                .inner
            });
        let fleche = cadre.inner;
        egui::Popup::menu(&fleche)
            .width(cadre.response.rect.width())
            .show(|ui| {
                for c in choices {
                    if ui
                        .selectable_label(value.as_str() == *c, RichText::new(*c).size(FIELD))
                        .clicked()
                    {
                        *value = (*c).to_string();
                    }
                }
            });
        note(ui, th, n);
    });
}

/// Une liste déroulante qui prend la largeur de sa colonne ; `shown` traduit une valeur pour l'affichage.
#[allow(clippy::too_many_arguments)]
fn select(
    ui: &mut Ui,
    th: &Theme,
    label: &str,
    id: &str,
    choices: &[&str],
    value: &mut String,
    shown: fn(&str) -> String,
    n: Note,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        let l = field_label(ui, th, label);
        // la hauteur d'un champ de saisie (14 points de texte, 8 de marge en haut et en bas, le trait) : le
        // prototype donne aux listes la même que celle des champs
        ui.spacing_mut().interact_size.y = FIELD + 2.0 * 8.0 + 6.0;
        ui.spacing_mut().button_padding = Vec2::new(10.0, 8.0);
        ui.visuals_mut().widgets.inactive.corner_radius = FIELD_RADIUS.into();
        ui.visuals_mut().widgets.hovered.corner_radius = FIELD_RADIUS.into();
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0, th.ui.mid);
        ui.visuals_mut().widgets.inactive.weak_bg_fill = th.ui.base;
        egui::ComboBox::from_id_salt(("fiche-drone", id))
            .width(ui.available_width())
            .selected_text(RichText::new(shown(value)).size(FIELD).color(th.ui.text))
            .show_ui(ui, |ui| {
                for c in choices {
                    ui.selectable_value(value, (*c).to_string(), shown(c));
                }
            })
            .response
            .labelled_by(l.id);
        note(ui, th, n);
    });
}

fn as_is(v: &str) -> String {
    v.to_string()
}

fn khz(v: &str) -> String {
    v.replace('.', ",")
}

/// Une valeur lue au branchement USB : encadrée, sur fond gris, en chiffres de machine.
fn read_only(ui: &mut Ui, th: &Theme, label: &str, value: &str, note: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        field_label(ui, th, label);
        egui::Frame::new()
            .fill(th.ui.alt)
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(FIELD_RADIUS)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(value).size(MONO).monospace().color(th.ui.muted)).truncate());
            });
        hint(ui, th, note);
    });
}

fn identity(ui: &mut Ui, form: &mut SheetForm, lib: &Library, th: &Theme) {
    let drone = form.id.as_ref().and_then(|id| lib.drone(id));
    // l'identifiant gravé ne se lit qu'au branchement USB ; la carte, aussi dans le dernier log (D-086)
    let attente = || "en attente du branchement".to_string();
    let uid = drone
        .map(|d| d.uid.clone())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(attente);
    let usb = drone.is_some_and(|d| !d.uid.is_empty());
    let carte = drone
        .map(|d| d.board.clone())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(attente);
    let carte_note = if !usb && drone.is_some_and(|d| !d.board.is_empty()) {
        "Lue dans le dernier log ; le branchement USB la confirmera."
    } else {
        "Lue au branchement USB."
    };
    section(ui, th, "Identité", None, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            text_field(
                ui,
                th,
                "Nom du drone",
                &mut form.name,
                None,
                Some((
                    "Doit être le nom du drone dans Betaflight (craft_name) : c’est lui qui relie les logs à cette fiche.",
                    false,
                )),
            );
        });
        ui.columns(2, |c| {
            read_only(
                &mut c[0],
                th,
                "Identifiant de la carte (UID)",
                &uid,
                "Lu automatiquement au branchement USB.",
            );
            read_only(&mut c[1], th, "Carte de vol", &carte, carte_note);
        });
    });
}

fn machine(ui: &mut Ui, form: &mut SheetForm, lib: &Library, th: &Theme) {
    let from_log = form
        .id
        .as_ref()
        .and_then(|id| lib.drone(id))
        .and_then(|d| d.from_log.clone());
    let s = &mut form.sheet;
    let taille = size_note(&s.size);
    let poles = log_note("poles", &s.poles, from_log.as_ref());
    let cells = log_note("cells", &s.cells, from_log.as_ref());
    let bf = log_note("betaflight", &s.betaflight, from_log.as_ref());
    let pid = log_note("pid", &s.pid_khz, from_log.as_ref());
    let pid = pid.map(|(t, w)| (format!("en kHz · {}", t.to_lowercase().trim_end_matches('.')), w));
    section(ui, th, "Machine", None, |ui| {
        fn as_note(n: &Option<(String, bool)>) -> Note<'_> {
            n.as_ref().map(|(t, w)| (t.as_str(), *w))
        }
        ui.columns(3, |c| {
            editable_select(
                &mut c[0],
                th,
                "Taille",
                &SIZES.map(|x| x.0),
                &mut s.size,
                Some((&taille, false)),
            );
            select(
                &mut c[1],
                th,
                "Batterie",
                "batterie",
                &CELLS,
                &mut s.cells,
                as_is,
                as_note(&cells),
            );
            text_field(&mut c[2], th, "Poids", &mut s.weight, Some("g"), None);
        });
        ui.columns(3, |c| {
            text_field(
                &mut c[0],
                th,
                "Taille des moteurs",
                &mut s.motor_size,
                None,
                Some(("Stator, par exemple 2207.", false)),
            );
            text_field(&mut c[1], th, "Moteurs", &mut s.kv, Some("KV"), None);
            text_field(
                &mut c[2],
                th,
                "Pôles moteur",
                &mut s.poles,
                None,
                as_note(&poles).or(Some(("Pour le filtre RPM.", false))),
            );
        });
        ui.columns(3, |c| {
            text_field(&mut c[0], th, "Hélices", &mut s.props, None, None);
            select(&mut c[1], th, "ESC", "esc", &ESC_CHOICES, &mut s.esc, as_is, None);
            select(
                &mut c[2],
                th,
                "Betaflight",
                "betaflight",
                &BF_VERSIONS,
                &mut s.betaflight,
                as_is,
                as_note(&bf),
            );
        });
        ui.columns(3, |c| {
            select(
                &mut c[0],
                th,
                "Boucle PID",
                "boucle-pid",
                &PID_KHZ,
                &mut s.pid_khz,
                khz,
                as_note(&pid).or(Some(("en kHz", false))),
            );
        });
    });
}

fn small_button(ui: &mut Ui, th: &Theme, text: &str, on: bool) -> egui::Response {
    ui.spacing_mut().button_padding = Vec2::new(9.0, 4.0);
    ui.add(
        egui::Button::new(RichText::new(text).size(12.0).color(th.ui.text))
            .fill(if on { th.ui.select } else { th.ui.base })
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(6)
            .selected(on),
    )
}

/// Les lignes de la section « Suivi des mesures » : où en est le calcul, combien de fragments sont mesurés, et ceux
/// qui n'ont pas pu l'être (avec la raison, pour que le pilote sache quoi faire). Fonction pure : elle se teste sans
/// écran (L-071).
pub fn tracking_lines(v: &TrackingView) -> Vec<String> {
    let pluriel = |n: usize| if n > 1 { "s" } else { "" };
    let mut out = Vec::new();
    match v.progress {
        Progress::Reading => out.push("Lecture des mesures déjà faites…".to_string()),
        Progress::Measuring { done, total } => out.push(format!(
            "Mesure en arrière-plan : {done} fragment{} sur {total}…",
            pluriel(done)
        )),
        Progress::Idle if v.total == 0 => {
            out.push("Aucun fragment : les mesures viendront avec les logs de ce drone.".to_string())
        }
        Progress::Idle => out.push(format!(
            "{} fragment{} mesuré{} sur {}.",
            v.measured,
            pluriel(v.measured),
            pluriel(v.measured),
            v.total
        )),
    }
    for (name, why) in &v.failed {
        out.push(format!("Non mesuré : « {name} » — {why}"));
    }
    out
}

// mesures de la fiche cible relevées dans le prototype : libellé souple, signe sur 26, case de 96
const TARGET_SIGN_W: f32 = 26.0;
const TARGET_FIELD_W: f32 = 96.0;
const TARGET_ROW: f32 = 13.0;
const TARGET_UNIT: f32 = 12.0;
const TARGET_GROUP: f32 = 11.0;
const TARGET_NOTE: f32 = 12.5;
pub const TARGET_SUBTITLE: &str =
    "Ce que ce drone doit atteindre. Le suivi de ses vols s'y compare ; l'assistant garde ses propres seuils.";

/// « Fiche cible » : dix cases en deux groupes, la note et « Valeurs proposées » (lot 31b, D-072). Tant que le pilote
/// n'a rien changé, les cases montrent les valeurs proposées pour la taille choisie **dans le formulaire** (elles
/// suivent donc la taille) ; la première case modifiée fige les dix.
fn target(ui: &mut Ui, form: &mut SheetForm, th: &Theme) {
    section(ui, th, "Fiche cible", Some(TARGET_SUBTITLE), |ui| {
        let mut texts = target_texts(&form.sheet);
        let mut changed = false;
        for group in [RESPONSE, FILTERING] {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    RichText::new(group.to_uppercase())
                        .size(TARGET_GROUP)
                        .strong()
                        .color(th.ui.muted),
                );
                for (row, text) in ROWS.iter().zip(texts.iter_mut()).filter(|(r, _)| r.group == group) {
                    changed |= target_row(ui, th, row, text);
                }
            });
        }
        if changed {
            form.sheet.target = Some(ROWS.iter().zip(texts).map(|(r, t)| (r.key.to_string(), t)).collect());
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let note = target_note(&form.sheet);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().button_padding = Vec2::new(10.0, 5.0);
                let b = egui::Button::new(RichText::new("Valeurs proposées").size(TARGET_NOTE).color(th.ui.text))
                    .fill(th.ui.base)
                    .stroke(egui::Stroke::new(1.0, th.ui.mid))
                    .corner_radius(FIELD_RADIUS);
                if ui.add(b).clicked() {
                    form.sheet.target = None;
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(note).size(TARGET_NOTE).color(th.ui.muted)).wrap());
                });
            });
        });
    });
}

/// Une case : libellé, signe (≤ ou ≥), saisie en chiffres de machine alignée à droite avec son unité. Rend `true`
/// si le texte a changé.
fn target_row(ui: &mut Ui, th: &Theme, row: &TargetRow, text: &mut String) -> bool {
    let mut changed = false;
    // une ligne : 4 points au-dessus et au-dessous, comme le `padding: 4px 0` du prototype
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(0, 4))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 0.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let case = ui
                        .allocate_ui_with_layout(
                            Vec2::new(TARGET_FIELD_W, 0.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                egui::Frame::new()
                                    .fill(th.ui.base)
                                    .stroke(egui::Stroke::new(1.0, th.ui.mid))
                                    .corner_radius(FIELD_RADIUS)
                                    .inner_margin(egui::Margin::symmetric(8, 6))
                                    .show(ui, |ui| {
                                        ui.set_width(TARGET_FIELD_W - 2.0 * 8.0 - 2.0);
                                        // l'unité d'abord, depuis la droite : la saisie prend exactement le reste, et les dix
                                        // cases gardent la même largeur quelle que soit leur unité
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            // la hauteur de la saisie d'abord : sans elle, l'unité, posée avant,
                                            // se centre sur sa propre hauteur et reste en haut de la case
                                            let mono = egui::FontId::monospace(MONO);
                                            ui.set_min_height(ui.fonts_mut(|f| f.row_height(&mono)));
                                            if !row.unit.is_empty() {
                                                ui.label(RichText::new(row.unit).size(TARGET_UNIT).color(th.ui.muted));
                                            }
                                            ui.add(
                                                egui::TextEdit::singleline(text)
                                                    .id_salt(("fiche-cible", row.key))
                                                    .frame(egui::Frame::NONE)
                                                    .font(egui::FontId::monospace(MONO))
                                                    .text_color(th.ui.text)
                                                    .align(egui::Align2::RIGHT_CENTER)
                                                    .desired_width(ui.available_width()),
                                            )
                                        })
                                        .inner
                                    })
                                    .inner
                            },
                        )
                        .inner;
                    changed = case.changed();
                    ui.allocate_ui_with_layout(
                        Vec2::new(TARGET_SIGN_W, case.rect.height()),
                        Layout::right_to_left(Align::Center),
                        |ui| ui.label(RichText::new(row.bound.sign()).size(TARGET_ROW).color(th.ui.muted)),
                    );
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let l = ui.add(
                            egui::Label::new(RichText::new(row.label).size(TARGET_ROW).color(th.ui.text)).truncate(),
                        );
                        case.labelled_by(l.id);
                    });
                },
            );
        });
    changed
}

/// « Suivi des mesures » : l'état du calcul des grandeurs de chaque fragment (lot 31a), puis leurs graphiques au fil
/// des vols (lot 31c) dès qu'un vol est mesuré.
fn tracking(ui: &mut Ui, th: &Theme, v: &TrackingView, charts: Option<&Charts>) {
    section(
        ui,
        th,
        "Suivi des mesures",
        Some(
            "Ce que l'assistant mesure sur chaque fragment de ce drone, gardé pour suivre son évolution d'un vol à l'autre.",
        ),
        |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for line in tracking_lines(v) {
                    hint(ui, th, &line);
                }
            });
            let Some(c) = charts.filter(|c| !c.flights.is_empty()) else {
                return;
            };
            chart_legend(ui, th);
            for chart in &c.charts {
                tracking_chart(ui, th, chart, &c.flights);
            }
        },
    );
}

// mesures des graphiques du suivi (absents du prototype : tailles reprises de la fiche cible et de l'Assistant)
const CHART_H: f32 = 110.0;
/// Opacité de la zone visée (vert du thème).
const BAND_ALPHA: u8 = 34;
const POINT_R: f32 = 3.0;
/// Largeur des graduations verticales : « -31,5 » y tient.
const CHART_Y_AXIS_W: f32 = 34.0;

/// Un nombre du suivi : virgule décimale, `decimals` chiffres (ceux de la case de la fiche cible).
fn chart_number(c: &Chart, x: f64) -> String {
    otb_format::fixed(x, c.decimals, false).replace('.', ",")
}

/// La cible d'un graphique, en clair : « cible ≤ 14 ms », « cible ≥ 35 ° », « cible 1,05 à 1,20 », « sans cible ».
pub fn band_text(c: &Chart) -> String {
    let unit = if c.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", c.unit)
    };
    match c.band {
        (Some(lo), Some(hi)) => format!("cible {} à {}{unit}", chart_number(c, lo), chart_number(c, hi)),
        (Some(lo), None) => format!("cible ≥ {}{unit}", chart_number(c, lo)),
        (None, Some(hi)) => format!("cible ≤ {}{unit}", chart_number(c, hi)),
        (None, None) => "sans cible".to_string(),
    }
}

/// La bulle d'un point survolé : le vol (numéro, nom, date), puis l'axe et la valeur. `x` est l'abscisse survolée ;
/// `None` hors des vols.
pub fn hover_text(c: &Chart, flights: &[Flight], series: &str, x: f64, y: f64) -> Option<String> {
    let n = x.round();
    let flight = flights.get((n as usize).checked_sub(1)?).filter(|_| n >= 1.0)?;
    let date = if flight.date.is_empty() {
        String::new()
    } else {
        format!(" — {}", date_fr(&flight.date))
    };
    let unit = if c.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", c.unit)
    };
    let what = if series.is_empty() {
        String::new()
    } else {
        format!("{series} : ")
    };
    Some(format!(
        "Vol {n} : {}{date}\n{what}{}{unit}",
        flight.name,
        chart_number(c, y)
    ))
}

/// Les bornes verticales d'un graphique : toutes les valeurs et les bords de la bande, avec une marge.
fn chart_y_bounds(c: &Chart) -> (f64, f64) {
    let values = c.lines.iter().flat_map(|l| l.points.iter().map(|p| p.value));
    let edges = [c.band.0, c.band.1].into_iter().flatten();
    let (lo, hi) = values
        .chain(edges)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    let pad = if hi > lo {
        (hi - lo) * 0.12
    } else {
        lo.abs().max(1.0) * 0.1
    };
    (lo - pad, hi + pad)
}

/// La légende des graphiques : couleur de chaque axe, et ce que veut dire un point creux.
fn chart_legend(ui: &mut Ui, th: &Theme) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (name, color) in AXIS_NAMES.iter().zip(plot::AXES) {
            swatch(ui, color);
            ui.label(RichText::new(*name).size(HINT).color(th.ui.text));
            ui.add_space(8.0);
        }
        let creux = format!(
            "○ mesure moins sûre (moins de {} fenêtres)",
            otb_advisor::rules::STEP_SURE_WINDOWS
        );
        ui.label(RichText::new(creux).size(HINT).color(th.ui.muted));
    });
}

/// Un graphique du suivi : titre et cible au-dessus, puis la zone visée teintée (vert du thème), ses bords en
/// pointillés, et une ligne par axe ; un point creux est une mesure moins sûre.
fn tracking_chart(ui: &mut Ui, th: &Theme, c: &Chart, flights: &[Flight]) {
    use egui_plot::{HLine, Line, LineStyle, Plot, PlotPoints, Points, Polygon};
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| {
            let unit = if c.unit.is_empty() {
                String::new()
            } else {
                format!(" ({})", c.unit)
            };
            ui.label(
                RichText::new(format!("{}{unit}", c.title))
                    .size(LABEL)
                    .strong()
                    .color(th.ui.text),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(band_text(c)).size(HINT).color(th.ui.muted));
            });
        });
        if c.lines.is_empty() {
            hint(ui, th, "Pas encore de mesure exploitable.");
            return;
        }
        let green = th.severity("ok");
        let (y_lo, y_hi) = chart_y_bounds(c);
        let (x_lo, x_hi) = (0.5, flights.len() as f64 + 0.5);
        let flights = flights.to_vec();
        let chart = c.clone();
        Plot::new(("suivi", c.key))
            .width(ui.available_width())
            .height(CHART_H)
            .auto_bounds(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .show_x(false)
            .show_y(false)
            .x_grid_spacer(integer_grid)
            .y_grid_spacer(|i| nice_grid(i, 3.0))
            .y_axis_formatter(|m, _| tick_fr(m.value, m.step_size))
            // même largeur de graduations partout : les vols tombent à la même verticale d'un graphique à l'autre
            .y_axis_min_width(CHART_Y_AXIS_W)
            .grid_color(th.grid_color())
            .label_formatter(move |pos| match pos {
                egui_plot::HoverPosition::NearDataPoint {
                    plot_name, position, ..
                } => hover_text(&chart, &flights, plot_name, position.x, position.y),
                egui_plot::HoverPosition::Elsewhere { .. } => None,
            })
            .show(ui, |pu| {
                pu.set_plot_bounds_x(x_lo..=x_hi);
                pu.set_plot_bounds_y(y_lo..=y_hi);
                if c.band != (None, None) {
                    let (b_lo, b_hi) = (c.band.0.unwrap_or(y_lo), c.band.1.unwrap_or(y_hi));
                    pu.polygon(
                        Polygon::new(
                            "",
                            PlotPoints::from(vec![[x_lo, b_lo], [x_hi, b_lo], [x_hi, b_hi], [x_lo, b_hi]]),
                        )
                        .fill_color(with_alpha(green, BAND_ALPHA))
                        .stroke(egui::Stroke::NONE)
                        .allow_hover(false),
                    );
                    for edge in [c.band.0, c.band.1].into_iter().flatten() {
                        pu.hline(
                            HLine::new("", edge)
                                .color(green)
                                .width(1.0)
                                .style(LineStyle::dashed_loose())
                                .allow_hover(false),
                        );
                    }
                }
                for line in &c.lines {
                    let (color, name) = match line.axis {
                        a @ 0..=2 => (plot::AXES[a as usize], AXIS_NAMES[a as usize]),
                        _ => (th.fg, ""),
                    };
                    let pts: Vec<[f64; 2]> = line.points.iter().map(|p| [p.flight as f64, p.value]).collect();
                    pu.line(Line::new(name, PlotPoints::from(pts)).color(color).width(1.5));
                    for sure in [true, false] {
                        let pts: Vec<[f64; 2]> = line
                            .points
                            .iter()
                            .filter(|p| p.sure == sure)
                            .map(|p| [p.flight as f64, p.value])
                            .collect();
                        if pts.is_empty() {
                            continue;
                        }
                        pu.points(
                            Points::new(name, PlotPoints::from(pts))
                                .color(color)
                                .radius(POINT_R)
                                .filled(sure),
                        );
                    }
                }
            });
    });
}

fn configurations(ui: &mut Ui, form: &mut SheetForm, lib: &Library, th: &Theme) {
    let configs: Vec<&Config> = form.id.as_ref().map(|id| lib.configs_of_drone(id)).unwrap_or_default();
    section(
        ui,
        th,
        "Configurations Betaflight",
        Some("Un « diff all » enregistré à chaque branchement, quand il a changé."),
        |ui| {
            if configs.is_empty() {
                ui.label(
                    RichText::new("Aucune configuration : elle sera enregistrée au premier branchement.")
                        .size(13.0)
                        .color(th.ui.muted),
                );
                return;
            }
            ui.spacing_mut().item_spacing.y = 0.0;
            // la plus récente en haut
            for i in (0..configs.len()).rev() {
                let c = configs[i];
                let ligne = ui.horizontal(|ui| {
                    ui.add_space(0.0);
                    ui.vertical(|ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(
                            RichText::new(date_fr(&c.recorded))
                                .size(13.0)
                                .strong()
                                .color(th.ui.text),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(config_summary(lib, &configs, i))
                                    .size(12.0)
                                    .color(th.ui.muted),
                            )
                            .truncate(),
                        );
                        ui.add_space(8.0);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let comparer = form.shown == Some((c.id.clone(), true));
                        let voir = form.shown == Some((c.id.clone(), false));
                        let b = ui
                            .add_enabled_ui(i > 0, |ui| small_button(ui, th, "Comparer", comparer))
                            .inner;
                        if b.on_disabled_hover_text("Première configuration : rien à comparer")
                            .clicked()
                        {
                            form.shown = if comparer { None } else { Some((c.id.clone(), true)) };
                        }
                        if small_button(ui, th, "Voir", voir).clicked() {
                            form.shown = if voir { None } else { Some((c.id.clone(), false)) };
                        }
                    });
                });
                let r = ligne.response.rect;
                ui.painter()
                    .hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0, th.ui.border));
                match &form.shown {
                    Some((id, false)) if id == &c.id => config_text(ui, th, &c.text),
                    Some((id, true)) if id == &c.id && i > 0 => {
                        let lignes: Vec<String> = config_changes(&configs[i - 1].text, &c.text)
                            .iter()
                            .map(change_text)
                            .collect();
                        let texte = if lignes.is_empty() {
                            "Aucun réglage changé.".to_string()
                        } else {
                            lignes.join("\n")
                        };
                        config_text(ui, th, &texte);
                    }
                    _ => {}
                }
            }
        },
    );
}

/// Le texte d'une configuration ou de ses changements, en chiffres de machine, dans un cadre qui défile.
fn config_text(ui: &mut Ui, th: &Theme, text: &str) {
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(th.ui.alt)
        .corner_radius(FIELD_RADIUS)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                ui.label(RichText::new(text).size(12.0).monospace().color(th.ui.text));
            });
        });
    ui.add_space(8.0);
}

fn delete(ui: &mut Ui, form: &mut SheetForm, lib: &Library, id: &str, th: &Theme, actions: &mut Vec<SheetAction>) {
    let n = lib.fragments_of_drone(id).len();
    let info = format!(
        "{n} fragment{s} de ce drone {v} retiré{s} de la bibliothèque et de la liste d’analyse.",
        s = if n > 1 { "s" } else { "" },
        v = if n > 1 { "seront" } else { "sera" },
    );
    let rouge = th.accent("red");
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, with_alpha(rouge, DANGER_LINE)))
        .corner_radius(SECTION_RADIUS)
        .inner_margin(egui::Margin::symmetric(SECTION_PAD_X, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // les boutons d'abord, à droite ; le texte prend ce qui reste et passe à la ligne (sinon il glisse
            // sous les boutons)
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().button_padding = Vec2::new(12.0, 6.0);
                if form.ask_delete {
                    let oui = egui::Button::new(
                        RichText::new("Supprimer définitivement")
                            .size(13.0)
                            .color(th.on_accent("red")),
                    )
                    .fill(rouge)
                    .corner_radius(6);
                    if ui.add(oui).clicked() {
                        actions.push(SheetAction::Delete(id.to_string(), form.delete_copies));
                    }
                    let garder = egui::Button::new(RichText::new("Garder").size(13.0).color(th.ui.text))
                        .fill(th.ui.base)
                        .stroke(egui::Stroke::new(1.0, th.ui.mid))
                        .corner_radius(6);
                    if ui.add(garder).clicked() {
                        form.ask_delete = false;
                    }
                } else {
                    let demander = egui::Button::new(RichText::new("Supprimer…").size(13.0).color(rouge))
                        .fill(th.ui.base)
                        .stroke(egui::Stroke::new(1.0, rouge))
                        .corner_radius(6);
                    if ui.add(demander).clicked() {
                        form.ask_delete = true;
                    }
                }
                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new("Supprimer ce drone")
                            .size(13.0)
                            .strong()
                            .color(th.ui.text),
                    );
                    ui.add(egui::Label::new(RichText::new(info).size(LABEL).color(th.ui.muted)).wrap());
                    // W-075 : les vols copiés par USB n'ont plus d'autre trace ; ils ne partent que sur un geste
                    let (n, octets) = lib.copies_of_drone(id);
                    if form.ask_delete && n > 0 {
                        ui.checkbox(&mut form.delete_copies, copies_text(n, octets));
                    }
                });
            });
        });
}

/// « Supprimer aussi les 4 fichiers copiés (31,0 Mo) » : la case de la suppression d'un drone (W-075).
pub fn copies_text(n: usize, bytes: u64) -> String {
    let s = if n > 1 { "s" } else { "" };
    format!(
        "Supprimer aussi le{s} {n} fichier{s} copié{s} par USB ({})",
        crate::usb::size_text(bytes)
    )
}

/// Au lancement : reprend l'ancien fichier des fiches du Guide dans la bibliothèque, l'écrit, puis renomme l'ancien
/// fichier en `.repris` (jamais effacé). Rend le message à montrer, `None` s'il n'y avait rien à reprendre.
///
/// ⚠️ Dans cet ordre : renommer **avant** d'avoir écrit la bibliothèque perdrait les fiches à la moindre coupure ;
/// ne pas renommer ferait revenir à chaque lancement un drone supprimé entre-temps.
pub fn import_old_file(lib: &mut Library, path: &std::path::Path) -> Option<String> {
    let cards = match crate::guide::read_old_cards(path) {
        Ok(c) if c.is_empty() => return None,
        Ok(c) => c,
        Err(e) => {
            return Some(format!(
                "Ancien fichier des fiches drone illisible ({}) : {e}. Il n'est pas repris et reste tel quel.",
                path.display()
            ));
        }
    };
    let n = import_cards(lib, &cards);
    if let Err(e) = lib.save() {
        return Some(format!(
            "Fiches drone reprises, mais la bibliothèque n'a pas pu s'écrire : {e}"
        ));
    }
    match crate::guide::set_aside_old_cards(path) {
        Ok(to) => Some(format!(
            "{n} fiche{s} drone reprise{s} dans la bibliothèque ; l'ancien fichier est gardé sous {}.",
            to.display(),
            s = if n > 1 { "s" } else { "" }
        )),
        Err(e) => Some(format!(
            "Fiches drone reprises ; l'ancien fichier n'a pas pu être renommé : {e}"
        )),
    }
}
