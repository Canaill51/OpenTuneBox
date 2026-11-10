//! État partagé de l'application : logs chargés, période d'analyse, curseur temporel (port de `gui/state.py`).
//!
//! Pas de signaux comme sous Qt : egui redessine à chaque cadre. Les pages comparent `AppState::revision`
//! (augmenté à chaque changement de logs ou de période) à la dernière valeur qu'elles ont calculée.

use std::any::Any;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use egui::Color32;
use otb_analysis::period::auto_period;
use otb_log::flightlog::FlightLog;

use crate::theme::file_color;

pub const MAX_LOGS: usize = 10;

/// Période de vol par défaut d'un log (`auto_period(log)` de Python : décollage à 8 %, marge de 0,5 s).
pub fn default_period(log: &FlightLog) -> (f64, f64) {
    auto_period(log, 8.0, 0.5).unwrap_or_else(|_| {
        (
            log.t.first().copied().unwrap_or(0.0),
            log.t.last().copied().unwrap_or(0.0),
        )
    })
}

/// Outil propriétaire d'une entrée du cache : c'est lui qui porte le **plafond**.
///
/// Un plafond par outil, et non un plafond global : sans cela, vingt coups de curseur du simulateur de filtres (90 ko
/// l'entrée) chasseraient les résultats de la réponse indicielle (jusqu'à 71 Mo l'entrée). Plafond en **nombre
/// d'entrées** et non en Mo, comme D-051 l'a tranché pour l'ouverture de plusieurs logs : un compteur se teste, une
/// mémoire non. Chiffres et pire cas mesurés : `docs/validation.md`, « Mémoire du cache des outils » (L-094).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Famille {
    /// Réponse indicielle : 72 jeux de réglages atteignables, jusqu'à 71 Mo l'entrée — le poste le plus lourd.
    Step,
    /// Simulateur de filtres : une entrée par jeu de réglages modifiés, donc un nombre **non borné** de clés possibles.
    Filtres,
    /// Assistant : une entrée par profil demandé (détection automatique + les profils de `PROFILES`).
    Assistant,
    /// Guide : vérification rapide et vérification complète d'une même période.
    Guide,
}

impl Famille {
    /// Nombre d'entrées gardées pour cet outil, par log. Au-delà, la **moins récemment utilisée** est jetée : son
    /// résultat sera recalculé si on y revient, à l'identique (un cache ne fait jamais perdre que du temps).
    ///
    /// ⚠️ Baisser un de ces chiffres sous le nombre d'entrées qu'un seul cadre demande ferait évincer un résultat
    /// **en cours d'affichage**, donc recalculer à chaque cadre : `Guide` en demande deux (rapide et complète),
    /// `Assistant` une par profil choisi (L-094).
    pub fn plafond(self) -> usize {
        match self {
            // le réglage affiché et les deux précédents : c'est l'aller-retour entre deux ou trois réglages qu'on fait
            // en réglant (arbitré par Cédric le 2026-11-10) ; pire cas 3 × 71 Mo au lieu de 72 × 71 Mo
            Famille::Step => 3,
            // 20 × 0,2 Mo au pire : de quoi comparer une longue série de coups de curseur sans jamais peser
            Famille::Filtres => 20,
            // détection automatique + les 5 profils : aucun rapport n'est jamais évincé, ils pèsent 0,83 Mo au pire
            Famille::Assistant => 6,
            // les deux vérifications d'une même période, toutes deux demandées au même cadre
            Famille::Guide => 2,
        }
    }
}

/// Une valeur rangée, avec son outil et la date de son dernier usage.
struct Entree {
    valeur: Arc<dyn Any + Send + Sync>,
    famille: Famille,
    /// Numéro du dernier `get` ou `put` : le plus petit est le premier jeté.
    vu: u64,
}

#[derive(Default)]
struct Table {
    entrees: HashMap<String, Entree>,
    /// Compteur monotone des usages (jamais remis à zéro tant que le log est ouvert).
    tic: u64,
}

/// Cache des résultats de calcul d'un log (W-032 point 1 : `e.extras` de Python).
///
/// Partagé avec le thread de calcul (un clone voit le même contenu). La **clé** est écrite par l'outil et doit contenir tout ce qui change le
/// résultat : réglages de calcul **et période** ; ainsi un calcul lancé avant un changement de période ne peut jamais servir après. Vidé quand la
/// période change, et borné par [`Famille::plafond`] entre deux changements de période (W-035 point 9, L-094).
#[derive(Clone, Default)]
pub struct Cache(Arc<Mutex<Table>>);

impl Cache {
    fn table(&self) -> std::sync::MutexGuard<'_, Table> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()) // un calcul qui a paniqué ne doit pas bloquer les suivants
    }

    /// Valeur rangée sous `key`, si elle existe **et** a le type `T`. Un accès rajeunit l'entrée : ce qu'on affiche
    /// n'est jamais ce qu'on jette.
    pub fn get<T: Any + Send + Sync>(&self, key: &str) -> Option<Arc<T>> {
        let mut t = self.table();
        t.tic += 1;
        let tic = t.tic;
        let e = t.entrees.get_mut(key)?;
        e.vu = tic;
        e.valeur.clone().downcast::<T>().ok()
    }

    /// Range `value` sous `key` pour l'outil `famille`, puis jette les entrées de **cette famille** au-delà de son
    /// plafond, la moins récemment utilisée d'abord.
    pub fn put<T: Any + Send + Sync>(&self, famille: Famille, key: impl Into<String>, value: Arc<T>) {
        let mut t = self.table();
        t.tic += 1;
        let (tic, key) = (t.tic, key.into());
        t.entrees.insert(
            key,
            Entree {
                valeur: value,
                famille,
                vu: tic,
            },
        );
        let plafond = famille.plafond();
        loop {
            let mut miennes: Vec<(u64, &String)> = t
                .entrees
                .iter()
                .filter(|(_, e)| e.famille == famille)
                .map(|(k, e)| (e.vu, k))
                .collect();
            if miennes.len() <= plafond {
                break;
            }
            miennes.sort_unstable();
            let vieille = miennes[0].1.clone();
            t.entrees.remove(&vieille);
        }
    }

    pub fn clear(&self) {
        let mut t = self.table();
        t.entrees.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.table().entrees.is_empty()
    }

    /// Nombre d'entrées gardées, tous outils confondus.
    pub fn len(&self) -> usize {
        self.table().entrees.len()
    }

    /// Nombre d'entrées gardées pour un outil : ce que les tests du plafond comparent à [`Famille::plafond`].
    pub fn len_of(&self, famille: Famille) -> usize {
        self.table().entrees.values().filter(|e| e.famille == famille).count()
    }
}

pub struct LoadedLog {
    pub log: Arc<FlightLog>,
    /// Identifiant propre à ce chargement, jamais réutilisé : une adresse mémoire, elle, peut resservir à un autre log (L-072).
    pub uid: u64,
    pub period: (f64, f64),
    pub color: Color32,
    pub label: String,
    /// Résultats de calculs par outil, vidés quand la période change.
    pub extras: Cache,
}

impl LoadedLog {
    /// Indices des points de la période.
    pub fn sel(&self) -> Range<usize> {
        self.log.slice_time(self.period.0, self.period.1)
    }
}

#[derive(Default)]
pub struct AppState {
    pub logs: Vec<LoadedLog>,
    /// Index du log courant (`None` : aucun).
    pub current: Option<usize>,
    pub cursor: f64,
    revision: u64,
    next_uid: u64,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Change à chaque modification des logs ou d'une période (pas du curseur ni du log courant).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn bump(&mut self) {
        self.revision += 1;
    }

    /// Ajoute un log ; `false` s'il y en a déjà `MAX_LOGS`. Il prend la première couleur libre et un
    /// libellé unique (« nom (2) »…), puis devient le log courant.
    pub fn add(&mut self, log: FlightLog) -> bool {
        if self.logs.len() >= MAX_LOGS {
            return false;
        }
        let color = (0..MAX_LOGS)
            .map(file_color)
            .find(|c| self.logs.iter().all(|l| l.color != *c))
            .unwrap_or_else(|| file_color(0));
        let name = log.name();
        let mut label = name.clone();
        let mut k = 2;
        while self.logs.iter().any(|l| l.label == label) {
            label = format!("{name} ({k})");
            k += 1;
        }
        let period = default_period(&log);
        let uid = self.next_uid;
        self.next_uid += 1;
        self.logs.push(LoadedLog {
            log: Arc::new(log),
            uid,
            period,
            color,
            label,
            extras: Cache::default(),
        });
        self.bump();
        self.current = Some(self.logs.len() - 1);
        true
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.logs.len() {
            self.logs.remove(index);
            self.bump();
            self.current = self
                .current
                .and_then(|c| self.logs.len().checked_sub(1).map(|last| c.min(last)));
        }
    }

    pub fn clear(&mut self) {
        self.logs.clear();
        self.bump();
        self.current = None;
    }

    pub fn set_current(&mut self, index: Option<usize>) {
        self.current = index.filter(|i| *i < self.logs.len());
    }

    pub fn current_log(&self) -> Option<&LoadedLog> {
        self.current.and_then(|i| self.logs.get(i))
    }

    /// Borne la période au log et l'ordonne ; ne fait rien si elle ne change pas.
    pub fn set_period(&mut self, index: usize, t0: f64, t1: f64) {
        let Some(ll) = self.logs.get_mut(index) else { return };
        let (Some(&lo), Some(&hi)) = (ll.log.t.first(), ll.log.t.last()) else {
            return;
        };
        let (a, b) = (t0.max(lo), t1.min(hi));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        if (a, b) != ll.period {
            ll.period = (a, b);
            ll.extras.clear();
            self.bump();
        }
    }

    pub fn reset_period(&mut self, index: usize) {
        if let Some(ll) = self.logs.get(index) {
            let p = default_period(&ll.log);
            self.set_period(index, p.0, p.1);
        }
    }

    pub fn set_cursor(&mut self, t: f64) {
        self.cursor = t;
    }
}
