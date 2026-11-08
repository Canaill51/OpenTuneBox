//! Parcours de réglage du guide intégré (`advisor/tuning_path.py`) : 4 phases, des cases à cocher, et leur état d'après les logs.
//!
//! Python garde des fonctions de la fiche dans le titre et l'aide d'une case (`Step.title`, `Step.help`) ; ici `phases(card)` les résout
//! tout de suite pour la fiche donnée : le texte rendu par `Step.text(card)` et `Step.help_text(card)` est celui de `Step::title` et
//! `Step::help`.

use crate::drone_card::{DYN_NOTCH_DEFAULTS, DroneCard, FF_SERIES, MASTER_SERIES, RPM_DEFAULTS};
use crate::log_check::{
    BLOCK, FILTERS, FilterTargets, LOG_ANGLE_LIMIT, LOG_RATE_DPS, LogCheck, OK, PID, WARN, configurator_slider,
};
use otb_format::fixed;

pub const AUTO_OK: &str = "ok";
pub const AUTO_BAD: &str = "bad";

/// `_r` de Python : un chiffre après la virgule, virgule française.
fn r(x: f64) -> String {
    fixed(x, 1, false).replace('.', ",")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub key: String,
    pub title: String,
    /// Markdown.
    pub help: String,
    /// Points de `log_check` qui doivent être bons.
    pub checks: Vec<&'static str>,
    /// Usages de `log_check` qui doivent être possibles.
    pub usages: Vec<&'static str>,
    /// Vérifiée dès qu'un log du drone est ouvert.
    pub any_log: bool,
    /// (catégorie PTB du curseur, valeur).
    pub flight: Option<(&'static str, f64)>,
    /// Usage de `log_check` qui rend le vol lisible.
    pub flight_usage: &'static str,
    /// Outil à ouvrir (clé de `gui/tools` REGISTRY).
    pub tool: Option<&'static str>,
    /// Partie du vol à analyser avec l'outil : « hover » ou « moving ».
    pub part: Option<&'static str>,
    /// Repères de fin de la phase Filtres.
    pub targets: bool,
    /// Rôle des logs lus par la case : `FILTERS`, `PID` ou vide (tous).
    pub logs: &'static str,
    /// Bouton « Ouvrir les logs » de ce rôle.
    pub open_logs: bool,
}

impl Step {
    fn new(key: &str, title: impl Into<String>, help: impl Into<String>) -> Step {
        Step {
            key: key.to_string(),
            title: title.into(),
            help: help.into(),
            checks: Vec::new(),
            usages: Vec::new(),
            any_log: false,
            flight: None,
            flight_usage: "series",
            tool: None,
            part: None,
            targets: false,
            logs: "",
            open_logs: false,
        }
    }

    fn checks(mut self, c: &[&'static str]) -> Step {
        self.checks = c.to_vec();
        self
    }

    fn usages(mut self, u: &[&'static str]) -> Step {
        self.usages = u.to_vec();
        self
    }

    fn any_log(mut self) -> Step {
        self.any_log = true;
        self
    }

    fn tool(mut self, tool: &'static str, part: Option<&'static str>) -> Step {
        self.tool = Some(tool);
        self.part = part;
        self
    }

    fn logs(mut self, role: &'static str) -> Step {
        self.logs = role;
        self
    }

    fn open_logs(mut self) -> Step {
        self.open_logs = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase {
    pub key: &'static str,
    pub title: &'static str,
    /// Markdown.
    pub intro: &'static str,
    pub steps: Vec<Step>,
}

// ------------------------------------------------------------------ textes adaptés à la fiche
fn khz(hz: f64, frac: &str) -> String {
    format!("{} kHz ({frac})", r(hz / 1000.0))
}

fn recording(c: &DroneCard) -> String {
    let (hz, frac) = c.log_rate();
    let (pid_hz, pid_frac) = c.pid_log_rate();
    format!(
        "Onglet Blackbox : fréquence {} pour les filtres, {} pour les séries PID, mode débug {}",
        khz(hz, &frac),
        khz(pid_hz, &pid_frac),
        c.debug_mode()
    )
}

fn recording_help(c: &DroneCard) -> String {
    let (hz, frac) = c.log_rate();
    let (pid_hz, pid_frac) = c.pid_log_rate();
    let unfilt = if c.bf == "4.4" {
        "En 4.4, GYRO_SCALED est le seul moyen d'avoir le gyro non filtré."
    } else {
        "« Champs de débogage inclus » : laissez **Gyro (Unfiltered)** coché."
    };
    format!(
        "Deux fréquences, selon le vol. **Filtres : {} Hz.** Le blackbox garde un échantillon sur N sans filtrer : tout bruit plus rapide que la moitié de la fréquence d'enregistrement crée de fausses raies ; il faut donc voir tout le bruit des moteurs (boucle PID à {} kHz, drone de cette taille). **Séries PID : {} Hz** suffisent pour mesurer la réponse à la milliseconde près, et la mémoire garde plus de vols. {unfilt}\n\nCLI : `set blackbox_sample_rate = {frac}` (filtres) ou `{pid_frac}` (séries PID), puis `set debug_mode = {}`, `save`.\n\nBoucle PID inconnue : onglet **Configuration**, « Fréquence de la boucle PID » ; type de gyro : commande `status` du CLI (ligne GYRO).",
        fixed(hz, 0, false),
        r(c.pid_khz),
        fixed(pid_hz, 0, false),
        c.debug_mode()
    )
}

fn bidir(c: &DroneCard) -> &'static str {
    if c.bidir() {
        "DShot bidirectionnel activé, pôles moteur justes"
    } else {
        "ESC BLHeli_S d'origine : flasher Bluejay pour le DShot bidirectionnel (sinon pas de filtre RPM)"
    }
}

fn defaults_help(c: &DroneCard) -> String {
    let (rpm, dn) = (&RPM_DEFAULTS, &DYN_NOTCH_DEFAULTS);
    format!(
        "On règle à partir d'un point connu. Onglet **Réglages de PID** : **Réinitialiser ce profil** (PID, curseurs, filtres du D). Les autres filtres sont des réglages généraux, à remettre à la main dans la partie filtres du même onglet :\n\n- « Multiplicateur du Filtre Gyro » et « Multiplicateur de filtre D Term » : **1,0** ;\n- Filtre Gyro RPM : harmoniques {}, Q {}, fréquence min {} Hz{} ;\n- Filtre coupe bande dynamique : {} coupures, Q {}, {} à {} Hz.\n\nRécepteur, modes, OSD et VTX ne sont pas touchés.",
        rpm.harmonics,
        rpm.q,
        rpm.min_hz,
        if c.bf != "4.4" { ", poids 100 / 100 / 100" } else { "" },
        dn.count,
        dn.q,
        dn.min_hz,
        dn.max_hz
    )
}

fn log_rates_help(c: &DroneCard) -> String {
    format!(
        "Les mouvements de réglage se font en **mode Angle**, ou en **Acro avec le feedforward à 0**, avec des rates linéaires : la réponse est la même, le mode Angle rend les mouvements plus faciles à doser (le firmware y coupe le feedforward). Onglet **Réglages de PID**, choisissez un profil de rates libre (ex. 3) et nommez-le « LOG » : « Type de rates » ACTUAL, « Sensibilité au centre » {LOG_RATE_DPS}, « Max Vel » {LOG_RATE_DPS}, Expo 0, sur les trois axes. Puis « Limite de l'Angle » : **{LOG_ANGLE_LIMIT}** (elle appartient au profil PID, et ne sert qu'en mode Angle).\n\nCLI : `set rates_type = ACTUAL`, `set roll_rc_rate = {}`, `set roll_srate = {}`, `set roll_expo = 0` (idem pitch et yaw), `set {} = {LOG_ANGLE_LIMIT}`.",
        LOG_RATE_DPS / 10,
        LOG_RATE_DPS / 10,
        c.angle_limit_setting()
    )
}

const FLIGHT_HELP: &str = "Un seul vol, en deux parties :\n\n1. **20 à 30 s de stationnaire**, avec des montées lentes des gaz de 0 à 100 %, sans manœuvre brusque : on y repère le bruit (moteurs, châssis).\n2. **20 à 30 s de wobbles** : mode Angle (ou Acro avec le feedforward à 0), profil de rates « LOG », mouvements **au hasard sur le roll et le pitch en même temps**, rapides, sans s'arrêter : on y mesure le retard des filtres.\n\nOpenTuneBox trouve tout seul les deux parties.";

pub const SERIES_FLIGHT_HELP: &str = "Mode Angle (ou Acro avec le feedforward à 0), profil « LOG », au moins **20 s de wobbles** au hasard sur le roll et le pitch en même temps (au moins 200 fenêtres de mesure). Même batterie, même charge. Au moindre bruit d'oscillation, atterrissez et revenez à la valeur précédente ; touchez les moteurs après chaque vol.";
pub const FF_FLIGHT_HELP: &str = "**Acro uniquement** (le firmware coupe le feedforward en mode Angle), profil « LOG », au moins **20 s de wobbles** au hasard sur le roll et le pitch en même temps. Même batterie, même charge, seul le curseur « Réponse des sticks » change.";

/// `_series` de Python : un vol par valeur du curseur.
fn series(cat: &'static str, values: &[f64], first_help: &str, flight_help: &str, usage: &'static str) -> Vec<Step> {
    let name = configurator_slider(cat);
    values
        .iter()
        .enumerate()
        .map(|(k, &v)| {
            let help = format!(
                "{}Curseur à {}, Sauvegarder, puis voler. \n\n{flight_help}",
                if k == 0 {
                    format!("{first_help}\n\n")
                } else {
                    String::new()
                },
                r(v)
            );
            let mut s = Step::new(
                &format!("{cat}_{}", fixed(v, 1, false)),
                format!("Vol « {name} » à {}", r(v)),
                help,
            )
            .logs(PID);
            s.flight = Some((cat, v));
            s.flight_usage = usage;
            s
        })
        .collect()
}

const FF_HELP: &str = "Le **feedforward** pousse les moteurs dès que le manche bouge, avant que l'erreur n'apparaisse : le drone suit le manche avec moins de retard. Trop haut, le gyro dépasse la consigne à chaque coup de manche. Série par pas de 0,5, en **Acro** : pour chaque valeur, l'Assistant donne le **retard consigne → gyro** en roll et en pitch (la Réponse indicielle, faussée par le feedforward, ne sert pas ici).";

fn i_help(c: &DroneCard) -> String {
    let (lo, hi) = c.i_range();
    format!(
        "Le **terme I** corrige les erreurs qui durent : dérive dans le vent, en virage, avec le centre de gravité décalé. Trop bas, le drone dérive ; trop haut, il oscille lentement après un mouvement (dépassement lent et étiré). Plage de départ : **{} à {}** (il manque plus souvent d'I qu'il n'y en a trop).",
        r(lo),
        r(hi)
    )
}

// ------------------------------------------------------------------ parcours
pub fn phases(card: &DroneCard) -> Vec<Phase> {
    let prep = Phase {
        key: "prep",
        title: "Préparer le drone",
        intro: "À faire une fois, avant le premier vol de réglage. Dès qu'un log du drone est ouvert, certaines cases se vérifient toutes seules : « ✔ Vérifié » ou « ▲ À revoir ».",
        steps: vec![
            Step::new(
                "expert",
                "Configurateur : activer le Mode Expert",
                "Fenêtre **Options** du configurateur. Sans lui, les curseurs sont limités à 0,7–1,4 et Master, I, Pitch Damping, Pitch Tracking et Dynamic Damping restent bloqués.",
            ),
            Step::new(
                "backup",
                "Sauvegarder le réglage actuel (diff all)",
                "Onglet **Ligne de commande (CLI)** : tapez `diff all`, puis **Enregistrer sous**. C'est le point de retour si un réglage tourne mal.",
            ),
            Step::new("defaults", "Repartir des valeurs par défaut", defaults_help(card)),
            Step::new(
                "mechanics",
                "Mécanique saine",
                "Hélices neuves et droites, vis du châssis et des moteurs serrées, moteurs qui tournent librement, câbles et antennes fixés. Aucun filtre ne rattrape une hélice tordue.",
            ),
            Step::new(
                "storage",
                "Mémoire blackbox prête",
                "Onglet **Blackbox** : stockage sur carte SD ou mémoire embarquée, vide avant la séance.",
            ),
            Step::new(
                "bidir",
                bidir(card),
                "Onglet **Moteurs** : « DShot bi-directionnel » et « Pôles moteur » (aimants de la cloche, souvent 14). Il active le filtre RPM et donne la vitesse réelle des moteurs.",
            )
            .checks(&["rpm"]),
            Step::new(
                "sliders",
                "Curseurs de réglage PID en mode RPY",
                "Onglet **Réglages de PID**, « Mode » : RPY. Les conseils sont des positions de curseurs.",
            )
            .checks(&["simplified"]),
            Step::new("recording", recording(card), recording_help(card))
                .checks(&["log_rate", "gyro_unfilt", "debug_mode"])
                .logs(FILTERS),
            Step::new(
                "log_rates",
                format!("Profil de rates « LOG » : ACTUAL {LOG_RATE_DPS} / {LOG_RATE_DPS} / 0, limite d'angle {LOG_ANGLE_LIMIT}°"),
                log_rates_help(card),
            )
            .checks(&["rates", "angle_limit"])
            .logs(PID),
            Step::new(
                "switches",
                "Interrupteurs : mode ANGLE et choix du profil de rates",
                "Onglet **Modes** : ANGLE sur un interrupteur (inutile si vous volez en Acro avec le feedforward à 0). Onglet **Ajustements** : « Sélection des profils de Rate » sur un interrupteur ou un potentiomètre, pour passer du profil « LOG » à vos rates habituelles en vol.",
            )
            .checks(&["flight_mode"])
            .logs(PID),
            Step::new(
                "batteries",
                "Mêmes batteries pour tous les vols comparés",
                "Chargées pareil : la tension change la réponse du drone.",
            ),
        ],
    };
    let filters = Phase {
        key: "filters",
        title: "Filtres",
        intro: "Un vol de réglage, puis un changement à la fois, dans l'ordre. Chaque changement : Sauvegarder, refaire le même vol, comparer les deux logs.",
        steps: vec![
            Step::new("filter_flight", "Vol de réglage : stationnaire, puis wobbles", FLIGHT_HELP)
                .usages(&["noise", "delay"])
                .checks(&["log_rate", "gyro_unfilt"])
                .logs(FILTERS),
            Step::new(
                "retrieve",
                "Récupérer le log et l'ouvrir",
                "Onglet **Blackbox** : **Activer le Mode de périphérique de stockage de masse** (le contrôleur apparaît comme une clé USB), ou retirez la carte SD et lisez-la sur l'ordinateur. Copiez les `.bbl` dans un dossier par drone, nommez-les (`filtres-1.bbl`, `damping-0.8.bbl`…), puis videz la mémoire (**Effacer la mémoire interne**). Ouvrez-le avec le bouton de cette case : il sera lu comme log des filtres.",
            )
            .any_log()
            .logs(FILTERS)
            .open_logs(),
            Step::new(
                "mechanics_read",
                "Mécanique : lire Fréquence × gaz (partie stationnaire)",
                "Une **diagonale** qui monte avec les gaz = moteurs (normal, le filtre RPM s'en charge). Une **ligne horizontale** = résonance du châssis ou pièce qui vibre : à corriger mécaniquement avant de toucher aux filtres.",
            )
            .tool("freq_throttle", Some("hover")),
            Step::new(
                "rpm",
                "1. Filtre RPM",
                "Spectre, case **Diagnostic du bruit**, bloc « Filtre RPM » : harmoniques, Q et poids conseillés. Onglet **Réglages de PID** → « Filtre Gyro RPM ».",
            )
            .tool("spectral", Some("hover")),
            Step::new(
                "notch",
                "2. Notch dynamique",
                "Bloc « Notch dynamique » du diagnostic : nombre de coupures, Q, fréquences. Avec un filtre RPM qui fonctionne, 1 ou 2 coupures suffisent souvent. → « Filtre coupe bande dynamique ».",
            )
            .tool("spectral", Some("hover")),
            Step::new(
                "lpf_gyro",
                "3. Passe-bas du gyro : curseur « Multiplicateur du Filtre Gyro »",
                "Vers la droite : moins de filtrage, moins de retard.",
            ),
            Step::new(
                "lpf_dterm",
                "4. Passe-bas du D : curseur « Multiplicateur de filtre D Term »",
                "Même sens. Touchez les moteurs après le vol : tièdes, c'est bon ; chauds, revenez en arrière.",
            ),
            {
                let mut s = Step::new(
                    "filter_targets",
                    "Repères atteints : gyro filtré sous −30 dB, terme D au plus −10 dB, retard sous l'objectif de la taille",
                    "Mesurés sur le dernier log du drone : bruit sur la partie stationnaire (échelle de PIDtoolbox), retard sur la partie avec mouvements, objectif de référence pour la taille. La phase est terminée quand les quatre repères sont atteints.",
                );
                s.targets = true;
                s
            },
        ],
    };
    let mut pid_steps = vec![
        Step::new(
            "series_conditions",
            "Conditions des séries : mode Angle (ou Acro, feedforward à 0), profil « LOG », curseur I à 0,25, Dynamic Damping à 0, Master à 1,0",
            "Curseurs en Mode Expert : « Dérive - Oscillations » 0,25 (le terme I déforme la réponse), « Atténuation dynamique » 0 (le D max change la réponse selon la vitesse du mouvement), « Multiplicateur Maître » 1,0 (0,8 pour un drone très puissant). Le feedforward fausse la réponse : le firmware le coupe en mode Angle ; en Acro, curseur « Réponse des sticks » à 0 (à remonter en finitions).",
        )
        .checks(&["i_slider", "dyn_damping", "flight_mode", "rates"])
        .logs(PID),
        Step::new(
            "pid_recording",
            {
                let (hz, frac) = card.pid_log_rate();
                format!("Onglet Blackbox : fréquence {} pour les séries", khz(hz, &frac))
            },
            "Plus lente que pour les filtres : la réponse se mesure encore à la milliseconde près, et la mémoire garde plus de vols. Remettez la fréquence des filtres pour un nouveau vol de réglage des filtres.",
        )
        .checks(&["pid_log_rate"])
        .logs(PID),
    ];
    pid_steps.extend(series(
        "Damping",
        card.damping_series(),
        "La première série règle l'équilibre P / D. Arrêtez-vous dès que le Diagnostic du réglage a trouvé la valeur (inutile de voler toute la liste).",
        SERIES_FLIGHT_HELP,
        "series",
    ));
    pid_steps.extend([
        Step::new(
            "pid_logs",
            "Ouvrir les logs de la série",
            "Après chaque série, copiez les `.bbl` dans le dossier du drone, puis ouvrez-les avec le bouton de cette case : ils seront lus comme logs de séries PID (ouverts autrement, un log sans partie stationnaire est aussi classé en PID). Ouvrez tous les vols de la série ensemble.",
        )
        .any_log()
        .logs(PID)
        .open_logs(),
        Step::new(
            "damping_read",
            "Lire la série Damping et garder la valeur conseillée",
            "Réponse indicielle → Diagnostic du réglage : première valeur sans dépassement visible sur le roll. Si le pitch dépasse encore, le diagnostic propose une série **Pitch Damping**.",
        )
        .tool("step", None),
        Step::new(
            "pitch_damping",
            "Si demandé : série Pitch Damping (3 vols vers le haut)",
            "Curseur « Atténuation du Pitch », 3 vols par pas de 0,2 au-dessus de la valeur actuelle (le pitch, plus lourd, dépasse davantage). Même lecture, sur le pitch.",
        )
        .tool("step", None),
    ]);
    pid_steps.extend(series(
        "Master",
        &MASTER_SERIES,
        "Le niveau des gains. On monte tant que la réponse devient plus rapide **sans cassure** (la montée s'arrête un instant avant la consigne) ni **vagues** sur le palier. Au premier vol « trop haut », on garde la valeur d'avant : inutile de voler toute la liste.",
        SERIES_FLIGHT_HELP,
        "series",
    ));
    pid_steps.extend([
        Step::new("master_read", "Lire la série Master et garder la valeur conseillée", "Réponse indicielle → Diagnostic du réglage.")
            .tool("step", None),
        Step::new(
            "pitch_tracking",
            "Série Pitch Tracking : 3 vols dans le sens du diagnostic",
            "Curseur « Suivi du Pitch » : il aligne la vitesse du pitch sur celle du roll. Pitch plus lent que le roll : 3 vols vers le haut ; plus rapide : vers le bas. Écart visé sous 1 ms, acceptable jusqu'à 2 ms.",
        )
        .tool("step", None),
    ]);
    let pid = Phase {
        key: "pid",
        title: "PID par séries",
        intro: "Méthode des séries de vols : **un seul curseur change** d'un vol à l'autre, par pas de 0,2. Ouvrez tous les logs de la série : l'outil **Réponse indicielle** (Diagnostic du réglage) dit quelle valeur garder. Les filtres ne changent plus pendant les séries.",
        steps: pid_steps,
    };
    let mut finish_steps = series("Stick Resp.", &FF_SERIES, FF_HELP, FF_FLIGHT_HELP, "ff_series");
    finish_steps.extend([
        Step::new(
            "ff_logs",
            "Ouvrir les logs de la série Feedforward",
            "Copiez les `.bbl` dans le dossier du drone, puis ouvrez les vols de la série ensemble avec le bouton de cette case : ils sont lus comme logs de séries.",
        )
        .any_log()
        .logs(PID)
        .open_logs(),
        Step::new(
            "ff_read",
            "Lire la série Feedforward et choisir la valeur",
            "Le retard consigne → gyro de chaque vol, en roll et en pitch, est donné sous les logs des séries (ligne de la série « Réponse des sticks ») et, vol par vol, dans l'**Assistant**. La Réponse indicielle ne sert pas ici : le feedforward la fausse. Gardez la valeur qui vous convient d'après ces retards et votre ressenti.",
        )
        .tool("advisor", None),
        Step::new(
            "i_gain",
            format!("Terme I (Dérive - Oscillations) : {} à {}", r(card.i_range().0), r(card.i_range().1)),
            i_help(card),
        ),
        Step::new(
            "dmax",
            "Atténuation dynamique (D max) à votre goût",
            "Plus de D dans les mouvements rapides et le propwash ; trop haut, moteurs chauds.",
        ),
        Step::new(
            "rates_back",
            "Revenir à vos rates habituelles",
            "Sélectionnez votre profil de rates habituel (interrupteur ou onglet Réglages de PID).",
        ),
        Step::new(
            "final_check",
            "Vol libre, puis Assistant ; garder un diff all du réglage final",
            "Un vol comme d'habitude et un vol dans le propwash (descentes, split-S). L'Assistant ne doit plus signaler de bruit, d'oscillation ni de moteurs saturés. Moteurs tièdes.",
        )
        .tool("advisor", None),
    ]);
    let finish = Phase {
        key: "finish",
        title: "Finitions",
        intro: "Remettre ce qui avait été mis de côté pendant les séries, à votre goût.",
        steps: finish_steps,
    };
    vec![prep, filters, pid, finish]
}

// ------------------------------------------------------------------ état automatique
/// État d'une case d'après les logs du drone (le plus récent compte) : `AUTO_OK`, `AUTO_BAD` ou `None` (rien à dire), avec une
/// explication courte. Seuls comptent les logs du rôle de la case (`Step::logs`) ; `targets` : repères du dernier log des filtres.
/// `Err` : l'usage de la case manque dans un log (cas sans appelant : `check_log` rend toujours les quatre usages).
pub fn auto_status(
    step: &Step,
    checks: &[LogCheck],
    targets: Option<&FilterTargets>,
) -> Result<(Option<&'static str>, String), String> {
    let checks: Vec<&LogCheck> = if step.logs.is_empty() {
        checks.iter().collect()
    } else {
        checks.iter().filter(|c| c.role == step.logs).collect()
    };
    if step.targets {
        let Some(t) = targets else {
            return Ok((None, String::new()));
        };
        let res = t.reached();
        let txt = t
            .checks()
            .iter()
            .map(|(name, _ok, t)| format!("{name} {t}"))
            .collect::<Vec<_>>()
            .join(" ; ");
        return Ok((
            match res {
                Some(true) => Some(AUTO_OK),
                Some(false) => Some(AUTO_BAD),
                None => None,
            },
            txt,
        ));
    }
    if let Some((cat, val)) = step.flight {
        // `c.sliders.get(cat, -1)` : -1 si le curseur est absent
        let hits: Vec<&&LogCheck> = checks
            .iter()
            .filter(|c| (c.sliders.iter().find(|(k, _)| *k == cat).map_or(-1.0, |(_, v)| *v) - val).abs() < 0.01)
            .collect();
        let Some(last) = hits.last() else {
            return Ok((None, String::new()));
        };
        let u = last
            .usage(step.flight_usage)
            .ok_or_else(|| format!("usage « {} » absent de la vérification du log", step.flight_usage))?;
        return Ok((
            Some(if u.ok { AUTO_OK } else { AUTO_BAD }),
            format!("{} : {}", last.label, u.reason),
        ));
    }
    if checks.is_empty() || !(!step.checks.is_empty() || !step.usages.is_empty() || step.any_log) {
        return Ok((None, String::new()));
    }
    let last = checks[checks.len() - 1];
    if step.any_log {
        return Ok((Some(AUTO_OK), format!("{} ouvert.", last.label)));
    }
    let bad: Vec<_> = step
        .checks
        .iter()
        .filter_map(|k| last.item(k))
        .filter(|it| it.status == WARN || it.status == BLOCK)
        .collect();
    for key in &step.usages {
        if let Some(u) = last.usage(key)
            && !u.ok
        {
            return Ok((
                Some(AUTO_BAD),
                format!("{} : {} — {}", last.label, u.label.to_lowercase(), u.reason),
            ));
        }
    }
    if !bad.is_empty() {
        let lines = bad
            .iter()
            .map(|it| format!("{} — {}", it.title, it.detail))
            .collect::<Vec<_>>()
            .join("\n");
        return Ok((Some(AUTO_BAD), format!("{} :\n{lines}", last.label)));
    }
    let seen: Vec<_> = step.checks.iter().filter_map(|k| last.item(k)).collect();
    if !step.usages.is_empty()
        || (!seen.is_empty() && seen.iter().all(|it| it.status == OK))
        || step.checks == ["simplified"]
    {
        return Ok((Some(AUTO_OK), format!("{} : vérifié dans le log.", last.label)));
    }
    Ok((None, String::new()))
}
