"""Parcours de réglage du guide intégré : 4 phases, des cases à cocher, et leur état d'après les logs.

Sans Qt. Chaque case (`Step`) a un titre et une aide adaptés à la fiche drone (`DroneCard`), et peut être
vérifiée automatiquement par les logs ouverts du drone (`LogCheck` de `log_check.py`) :
- `checks` : points de `log_check` qui doivent être bons ;
- `usages` : usages de `log_check` qui doivent être possibles (bruit, retard, série) ;
- `any_log` : un log du drone est ouvert ;
- `flight` : vol d'une série (curseur, valeur) présent parmi les logs, et lisible comme vol de série.
Choix de Cédric (2026-10-12) : configurateur d'abord, CLI en repli ; protocole de Brian White pour les
mouvements ; listes des séries et plages de finitions dans `drone_card.py`.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from .drone_card import DRONE_FILTER_DEFAULTS, MASTER_SERIES, DroneCard
from .log_check import (BLOCK, CONFIGURATOR_SLIDERS, LOG_ANGLE_LIMIT, LOG_RATE_DPS, OK, WARN, FilterTargets,
                        LogCheck)

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
    tool: str | None = None  # outil à ouvrir (clé de gui/tools REGISTRY)
    part: str | None = None  # partie du vol à analyser avec l'outil : "hover" ou "moving"
    targets: bool = False  # repères de fin de la phase Filtres

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
def _recording(c: DroneCard) -> str:
    hz, frac = c.log_rate()
    return f"Onglet Blackbox : fréquence {hz / 1000:.1f} kHz ({frac}), mode débug {c.debug_mode()}".replace(".", ",")


def _recording_help(c: DroneCard) -> str:
    hz, frac = c.log_rate()
    unfilt = ("En 4.4, GYRO_SCALED est le seul moyen d'avoir le gyro non filtré." if c.bf == "4.4" else
              "« Champs de débogage inclus » : laissez **Gyro (Unfiltered)** coché.")
    return (f"Le blackbox garde un échantillon sur N sans filtrer : tout bruit plus rapide que la moitié de la "
            f"fréquence d'enregistrement crée de fausses raies. Pour une boucle PID à {_r(c.pid_khz)} kHz et un "
            f"drone de cette taille : **{hz:.0f} Hz**. {unfilt}\n\n"
            f"CLI : `set blackbox_sample_rate = {frac}` puis `set debug_mode = {c.debug_mode()}`, `save`.\n\n"
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
    return (f"Les mouvements de réglage se font en **mode Angle** avec des rates linéaires : la réponse est la même "
            f"qu'en acro, les mouvements sont plus faciles à doser. Onglet **Réglages de PID**, choisissez un profil "
            f"de rates libre (ex. 3) et nommez-le « LOG » : « Type de rates » ACTUAL, « Sensibilité au centre » "
            f"{LOG_RATE_DPS}, « Max Vel » {LOG_RATE_DPS}, Expo 0, sur les trois axes. Puis « Limite de l'Angle » : "
            f"**{LOG_ANGLE_LIMIT}** (Brian White ; elle appartient au profil PID).\n\n"
            f"CLI : `set rates_type = ACTUAL`, `set roll_rc_rate = {LOG_RATE_DPS // 10}`, "
            f"`set roll_srate = {LOG_RATE_DPS // 10}`, `set roll_expo = 0` (idem pitch et yaw), "
            f"`set {c.angle_limit_setting()} = {LOG_ANGLE_LIMIT}`.")


def _flight_help(_c: DroneCard) -> str:
    return ("Un seul vol, en deux parties :\n\n"
            "1. **20 à 30 s de stationnaire**, avec des montées lentes des gaz de 0 à 100 %, sans manœuvre "
            "brusque : on y repère le bruit (moteurs, châssis).\n"
            "2. **20 à 30 s de wobbles** : mode Angle, profil de rates « LOG », mouvements **au hasard sur le roll "
            "et le pitch en même temps**, rapides, sans s'arrêter : on y mesure le retard des filtres.\n\n"
            "OpenTuneBox trouve tout seul les deux parties.")


SERIES_FLIGHT_HELP = ("Mode Angle, profil « LOG », au moins **20 s de wobbles** au hasard sur le roll et le pitch "
                      "en même temps (au moins 200 fenêtres de mesure). Même batterie, même charge. Au moindre bruit "
                      "d'oscillation, atterrissez et revenez à la valeur précédente ; touchez les moteurs après "
                      "chaque vol.")


def _series(cat: str, values: tuple[float, ...], first_help: str) -> list[Step]:
    name = CONFIGURATOR_SLIDERS[cat]
    out = []
    for k, v in enumerate(values):
        out.append(Step(f"{cat}_{v:.1f}", f"Vol « {name} » à {_r(v)}",
                        (first_help + "\n\n" if k == 0 else "") + "Curseur à " + _r(v) + ", Sauvegarder, puis voler. "
                        + "\n\n" + SERIES_FLIGHT_HELP,
                        flight=(cat, v)))
    return out


def _damping_steps(c: DroneCard) -> list[Step]:
    return _series("Damping", c.damping_series(),
                   "La première série règle l'équilibre P / D. Arrêtez-vous dès que le Diagnostic du réglage a "
                   "trouvé la valeur (inutile de voler toute la liste).")


def _finish_help(kind: str):
    def f(c: DroneCard) -> str:
        if kind == "ff":
            lo, hi = c.ff_range()
            return (f"Le **feedforward** pousse les moteurs dès que le manche bouge, avant que l'erreur n'apparaisse : "
                    f"le drone suit le manche avec moins de retard. Trop haut, le gyro dépasse la consigne à chaque "
                    f"coup de manche et la croise sans cesse. Plage de départ pour cette taille : **{_r(lo)} à "
                    f"{_r(hi)}** (Brian White). Volez en acro, avec vos rates habituelles, et regardez le gyro suivre "
                    "la consigne dans le visualiseur.")
        lo, hi = c.i_range()
        return (f"Le **terme I** corrige les erreurs qui durent : dérive dans le vent, en virage, avec le centre de "
                f"gravité décalé. Trop bas, le drone dérive ; trop haut, il oscille lentement après un mouvement "
                f"(dépassement lent et étiré). Plage de départ : **{_r(lo)} à {_r(hi)}** (Brian White : il manque "
                "plus souvent d'I qu'il n'y en a trop).")
    return f


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
                          checks=("log_rate", "gyro_unfilt", "debug_mode")),
                     Step("log_rates", f"Profil de rates « LOG » : ACTUAL {LOG_RATE_DPS} / {LOG_RATE_DPS} / 0, "
                          f"limite d'angle {LOG_ANGLE_LIMIT}°", _log_rates_help, checks=("rates", "angle_limit")),
                     Step("switches", "Interrupteurs : mode ANGLE et choix du profil de rates",
                          "Onglet **Modes** : ANGLE sur un interrupteur. Onglet **Ajustements** : « Sélection des "
                          "profils de Rate » sur un interrupteur ou un potentiomètre, pour passer du profil « LOG » "
                          "à vos rates habituelles en vol.", checks=("angle_mode",)),
                     Step("batteries", "Mêmes batteries pour tous les vols comparés",
                          "Chargées pareil : la tension change la réponse du drone."),
                 ])
    filters = Phase("filters", "Filtres",
                    "Un vol de réglage, puis un changement à la fois, dans l'ordre. Chaque changement : Sauvegarder, "
                    "refaire le même vol, comparer les deux logs.", [
                        Step("filter_flight", "Vol de réglage : stationnaire, puis wobbles", _flight_help,
                             usages=("noise", "delay"), checks=("log_rate", "gyro_unfilt")),
                        Step("retrieve", "Récupérer le log et l'ouvrir",
                             "Onglet **Blackbox** : **Activer le Mode de périphérique de stockage de masse** (le "
                             "contrôleur apparaît comme une clé USB), ou retirez la carte SD et lisez-la sur "
                             "l'ordinateur. Copiez les `.bbl` dans un dossier par drone, nommez-les "
                             "(`filtres-1.bbl`, `damping-0.8.bbl`…), puis videz la mémoire (**Effacer la mémoire "
                             "interne**). Ouvrez le log ici (bouton **Ouvrir** ou glisser-déposer).", any_log=True),
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
                             "Vers la droite : moins de filtrage, moins de retard. L'outil **Filtres** montre avant de "
                             "voler le bruit qui passerait et le retard gagné.", tool="filter_sim", part="moving"),
                        Step("lpf_dterm", "4. Passe-bas du D : curseur « Multiplicateur de filtre D Term »",
                             "Même sens. Touchez les moteurs après le vol : tièdes, c'est bon ; chauds, revenez en "
                             "arrière.", tool="filter_sim", part="moving"),
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
                    Step("series_conditions", "Conditions des séries : mode Angle, profil « LOG », curseur I à 0,25, "
                         "Dynamic Damping à 0, Master à 1,0",
                         "Curseurs en Mode Expert : « Dérive - Oscillations » 0,25 (le terme I déforme la réponse), "
                         "« Atténuation dynamique » 0 (le D max change la réponse selon la vitesse du mouvement), "
                         "« Multiplicateur Maître » 1,0 (0,8 pour un drone très puissant). Le feedforward est coupé "
                         "en mode Angle.", checks=("i_slider", "dyn_damping", "angle_mode", "rates", "feedforward")),
                    *_damping_steps(card),
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
                       Step("ff", lambda c: f"Feedforward (Réponse des sticks) : {_r(c.ff_range()[0])} à "
                            f"{_r(c.ff_range()[1])}", _finish_help("ff")),
                       Step("i_gain", lambda c: f"Terme I (Dérive - Oscillations) : {_r(c.i_range()[0])} à "
                            f"{_r(c.i_range()[1])}", _finish_help("i")),
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
    dire), avec une explication courte."""
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
        u = hits[-1].usage("series")
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
