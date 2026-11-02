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

/// Cache des résultats de calcul d'un log (W-032 point 1 : `e.extras` de Python).
///
/// Partagé avec le thread de calcul (un clone voit le même contenu). La **clé** est écrite par l'outil et doit contenir tout ce qui change le
/// résultat : réglages de calcul **et période** ; ainsi un calcul lancé avant un changement de période ne peut jamais servir après. Vidé quand la
/// période change, pour borner la mémoire.
#[derive(Clone, Default)]
pub struct Cache(Arc<Mutex<HashMap<String, Arc<dyn Any + Send + Sync>>>>);

impl Cache {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<dyn Any + Send + Sync>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()) // un calcul qui a paniqué ne doit pas bloquer les suivants
    }

    /// Valeur rangée sous `key`, si elle existe **et** a le type `T`.
    pub fn get<T: Any + Send + Sync>(&self, key: &str) -> Option<Arc<T>> {
        self.map().get(key).cloned()?.downcast::<T>().ok()
    }

    pub fn put<T: Any + Send + Sync>(&self, key: impl Into<String>, value: Arc<T>) {
        self.map().insert(key.into(), value);
    }

    pub fn clear(&self) {
        self.map().clear();
    }

    pub fn is_empty(&self) -> bool {
        self.map().is_empty()
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
