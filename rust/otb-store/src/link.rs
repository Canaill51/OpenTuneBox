//! Le lien entre un fragment enregistré et un log ouvert.
//!
//! Un fragment garde des **numéros d'échantillon**, pas des secondes : c'est ce que les outils attendent
//! (`sel: Range<usize>`) et cela ne bouge pas quand on change de fenêtre d'analyse. Mais un log peut avoir été
//! déplacé, remplacé ou tronqué depuis : ces fonctions disent **clairement** ce qui ne va pas plutôt que de
//! rendre un morceau de vol silencieusement faux.
//!
//! Aucune dépendance au décodeur ni à l'analyse : l'appelant passe le nombre d'échantillons du log
//! (`FlightLog::n()`). C'est ce qui garde la bibliothèque utilisable en ligne de commande comme dans
//! l'interface, et testable sans ouvrir de fichier.

use crate::model::Fragment;
use std::ops::Range;

/// La période d'un fragment dans un log de `samples` échantillons.
///
/// `Err` avec un message en français si le fragment ne peut pas s'appliquer à ce log : bornes à l'envers, début
/// hors du log, ou fin au-delà de ce que le fichier contient (log remplacé ou tronqué). Le message nomme le
/// fragment et les deux nombres, parce qu'il finit dans la barre d'état.
pub fn range_of(fragment: &Fragment, samples: usize) -> Result<Range<usize>, String> {
    let nom = if fragment.name.is_empty() {
        &fragment.id
    } else {
        &fragment.name
    };
    if fragment.end <= fragment.start {
        return Err(format!(
            "fragment « {nom} » : bornes impossibles ({} à {})",
            fragment.start, fragment.end
        ));
    }
    let (start, end) = (fragment.start as usize, fragment.end as usize);
    if start >= samples {
        return Err(format!(
            "fragment « {nom} » : il commence à l'échantillon {start}, or le log n'en a que {samples} — \
             le fichier a changé depuis l'enregistrement du fragment"
        ));
    }
    if end > samples {
        return Err(format!(
            "fragment « {nom} » : il finit à l'échantillon {end}, or le log n'en a que {samples} — \
             le fichier a changé depuis l'enregistrement du fragment"
        ));
    }
    Ok(start..end)
}

/// Le nombre d'échantillons d'un fragment, sans ouvrir son log.
pub fn length(fragment: &Fragment) -> u64 {
    fragment.end.saturating_sub(fragment.start)
}

/// Deux fragments se chevauchent-ils dans le même log ? Sert à prévenir l'utilisateur, jamais à refuser : deux
/// analyses du même passage sont légitimes.
pub fn overlaps(a: &Fragment, b: &Fragment) -> bool {
    a.log_id == b.log_id && a.session == b.session && a.start < b.end && b.start < a.end
}

/// Identifiant du **fragment d'une session** : l'empreinte du fichier et le numéro de session. Il est créé à chaque
/// ouverture du fichier et revient même supprimé ; c'est lui que l'ouverture d'un fichier met en analyse.
pub fn session_fragment_id(log_id: &str, session: u64) -> String {
    format!("{log_id}-{session}")
}

/// Le fragment a-t-il été **ajouté** à sa session dans l'écran d'extraction (« Enregistrer comme nouveau
/// fragment ») ? Seuls ceux-là se suppriment : le fragment de la session reviendrait à la prochaine ouverture.
pub fn is_added(fragment: &Fragment) -> bool {
    fragment.id != session_fragment_id(&fragment.log_id, fragment.session)
}

/// Premier identifiant libre pour un fragment ajouté à la session `session` du log `log_id` :
/// `<log>-<session>-2`, puis `-3`… Un numéro libéré par une suppression peut resservir — sans risque, ses mesures
/// et sa place dans la liste d'analyse sont parties avec lui.
pub fn added_fragment_id<'a>(log_id: &str, session: u64, taken: impl IntoIterator<Item = &'a Fragment>) -> String {
    let base = session_fragment_id(log_id, session);
    let pris: std::collections::HashSet<&str> = taken.into_iter().map(|f| f.id.as_str()).collect();
    (2..)
        .map(|k| format!("{base}-{k}"))
        .find(|id| !pris.contains(id.as_str()))
        .expect("une suite infinie a toujours un numéro libre")
}
