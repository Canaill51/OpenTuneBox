//! Ce que l'assistant retient d'un vol à l'autre (lot 31d, D-073) : le vol auquel comparer le fragment analysé, ce qui
//! a changé depuis lui, et les choix du pilote sur les conseils (appliqué, ou écarté avec sa raison).
//!
//! Fonctions pures sur la bibliothèque et les rapports : elles se testent sans écran (L-071). La page Assistant
//! (`tools/advisor.rs`) ne fait que les afficher.
//!
//! ⚠️ La comparaison ne recalcule **aucune** mesure : la valeur du vol comparé est la jauge du **même constat** dans son
//! propre rapport (D-070 : la jauge vient des variables de la règle, jamais d'un second calcul).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use otb_advisor::rules::{self, Finding, Report};
use otb_store::Library;
use otb_store::model::{Advice, Drone, Fragment};

use crate::tools::advisor::{card_id, split_cards};

/// Nom affiché d'un fragment (son identifiant s'il n'a pas de nom).
pub fn fragment_name(f: &Fragment) -> String {
    if f.name.is_empty() {
        f.id.clone()
    } else {
        f.name.clone()
    }
}

/// Le drone d'un fragment, par le log dont il vient.
pub fn drone_of<'a>(lib: &'a Library, fragment_id: &str) -> Option<&'a Drone> {
    let f = lib.fragment(fragment_id)?;
    lib.drone(&lib.log(&f.log_id)?.drone_id)
}

/// Les vols du drone du fragment, dans l'ordre (`Library::flights_of_drone`), et la place du fragment parmi eux.
fn flights_around<'a>(lib: &'a Library, fragment_id: &str) -> Option<(Vec<&'a Fragment>, usize)> {
    let drone = drone_of(lib, fragment_id)?;
    let flights = lib.flights_of_drone(&drone.id);
    let i = flights.iter().position(|f| f.id == fragment_id)?;
    Some((flights, i))
}

/// Le vol précédent du même drone, dans l'ordre des vols du suivi (`None` : c'est son premier vol connu).
pub fn previous_flight<'a>(lib: &'a Library, fragment_id: &str) -> Option<&'a Fragment> {
    let (flights, i) = flights_around(lib, fragment_id)?;
    i.checked_sub(1).map(|p| flights[p])
}

/// Les vols du même drone rangés **après** le fragment : s'il y en a, il est « déjà traité » (prototype).
pub fn newer_flights<'a>(lib: &'a Library, fragment_id: &str) -> Vec<&'a Fragment> {
    flights_around(lib, fragment_id).map_or_else(Vec::new, |(flights, i)| flights[i + 1..].to_vec())
}

/// Les fragments auxquels on peut comparer : les autres vols du drone, du plus récent au plus ancien, puis les autres
/// fragments de la liste d'analyse (un autre drone, au choix du pilote).
pub fn candidates<'a>(lib: &'a Library, fragment_id: &str) -> Vec<&'a Fragment> {
    let mut out: Vec<&Fragment> =
        flights_around(lib, fragment_id).map_or_else(Vec::new, |(flights, _)| flights.into_iter().rev().collect());
    for item in &lib.analysis().items {
        if let Some(f) = lib.fragment(&item.fragment_id)
            && !out.iter().any(|o| o.id == f.id)
        {
            out.push(f);
        }
    }
    out.retain(|f| f.id != fragment_id);
    out
}

/// Le vol auquel l'assistant compare, avec ce qu'il faut pour l'analyser **sur le fil de calcul** (le fichier, la
/// session et les bornes) : il n'est pas forcément ouvert.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareTarget {
    pub fragment_id: String,
    pub name: String,
    /// Sous le nom : « vol précédent · 2026-11-13 » ou « choisi · JeNo 5 · 2026-11-13 ».
    pub meta: String,
    /// Choisi d'office (le vol précédent), ou par le pilote.
    pub automatic: bool,
    /// Le fichier du log et le fragment tel que rangé (session et bornes).
    pub path: PathBuf,
    pub fragment: Fragment,
}

/// Le vol comparé au fragment `fragment_id` : celui que le pilote a choisi (`pick`) s'il existe encore, sinon le vol
/// précédent du drone ; `None` sans l'un ni l'autre (« aucun vol à comparer »).
pub fn compare_target(lib: &Library, fragment_id: &str, pick: Option<&str>) -> Option<CompareTarget> {
    let picked = pick.filter(|p| *p != fragment_id).and_then(|p| lib.fragment(p));
    let (f, automatic) = match picked {
        Some(f) => (f, false),
        None => (previous_flight(lib, fragment_id)?, true),
    };
    let log = lib.log(&f.log_id)?;
    let meta = if automatic {
        format!("vol précédent · {}", f.date)
    } else {
        let drone = lib
            .drone(&log.drone_id)
            .map_or(log.drone_id.as_str(), |d| d.name.as_str());
        format!("choisi · {drone} · {}", f.date)
    };
    Some(CompareTarget {
        fragment_id: f.id.clone(),
        name: fragment_name(f),
        meta,
        automatic,
        path: PathBuf::from(&log.path),
        fragment: f.clone(),
    })
}

/// Ce qui a changé entre le vol comparé et le fragment analysé, carte par carte (`card_id`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comparison {
    /// Nom du vol comparé.
    pub name: String,
    /// Valeur de la jauge du même constat sur le vol comparé, pour les cartes qui ont une jauge des deux côtés.
    pub values: HashMap<String, f64>,
    /// Cartes du fragment absentes du vol comparé : « nouveau ».
    pub new_cards: HashSet<String>,
    /// Titres des cartes du vol comparé qui ne sont plus là, dans l'ordre de ses cartes.
    pub gone: Vec<String>,
}

/// Compare les cartes (hors vérifications OK) des deux rapports. Une carte se reconnaît par sa clé et son axe, jamais
/// par son titre (D-028) : « Réponse lente » en roll n'est pas la même carte qu'en pitch.
pub fn compare_cards(name: &str, main: &Report, other: &Report) -> Comparison {
    let (cards, _) = split_cards(main);
    let (before, _) = split_cards(other);
    let before_by_id: HashMap<String, &Finding> = before.iter().map(|f| (card_id(f), f)).collect();
    let now: HashSet<String> = cards.iter().map(card_id).collect();
    let mut out = Comparison {
        name: name.to_string(),
        ..Comparison::default()
    };
    for f in &cards {
        let id = card_id(f);
        match before_by_id.get(&id) {
            None => {
                out.new_cards.insert(id);
            }
            Some(b) => {
                // même unité exigée : une règle qui change d'échelle ne doit pas poser un repère faux
                if let (Some(g), Some(gb)) = (&f.gauge, &b.gauge)
                    && g.unit == gb.unit
                    && gb.value.is_finite()
                {
                    out.values.insert(id, gb.value);
                }
            }
        }
    }
    out.gone = before
        .iter()
        .filter(|f| !now.contains(&card_id(f)))
        .map(|f| f.title.clone())
        .collect();
    out
}

/// Un conseil se vote s'il propose un geste : ni une vérification OK, ni une carte sans action.
pub fn can_vote(f: &Finding) -> bool {
    f.severity != rules::OK && !f.actions.is_empty()
}

/// Le premier vol du drone rangé après le fragment et daté du jour du choix ou après : celui qui a pu « prendre en
/// compte » un conseil appliqué ce jour-là. La date d'un vol est au jour près : un vol fait le même jour **avant** le
/// choix compterait aussi (limite dite dans D-073).
pub fn taken_by<'a>(lib: &'a Library, fragment_id: &str, when: &str) -> Option<&'a Fragment> {
    let day = when.get(..10).unwrap_or(when);
    newer_flights(lib, fragment_id)
        .into_iter()
        .find(|f| !f.date.is_empty() && f.date.as_str() >= day)
}

/// « 23/11/2026 » d'une date ISO (`2026-11-23…`) ; le texte tel quel s'il n'en est pas une. L'heure, en temps
/// universel, n'est pas montrée : elle serait fausse d'une ou deux heures en France.
pub fn day_text(iso: &str) -> String {
    let b = iso.as_bytes();
    let ok = b.len() >= 10 && b[4] == b'-' && b[7] == b'-';
    if !ok {
        return iso.to_string();
    }
    format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4])
}

/// L'instant présent, ISO 8601 en temps universel (`AAAA-MM-JJTHH:MM:SSZ`).
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    iso_of_secs(secs)
}

/// `AAAA-MM-JJTHH:MM:SSZ` d'un instant en secondes depuis le 1er janvier 1970.
pub fn iso_of_secs(secs: i64) -> String {
    let (y, m, d) = crate::export::date_from_secs(secs);
    let s = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s / 60 % 60, s % 60)
}

/// Ce qu'une carte dit d'un conseil déjà choisi : « Appliqué le 23/11/2026 » et le vol qui l'a pris en compte, ou
/// « Écarté le 23/11/2026 » et la raison.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryLine {
    pub applied: bool,
    pub status: String,
    pub title: String,
    pub detail: String,
}

/// La ligne d'historique d'un choix.
pub fn history_line(lib: &Library, a: &Advice) -> HistoryLine {
    let day = day_text(&a.when);
    if a.applied {
        let detail = match taken_by(lib, &a.fragment_id, &a.when) {
            Some(f) => format!("Pris en compte par {}", fragment_name(f)),
            None => "En attente du prochain vol".to_string(),
        };
        HistoryLine {
            applied: true,
            status: format!("Appliqué le {day}"),
            title: a.title.clone(),
            detail,
        }
    } else {
        HistoryLine {
            applied: false,
            status: format!("Écarté le {day}"),
            title: a.title.clone(),
            detail: format!("Raison : « {} »", a.reason),
        }
    }
}

/// L'historique des choix faits sur les conseils d'un fragment.
pub fn history(lib: &Library, fragment_id: &str) -> Vec<HistoryLine> {
    lib.advice_of(fragment_id)
        .into_iter()
        .map(|a| history_line(lib, a))
        .collect()
}

/// « · 2 vols plus récents : FINAL.02, FINAL.03 » après « Fragment déjà traité » (prototype) ; vide sans vol plus récent.
pub fn newer_text(newer: &[&Fragment]) -> String {
    let n = newer.len();
    if n == 0 {
        return String::new();
    }
    let s = if n > 1 { "s" } else { "" };
    let names: Vec<String> = newer.iter().map(|f| fragment_name(f)).collect();
    format!("· {n} vol{s} plus récent{s} : {}", names.join(", "))
}
