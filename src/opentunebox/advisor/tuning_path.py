"""Parcours de réglage du guide intégré : 4 phases, des cases à cocher, et leur état d'après les logs.

Sans Qt. Chaque case (`Step`) a un titre et une aide adaptés à la fiche drone (`DroneCard`), et peut être
vérifiée automatiquement par les logs ouverts du drone (`LogCheck` de `log_check.py`) :
- `checks` : points de `log_check` qui doivent être bons ;
- `usages` : usages de `log_check` qui doivent être possibles (bruit, retard, série) ;
- `any_log` : un log du drone est ouvert ;
- `flight` : vol d'une série (curseur, valeur) présent parmi les logs, et lisible comme vol de série
  (usage `flight_usage` de `log_check` : « series », ou « ff_series » pour la série Feedforward, en Acro) ;
- `logs` : rôle des logs lus par la case (`log_check.FILTERS` ou `PID`, vide : tous) ; avec `open_logs`, la
  case porte le bouton qui ouvre les logs de ce rôle. Les logs des filtres et ceux des séries PID sont ainsi
  demandés et jugés à part (un vol de série PID à 1 kHz ne fait pas échouer les cases des filtres).
Choix de Cédric (2026-10-12) : configurateur d'abord, CLI en repli ; protocole de Brian White pour les
mouvements ; listes des séries et plages de finitions dans `drone_card.py`.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from .drone_card import DRONE_FILTER_DEFAULTS, FF_SERIES, MASTER_SERIES, DroneCard
from .log_check import (BLOCK, CONFIGURATOR_SLIDERS, FILTERS, LOG_ANGLE_LIMIT, LOG_RATE_DPS, OK, PID, WARN,
                        FilterTargets, LogCheck)

AUTO_OK, AUTO_BAD = "ok", "bad"


def _r(x: float) -> str:
    return f"{x:.1f}".replace(".", ",")


@dataclass
class Step:
    key: str
    title: Callable[[DroneCard], str] | str
    help: Callable[[DroneCard], str] | str = ""  # Markdown
    checks: tuple[str, ...] = ()
    usages: tuple[str, ...] = ()
    any_log: bool = False  # vérifiée dès qu'un log du drone est ouvert
    flight: tuple[str, float] | None = None  # (catégorie PTB du curseur, valeur)
    flight_usage: str = "series"  # usage de log_check qui rend le vol lisible
    tool: str | None = None  # outil à ouvrir (clé de gui/tools REGISTRY)
    part: str | None = None  # partie du vol à analyser avec l'outil : "hover" ou "moving"
    targets: bool = False  # repères de fin de la phase Filtres
    logs: str = ""  # rôle des logs lus par la case : FILTERS, PID ou "" (tous)
    open_logs: bool = False  # bouton « Ouvrir les logs » de ce rôle

    def text(self, card: DroneCard) -> str:
        return self.title(card) if callable(self.title) else self.title

    def help_text(self, card: DroneCard) -> str:
        return self.help(card) if callable(self.help) else self.help


@dataclass
class Phase:
    key: str
    title: str
    intro: str  # Markdown
    steps: list[Step]


# ------------------------------------------------------------------ textes adaptés à la fiche
def _khz(hz: float, frac: str) -> str:
    return f"{_r(hz / 1000)} kHz ({frac})"


def _recording(c: DroneCard) -> str:
    return (f"Onglet Blackbox : fréquence {_khz(*c.log_rate())} pour les filtres, {_khz(*c.pid_log_rate())} pour "
            f"les séries PID, mode débug {c.debug_mode()}")


def _recording_help(c: DroneCard) -> str:
    hz, frac = c.log_rate()
    pid_hz, pid_frac = c.pid_log_rate()
    unfilt = ("En 4.4, GYRO_SCALED est le seul moyen d'avoir le gyro non filtré." if c.bf == "4.4" else
              "« Champs de débogage inclus » : laissez **Gyro (Unfiltered)** coché.")
    return (f"Deux fréquences, selon le vol. **Filtres : {hz:.0f} Hz.** Le blackbox garde un échantillon sur N "
            "sans filtrer : tout bruit plus rapide que la moitié de la fréquence d'enregistrement crée de fausses "
            f"raies ; il faut donc voir tout le bruit des moteurs (boucle PID à {_r(c.pid_khz)} kHz, drone de cette "
            f"taille). **Séries PID : {pid_hz:.0f} Hz** suffisent pour mesurer la réponse à la milliseconde près, "
            f"et la mémoire garde plus de vols. {unfilt}\n\n"
            f"CLI : `set blackbox_sample_rate = {frac}` (filtres) ou `{pid_frac}` (séries PID), puis "
            f"`set debug_mode = {c.debug_mode()}`, `save`.\n\n"
            "Boucle PID inconnue : onglet **Configuration**, « Fréquence de la boucle PID » ; type de gyro : "
            "commande `status` du CLI (ligne GYRO).")


def _bidir(c: DroneCard) -> str:
    return ("DShot bidirectionnel activé, pôles moteur justes" if c.bidir else
            "ESC BLHeli_S d'origine : flasher Bluejay pour le DShot bidirectionnel (sinon pas de filtre RPM)")


def _defaults_help(c: DroneCard) -> str:
    rpm, dn = DRONE_FILTER_DEFAULTS["rpm"], DRONE_FILTER_DEFAULTS["dyn_notch"]
    return ("On règle à partir d'un point connu. Onglet **Réglages de PID** : **Réinitialiser ce profil** (PID, "
            "curseurs, filtres du D). Les autres filtres sont des réglages généraux, à remettre à la main dans la "
            "partie filtres du même onglet :\n\n"
            "- « Multiplicateur du Filtre Gyro » et « Multiplicateur de filtre D Term » : **1,0** ;\n"
            f"- Filtre Gyro RPM : harmoniques {rpm['harmonics']}, Q {rpm['q']}, fréquence min {rpm['min_hz']} Hz"
            + (", poids 100 / 100 / 100" if c.bf != "4.4" else "") + " ;\n"
            f"- Filtre coupe bande dynamique : {dn['count']} coupures, Q {dn['q']}, {dn['min_hz']} à {dn['max_hz']} Hz.\n\n"
            "Récepteur, modes, OSD et VTX ne sont pas touchés.")


def _log_rates_help(c: DroneCard) -> str:
    return (f"Les mouvements de réglage se font en **mode Angle**, ou en **Acro avec le feedforward à 0**, avec des "
            f"rates linéaires : la réponse est la même, le mode Angle rend les mouvements plus faciles à doser (le "
            f"firmware y coupe le feedforward). Onglet **Réglages de PID**, choisissez un profil "
            f"de rates libre (ex. 3) et nommez-le « LOG » : « Type de rates » ACTUAL, « Sensibilité au centre » "
            f"{LOG_RATE_DPS}, « Max Vel » {LOG_RATE_DPS}, Expo 0, sur les trois axes. Puis « Limite de l'Angle » : "
            f"**{LOG_ANGLE_LIMIT}** (Brian White ; elle appartient au profil PID, et ne sert qu'en mode Angle).\n\n"
            f"CLI : `set rates_type = ACTUAL`, `set roll_rc_rate = {LOG_RATE_DPS // 10}`, "
            f"`set roll_srate = {LOG_RATE_DPS // 10}`, `set roll_expo = 0` (idem pitch et yaw), "
            f"`set {c.angle_limit_setting()} = {LOG_ANGLE_LIMIT}`.")


def _flight_help(_c: DroneCard) -> str:
    return ("Un seul vol, en deux parties :\n\n"
            "1. **20 à 30 s de stationnaire**, avec des montées lentes des gaz de 0 à 100 %, sans manœuvre "
            "brusque : on y repère le bruit (moteurs, châssis).\n"
            "2. **20 à 30 s de wobbles** : mode Angle (ou Acro avec le feedforward à 0), profil de rates « LOG », "
            "mouvements **au hasard sur le roll et le pitch en même temps**, rapides, sans s'arrêter : on y mesure le retard des filtres.\n\n"
            "OpenTuneBox trouve tout seul les deux parties.")


SERIES_FLIGHT_HELP = ("Mode Angle (ou Acro avec le feedforward à 0), profil « LOG », au moins **20 s de wobbles** au "
                      "hasard sur le roll et le pitch en même temps (au moins 200 fenêtres de mesure). Même batterie, même charge. Au moindre bruit "
                      "d'oscillation, atterrissez et revenez à la valeur précédente ; touchez les moteurs après "
                      "chaque vol.")
FF_FLIGHT_HELP = ("**Acro uniquement** (le firmware coupe le feedforward en mode Angle), profil « LOG », au moins "
                  "**20 s de wobbles** au hasard sur le roll et le pitch en même temps. Même batterie, même charge, "
                  "seul le curseur « Réponse des sticks » change.")


def _series(cat: str, values: tuple[float, ...], first_help: str, flight_help: str = SERIES_FLIGHT_HELP,
            usage: str = "series") -> list[Step]:
    name = CONFIGURATOR_SLIDERS[cat]
    out = []
    for k, v in enumerate(values):
        out.append(Step(f"{cat}_{v:.1f}", f"Vol « {name} » à {_r(v)}",
                        (first_help + "\n\n" if k == 0 else "") + "Curseur à " + _r(v) + ", Sauvegarder, puis voler. "
                        + "\n\n" + flight_help,
                        flight=(cat, v), flight_usage=usage, logs=PID))
    return out


def _damping_steps(c: DroneCard) -> list[Step]:
    return _series("Damping", c.damping_series(),
                   "La première série règle l'équilibre P / D. Arrêtez-vous dès que le Diagnostic du réglage a "
                   "trouvé la valeur (inutile de voler toute la liste).")


FF_HELP = ("Le **feedforward** pousse les moteurs dès que le manche bouge, avant que l'erreur n'apparaisse : le "
           "drone suit le manche avec moins de retard. Trop haut, le gyro dépasse la consigne à chaque coup de manche. "
           "Série par pas de 0,5, en **Acro** : pour chaque valeur, l'Assistant donne le **retard consigne → gyro** "
           "en roll et en pitch (la Réponse indicielle, faussée par le feedforward, ne sert pas ici).")


def _i_help(c: DroneCard) -> str:
    lo, hi = c.i_range()
    return (f"Le **terme I** corrige les erreurs qui durent : dérive dans le vent, en virage, avec le centre de "
            f"gravité décalé. Trop bas, le drone dérive ; trop haut, il oscille lentement après un mouvement "
            f"(dépassement lent et étiré). Plage de départ : **{_r(lo)} à {_r(hi)}** (Brian White : il manque "
            "plus souvent d'I qu'il n'y en a trop).")


# ------------------------------------------------------------------ parcours
def phases(card: DroneCard) -> list[Phase]:
    prep = Phase("prep", "Préparer le drone",
                 "À faire une fois, avant le premier vol de réglage. Dès qu'un log du drone est ouvert, certaines "
                 "cases se vérifient toutes seules : « ✔ Vérifié » ou « ▲ À revoir ».", [
                     Step("expert", "Configurateur : activer le Mode Expert",
                          "Fenêtre **Options** du configurateur. Sans lui, les curseurs sont limités à 0,7–1,4 et "
                          "Master, I, Pitch Damping, Pitch Tracking et Dynamic Damping restent bloqués."),
                     Step("backup", "Sauvegarder le réglage actuel (diff all)",
                          "Onglet **Ligne de commande (CLI)** : tapez `diff all`, puis **Enregistrer sous**. C'est "
                          "le point de retour si un réglage tourne mal."),
                     Step("defaults", "Repartir des valeurs par défaut", _defaults_help),
                     Step("mechanics", "Mécanique saine",
                          "Hélices neuves et droites, vis du châssis et des moteurs serrées, moteurs qui tournent "
                          "librement, câbles et antennes fixés. Aucun filtre ne rattrape une hélice tordue."),
                     Step("storage", "Mémoire blackbox prête",
                          "Onglet **Blackbox** : stockage sur carte SD ou mémoire embarquée, vide avant la séance."),
                     Step("bidir", _bidir,
                          "Onglet **Moteurs** : « DShot bi-directionnel » et « Pôles moteur » (aimants de la cloche, "
                          "souvent 14). Il active le filtre RPM et donne la vitesse réelle des moteurs.",
                          checks=("rpm",)),
                     Step("sliders", "Curseurs de réglage PID en mode RPY",
                          "Onglet **Réglages de PID**, « Mode » : RPY. Les conseils sont des positions de curseurs.",
                          checks=("simplified",)),
                     Step("recording", _recording, _recording_help,
                          checks=("log_rate", "gyro_unfilt", "debug_mode"), logs=FILTERS),
                     Step("log_rates", f"Profil de rates « LOG » : ACTUAL {LOG_RATE_DPS} / {LOG_RATE_DPS} / 0, "
                          f"limite d'angle {LOG_ANGLE_LIMIT}°", _log_rates_help, checks=("rates", "angle_limit"),
                          logs=PID),
                     Step("switches", "Interrupteurs : mode ANGLE et choix du profil de rates",
                          "Onglet **Modes** : ANGLE sur un interrupteur (inutile si vous volez en Acro avec le "
                          "feedforward à 0). Onglet **Ajustements** : « Sélection des "
                          "profils de Rate » sur un interrupteur ou un potentiomètre, pour passer du profil « LOG » "
                          "à vos rates habituelles en vol.", checks=("flight_mode",), logs=PID),
                     Step("batteries", "Mêmes batteries pour tous les vols comparés",
                          "Chargées pareil : la tension change la réponse du drone."),
                 ])
    filters = Phase("filters", "Filtres",
                    "Un vol de réglage, puis un changement à la fois, dans l'ordre. Chaque changement : Sauvegarder, "
                    "refaire le même vol, comparer les deux logs.", [
                        Step("filter_flight", "Vol de réglage : stationnaire, puis wobbles", _flight_help,
                             usages=("noise", "delay"), checks=("log_rate", "gyro_unfilt"), logs=FILTERS),
                        Step("retrieve", "Récupérer le log et l'ouvrir",
                             "Onglet **Blackbox** : **Activer le Mode de périphérique de stockage de masse** (le "
                             "contrôleur apparaît comme une clé USB), ou retirez la carte SD et lisez-la sur "
                             "l'ordinateur. Copiez les `.bbl` dans un dossier par drone, nommez-les "
                             "(`filtres-1.bbl`, `damping-0.8.bbl`…), puis videz la mémoire (**Effacer la mémoire "
                             "interne**). Ouvrez-le avec le bouton de cette case : il sera lu comme log des filtres.",
                             any_log=True, logs=FILTERS, open_logs=True),
                        Step("mechanics_read", "Mécanique : lire Fréquence × gaz (partie stationnaire)",
                             "Une **diagonale** qui monte avec les gaz = moteurs (normal, le filtre RPM s'en charge). "
                             "Une **ligne horizontale** = résonance du châssis ou pièce qui vibre : à corriger "
                             "mécaniquement avant de toucher aux filtres.", tool="freq_throttle", part="hover"),
                        Step("rpm", "1. Filtre RPM",
                             "Spectre, case **Diagnostic du bruit**, bloc « Filtre RPM » : harmoniques, Q et poids "
                             "conseillés. Onglet **Réglages de PID** → « Filtre Gyro RPM ».", tool="spectral",
                             part="hover"),
                        Step("notch", "2. Notch dynamique",
                             "Bloc « Notch dynamique » du diagnostic : nombre de coupures, Q, fréquences. Avec un "
                             "filtre RPM qui fonctionne, 1 ou 2 coupures suffisent souvent. → « Filtre coupe bande "
                             "dynamique ».", tool="spectral", part="hover"),
                        Step("lpf_gyro", "3. Passe-bas du gyro : curseur « Multiplicateur du Filtre Gyro »",
                             "Vers la droite : moins de filtrage, moins de retard."),
                        Step("lpf_dterm", "4. Passe-bas du D : curseur « Multiplicateur de filtre D Term »",
                             "Même sens. Touchez les moteurs après le vol : tièdes, c'est bon ; chauds, revenez en "
                             "arrière."),
                        Step("filter_targets", "Repères atteints : gyro filtré sous −30 dB, terme D au plus −10 dB, "
                             "retard sous l'objectif de la taille",
                             "Mesurés sur le dernier log du drone : bruit sur la partie stationnaire (échelle de "
                             "PIDtoolbox), retard sur la partie avec mouvements, objectif de Brian White pour la "
                             "taille. La phase est terminée quand les quatre repères sont atteints.", targets=True),
                    ])
    pid = Phase("pid", "PID par séries",
                "Méthode de Brian White : une série de vols où **un seul curseur change**, par pas de 0,2. Ouvrez "
                "tous les logs de la série : l'outil **Réponse indicielle** (Diagnostic du réglage) dit quelle valeur "
                "garder. Les filtres ne changent plus pendant les séries.", [
                    Step("series_conditions", "Conditions des séries : mode Angle (ou Acro, feedforward à 0), profil "
                         "« LOG », curseur I à 0,25, "
                         "Dynamic Damping à 0, Master à 1,0",
                         "Curseurs en Mode Expert : « Dérive - Oscillations » 0,25 (le terme I déforme la réponse), "
                         "« Atténuation dynamique » 0 (le D max change la réponse selon la vitesse du mouvement), "
                         "« Multiplicateur Maître » 1,0 (0,8 pour un drone très puissant). Le feedforward fausse la "
                         "réponse : le firmware le coupe en mode Angle ; en Acro, curseur « Réponse des sticks » à 0 "
                         "(à remonter en finitions).", checks=("i_slider", "dyn_damping", "flight_mode", "rates"),
                         logs=PID),
                    Step("pid_recording", lambda c: f"Onglet Blackbox : fréquence {_khz(*c.pid_log_rate())} pour "
                         "les séries",
                         "Plus lente que pour les filtres : la réponse se mesure encore à la milliseconde près, et la "
                         "mémoire garde plus de vols. Remettez la fréquence des filtres pour un nouveau vol de "
                         "réglage des filtres.", checks=("pid_log_rate",), logs=PID),
                    *_damping_steps(card),
                    Step("pid_logs", "Ouvrir les logs de la série",
                         "Après chaque série, copiez les `.bbl` dans le dossier du drone, puis ouvrez-les avec le "
                         "bouton de cette case : ils seront lus comme logs de séries PID (ouverts autrement, un log "
                         "sans partie stationnaire est aussi classé en PID). Ouvrez tous les vols de la série "
                         "ensemble.", any_log=True, logs=PID, open_logs=True),
                    Step("damping_read", "Lire la série Damping et garder la valeur conseillée",
                         "Réponse indicielle → Diagnostic du réglage : première valeur sans dépassement visible sur le "
                         "roll. Si le pitch dépasse encore, le diagnostic propose une série **Pitch Damping**.",
                         tool="step"),
                    Step("pitch_damping", "Si demandé : série Pitch Damping (3 vols vers le haut)",
                         "Curseur « Atténuation du Pitch », 3 vols par pas de 0,2 au-dessus de la valeur actuelle "
                         "(le pitch, plus lourd, dépasse davantage). Même lecture, sur le pitch.", tool="step"),
                    *_series("Master", MASTER_SERIES,
                             "Le niveau des gains. On monte tant que la réponse devient plus rapide **sans cassure** "
                             "(la montée s'arrête un instant avant la consigne) ni **vagues** sur le palier. Au premier "
                             "vol « trop haut », on garde la valeur d'avant : inutile de voler toute la liste."),
                    Step("master_read", "Lire la série Master et garder la valeur conseillée",
                         "Réponse indicielle → Diagnostic du réglage.", tool="step"),
                    Step("pitch_tracking", "Série Pitch Tracking : 3 vols dans le sens du diagnostic",
                         "Curseur « Suivi du Pitch » : il aligne la vitesse du pitch sur celle du roll. Pitch plus "
                         "lent que le roll : 3 vols vers le haut ; plus rapide : vers le bas. Écart visé sous 1 ms, "
                         "acceptable jusqu'à 2 ms.", tool="step"),
                ])
    finish = Phase("finish", "Finitions",
                   "Remettre ce qui avait été mis de côté pendant les séries, à votre goût.", [
                       *_series("Stick Resp.", FF_SERIES, FF_HELP, FF_FLIGHT_HELP, "ff_series"),
                       Step("ff_logs", "Ouvrir les logs de la série Feedforward",
                            "Copiez les `.bbl` dans le dossier du drone, puis ouvrez les vols de la série ensemble avec "
                            "le bouton de cette case : ils sont lus comme logs de séries.", any_log=True, logs=PID,
                            open_logs=True),
                       Step("ff_read", "Lire la série Feedforward et choisir la valeur",
                            "Le retard consigne → gyro de chaque vol, en roll et en pitch, est donné sous les logs "
                            "des séries (ligne de la série « Réponse des sticks ») et, vol par vol, dans l'**Assistant**. "
                            "La Réponse indicielle ne sert pas ici : le feedforward la fausse. Gardez la valeur qui "
                            "vous convient d'après ces retards et votre ressenti.", tool="advisor"),
                       Step("i_gain", lambda c: f"Terme I (Dérive - Oscillations) : {_r(c.i_range()[0])} à "
                            f"{_r(c.i_range()[1])}", _i_help),
                       Step("dmax", "Atténuation dynamique (D max) à votre goût",
                            "Plus de D dans les mouvements rapides et le propwash ; trop haut, moteurs chauds."),
                       Step("rates_back", "Revenir à vos rates habituelles",
                            "Sélectionnez votre profil de rates habituel (interrupteur ou onglet Réglages de PID)."),
                       Step("final_check", "Vol libre, puis Assistant ; garder un diff all du réglage final",
                            "Un vol comme d'habitude et un vol dans le propwash (descentes, split-S). L'Assistant ne "
                            "doit plus signaler de bruit, d'oscillation ni de moteurs saturés. Moteurs tièdes.",
                            tool="advisor"),
                   ])
    return [prep, filters, pid, finish]


# ------------------------------------------------------------------ état automatique
def auto_status(step: Step, checks: list[LogCheck], targets: FilterTargets | None = None) -> tuple[str | None, str]:
    """État d'une case d'après les logs du drone (le plus récent compte) : AUTO_OK, AUTO_BAD ou None (rien à
    dire), avec une explication courte. Seuls comptent les logs du rôle de la case (`Step.logs`) ; `targets` : repères
    du dernier log des filtres."""
    if step.logs:
        checks = [c for c in checks if c.role == step.logs]
    if step.targets:
        if targets is None:
            return None, ""
        res = targets.reached()
        txt = " ; ".join(f"{name} {t}" for name, _ok, t in targets.checks())
        return (AUTO_OK if res else AUTO_BAD if res is False else None), txt
    if step.flight:
        cat, val = step.flight
        hits = [c for c in checks if abs(c.sliders.get(cat, -1) - val) < 0.01]
        if not hits:
            return None, ""
        u = hits[-1].usage(step.flight_usage)
        return (AUTO_OK if u.ok else AUTO_BAD), f"{hits[-1].label} : {u.reason}"
    if not checks or not (step.checks or step.usages or step.any_log):
        return None, ""
    last = checks[-1]
    if step.any_log:
        return AUTO_OK, f"{last.label} ouvert."
    bad = [it for it in (last.item(k) for k in step.checks) if it and it.status in (WARN, BLOCK)]
    for key in step.usages:
        u = last.usage(key)
        if u and not u.ok:
            return AUTO_BAD, f"{last.label} : {u.label.lower()} — {u.reason}"
    if bad:
        return AUTO_BAD, f"{last.label} :\n" + "\n".join(f"{it.title} — {it.detail}" for it in bad)
    seen = [it for it in (last.item(k) for k in step.checks) if it]
    if step.usages or (seen and all(it.status == OK for it in seen)) or (step.checks == ("simplified",)):
        return AUTO_OK, f"{last.label} : vérifié dans le log."
    return None, ""
