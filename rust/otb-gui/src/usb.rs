//! Récupération des logs par USB, côté interface (lot 32b, D-063, D-074) : un **fil de travail** qui tient la
//! connexion à la carte (identifier, copier, effacer, lire la configuration), et les **fonctions pures** qui rangent
//! le résultat dans la bibliothèque. L'écran est dans `usb_page.rs`.
//!
//! ⚠️ **Un seul chemin vers l'effacement** (critère d'arrêt du lot 32) : la copie vérifiée (`otb_usb::VerifiedCopy`,
//! fichier écrit, relu et décodé) ne quitte jamais ce fil, et seule la demande `Request::Erase` la change en permis
//! (`VerifiedCopy::confirm`) pour `otb_usb::erase_flash`, qui relit l'identifiant, la place occupée et l'état armé
//! juste avant d'effacer. Sans copie vérifiée sur **cette** connexion, l'effacement est refusé sans rien envoyer.
//! L'écran, lui, ne propose d'effacer qu'une fois la copie rangée dans la bibliothèque et la bibliothèque écrite.
//!
//! Lot 32c (D-075) : une copie de la mémoire faite **par le mode disque** (`usb_disk.rs`) n'a pas de connexion MSP. Elle
//! attend dans une boîte fermée ([`Held`]) que la carte soit rebranchée ; seule une connexion à **la même carte**
//! (identifiant gravé connu et identique, place occupée inchangée) l'en sort, sur `Request::Erase`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use otb_log::flightlog::{self, FlightLog};
use otb_store::Library;
use otb_store::model::{Drone, Fragment, LogEntry};
use otb_usb::erase::{ErasePolling, erase_flash, verify_and_save};
use otb_usb::{BeforeDisk, ChipId, CopyReport, FlashSummary, Identity, Link, Msp, VerifiedCopy};

// ---------------------------------------------------------------- le port

/// Un port ouvert, de n'importe quel genre (série, ou carte simulée des tests).
pub struct AnyLink(pub Box<dyn Link + Send>);

impl Link for AnyLink {
    fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.0.send(bytes)
    }
    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> std::io::Result<usize> {
        self.0.recv(buf, timeout)
    }
}

/// Liste les ports où une carte peut répondre.
pub type ListPorts = Arc<dyn Fn() -> Vec<PathBuf> + Send + Sync>;
/// Ouvre un port, ou dit pourquoi c'est impossible (droits, port occupé, pilote absent…).
pub type OpenPort = Arc<dyn Fn(&Path) -> Result<AnyLink, String> + Send + Sync>;

/// Ce qui ouvre un port (le vrai, ou la carte simulée des tests) et liste ceux où une carte peut répondre.
#[derive(Clone)]
pub struct Ports {
    pub list: ListPorts,
    pub open: OpenPort,
}

impl Ports {
    /// Les vrais ports série (`serial2`, D-063).
    pub fn serial() -> Self {
        Ports {
            list: Arc::new(|| {
                let all = otb_usb::link::list_ports().unwrap_or_default();
                otb_usb::link::board_candidates(all, cfg!(target_os = "linux"))
            }),
            open: Arc::new(|p| {
                otb_usb::SerialLink::open(p)
                    .map(|l| AnyLink(Box::new(l)))
                    .map_err(|e| e.to_string())
            }),
        }
    }
}

// ---------------------------------------------------------------- le fil de travail

/// Ce que l'écran demande au fil.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Relire l'identité, la mémoire et l'état armé.
    Refresh,
    /// Copier toute la mémoire dans ce fichier, puis la vérifier.
    Copy(PathBuf),
    /// Effacer la mémoire : seulement après une copie vérifiée sur cette connexion.
    Erase,
    /// Lire `diff all` ; la carte redémarre et la connexion se ferme ensuite.
    Config,
    /// Redémarrer la carte en mode stockage de masse (lot 32c) ; la connexion se ferme ensuite.
    ToDisk,
}

/// Ce que la carte a dit d'elle.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub identity: Identity,
    pub flash: FlashSummary,
    pub armed: bool,
}

/// Une session (un vol) du fichier copié, avec ce qu'il faut pour en faire un fragment sans relire le fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub session: u64,
    pub name: String,
    pub start: u64,
    pub end: u64,
}

/// Une copie vérifiée, telle que l'écran la range : le rapport des contrôles, l'empreinte et les vols du fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Copied {
    pub report: CopyReport,
    pub fingerprint: String,
    pub sessions: Vec<Session>,
}

/// Ce que le fil rend à l'écran.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Connected(Board),
    /// La connexion n'a pas pu se faire, ou s'est coupée : le fil s'arrête.
    Lost(String),
    /// Une demande a échoué ; la connexion reste.
    Failed(String),
    Progress {
        done: u64,
        total: u64,
    },
    Copied(Copied),
    /// La copie n'a pas passé les contrôles : effacement refusé (le fichier, s'il a été écrit, reste).
    NotVerified(String),
    Erasing(Duration),
    /// Mémoire effacée ; place occupée relue.
    Erased(u32),
    /// `diff all` lu ; la carte redémarre, la connexion est fermée.
    Config(String),
    /// La carte redémarre en disque : ce qui la désignera ensuite. La connexion est fermée.
    ToDisk(BeforeDisk),
}

/// La copie de la mémoire faite par le mode disque, en attente du rebranchement (lot 32c, D-075). ⚠️ Elle ne sort
/// d'ici que vers `erase_flash`, par le fil d'une connexion à la même carte ; l'écran ne voit que son rapport.
#[derive(Clone, Default)]
pub struct Held(Arc<Mutex<Option<HeldCopy>>>);

struct HeldCopy {
    copy: VerifiedCopy,
    chip_id: ChipId,
    used: u32,
}

impl Held {
    /// Une copie attend la carte.
    pub fn waiting(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }

    /// Le rapport de la copie qui attend.
    pub fn report(&self) -> Option<CopyReport> {
        self.0.lock().unwrap().as_ref().map(|h| h.copy.report().clone())
    }

    /// Cette carte est-elle celle de la copie ? Identifiant gravé **connu** et identique, place occupée inchangée (aucun
    /// vol enregistré depuis). Un identifiant nul ne prouve rien : jamais.
    pub fn matches(&self, b: &Board) -> bool {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|h| h.chip_id.is_known() && h.chip_id == b.identity.chip_id && h.used == b.flash.used)
    }

    /// Oublie la copie (une nouvelle commence) : elle reste dans la bibliothèque, seul le droit d'effacer se perd.
    pub fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }

    pub(crate) fn put(&self, copy: VerifiedCopy, chip_id: ChipId, used: u32) {
        *self.0.lock().unwrap() = Some(HeldCopy { copy, chip_id, used });
    }

    /// La copie, seulement pour la carte qui l'a donnée.
    fn take_for(&self, b: &Board) -> Option<VerifiedCopy> {
        if !self.matches(b) {
            return None;
        }
        self.0.lock().unwrap().take().map(|h| h.copy)
    }
}

/// La connexion à une carte, tenue par un fil de travail.
pub struct Connection {
    tx: Sender<Request>,
    rx: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    /// Demandes en cours (le fil n'en traite qu'une à la fois).
    busy: Arc<Mutex<bool>>,
}

impl Connection {
    /// Ouvre `port` sur un autre fil et identifie la carte ; la réponse arrive par `poll` (`Connected` ou `Lost`).
    pub fn open(ports: &Ports, port: PathBuf, polling: ErasePolling, ctx: egui::Context) -> Self {
        Self::open_with(ports, port, polling, Held::default(), ctx)
    }

    /// Comme [`Connection::open`], avec la copie faite par le mode disque qui attend cette carte (lot 32c).
    pub fn open_with(ports: &Ports, port: PathBuf, polling: ErasePolling, held: Held, ctx: egui::Context) -> Self {
        let (tx, rx_req) = channel();
        let (tx_ev, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(Mutex::new(true));
        let open = ports.open.clone();
        let held = held.clone();
        let (c, b) = (cancel.clone(), busy.clone());
        std::thread::spawn(move || {
            let send = |e: Event| {
                let _ = tx_ev.send(e);
                ctx.request_repaint();
            };
            let link = match open(&port) {
                Ok(l) => l,
                Err(e) => {
                    *b.lock().unwrap() = false;
                    return send(Event::Lost(e));
                }
            };
            worker(Msp::new(link), rx_req, &send, &c, &b, polling, &held);
        });
        Connection { tx, rx, cancel, busy }
    }

    pub fn request(&self, r: Request) {
        self.cancel.store(false, Ordering::Relaxed);
        *self.busy.lock().unwrap() = true;
        let _ = self.tx.send(r);
    }

    /// Arrête la copie en cours (rien n'est gardé ; la suivante reprend au début).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Une demande est en cours (ou l'ouverture).
    pub fn busy(&self) -> bool {
        *self.busy.lock().unwrap()
    }

    /// Les nouvelles du fil.
    pub fn poll(&self) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }
}

fn board<L: Link>(msp: &mut Msp<L>) -> Result<Board, String> {
    let identity = msp.identify().map_err(|e| e.to_string())?;
    let flash = msp.flash_summary().map_err(|e| e.to_string())?;
    let armed = msp.armed().map_err(|e| e.to_string())?;
    Ok(Board { identity, flash, armed })
}

/// La boucle du fil : une demande à la fois, jusqu'à la fermeture de l'écran ou de la connexion.
fn worker<L: Link>(
    mut msp: Msp<L>,
    rx: Receiver<Request>,
    send: &dyn Fn(Event),
    cancel: &AtomicBool,
    busy: &Mutex<bool>,
    polling: ErasePolling,
    held: &Held,
) {
    let done = || *busy.lock().unwrap() = false;
    // la dernière carte lue : une copie faite par le disque ne sert qu'à elle
    let mut current = match board(&mut msp) {
        Ok(b) => {
            done();
            send(Event::Connected(b.clone()));
            b
        }
        Err(e) => {
            done();
            return send(Event::Lost(e));
        }
    };
    // ⚠️ la seule copie qui peut devenir un permis d'effacer : celle de cette connexion (ou, pour la même carte,
    // celle faite par le disque : `Held`)
    let mut verified: Option<VerifiedCopy> = None;
    while let Ok(req) = rx.recv() {
        match req {
            Request::Refresh => match board(&mut msp) {
                Ok(b) => {
                    current = b.clone();
                    send(Event::Connected(b));
                }
                Err(e) => send(Event::Failed(e)),
            },
            Request::Copy(path) => {
                verified = None;
                send(copy(&mut msp, &path, cancel, send, &mut verified));
            }
            Request::Erase => {
                // ⚠️ la copie de cette connexion, sinon celle du disque si c'est la même carte (`erase_flash` relit tout)
                let Some(v) = verified.take().or_else(|| held.take_for(&current)) else {
                    send(Event::Failed(
                        "Aucune copie vérifiée de cette carte : effacement refusé.".into(),
                    ));
                    done();
                    continue;
                };
                let r = erase_flash(&mut msp, v.confirm(), polling, |t| send(Event::Erasing(t)));
                match r
                    .map_err(|e| e.to_string())
                    .and_then(|()| msp.flash_summary().map(|s| s.used).map_err(|e| e.to_string()))
                {
                    Ok(used) => send(Event::Erased(used)),
                    Err(e) => send(Event::Failed(e)),
                }
            }
            Request::Config => {
                done();
                return send(match msp.diff_all() {
                    Ok(t) => Event::Config(t),
                    Err(e) => Event::Lost(e.to_string()),
                });
            }
            // ⚠️ armé : `reboot_to_disk` refuse sans rien envoyer ; un refus garde la connexion
            Request::ToDisk => match otb_usb::reboot_to_disk(&mut msp) {
                Ok(before) => {
                    done();
                    return send(Event::ToDisk(before));
                }
                Err(e) => send(Event::Failed(e.to_string())),
            },
        }
        done();
    }
}

/// Copie, vérifie et décrit la copie. ⚠️ Drone armé : rien n'est lu (« drone armé → aucune opération »).
fn copy<L: Link>(
    msp: &mut Msp<L>,
    path: &Path,
    cancel: &AtomicBool,
    send: &dyn Fn(Event),
    verified: &mut Option<VerifiedCopy>,
) -> Event {
    match msp.armed() {
        Ok(false) => {}
        Ok(true) => return Event::Failed(ARMED.into()),
        Err(e) => return Event::Failed(e.to_string()),
    }
    let copy = match msp.copy_flash(|done, total| {
        send(Event::Progress { done, total });
        !cancel.load(Ordering::Relaxed)
    }) {
        Ok(c) => c,
        Err(e) => return Event::Failed(e.to_string()),
    };
    let v = match verify_and_save(copy, path) {
        Ok(v) => v,
        Err(e) => return Event::NotVerified(e.to_string()),
    };
    let report = v.report().clone();
    match describe(&report.path) {
        Ok((fingerprint, sessions)) => {
            *verified = Some(v);
            Event::Copied(Copied {
                report,
                fingerprint,
                sessions,
            })
        }
        Err(e) => Event::NotVerified(format!("{e} Effacement refusé.")),
    }
}

/// L'empreinte du fichier copié et ses vols, avec les limites automatiques de chacun (comme à l'ouverture d'un log).
pub fn describe(path: &Path) -> Result<(String, Vec<Session>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Relecture de {} impossible : {e}.", path.display()))?;
    let logs = flightlog::load(path).map_err(|e| format!("Le décodeur ne lit pas la copie : {e}."))?;
    Ok((otb_store::fingerprint(&bytes), logs.iter().map(session_of).collect()))
}

fn session_of(log: &FlightLog) -> Session {
    let p = crate::state::default_period(log);
    let auto = log.slice_time(p.0, p.1);
    Session {
        session: log.session as u64,
        name: log.name(),
        start: auto.start as u64,
        end: auto.end as u64,
    }
}

pub const ARMED: &str =
    "Le drone est armé : rien n'est lu ni effacé. Le désarmer (et retirer les hélices), puis réessayer.";

// ---------------------------------------------------------------- le drone

/// À quel drone ranger ce que la carte apporte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DroneChoice {
    /// Un drone de la bibliothèque (reconnu par l'identifiant de sa carte, ou choisi par le pilote).
    Existing(String),
    /// Un nouveau drone, de ce nom.
    New(String),
}

/// L'identifiant de carte gardé dans la bibliothèque : vide pour un identifiant nul (il ne prouve rien, D-063).
pub fn uid_of(identity: &Identity) -> String {
    if identity.chip_id.is_known() {
        identity.chip_id.to_hex()
    } else {
        String::new()
    }
}

/// « STM32F7X2 (S7X2) » : la cible et l'identifiant de carte à quatre lettres.
pub fn board_text(identity: &Identity) -> String {
    match (identity.target.is_empty(), identity.board.is_empty()) {
        (false, false) => format!("{} ({})", identity.target, identity.board),
        (false, true) => identity.target.clone(),
        _ => identity.board.clone(),
    }
}

/// « Betaflight 4.5.1 », ou « Betaflight 2025.12.0 » : depuis 2025, le premier nombre de la version MSP est l'année
/// depuis 2000 (`otb_usb::Identity::version`), affiché en entier comme Betaflight l'écrit.
pub fn firmware_text(identity: &Identity) -> String {
    let (a, b, c) = identity.version;
    let major = if a >= 25 { 2000 + u32::from(a) } else { u32::from(a) };
    format!("Betaflight {major}.{b}.{c}")
}

/// Le drone reconnu par l'identifiant de la carte (`true` : rien à demander), sinon la proposition à montrer :
/// le drone du même nom s'il existe, sinon un nouveau drone de ce nom (« Carte S7X2 » sans nom).
pub fn default_choice(lib: &Library, identity: &Identity) -> (DroneChoice, bool) {
    if let Some(d) = lib.drone_by_uid(&uid_of(identity)) {
        return (DroneChoice::Existing(d.id.clone()), true);
    }
    let name = identity.name.trim();
    if let Some(d) = (!name.is_empty()).then(|| lib.drone_by_name(name)).flatten() {
        return (DroneChoice::Existing(d.id.clone()), false);
    }
    let fallback = format!("Carte {}", identity.board);
    (
        DroneChoice::New(if name.is_empty() { fallback } else { name.to_string() }),
        false,
    )
}

/// Range l'identité de la carte sur le drone choisi (le crée s'il est nouveau) et rend son identifiant. Un nom vide
/// ou déjà pris par un autre drone est refusé : deux drones de même nom se mélangeraient à l'ouverture des logs.
pub fn apply_choice(
    lib: &mut Library,
    choice: &DroneChoice,
    identity: &Identity,
    today: &str,
) -> Result<String, String> {
    let mut drone = resolve_choice(lib, choice, today)?;
    let uid = uid_of(identity);
    if !uid.is_empty() {
        drone.uid = uid;
    }
    drone.board = board_text(identity);
    if drone.firmware.is_empty() {
        drone.firmware = firmware_text(identity);
    }
    drone.last_seen = today.to_string();
    let id = drone.id.clone();
    lib.put_drone(drone);
    Ok(id)
}

/// Le drone choisi pour une carte SD venue d'un lecteur (lot 32c) : pas de carte branchée, donc rien de la carte à
/// ranger (ni identifiant, ni firmware) ; le drone est créé s'il est nouveau.
pub fn apply_reader_choice(lib: &mut Library, choice: &DroneChoice, today: &str) -> Result<String, String> {
    let mut drone = resolve_choice(lib, choice, today)?;
    drone.last_seen = today.to_string();
    let id = drone.id.clone();
    lib.put_drone(drone);
    Ok(id)
}

/// La proposition pour une carte SD venue d'un lecteur : le drone du nom lu dans le log (`craft`), sinon un nouveau
/// drone de ce nom (vide : le pilote le donne).
pub fn reader_choice(lib: &Library, craft: &str) -> DroneChoice {
    let name = craft.trim();
    match (!name.is_empty()).then(|| lib.drone_by_name(name)).flatten() {
        Some(d) => DroneChoice::Existing(d.id.clone()),
        None => DroneChoice::New(name.to_string()),
    }
}

/// Le drone désigné par `choice`, créé (sans l'écrire) s'il est nouveau. Un nom vide ou déjà pris est refusé.
fn resolve_choice(lib: &Library, choice: &DroneChoice, today: &str) -> Result<Drone, String> {
    Ok(match choice {
        DroneChoice::Existing(id) => lib
            .drone(id)
            .cloned()
            .ok_or_else(|| "Ce drone n'existe plus dans la bibliothèque.".to_string())?,
        DroneChoice::New(name) => {
            let name = name.trim();
            if name.is_empty() {
                return Err("Donnez un nom au nouveau drone.".into());
            }
            if lib.drone_by_name(name).is_some() || lib.drone(name).is_some() {
                return Err(format!(
                    "Un drone s'appelle déjà « {name} » : choisissez-le, ou un autre nom."
                ));
            }
            let mut d = Drone::new(name, name);
            d.first_seen = today.to_string();
            d
        }
    })
}

/// Où copier un disque de carte (lot 32c) : `logs/<drone>/<instant>.bbl` pour la mémoire, et pour chaque log d'une
/// carte SD `logs/<drone>/<instant>_<LOG00001>.bbl` ; avec les copies **faites par OpenTuneBox** déjà rangées, par
/// empreinte. ⚠️ Seulement les copies (`LogEntry::copied`) : un log ouvert depuis la carte elle-même n'est pas un double.
pub fn disk_plan(
    lib: &Library,
    disk: &otb_usb::Disk,
    drone_name: &str,
    stamp: &str,
) -> Result<crate::usb_disk::DiskPlan, String> {
    let path = |stamp: &str| match lib.copy_path(drone_name, stamp) {
        None => Err("Bibliothèque sans dossier : rien ne peut y être copié.".to_string()),
        Some(Err(e)) => Err(format!("Dossier des copies impossible à créer : {e}.")),
        Some(Ok(p)) => Ok(p),
    };
    let dests = match &disk.kind {
        otb_usb::DiskKind::Flash { .. } => vec![path(stamp)?],
        otb_usb::DiskKind::SdCard { logs } => logs
            .iter()
            .map(|f| {
                let stem = f
                    .path
                    .file_stem()
                    .map_or("log".into(), |s| s.to_string_lossy().into_owned());
                path(&format!("{stamp}_{stem}"))
            })
            .collect::<Result<_, _>>()?,
    };
    let existing = lib
        .logs()
        .iter()
        .filter(|l| l.copied)
        .map(|l| (l.id.clone(), PathBuf::from(&l.path)))
        .collect();
    Ok(crate::usb_disk::DiskPlan { dests, existing })
}

// ---------------------------------------------------------------- le rangement

/// Range une copie vérifiée dans la bibliothèque, **sous le drone choisi** : un log marqué copié, et un fragment par
/// vol, comme à l'ouverture d'un fichier. `false` si cette copie y était déjà (même empreinte : rien ne change).
pub fn register_copy(lib: &mut Library, c: &Copied, drone_id: &str, today: &str) -> bool {
    let path = &c.report.path;
    let added = lib.add_log(LogEntry {
        id: c.fingerprint.clone(),
        drone_id: drone_id.to_string(),
        name: path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned()),
        path: path.display().to_string(),
        copied: true,
        bytes: c.report.bytes,
        added: today.to_string(),
    });
    if !added {
        return false;
    }
    for s in &c.sessions {
        let id = otb_store::link::session_fragment_id(&c.fingerprint, s.session);
        if lib.fragment(&id).is_none() {
            lib.put_fragment(Fragment {
                id,
                log_id: c.fingerprint.clone(),
                session: s.session,
                start: s.start,
                end: s.end,
                name: s.name.clone(),
                role: String::new(),
                date: today.to_string(),
                chosen: false,
            });
        }
    }
    true
}

// ---------------------------------------------------------------- textes

/// « 7,8 Mo » d'un nombre d'octets (Mo de 1 048 576 octets, une décimale, virgule).
pub fn size_text(bytes: u64) -> String {
    let mo = bytes as f64 / 1_048_576.0;
    format!("{} Mo", format!("{mo:.1}").replace('.', ","))
}

/// Débit mesuré sur la carte G474 (3,25 Mo par minute, `docs/spec-usb.md` §2) : sert à **annoncer** la durée
/// avant de copier ; pendant la copie, l'écran donne le temps restant au débit réel.
pub const MSP_MO_PER_MIN: f64 = 3.25;

/// « ≈ 3 min » (au moins 1 min) pour copier `bytes` au débit `mo_per_min`.
pub fn duration_text(bytes: u64, mo_per_min: f64) -> String {
    let min = (bytes as f64 / 1_048_576.0 / mo_per_min.max(1e-6)).ceil().max(1.0);
    format!("≈ {min:.0} min")
}

/// `AAAA-MM-JJ_HH-MM-SS` de l'instant présent (temps universel) : le nom d'une copie.
pub fn stamp_now() -> String {
    crate::advice::now_iso()
        .trim_end_matches('Z')
        .replace('T', "_")
        .replace(':', "-")
}
