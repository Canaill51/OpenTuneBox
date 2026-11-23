//! Récupération par le mode stockage de masse, côté interface (lot 32c, D-075) : un **fil de travail** qui attend le
//! disque de la carte, le copie, garde les copies vérifiées, et supprime les logs d'une carte SD sur demande. L'écran
//! est dans `usb_page.rs` ; la connexion MSP (avant le redémarrage, et après le rebranchement) dans `usb.rs`.
//!
//! ⚠️ **Mémoire interne** : la copie vérifiée ne quitte ce fil que pour la boîte [`Held`], d'où seule une connexion MSP
//! à la même carte la sort pour effacer. **Carte SD** : les copies vérifiées restent ici ; seule la demande
//! `DiskRequest::EraseSd` en fait un permis (`otb_usb::confirm_sd`) pour `otb_usb::delete_sd_logs`, qui relit chaque
//! fichier juste avant de le supprimer. L'écran ne la propose qu'une fois les copies rangées et la bibliothèque écrite.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use otb_usb::erase::verify_and_save;
use otb_usb::msc::{DiskKind, copy_flash_disk, inspect};
use otb_usb::{BeforeDisk, Disk, SdCopy, Storage, confirm_sd, copy_sd_log, delete_sd_logs};

use crate::usb::{Copied, Held, describe};

/// Les volumes montés (le vrai relevé du système, ou un dossier des tests).
pub type Volumes = Arc<dyn Fn() -> Vec<PathBuf> + Send + Sync>;

/// Le vrai relevé : `otb_usb::msc::mounted_volumes`.
pub fn system_volumes() -> Volumes {
    Arc::new(otb_usb::msc::mounted_volumes)
}

/// Délai entre deux relevés des volumes pendant l'attente du disque.
const POLL: Duration = Duration::from_millis(500);

/// D'où vient le disque.
#[derive(Clone, Debug, PartialEq)]
pub enum DiskSource {
    /// Une carte redémarrée en disque : ce qu'elle a dit d'elle, et les volumes montés avant (le sien sera nouveau).
    Board(BeforeDisk, Vec<PathBuf>),
    /// Une carte SD déjà montée, dans un lecteur de cartes : pas d'identifiant, pas d'attente.
    Reader(Disk),
}

/// Où écrire les copies : une par log de la carte SD (dans l'ordre de `DiskKind::SdCard`), une seule pour la mémoire.
/// `existing` : empreinte → copie déjà rangée dans la bibliothèque (comparée au lieu d'en écrire une seconde).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiskPlan {
    pub dests: Vec<PathBuf>,
    pub existing: HashMap<String, PathBuf>,
}

/// Ce que l'écran demande au fil.
#[derive(Clone, Debug, PartialEq)]
pub enum DiskRequest {
    /// Le pilote désigne le disque à la main (« Choisir le dossier… »).
    Folder(PathBuf),
    Copy(DiskPlan),
    /// Supprimer de la carte SD les logs copiés et vérifiés.
    EraseSd,
}

/// Ce que le fil rend à l'écran.
#[derive(Clone, Debug, PartialEq)]
pub enum DiskEvent {
    Found(Disk),
    /// Le dossier choisi n'est pas le disque attendu (la raison).
    NotThisDisk(String),
    Progress {
        done: u64,
        total: u64,
    },
    /// Une copie vérifiée (pour la mémoire : elle attend la carte dans `Held`).
    Copied(Copied),
    /// Un log refusé par les contrôles : il ne sera pas supprimé (le texte dit pourquoi).
    Refused(String),
    /// Fin de la copie (arrêtée ou non).
    Done,
    /// Logs supprimés de la carte SD, et ceux restés (avec la raison).
    SdErased {
        deleted: usize,
        kept: Vec<String>,
    },
    Failed(String),
}

/// Le fil du disque.
pub struct DiskJob {
    tx: Sender<DiskRequest>,
    rx: Receiver<DiskEvent>,
    cancel: Arc<AtomicBool>,
    busy: Arc<Mutex<bool>>,
}

impl DiskJob {
    /// Lance le fil : pour une carte, il attend son disque (`Found`) ; pour un lecteur, le disque est déjà là.
    pub fn start(source: DiskSource, volumes: Volumes, held: Held, ctx: egui::Context) -> Self {
        let (tx, rx_req) = channel();
        let (tx_ev, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(Mutex::new(false));
        let (c, b) = (cancel.clone(), busy.clone());
        std::thread::spawn(move || {
            let send = |e: DiskEvent| {
                let _ = tx_ev.send(e);
                ctx.request_repaint();
            };
            run(source, &volumes, &held, &rx_req, &send, &c, &b);
        });
        DiskJob { tx, rx, cancel, busy }
    }

    pub fn request(&self, r: DiskRequest) {
        self.cancel.store(false, Ordering::Relaxed);
        if !matches!(r, DiskRequest::Folder(_)) {
            *self.busy.lock().unwrap() = true;
        }
        let _ = self.tx.send(r);
    }

    /// Arrête la copie : le log en cours n'est pas gardé ; ceux déjà vérifiés restent.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn busy(&self) -> bool {
        *self.busy.lock().unwrap()
    }

    pub fn poll(&self) -> Vec<DiskEvent> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }
}

/// Le disque d'une carte parmi les volumes apparus depuis `before`, s'il n'y en a qu'un.
fn find_board_disk(before: &BeforeDisk, old: &[PathBuf], now: Vec<PathBuf>) -> Result<Option<Disk>, String> {
    let found: Vec<Disk> = otb_usb::msc::find_disks(now.into_iter().filter(|v| !old.contains(v)))
        .into_iter()
        .filter(|d| before.matches(d))
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.into_iter().next()),
        _ => Err("Plusieurs disques de carte sont apparus : choisissez le bon dossier.".into()),
    }
}

fn run(
    source: DiskSource,
    volumes: &Volumes,
    held: &Held,
    rx: &Receiver<DiskRequest>,
    send: &dyn Fn(DiskEvent),
    cancel: &AtomicBool,
    busy: &Mutex<bool>,
) {
    let done = || *busy.lock().unwrap() = false;
    // 1. Le disque.
    let (before, disk) = match source {
        DiskSource::Reader(d) => (None, d),
        DiskSource::Board(before, old) => {
            let mut told = false;
            let disk = loop {
                match find_board_disk(&before, &old, volumes()) {
                    Ok(Some(d)) => break d,
                    Ok(None) => {}
                    Err(e) if !told => {
                        told = true;
                        send(DiskEvent::NotThisDisk(e));
                    }
                    Err(_) => {}
                }
                match rx.recv_timeout(POLL) {
                    Ok(DiskRequest::Folder(p)) => match inspect(&p) {
                        Some(d) if before.matches(&d) => break d,
                        Some(_) => send(DiskEvent::NotThisDisk(format!(
                            "{} n'est pas le disque de cette carte (taille ou genre différents).",
                            p.display()
                        ))),
                        None => send(DiskEvent::NotThisDisk(format!(
                            "{} n'est pas un disque de carte Betaflight (ni BTFL_ALL.BBL, ni dossier LOGS).",
                            p.display()
                        ))),
                    },
                    Ok(_) => done(),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            };
            (Some(before), disk)
        }
    };
    send(DiskEvent::Found(disk.clone()));
    // 2. Les demandes : copie, puis suppression (carte SD).
    let mut sd_copies: Vec<SdCopy> = Vec::new();
    while let Ok(req) = rx.recv() {
        match req {
            DiskRequest::Folder(_) => {}
            DiskRequest::Copy(plan) => {
                sd_copies.clear();
                held.clear();
                copy(before.as_ref(), &disk, &plan, held, &mut sd_copies, send, cancel);
                send(DiskEvent::Done);
            }
            DiskRequest::EraseSd => {
                if sd_copies.is_empty() {
                    send(DiskEvent::Failed(
                        "Aucun log de la carte SD copié et vérifié : rien n'est supprimé.".into(),
                    ));
                } else {
                    let r = delete_sd_logs(confirm_sd(std::mem::take(&mut sd_copies)));
                    send(DiskEvent::SdErased {
                        deleted: r.deleted.len(),
                        kept: r
                            .kept
                            .iter()
                            .map(|(p, why)| format!("{} : {why}", file_name(p)))
                            .collect(),
                    });
                }
            }
        }
        done();
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Copie le disque selon `plan` : la mémoire en une copie (gardée dans `held`), ou chaque log de la carte SD.
fn copy(
    before: Option<&BeforeDisk>,
    disk: &Disk,
    plan: &DiskPlan,
    held: &Held,
    sd_copies: &mut Vec<SdCopy>,
    send: &dyn Fn(DiskEvent),
    cancel: &AtomicBool,
) {
    match &disk.kind {
        DiskKind::Flash { .. } => {
            let (Some(before), Some(dest)) = (before, plan.dests.first()) else {
                return send(DiskEvent::Failed("Copie de la mémoire sans carte identifiée.".into()));
            };
            let Storage::Flash(summary) = before.storage else {
                return send(DiskEvent::Failed("Ce disque n'est pas la mémoire de la carte.".into()));
            };
            let copy = match copy_flash_disk(before, disk, |done, total| {
                send(DiskEvent::Progress { done, total });
                !cancel.load(Ordering::Relaxed)
            }) {
                Ok(c) => c,
                Err(e) => return send(DiskEvent::Failed(e.to_string())),
            };
            let v = match verify_and_save(copy, dest) {
                Ok(v) => v,
                Err(e) => return send(DiskEvent::Refused(e.to_string())),
            };
            let report = v.report().clone();
            match describe(&report.path) {
                Ok((fingerprint, sessions)) => {
                    held.put(v, before.identity.chip_id, summary.used);
                    send(DiskEvent::Copied(Copied {
                        report,
                        fingerprint,
                        sessions,
                    }));
                }
                Err(e) => send(DiskEvent::Refused(e)),
            }
        }
        DiskKind::SdCard { logs } => {
            let total: u64 = logs.iter().map(|f| f.len).sum();
            let mut before_this = 0;
            for (file, dest) in logs.iter().zip(&plan.dests) {
                let name = file_name(&file.path);
                let r = copy_sd_log(
                    file,
                    dest,
                    |bytes| plan.existing.get(&otb_store::fingerprint(bytes)).cloned(),
                    |n, _| {
                        send(DiskEvent::Progress {
                            done: before_this + n,
                            total,
                        });
                        !cancel.load(Ordering::Relaxed)
                    },
                );
                before_this += file.len;
                match r {
                    Ok(c) => match describe(&c.report().path) {
                        Ok((fingerprint, sessions)) => {
                            send(DiskEvent::Copied(Copied {
                                report: c.report().clone(),
                                fingerprint,
                                sessions,
                            }));
                            sd_copies.push(c);
                        }
                        Err(e) => send(DiskEvent::Refused(format!("{name} : {e}"))),
                    },
                    Err(e) if matches!(e.reason, otb_usb::erase::CheckFailure::Cancelled) => {
                        return send(DiskEvent::Refused(format!("{name} : {e}")));
                    }
                    Err(e) => send(DiskEvent::Refused(format!("{name} : {e}"))),
                }
            }
        }
    }
}

/// Le nom du drone lu dans l'en-tête du premier log d'une carte SD (`H Craft name:`), pour le proposer quand la carte
/// vient d'un lecteur (pas d'identifiant). Vide s'il n'y en a pas.
pub fn craft_name(disk: &Disk) -> String {
    let DiskKind::SdCard { logs } = &disk.kind else {
        return String::new();
    };
    let Some(first) = logs.first() else {
        return String::new();
    };
    let mut head = vec![0u8; 64 * 1024];
    let n = std::fs::File::open(&first.path)
        .and_then(|mut f| std::io::Read::read(&mut f, &mut head))
        .unwrap_or(0);
    String::from_utf8_lossy(&head[..n])
        .lines()
        .find_map(|l| l.strip_prefix("H Craft name:"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}
