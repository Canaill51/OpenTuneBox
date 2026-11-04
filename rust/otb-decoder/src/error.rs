//! Erreurs de décodage : un fichier malformé ou hostile est refusé avec un message, jamais cru sur parole.

use std::fmt;

/// Pourquoi une session n'a pas pu être décodée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Un réglage de l'en-tête sort des bornes admises (D-044) : `what` le nomme, `value` est la valeur annoncée.
    HeaderOutOfBounds {
        session: usize,
        what: &'static str,
        value: i64,
    },
    /// La mémoire disponible ne suffit pas pour ranger les trames de la session.
    OutOfMemory { session: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::HeaderOutOfBounds { session, what, value } => write!(
                f,
                "session {session} : l'en-tête annonce {what} = {value}, valeur invraisemblable ; fichier refusé"
            ),
            DecodeError::OutOfMemory { session } => {
                write!(f, "session {session} : fichier trop grand pour la mémoire disponible")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Erreur interne de `decode_stream` : la mémoire manque (devient `DecodeError::OutOfMemory` avec le numéro de session).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfMemory;
