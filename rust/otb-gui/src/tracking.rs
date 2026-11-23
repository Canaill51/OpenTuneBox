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

// ---------------------------------------------------------------- graphiques du suivi (lot 31c)

/// Un vol du suivi : un fragment mesuré, à sa place dans l'ordre des vols (abscisse 1, 2, 3…).
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub fragment_id: String,
    pub name: String,
    /// Date ISO du fragment (celle du fichier, au jour près).
    pub date: String,
}

/// Un point d'un graphique : le numéro du vol (à partir de 1), la valeur, et si la mesure est sûre.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartPoint {
    pub flight: usize,
    pub value: f64,
    /// `false` sous `STEP_SURE_WINDOWS` fenêtres de réponse indicielle : la « mesure moins sûre » de l'assistant.
    pub sure: bool,
}

/// Les points d'un axe (0, 1, 2 : roll, pitch, lacet ; `ALL_AXES` pour une grandeur du fragment entier).
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLine {
    pub axis: u64,
    pub points: Vec<ChartPoint>,
}

/// Le graphique d'une grandeur : ses lignes et la bande visée par la fiche cible (`target::band`).
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    /// La grandeur suivie (`otb_advisor::tracked`).
    pub key: &'static str,
    pub title: &'static str,
    pub unit: &'static str,
    pub decimals: usize,
    pub band: (Option<f64>, Option<f64>),
    /// Sans les axes qui n'ont aucun point.
    pub lines: Vec<ChartLine>,
}

/// Les graphiques du suivi d'un drone : la liste des vols (l'abscisse) et un graphique par grandeur de la fiche cible.
#[derive(Clone, Debug, PartialEq)]
pub struct Charts {
    pub flights: Vec<Flight>,
    pub charts: Vec<Chart>,
}

/// La mesure rangée d'un fragment, **si elle vaut pour ses bornes actuelles et le calcul en vigueur** (comme
/// `Library::is_measured`) ; une mesure périmée ne se trace pas.
fn current(lib: &Library, f: &Fragment, axis: u64, key: &str) -> Option<f64> {
    let m = lib.measure(&f.id, axis, key)?;
    let valid = m.calc_version >= CALC_VERSION && (m.start, m.end) == (f.start, f.end);
    (valid && m.value.is_finite()).then_some(m.value)
}

/// Les graphiques du suivi du drone `drone_id`, la bande lue dans `targets` (les dix cases de la fiche cible, dans
/// l'ordre de `target::ROWS`, `None` pour une case vide). Fonction pure : elle se teste sans écran (L-071).
///
/// - Les vols sont les fragments du drone qui ont des mesures valables, rangés par date, puis par log, session et
///   début : un fragment pas encore mesuré n'a pas de numéro.
/// - Une grandeur de la réponse indicielle n'a **pas de point** sous `STEP_MIN_WINDOWS` fenêtres (l'assistant ne la
///   juge pas), et un point « moins sûr » sous `STEP_SURE_WINDOWS` — sauf les marges de la boucle ouverte, que
///   l'assistant ne marque pas ainsi (`rules::step_rules`).
pub fn charts_of(lib: &Library, drone_id: &str, targets: &[Option<f64>; 10]) -> Charts {
    use otb_advisor::rules::{STEP_MIN_WINDOWS, STEP_SURE_WINDOWS};
    use otb_advisor::target::{ROWS, band, charted};
    use otb_advisor::tracked::{ALL_AXES, AXIS_KEYS, GAIN_MARGIN_DB, PHASE_MARGIN_DEG, STEP_WINDOWS};

    let mut fragments: Vec<&Fragment> = lib
        .fragments_of_drone(drone_id)
        .into_iter()
        .filter(|f| lib.is_measured(f, CALC_VERSION))
        .collect();
    let log_name = |f: &Fragment| lib.log(&f.log_id).map(|l| l.name.clone()).unwrap_or_default();
    fragments
        .sort_by(|a, b| (&a.date, log_name(a), a.session, a.start).cmp(&(&b.date, log_name(b), b.session, b.start)));
    let flights = fragments
        .iter()
        .map(|f| Flight {
            fragment_id: f.id.clone(),
            name: if f.name.is_empty() {
                f.id.clone()
            } else {
                f.name.clone()
            },
            date: f.date.clone(),
        })
        .collect();
    let charts = charted()
        .into_iter()
        .map(|key| {
            let row = ROWS.iter().find(|r| r.tracked == key).expect("grandeur tirée de ROWS");
            let per_axis = AXIS_KEYS.contains(&key);
            let axes: Vec<u64> = if per_axis { vec![0, 1, 2] } else { vec![ALL_AXES] };
            let lines = axes
                .into_iter()
                .map(|axis| {
                    let points = fragments
                        .iter()
                        .enumerate()
                        .filter_map(|(i, f)| {
                            let value = current(lib, f, axis, key)?;
                            let mut sure = true;
                            if per_axis {
                                let n = current(lib, f, axis, STEP_WINDOWS).unwrap_or(0.0);
                                if n < STEP_MIN_WINDOWS as f64 {
                                    return None;
                                }
                                let margin = key == PHASE_MARGIN_DEG || key == GAIN_MARGIN_DB;
                                sure = margin || n >= STEP_SURE_WINDOWS as f64;
                            }
                            Some(ChartPoint {
                                flight: i + 1,
                                value,
                                sure,
                            })
                        })
                        .collect::<Vec<_>>();
                    ChartLine { axis, points }
                })
                .filter(|l| !l.points.is_empty())
                .collect();
            Chart {
                key,
                // « Pic, bas de la zone » → « Pic » : le graphique porte les deux bords
                title: row.label.split(',').next().unwrap_or(row.label),
                unit: row.unit,
                decimals: row.decimals,
                band: band(key, targets),
                lines,
            }
        })
        .collect();
    Charts { flights, charts }
}
