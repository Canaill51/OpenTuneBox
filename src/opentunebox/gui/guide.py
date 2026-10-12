"""Guide intégré : parcours de réglage à cocher, par drone.

En haut, la fiche drone (choix, création, modification) et la frise des phases ; à gauche, l'accueil et les
4 phases avec leur avancement ; à droite, les cases de la phase. Le parcours, les textes des cases et leur
état automatique viennent de `advisor/tuning_path.py` ; la fiche, de `advisor/drone_card.py`. Les logs
ouverts du drone (même nom que la fiche, `craft_name`) sont vérifiés par `advisor/log_check.py` : d'abord
en version rapide, puis en arrière-plan avec le vrai nombre de fenêtres et les repères des filtres.
Logs des filtres et logs des séries PID à part : un log ouvert par le bouton d'une case prend le rôle de cette
case (retenu par fichier dans les réglages), sinon le rôle deviné par `log_check` ; le panneau des logs les
range en deux parties, avec un lien pour changer de rôle.
L'accueil est le fichier `assets/guide/accueil.md` ; liens `otb:phase/<clé>` et `otb:tool/<clé>`.

Fiches enregistrées en JSON dans le dossier de données de l'utilisateur (`QStandardPaths.AppDataLocation`).
"""

from __future__ import annotations

import dataclasses
import html
import json
from pathlib import Path

from PySide6.QtCore import QSettings, QStandardPaths, Qt, QUrl
from PySide6.QtGui import QDesktopServices, QGuiApplication
from PySide6.QtWidgets import (QCheckBox, QComboBox, QDialog, QDialogButtonBox, QFileDialog, QFormLayout, QFrame,
                               QHBoxLayout, QLabel, QLineEdit, QListWidget, QMessageBox, QPushButton, QScrollArea, QSplitter, QTextBrowser,
                               QVBoxLayout, QWidget)

from ..advisor.drone_card import BF_FAMILIES, ESCS, PID_RATES_KHZ, DroneCard, card_for_log
from ..advisor.log_check import (BLOCK, FILTERS, INFO, OK, PID, ROLE_NAMES, WARN, check_log, check_logs,
                                 filter_targets)
from ..advisor.profiles import PROFILES, detect_profile
from ..advisor.tuning_path import AUTO_OK, Phase, Step, auto_status, phases
from . import theme
from .worker import run_async

GUIDE_DIR = Path(__file__).parent / "assets" / "guide"
HOME_FILE = "accueil.md"
HOME = "home"
PHASE_KEYS = ("prep", "filters", "pid", "finish")
TOOL_NAMES = {"spectral": "Spectre", "freq_throttle": "Fréq. × gaz", "freq_time": "Fréq. × temps",
              "step": "Réponse indicielle", "sliders": "Curseurs PID", "filter_sim": "Filtres",
              "setup": "Configuration", "advisor": "Assistant", "viewer": "Visualiseur"}
PART_NAMES = {"hover": "partie stationnaire", "moving": "partie avec mouvements"}
OPEN_LABELS = {FILTERS: "Ouvrir les logs des filtres…", PID: "Ouvrir les logs de la série…"}
_STATUS = {OK: ("green", "✔"), WARN: ("orange", "▲"), BLOCK: ("red", "✖"), INFO: ("blue", "ℹ")}


def cards_path() -> Path:
    base = QStandardPaths.writableLocation(QStandardPaths.AppDataLocation) or str(Path.home() / ".opentunebox")
    return Path(base) / "drones.json"


def load_cards(path: Path) -> list[DroneCard]:
    try:
        return [DroneCard.from_dict(d) for d in json.loads(path.read_text(encoding="utf-8"))]
    except (OSError, ValueError, TypeError):
        return []


def save_cards(path: Path, cards: list[DroneCard]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps([c.to_dict() for c in cards], ensure_ascii=False, indent=1), encoding="utf-8")


def card_from_log(log) -> DroneCard:
    """Fiche préremplie d'après un log : nom, version, taille détectée, boucle PID, ESC (DShot bidirectionnel)."""
    s = log.settings
    fam = s.family
    bf = fam if fam in ("4.4", "4.5") else "2025.12"
    khz = min((k for k, _ in PID_RATES_KHZ), key=lambda k: abs(k * 1000 - s.pid_rate_hz))
    esc = "BLHeli_32" if log.motor_hz is not None or s.dshot_bidir else "BLHeli_S"
    return DroneCard(s.craft_name or "Mon drone", bf, detect_profile(log)[0].key, khz, esc)


class CardDialog(QDialog):
    """Création ou modification d'une fiche drone."""

    def __init__(self, card: DroneCard, parent=None, from_log=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Fiche drone")
        self._from_log = from_log
        form = QFormLayout(self)
        self.name = QLineEdit(card.name)
        self.name.setToolTip("Le même nom que dans Betaflight (onglet Configuration, « Nom de l'appareil ») : "
                             "les logs du drone sont reconnus grâce à lui")
        self.bf = self._combo(BF_FAMILIES, card.bf)
        self.size = self._combo([(k, p.label) for k, p in PROFILES.items()], card.size)
        self.pid = self._combo(PID_RATES_KHZ, card.pid_khz)
        self.esc = self._combo(ESCS, card.esc)
        form.addRow("Nom (comme dans Betaflight)", self.name)
        form.addRow("Version de Betaflight", self.bf)
        form.addRow("Taille", self.size)
        form.addRow("Boucle PID", self.pid)
        form.addRow("ESC", self.esc)
        hint = QLabel("Boucle PID : onglet Configuration, « Fréquence de la boucle PID ». Gyro : commande "
                      "« status » du CLI (BMI270 → 3,2 kHz).")
        hint.setWordWrap(True)
        hint.setStyleSheet(theme.muted_style())
        form.addRow(hint)
        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        if from_log is not None:
            fill = buttons.addButton("Remplir d'après le log ouvert", QDialogButtonBox.ResetRole)
            fill.clicked.connect(self._fill)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        form.addRow(buttons)

    @staticmethod
    def _combo(items, current) -> QComboBox:
        cb = QComboBox()
        for value, label in items:
            cb.addItem(label, value)
        cb.setCurrentIndex(max(0, cb.findData(current)))
        return cb

    def _fill(self) -> None:
        c = card_from_log(self._from_log)
        self.name.setText(c.name)
        for cb, v in ((self.bf, c.bf), (self.size, c.size), (self.pid, c.pid_khz), (self.esc, c.esc)):
            cb.setCurrentIndex(max(0, cb.findData(v)))

    def card(self, done: dict | None = None) -> DroneCard:
        return DroneCard(self.name.text().strip() or "Mon drone", self.bf.currentData(), self.size.currentData(),
                         float(self.pid.currentData()), self.esc.currentData(), dict(done or {}))


class StepWidget(QFrame):
    """Une case du parcours : case à cocher, état automatique, aide repliable, bouton d'outil."""

    def __init__(self, page: GuidePage, step: Step, card: DroneCard) -> None:
        super().__init__()
        self.page, self.step = page, step
        lay = QVBoxLayout(self)
        lay.setContentsMargins(4, 4, 4, 4)
        row = QHBoxLayout()
        self.box = QCheckBox(step.text(card))
        self.box.setChecked(bool(card.done.get(step.key)))
        self.box.toggled.connect(lambda on: page.set_done(step.key, on))
        row.addWidget(self.box)  # boutons juste après le texte de la case, pas au bord droit
        self.help_btn = QPushButton("?")
        self.help_btn.setCheckable(True)
        self.help_btn.setFixedWidth(self.help_btn.fontMetrics().horizontalAdvance("MM") + 12)
        self.help_btn.setToolTip("Afficher l'aide de cette étape")
        row.addWidget(self.help_btn)
        if step.tool:
            what = TOOL_NAMES.get(step.tool, step.tool) + (f" ({PART_NAMES[step.part]})" if step.part else "")
            tool = QPushButton(f"Ouvrir {what}")
            tool.clicked.connect(lambda: page.open_tool(step.tool, step.part))
            row.addWidget(tool)
        if step.open_logs:
            opener = QPushButton(OPEN_LABELS[step.logs])
            opener.clicked.connect(lambda: page.open_logs(step.logs))
            row.addWidget(opener)
        row.addStretch(1)
        lay.addLayout(row)
        self.status = QLabel()
        self.status.setWordWrap(True)
        self.status.setVisible(False)
        lay.addWidget(self.status)
        self.help = QLabel()
        self.help.setTextFormat(Qt.MarkdownText)
        self.help.setWordWrap(True)
        self.help.setText(step.help_text(card))
        self.help.setVisible(False)
        self.help.setTextInteractionFlags(Qt.TextBrowserInteraction)
        self.help_btn.toggled.connect(self.help.setVisible)
        lay.addWidget(self.help)

    def set_auto(self, state: str | None, text: str) -> None:
        self.auto = state
        if state is None:
            self.status.setVisible(False)
            return
        acc, sym = ("green", "✔ Vérifié") if state == AUTO_OK else ("orange", "▲ À revoir")
        body = html.escape(text).replace("\n", "<br>")
        self.status.setText(f"<span style='color:{theme.accent(acc)}'><b>{sym}</b></span> {body}")
        self.status.setVisible(True)
        if state == AUTO_OK and not self.box.isChecked():  # vérifiée par un log : cochée, sans être enregistrée
            self.box.blockSignals(True)
            self.box.setChecked(True)
            self.box.blockSignals(False)


class GuidePage(QWidget):
    def __init__(self, state, main_window=None, cards_file: Path | None = None) -> None:
        super().__init__()
        self.state = state
        self.main_window = main_window
        self.settings = QSettings("OpenTuneBox", "OpenTuneBox")
        self.cards_file = cards_file or cards_path()
        self.cards = load_cards(self.cards_file)
        self.card: DroneCard | None = None
        self._widgets: list[StepWidget] = []
        self._pending: set[int] = set()  # logs en cours de vérification complète
        self._cli: list[str] = []
        try:  # rôle choisi pour chaque fichier de log (chemin → FILTERS / PID)
            self._roles: dict[str, str] = dict(json.loads(str(self.settings.value("guide/log_roles", "{}"))))
        except (ValueError, TypeError):
            self._roles = {}

        # ---- fiche drone et frise
        top = QHBoxLayout()
        top.addWidget(QLabel("Drone :"))
        self.card_combo = QComboBox()
        self.card_combo.setMinimumWidth(220)
        self.card_combo.currentIndexChanged.connect(self._on_card_changed)
        top.addWidget(self.card_combo)
        new_btn = QPushButton("Nouveau drone")
        new_btn.clicked.connect(self.new_card)
        top.addWidget(new_btn)
        self.edit_btn = QPushButton("Modifier")
        self.edit_btn.clicked.connect(self.edit_card)
        top.addWidget(self.edit_btn)
        self.reset_btn = QPushButton("Recommencer")
        self.reset_btn.setToolTip("Décocher toutes les cases de ce drone pour reprendre le parcours au début")
        self.reset_btn.clicked.connect(self.reset_progress)
        top.addWidget(self.reset_btn)
        self.delete_btn = QPushButton("Supprimer")
        self.delete_btn.setToolTip("Supprimer la fiche de ce drone et son avancement (les logs ne sont pas touchés)")
        self.delete_btn.clicked.connect(self.delete_card)
        top.addWidget(self.delete_btn)
        top.addSpacing(16)
        self.strip_buttons: dict[str, QPushButton] = {}
        for key in PHASE_KEYS:
            b = QPushButton()
            b.setFlat(True)
            b.clicked.connect(lambda _c=False, k=key: self.show_phase(k))
            self.strip_buttons[key] = b
            top.addWidget(b)
        top.addStretch(1)

        # ---- liste des phases, contenu, logs du drone
        self.phase_list = QListWidget()
        self.phase_list.addItems([""] * (len(PHASE_KEYS) + 1))
        self.phase_list.currentRowChanged.connect(self._show_row)
        self.home = QTextBrowser()
        self.home.setOpenLinks(False)
        self.home.anchorClicked.connect(self._on_link)
        self.content = QScrollArea()
        self.content.setWidgetResizable(True)
        self.logs_view = QTextBrowser()
        self.logs_view.setOpenLinks(False)
        self.logs_view.anchorClicked.connect(self._on_link)
        self.copy_btn = QPushButton("Copier les commandes CLI")
        self.copy_btn.setToolTip("Commandes équivalentes aux corrections proposées pour le dernier log du drone, à "
                                 "coller dans l'onglet « Ligne de commande (CLI) », si vous préférez le CLI")
        self.copy_btn.clicked.connect(self._copy_cli)
        self.copy_note = QLabel()
        self.copy_note.setStyleSheet(theme.muted_style())

        pages = QWidget()
        pl = QVBoxLayout(pages)
        pl.setContentsMargins(0, 0, 0, 0)
        pl.addWidget(self.home)
        pl.addWidget(self.content)
        logs_box = QWidget()
        lb = QVBoxLayout(logs_box)
        lb.setContentsMargins(0, 0, 0, 0)
        lb.addWidget(self.logs_view)
        row = QHBoxLayout()
        row.addWidget(self.copy_btn)
        row.addWidget(self.copy_note, 1)
        lb.addLayout(row)
        right = QSplitter(Qt.Vertical)
        right.addWidget(pages)
        right.addWidget(logs_box)
        right.setSizes([700, 260])
        split = QSplitter(Qt.Horizontal)
        split.addWidget(self.phase_list)
        split.addWidget(right)
        split.setStretchFactor(1, 1)
        split.setSizes([240, 1360])
        lay = QVBoxLayout(self)
        lay.setContentsMargins(6, 6, 6, 6)
        lay.addLayout(top)
        lay.addWidget(split, 1)

        state.logs_changed.connect(self._on_logs_changed)
        state.period_changed.connect(lambda _i: self.refresh())
        last = str(self.settings.value("guide/phase", HOME))
        self.phase_list.blockSignals(True)
        self.phase_list.setCurrentRow(PHASE_KEYS.index(last) + 1 if last in PHASE_KEYS else 0)
        self.phase_list.blockSignals(False)
        self._fill_card_combo(str(self.settings.value("guide/card", "")))

    # ------------------------------------------------------------ fiches
    def _fill_card_combo(self, select: str = "") -> None:
        self.card_combo.blockSignals(True)
        self.card_combo.clear()
        for c in self.cards:
            self.card_combo.addItem(c.name)
        idx = next((i for i, c in enumerate(self.cards) if c.name == select), 0 if self.cards else -1)
        self.card_combo.setCurrentIndex(idx)
        self.card_combo.blockSignals(False)
        self._on_card_changed(idx)

    def _on_card_changed(self, idx: int) -> None:
        self.card = self.cards[idx] if 0 <= idx < len(self.cards) else None
        self.edit_btn.setEnabled(self.card is not None)
        self.reset_btn.setEnabled(self.card is not None)
        self.delete_btn.setEnabled(self.card is not None)
        if self.card:
            self.settings.setValue("guide/card", self.card.name)
        self.refresh()

    def _current_log(self):
        ll = self.state.current_log()
        return ll.log if ll else None

    def new_card(self) -> None:
        log = self._current_log()
        dlg = CardDialog(card_from_log(log) if log is not None else DroneCard("Mon drone"), self, log)
        if dlg.exec() == QDialog.Accepted:
            self.add_card(dlg.card())

    def add_card(self, card: DroneCard) -> None:
        self.cards = [c for c in self.cards if c.name != card.name] + [card]
        save_cards(self.cards_file, self.cards)
        self._fill_card_combo(card.name)

    def edit_card(self) -> None:
        if self.card is None:
            return
        dlg = CardDialog(self.card, self, self._current_log())
        if dlg.exec() == QDialog.Accepted:
            old, new = self.card.name, dlg.card(self.card.done)
            self.cards = [new if c.name == old else c for c in self.cards]
            save_cards(self.cards_file, self.cards)
            self._fill_card_combo(new.name)

    def set_done(self, key: str, on: bool) -> None:
        if self.card is None:
            return
        if on:
            self.card.done[key] = True
        else:
            self.card.done.pop(key, None)
        save_cards(self.cards_file, self.cards)
        self._update_progress()

    def delete_card(self, confirm: bool = True) -> None:
        """Supprime la fiche choisie (et son avancement) du fichier des fiches ; les logs ne sont pas touchés."""
        if self.card is None:
            return
        if confirm and QMessageBox.question(
                self, "Supprimer la fiche",
                f"Supprimer la fiche « {self.card.name} » et son avancement ? Les logs ne sont pas touchés ; cette "
                "suppression ne peut pas être annulée.") != QMessageBox.Yes:
            return
        self.cards = [c for c in self.cards if c is not self.card]
        save_cards(self.cards_file, self.cards)
        self._fill_card_combo()

    def reset_progress(self, confirm: bool = True) -> None:
        """Décoche toutes les cases du drone (les cases vérifiées par un log ouvert se recochent seules)."""
        if self.card is None:
            return
        if confirm and QMessageBox.question(
                self, "Recommencer le parcours",
                f"Décocher toutes les cases de « {self.card.name} » ? Les cases vérifiées par un log ouvert "
                "resteront cochées tant que ce log est ouvert.") != QMessageBox.Yes:
            return
        self.card.done.clear()
        save_cards(self.cards_file, self.cards)
        self.refresh()

    # ------------------------------------------------------------ navigation
    def current_key(self) -> str:
        row = self.phase_list.currentRow()
        return HOME if row <= 0 else PHASE_KEYS[row - 1]

    def show_phase(self, key: str) -> None:
        row = 0 if key == HOME else PHASE_KEYS.index(key) + 1
        if self.phase_list.currentRow() == row:
            self._show_row(row)
        else:
            self.phase_list.setCurrentRow(row)

    def refresh(self) -> None:
        """Réaffiche la page (fiche changée, logs ouverts ou fermés, période, thème)."""
        self.copy_note.setStyleSheet(theme.muted_style())
        self._show_row(max(0, self.phase_list.currentRow()))

    def _phases(self) -> list[Phase]:
        return phases(self.card) if self.card else []

    def _show_row(self, row: int) -> None:
        if row < 0:
            return
        key = HOME if row == 0 else PHASE_KEYS[row - 1]
        self.settings.setValue("guide/phase", key)
        self.home.setVisible(key == HOME)
        self.content.setVisible(key != HOME)
        self._widgets = []
        if key == HOME:
            self.home.setMarkdown((GUIDE_DIR / HOME_FILE).read_text(encoding="utf-8"))
        else:
            self.content.setWidget(self._phase_widget(key))
        self._render_logs()
        self._update_progress()

    def _phase_widget(self, key: str) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        if self.card is None:
            msg = QLabel("Créez d'abord la fiche de votre drone (bouton « Nouveau drone » en haut) : les valeurs du "
                         "guide s'y adaptent.")
            msg.setWordWrap(True)
            lay.addWidget(msg)
            lay.addStretch(1)
            return w
        ph = next(p for p in self._phases() if p.key == key)
        lay.addWidget(QLabel(f"<h2>{html.escape(ph.title)}</h2>"))
        intro = QLabel(ph.intro)
        intro.setTextFormat(Qt.MarkdownText)
        intro.setWordWrap(True)
        lay.addWidget(intro)
        checks, targets = self._card_checks()
        for step in ph.steps:
            sw = StepWidget(self, step, self.card)
            sw.set_auto(*auto_status(step, checks, targets))
            self._widgets.append(sw)
            lay.addWidget(sw)
        lay.addStretch(1)
        return w

    def _on_link(self, url: QUrl) -> None:
        if url.scheme() != "otb":
            QDesktopServices.openUrl(url)  # lien externe : navigateur, au choix de l'utilisateur
            return
        kind, _, key = url.path().partition("/")
        if kind == "phase" and (key in PHASE_KEYS or key == HOME):
            self.show_phase(key)
        elif kind == "tool":
            self.open_tool(key)
        elif kind == "open" and key in ROLE_NAMES:
            self.open_logs(key)
        elif kind == "role":
            i, _, role = key.partition("/")
            if i.isdigit() and int(i) < len(self.state.logs) and role in ROLE_NAMES:
                self.set_role(str(self.state.logs[int(i)].log.path), role)

    def open_tool(self, key: str, part: str | None = None) -> None:
        """Ouvre un outil ; avec `part`, règle d'abord la période d'analyse du dernier log du drone sur cette
        partie du vol."""
        if self.main_window is None:
            return
        if part:
            idx, chk = self._last_card_log(FILTERS)
            if idx is not None and chk.parts.get(part):
                self.state.set_current(idx)
                self.state.set_period(idx, *chk.parts[part])
        if key == "viewer":
            self.main_window.show_viewer()
        else:
            self.main_window.open_tool(key)

    # ------------------------------------------------------------ logs du drone
    def _card_logs(self) -> list[int]:
        if self.card is None:
            return []
        return [i for i, ll in enumerate(self.state.logs)
                if card_for_log([self.card], ll.log.settings.craft_name) is not None]

    def _check(self, i: int):
        """(LogCheck, FilterTargets) d'un log : complets s'ils sont prêts, sinon vérification rapide (la complète
        est lancée en arrière-plan et réaffiche la page à la fin)."""
        ll = self.state.logs[i]
        full = ll.extras.get(("guide_check_full", ll.period))
        if full is not None:
            return full
        key = ("guide_check", ll.period)
        if key not in ll.extras:
            ll.extras[key] = (check_log(ll.log, ll.sel, ll.label), None)
        if id(ll) not in self._pending:
            self._pending.add(id(ll))
            inch = PROFILES[self.card.size].nominal_inch if self.card else None
            period = ll.period

            def work(log=ll.log, sel=ll.sel, label=ll.label):
                c = check_log(log, sel, label, exact=True)
                return c, filter_targets(log, c.parts, inch)

            def done(res, ll=ll):
                self._pending.discard(id(ll))
                if ll.period == period:
                    ll.extras[("guide_check_full", period)] = res
                    self.refresh()

            run_async(work, on_done=done, on_error=lambda _e, ll=ll: self._pending.discard(id(ll)))
        return ll.extras[key]

    def open_logs(self, role: str) -> None:
        """Fenêtre d'ouverture : les logs choisis seront lus comme logs de ce rôle (filtres ou séries PID)."""
        if self.main_window is None:
            return
        start = str(self.settings.value("last_dir", str(Path.home())))
        files, _ = QFileDialog.getOpenFileNames(self, f"Ouvrir les logs des {ROLE_NAMES[role]}", start,
                                                "Logs Blackbox (*.bbl *.bfl *.BBL *.BFL *.txt *.TXT);;Tous (*)")
        if files:
            for f in files:
                self._roles[str(Path(f))] = role
            self._save_roles()
            self.main_window.load_paths(files)

    def set_role(self, path: str, role: str) -> None:
        self._roles[path] = role
        self._save_roles()
        self.refresh()

    def _save_roles(self) -> None:
        self.settings.setValue("guide/log_roles", json.dumps(self._roles))

    def _role_check(self, i: int):
        """(LogCheck avec le rôle choisi par l'utilisateur, sinon deviné ; FilterTargets)."""
        c, t = self._check(i)
        role = self._roles.get(str(self.state.logs[i].log.path))
        return (dataclasses.replace(c, role=role) if role and role != c.role else c), t

    def _card_checks(self):
        res = [self._role_check(i) for i in self._card_logs()]
        filt = [t for c, t in res if c.role == FILTERS]
        return [c for c, _t in res], (filt[-1] if filt else None)

    def _last_card_log(self, role: str | None = None):
        for i in reversed(self._card_logs()):
            c = self._role_check(i)[0]
            if role is None or c.role == role:
                return i, c
        return None, None

    def _on_logs_changed(self) -> None:
        # log d'un drone qui a déjà une fiche : on passe sur cette fiche
        if self.state.logs:
            match = card_for_log(self.cards, self.state.logs[-1].log.settings.craft_name)
            if match is not None and match is not self.card:
                self._fill_card_combo(match.name)
                return
        self.refresh()

    def _render_logs(self) -> None:
        muted = theme.current().muted
        self._cli = []
        if self.card is None:
            self.logs_view.setHtml(f"<p style='color:{muted}'>Pas encore de fiche drone : bouton « Nouveau drone » "
                                   "en haut (préremplie d'après le log ouvert, s'il y en a un).</p>")
            self.copy_btn.setVisible(False)
            return
        idx = self._card_logs()
        others = [ll.label for i, ll in enumerate(self.state.logs) if i not in idx]
        parts = [f"<h3>Logs de « {html.escape(self.card.name)} »</h3>"]
        if not idx:
            parts.append(f"<p style='color:{muted}'>Aucun log de ce drone ouvert. Les logs sont reconnus par le "
                         "nom du drone dans Betaflight (onglet Configuration), qui doit être celui de la fiche.</p>")
        checks = {i: self._role_check(i)[0] for i in idx}
        for role in (FILTERS, PID):
            mine = [i for i in idx if checks[i].role == role]
            other = PID if role == FILTERS else FILTERS
            parts.append(f"<h4>Logs des {ROLE_NAMES[role]}</h4>")
            if not mine:
                parts.append(f"<p style='color:{muted}'>Aucun. <a href='otb:open/{role}'>{OPEN_LABELS[role]}</a></p>")
            for i in mine:
                ll, c = self.state.logs[i], checks[i]
                self._cli = c.cli_commands()
                pending = "" if c.exact else f" <span style='color:{muted}'>(vérification complète en cours…)</span>"
                usages = "<br>".join(
                    f"<span style='color:{theme.accent('green' if u.ok else 'red')}'>{'✔' if u.ok else '✖'}</span> "
                    f"{html.escape(u.label)}" + ("" if u.ok else f" : {html.escape(u.reason)}") for u in c.usages_for())
                move = f" <a href='otb:role/{i}/{other}'>→ classer dans les {ROLE_NAMES[other]}</a>"
                parts.append(f"<p><b style='color:{ll.color}'>{html.escape(ll.label)}</b>{pending}{move}<br>{usages}"
                             "</p><ul>")
                for it in c.items_for():
                    if it.status in (WARN, BLOCK):
                        acc, sym = _STATUS[it.status]
                        li = (f"<li><span style='color:{theme.accent(acc)}'>{sym} <b>{html.escape(it.title)}</b>"
                              f"</span> : {html.escape(it.detail)}")
                        for f in it.fixes:
                            li += f"<br>→ {html.escape(f.where)}"
                        parts.append(li + "</li>")
                parts.append("</ul>")
            for g in check_logs([(self.state.logs[i].label, self.state.logs[i].log) for i in mine]
                                if role == PID else [],  # une série : un seul curseur change
                                {self.state.logs[i].label: checks[i].sp_delay_ms for i in mine}):
                acc, sym = _STATUS[g.status]
                parts.append(f"<p><span style='color:{theme.accent(acc)}'>{sym}</span> {html.escape(g.detail)}</p>")
        if others:
            parts.append(f"<p style='color:{muted}'>Autres logs ouverts (autre drone) : "
                         f"{html.escape(', '.join(others))}.</p>")
        self.logs_view.setHtml("".join(parts))
        self.copy_btn.setVisible(bool(self._cli))
        self.copy_note.setText("")

    def _copy_cli(self) -> None:
        QGuiApplication.clipboard().setText("\n".join(self._cli + ["save"]))
        self.copy_note.setText(f"{len(self._cli)} commande(s) copiée(s), suivies de « save ».")

    # ------------------------------------------------------------ avancement
    def progress(self) -> dict[str, tuple[int, int]]:
        """Cases faites / total par phase (cochées à la main, ou vérifiées par un log)."""
        if self.card is None:
            return {}
        checks, targets = self._card_checks()
        return {ph.key: (sum(1 for s in ph.steps if self.card.done.get(s.key)
                             or auto_status(s, checks, targets)[0] == AUTO_OK), len(ph.steps))
                for ph in self._phases()}

    def _update_progress(self) -> None:
        prog = self.progress()
        titles = dict(zip(PHASE_KEYS, ("Préparer le drone", "Filtres", "PID par séries", "Finitions")))
        current = self.current_key()
        self.phase_list.blockSignals(True)
        self.phase_list.item(0).setText("Accueil")
        for k, key in enumerate(PHASE_KEYS, start=1):
            n, tot = prog.get(key, (0, 0))
            mark = "✔ " if tot and n == tot else ""
            count = f" ({n}/{tot})" if tot else ""
            self.phase_list.item(k).setText(f"{mark}{k}. {titles[key]}{count}")
            b = self.strip_buttons[key]
            b.setText(f"{mark}{k}. {titles[key]}{count}")
            b.setStyleSheet(theme.accent_style("blue") if key == current else "")
        self.phase_list.blockSignals(False)
