//! Sauvegarde de la configuration par le CLI : `diff all` (`docs/spec-usb.md`, « Sauvegarde de la configuration »).
//!
//! Lecture seule : `diff all` affiche les réglages qui diffèrent des valeurs par défaut, sans rien enregistrer.
//! ⚠️ Pour sortir du CLI, `exit` **redémarre la carte** et ferme le port ; `exit noreboot` n'existe que depuis
//! 2024-09 et laisse l'armement bloqué jusqu'au prochain redémarrage. La sauvegarde se fait donc **en dernier**, et
//! [`Msp::diff_all`] consomme la connexion.

use std::time::{Duration, Instant};

use crate::client::{Msp, MspError};
use crate::link::Link;

/// L'invite du CLI (`cliPrompt`). ⚠️ La sortie de `diff all` contient aussi des lignes `# version`, `# master`… :
/// la fin se reconnaît à l'invite **suivie d'un silence**, pas à l'invite seule.
const PROMPT: &[u8] = b"\r\n# ";
const QUIET: Duration = Duration::from_millis(250);
const ENTER_LIMIT: Duration = Duration::from_secs(3);
const DIFF_LIMIT: Duration = Duration::from_secs(20);

/// Numéro réservé dans les erreurs du CLI, qui n'a pas de commande MSP.
const CLI: u16 = 0;

impl<L: Link> Msp<L> {
    /// Entre dans le CLI, lit `diff all`, puis sort par `exit` (la carte redémarre). Rend le texte, lignes en `\n`.
    pub fn diff_all(mut self) -> Result<String, MspError> {
        self.link.send(b"#")?;
        read_until_prompt(&mut self.link, ENTER_LIMIT)?;
        self.link.send(b"diff all\n")?;
        let raw = read_until_prompt(&mut self.link, DIFF_LIMIT)?;
        // La carte redémarre : une erreur ici ne change rien à ce qui a été lu.
        let _ = self.link.send(b"exit\n");
        let text = clean_diff(&raw);
        if !text.lines().any(|l| l.starts_with("# version")) {
            return Err(MspError::Malformed {
                cmd: CLI,
                why: "la réponse à « diff all » ne commence pas par « # version »".into(),
            });
        }
        Ok(text)
    }
}

/// Lit jusqu'à l'invite suivie d'un silence de [`QUIET`].
fn read_until_prompt<L: Link>(link: &mut L, limit: Duration) -> Result<Vec<u8>, MspError> {
    let start = Instant::now();
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = link.recv(&mut buf, QUIET)?;
        out.extend_from_slice(&buf[..n]);
        if n == 0 && out.ends_with(PROMPT) {
            return Ok(out);
        }
        if start.elapsed() > limit {
            return Err(MspError::Timeout { cmd: CLI });
        }
    }
}

/// Retire l'écho de la commande et l'invite finale ; fins de ligne en `\n`. Fonction pure.
pub fn clean_diff(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw).replace("\r\n", "\n").replace('\r', "\n");
    let body = text.strip_suffix("\n# ").unwrap_or(&text);
    let mut lines: Vec<&str> = body.lines().collect();
    // L'écho des caractères tapés (`cliWrite` en mode interactif).
    if lines.first().is_some_and(|l| l.trim() == "diff all") {
        lines.remove(0);
    }
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

/// La configuration a-t-elle changé depuis la sauvegarde précédente ? Les lignes vides et les espaces de fin ne
/// comptent pas. Une sauvegarde identique n'est pas gardée (lot 32).
pub fn config_changed(previous: Option<&str>, new: &str) -> bool {
    fn norm(s: &str) -> Vec<&str> {
        s.lines().map(str::trim_end).filter(|l| !l.is_empty()).collect()
    }
    previous.is_none_or(|p| norm(p) != norm(new))
}
