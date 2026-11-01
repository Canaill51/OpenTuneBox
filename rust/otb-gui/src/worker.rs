//! Calculs en arrière-plan pour ne pas figer l'interface (port de `gui/worker.py` et de la file d'attente de
//! `ToolWindow.run`).
//!
//! Au plus un calcul à la fois par outil. Une demande arrivée pendant un calcul est gardée (la dernière
//! remplace les précédentes) et lancée dès que le calcul en cours finit : il n'y a jamais de calcul en trop,
//! et le résultat final correspond toujours aux derniers réglages.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

type Job<R> = Box<dyn FnOnce() -> Result<R, String> + Send + 'static>;

pub struct Runner<R: Send + 'static> {
    running: Option<Receiver<Result<R, String>>>,
    pending: Option<Job<R>>,
}

impl<R: Send + 'static> Default for Runner<R> {
    fn default() -> Self {
        Self {
            running: None,
            pending: None,
        }
    }
}

fn spawn<R: Send + 'static>(ctx: &egui::Context, job: Job<R>) -> Receiver<Result<R, String>> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        // une panique du calcul finit dans la barre d'état, comme une exception Python
        let res = catch_unwind(AssertUnwindSafe(job)).unwrap_or_else(|p| {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panique sans message".into());
            Err(format!("panique : {msg}"))
        });
        let _ = tx.send(res);
        ctx.request_repaint(); // réveille l'interface pour qu'elle lise le résultat
    });
    rx
}

impl<R: Send + 'static> Runner<R> {
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// Une demande attend la fin du calcul en cours.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Lance le calcul, ou le met en attente s'il y en a déjà un.
    pub fn request(&mut self, ctx: &egui::Context, job: impl FnOnce() -> Result<R, String> + Send + 'static) {
        if self.running.is_some() {
            self.pending = Some(Box::new(job));
        } else {
            self.running = Some(spawn(ctx, Box::new(job)));
        }
    }

    /// Résultat d'un calcul terminé, s'il y en a un ; lance alors la demande en attente.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<Result<R, String>> {
        let rx = self.running.as_ref()?;
        let res = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("le calcul s'est arrêté sans résultat".into()),
        };
        self.running = self.pending.take().map(|job| spawn(ctx, job));
        Some(res)
    }
}
