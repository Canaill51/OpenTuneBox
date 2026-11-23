//! Mesures suivies dans le temps (lot 31a) : lire la table des mesures de la bibliothèque **hors du fil de l'écran**
//! (D-064), puis mesurer en arrière-plan les fragments d'un drone qui n'ont pas encore de mesure valable.
//!
//! Les grandeurs viennent de `otb_advisor::tracked` (le rapport de l'assistant, jamais un second calcul) ; ce module
//! ne fait que les promener : du fichier de log au fil de calcul, puis du fil de calcul à la bibliothèque.
//!
//! ⚠️ Un résultat qui revient du fil de calcul n'est rangé **que si le fragment a encore les bornes mesurées** : il
//! a pu être déplacé dans l'écran d'extraction pendant le calcul, et sa mesure parlerait alors d'un autre morceau de
//! vol.

use otb_advisor::tracked::{self, CALC_VERSION, Tracked};
use otb_log::flightlog::{self, FlightLog};
use otb_store::model::{Fragment, Measure};
use otb_store::{Library, MeasuresFile};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

/// Un fragment mesuré sur le fil de calcul : les réglages de calcul et les grandeurs, ou la raison de l'échec.
struct Measured {
    fragment: Fragment,
    result: Result<(String, Vec<Tracked>), String>,
}

/// Le calcul en cours, pour un drone.
struct Job {
    drone: String,
    rx: Receiver<Measured>,
    stop: Arc<AtomicBool>,
    total: usize,
    done: usize,
}

/// Où en est le suivi d'un drone, pour l'écran.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    /// La table des mesures se lit (une seconde au plus au volume plein).
    Reading,
    /// `done` fragments mesurés sur les `total` de ce calcul.
    Measuring { done: usize, total: usize },
    /// Rien en cours.
    Idle,
}

/// Le suivi d'un drone tel que la fiche l'affiche.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackingView {
    pub progress: Progress,
    /// Fragments du drone mesurés sur leurs bornes actuelles, et fragments du drone en tout.
    pub measured: usize,
    pub total: usize,
    /// Fragments qui n'ont pas pu se mesurer pendant cette séance : nom et raison.
    pub failed: Vec<(String, String)>,
}

/// La lecture de la table des mesures et le calcul en arrière-plan.
#[derive(Default)]
pub struct Tracking {
    loading: Option<Receiver<MeasuresFile>>,
    job: Option<Job>,
    /// ⚠️ Fragments en échec pendant cette séance (log déplacé, fichier tronqué…), avec la raison : sans cette liste,
    /// ils seraient relancés à chaque cadre tant que la fiche est ouverte.
    failed: BTreeMap<String, String>,
}

/// Les grandeurs d'un fragment, en lignes de la bibliothèque.
pub fn measures_of(fragment: &Fragment, settings: &str, values: &[Tracked]) -> Vec<Measure> {
    values
        .iter()
        .map(|t| Measure {
            fragment_id: fragment.id.clone(),
            axis: t.axis,
            key: t.key.to_string(),
            value: t.value,
            start: fragment.start,
            end: fragment.end,
            settings: settings.to_string(),
            calc_version: CALC_VERSION,
        })
        .collect()
}

/// Les fragments du drone `drone_id` qui n'ont pas de mesure valable, avec le chemin de leur log.
fn to_measure(lib: &Library, drone_id: &str, failed: &BTreeMap<String, String>) -> Vec<(Fragment, PathBuf)> {
    lib.fragments_of_drone(drone_id)
        .into_iter()
        .filter(|f| !failed.contains_key(&f.id) && !lib.is_measured(f, CALC_VERSION))
        .filter_map(|f| Some((f.clone(), PathBuf::from(&lib.log(&f.log_id)?.path))))
        .collect()
}

/// Mesure un fragment de `log` ; une panique du calcul devient une erreur, comme dans `worker`.
fn measure_one(log: &FlightLog, fragment: &Fragment) -> Result<(String, Vec<Tracked>), String> {
    let range = otb_store::link::range_of(fragment, log.n())?;
    catch_unwind(AssertUnwindSafe(|| tracked::measure(log, range))).unwrap_or_else(|_| Err("panique du calcul".into()))
}

/// Les sessions d'un fichier décodé, ou la raison de l'échec.
type Decoded = Result<Vec<Arc<FlightLog>>, String>;

/// Le fil de calcul : chaque fichier n'est décodé qu'une fois, et pas du tout si sa session est déjà ouverte.
fn run(
    mut todo: Vec<(Fragment, PathBuf)>,
    open: Vec<Arc<FlightLog>>,
    tx: Sender<Measured>,
    stop: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    todo.sort_by(|a, b| a.1.cmp(&b.1));
    let mut decoded: Option<(PathBuf, Decoded)> = None;
    for (fragment, path) in todo {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let session = |l: &&Arc<FlightLog>| l.path == path && l.session as u64 == fragment.session;
        let result = match open.iter().find(session) {
            Some(log) => measure_one(log, &fragment),
            None => {
                if decoded.as_ref().is_none_or(|(p, _)| *p != path) {
                    let logs = flightlog::load(&path)
                        .map(|v| v.into_iter().map(Arc::new).collect())
                        .map_err(|e| format!("log illisible ({e})"));
                    decoded = Some((path.clone(), logs));
                }
                match &decoded.as_ref().expect("décodé juste au-dessus").1 {
                    Err(e) => Err(e.clone()),
                    Ok(logs) => match logs.iter().find(session) {
                        Some(log) => measure_one(log, &fragment),
                        None => Err(format!("session {} absente du fichier", fragment.session)),
                    },
                }
            }
        };
        if tx.send(Measured { fragment, result }).is_err() {
            return; // plus personne n'attend
        }
        ctx.request_repaint();
    }
}

impl Tracking {
    /// La fiche du drone `drone_id` est affichée : lire la table des mesures si ce n'est pas fait, puis mesurer ses
    /// fragments qui n'ont pas de mesure valable. Sans effet si le calcul de ce drone tourne déjà ; le calcul d'un
    /// autre drone est arrêté (après son fragment en cours). `open` : les sessions déjà décodées, pour ne pas relire
    /// leur fichier.
    pub fn wish(&mut self, ctx: &egui::Context, lib: &Library, drone_id: &str, open: &[Arc<FlightLog>]) {
        if !lib.measures_loaded() {
            if self.loading.is_none() {
                let source = lib.measures_source();
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(source.read());
                    ctx.request_repaint();
                });
                self.loading = Some(rx);
            }
            return;
        }
        match &self.job {
            Some(job) if job.drone == drone_id => return,
            Some(job) => job.stop.store(true, Ordering::Relaxed),
            None => {}
        }
        self.job = None;
        let todo = to_measure(lib, drone_id, &self.failed);
        if todo.is_empty() {
            return;
        }
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let total = todo.len();
        let open = open.to_vec();
        let (stop2, ctx) = (stop.clone(), ctx.clone());
        std::thread::spawn(move || run(todo, open, tx, stop2, ctx));
        self.job = Some(Job {
            drone: drone_id.to_string(),
            rx,
            stop,
            total,
            done: 0,
        });
    }

    /// Range dans la bibliothèque ce qui est arrivé des fils (table lue, fragments mesurés). À appeler à chaque cadre.
    pub fn poll(&mut self, lib: &mut Library) {
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(file) => {
                    lib.attach_measures(file);
                    lib.drop_measures_before_version(CALC_VERSION);
                    self.loading = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.loading = None, // relancée à la prochaine demande
            }
        }
        let Some(job) = &mut self.job else { return };
        loop {
            match job.rx.try_recv() {
                Ok(m) => {
                    job.done += 1;
                    match m.result {
                        Err(e) => {
                            self.failed.insert(m.fragment.id.clone(), e);
                        }
                        Ok((settings, values)) => {
                            let same = lib
                                .fragment(&m.fragment.id)
                                .is_some_and(|f| (f.start, f.end) == (m.fragment.start, m.fragment.end));
                            if same {
                                for measure in measures_of(&m.fragment, &settings, &values) {
                                    lib.set_measure(measure);
                                }
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.job = None;
                    return;
                }
            }
        }
    }

    /// Un calcul ou une lecture est en cours (les tests attendent qu'il finisse).
    pub fn busy(&self) -> bool {
        self.loading.is_some() || self.job.is_some()
    }

    /// Le suivi du drone `drone_id`, pour sa fiche.
    pub fn view(&self, lib: &Library, drone_id: &str) -> TrackingView {
        let fragments = lib.fragments_of_drone(drone_id);
        let progress = if self.loading.is_some() || !lib.measures_loaded() {
            Progress::Reading
        } else {
            match &self.job {
                Some(job) if job.drone == drone_id => Progress::Measuring {
                    done: job.done,
                    total: job.total,
                },
                _ => Progress::Idle,
            }
        };
        let failed = fragments
            .iter()
            .filter_map(|f| {
                let why = self.failed.get(&f.id)?;
                let name = if f.name.is_empty() {
                    f.id.clone()
                } else {
                    f.name.clone()
                };
                Some((name, why.clone()))
            })
            .collect();
        TrackingView {
            progress,
            measured: fragments.iter().filter(|f| lib.is_measured(f, CALC_VERSION)).count(),
            total: fragments.len(),
            failed,
        }
    }
}
