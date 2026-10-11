"""Guide intégré : la procédure de réglage pas à pas, et la vérification des logs ouverts.

Le texte de chaque étape est un fichier Markdown de `assets/guide/` (livré avec l'application). Liens
internes : `otb:step/<clé>` ouvre une autre étape, `otb:tool/<clé>` ouvre un outil (`viewer` : visualiseur).
L'étape « Vérifier mes logs » est calculée par `advisor/log_check.py`.
"""

from __future__ import annotations

import html
from pathlib import Path

from PySide6.QtCore import QSettings, Qt, QUrl
from PySide6.QtGui import QDesktopServices, QGuiApplication
from PySide6.QtWidgets import (QHBoxLayout, QLabel, QListWidget, QPushButton, QSplitter, QTextBrowser, QVBoxLayout,
                               QWidget)

from ..advisor.log_check import BLOCK, INFO, OK, WARN, check_log, check_logs
from . import theme

GUIDE_DIR = Path(__file__).parent / "assets" / "guide"
CHECK_STEP = "verifier"

# (clé, titre dans la liste, fichier Markdown ; None : étape calculée)
STEPS = [
    ("bienvenue", "Bienvenue", "01-bienvenue.md"),
    ("prerequis", "1. Prérequis", "02-prerequis.md"),
    ("enregistrement", "2. Préparer l'enregistrement", "03-enregistrement.md"),
    ("recuperer", "3. Récupérer et ouvrir les logs", "04-recuperer-logs.md"),
    (CHECK_STEP, "4. Vérifier mes logs", None),
    ("appliquer", "5. Appliquer une modification", "05-appliquer.md"),
    ("filtres", "6. Régler les filtres", "06-filtres.md"),
    ("pid", "7. Régler les PID par séries", "07-pid-series.md"),
    ("finitions", "8. Finitions", "08-finitions.md"),
    ("depannage", "Dépannage", "09-depannage.md"),
]
STEP_KEYS = [s[0] for s in STEPS]

# état d'un point de contrôle → (accent du thème, symbole)
_STATUS = {OK: ("green", "✔"), WARN: ("orange", "▲"), BLOCK: ("red", "✖"), INFO: ("blue", "ℹ")}


def step_markdown(key: str) -> str:
    fname = next(f for k, _t, f in STEPS if k == key)
    return (GUIDE_DIR / fname).read_text(encoding="utf-8")


class GuidePage(QWidget):
    def __init__(self, state, main_window=None) -> None:
        super().__init__()
        self.state = state
        self.main_window = main_window
        self.settings = QSettings("OpenTuneBox", "OpenTuneBox")

        self.steps = QListWidget()
        for _k, title, _f in STEPS:
            self.steps.addItem(title)
        self.steps.currentRowChanged.connect(self._show_row)

        self.text = QTextBrowser()
        self.text.setOpenLinks(False)
        self.text.anchorClicked.connect(self._on_link)

        self.prev_btn = QPushButton("◀ Précédent")
        self.prev_btn.clicked.connect(lambda: self.steps.setCurrentRow(self.steps.currentRow() - 1))
        self.next_btn = QPushButton("Suivant ▶")
        self.next_btn.clicked.connect(lambda: self.steps.setCurrentRow(self.steps.currentRow() + 1))
        self.copy_btn = QPushButton("Copier les commandes CLI")
        self.copy_btn.setToolTip("Commandes équivalentes aux corrections proposées pour le log affiché, à coller dans "
                                 "l'onglet « Ligne de commande (CLI) » du configurateur, si vous préférez le CLI")
        self.copy_btn.clicked.connect(self._copy_cli)
        self.copy_note = QLabel()
        self.copy_note.setStyleSheet(theme.muted_style())

        right = QWidget()
        rl = QVBoxLayout(right)
        rl.setContentsMargins(0, 0, 0, 0)
        rl.addWidget(self.text, 1)
        row = QHBoxLayout()
        row.addWidget(self.prev_btn)
        row.addWidget(self.copy_btn)
        row.addWidget(self.copy_note)
        row.addStretch(1)
        row.addWidget(self.next_btn)
        rl.addLayout(row)

        split = QSplitter(Qt.Horizontal)
        split.addWidget(self.steps)
        split.addWidget(right)
        split.setStretchFactor(1, 1)
        split.setSizes([260, 1340])
        lay = QVBoxLayout(self)
        lay.setContentsMargins(6, 6, 6, 6)
        lay.addWidget(split)

        self._cli: list[str] = []
        state.logs_changed.connect(self._on_logs_changed)
        state.current_changed.connect(lambda _i: self._on_logs_changed())
        state.period_changed.connect(lambda _i: self._on_logs_changed())
        last = str(self.settings.value("guide/step", STEPS[0][0]))
        self.steps.setCurrentRow(STEP_KEYS.index(last) if last in STEP_KEYS else 0)

    # ------------------------------------------------------------ navigation
    def current_key(self) -> str:
        return STEP_KEYS[max(0, self.steps.currentRow())]

    def show_step(self, key: str) -> None:
        self.steps.setCurrentRow(STEP_KEYS.index(key))

    def refresh(self) -> None:
        """Réaffiche l'étape (changement de thème, logs ouverts ou fermés)."""
        self.copy_note.setStyleSheet(theme.muted_style())
        self._show_row(self.steps.currentRow())

    def _show_row(self, row: int) -> None:
        if row < 0:
            return
        key = STEP_KEYS[row]
        self.settings.setValue("guide/step", key)
        self.prev_btn.setEnabled(row > 0)
        self.next_btn.setEnabled(row < len(STEPS) - 1)
        check = key == CHECK_STEP
        if check:
            self.text.setHtml(self._check_html())
        else:
            self.text.setMarkdown(step_markdown(key))
        self.copy_btn.setVisible(check and bool(self._cli))
        self.copy_note.setText("")
        self.text.verticalScrollBar().setValue(0)

    def _on_link(self, url: QUrl) -> None:
        if url.scheme() != "otb":
            QDesktopServices.openUrl(url)  # lien externe : navigateur, au choix de l'utilisateur
            return
        kind, _, key = url.path().partition("/")
        if kind == "step" and key in STEP_KEYS:
            self.show_step(key)
        elif kind == "tool" and self.main_window is not None:
            if key == "viewer":
                self.main_window.show_viewer()
            else:
                self.main_window.open_tool(key)

    def _on_logs_changed(self) -> None:
        if self.current_key() == CHECK_STEP:
            self.refresh()

    def _copy_cli(self) -> None:
        QGuiApplication.clipboard().setText("\n".join(self._cli + ["save"]))
        self.copy_note.setText(f"{len(self._cli)} commande(s) copiée(s), suivies de « save ».")

    # ------------------------------------------------------------ vérification des logs
    def _check_html(self) -> str:
        ui = theme.current().ui
        muted = theme.current().muted
        parts = ["<h2>4. Vérifier mes logs</h2>"]
        self._cli = []
        if not self.state.logs:
            parts.append("<p>Aucun log ouvert. Ouvrez un ou plusieurs logs (bouton <b>Ouvrir</b>, ou glissez les "
                         "fichiers dans la fenêtre) : cette page dit alors, pour chacun, à quoi il peut servir et "
                         "quoi corriger avant le prochain vol.</p>"
                         "<p><a href='otb:step/recuperer'>Comment récupérer les logs du drone</a></p>")
            return "".join(parts)
        parts.append(f"<p style='color:{muted}'>Sur la période d'analyse de chaque log (réglable dans le "
                     "visualiseur). Les corrections indiquent d'abord où les faire dans le configurateur Betaflight, "
                     "puis la commande CLI équivalente. Changez-les <b>avant</b> le prochain vol.</p>")
        for i, ll in enumerate(self.state.logs):
            key = ("log_check", ll.period)
            c = ll.extras.get(key)
            if c is None:
                c = ll.extras[key] = check_log(ll.log, ll.sel, ll.label)
            if i == self.state.current:  # commandes du log affiché seulement (les autres peuvent être d'un autre drone)
                self._cli = c.cli_commands()
                self.copy_btn.setText(f"Copier les commandes CLI de « {ll.label} »")
            parts.append(f"<h3 style='color:{ll.color}'>{html.escape(ll.label)}</h3>")
            parts.append("<table cellspacing='0' cellpadding='3'>")
            for u in c.usages:
                acc = "green" if u.ok else "red"
                parts.append(f"<tr><td style='color:{theme.accent(acc)}'><b>{'✔ oui' if u.ok else '✖ non'}</b></td>"
                             f"<td><b>{html.escape(u.label)}</b> — {html.escape(u.reason)}</td></tr>")
            parts.append("</table><ul>")
            for it in c.items:
                acc, sym = _STATUS[it.status]
                li = (f"<li><span style='color:{theme.accent(acc)}'><b>{sym} {html.escape(it.title)}</b></span> : "
                      f"{html.escape(it.detail)}")
                if it.status in (WARN, BLOCK):
                    for f in it.fixes:
                        li += f"<br>→ {html.escape(f.where)}"
                        if f.cli:
                            li += (f" <span style='color:{muted}'>CLI :</span> "
                                   f"<code style='background:{ui['alt']}'>{html.escape(f.cli)}</code>")
                parts.append(li + "</li>")
            parts.append("</ul>")
        groups = check_logs([(ll.label, ll.log) for ll in self.state.logs])
        if groups:
            parts.append("<h3>Logs du même drone</h3><ul>")
            for g in groups:
                acc, sym = _STATUS[g.status]
                parts.append(f"<li><span style='color:{theme.accent(acc)}'><b>{sym} {html.escape(g.craft)}</b></span> "
                             f"({html.escape(', '.join(g.labels))}) : {html.escape(g.detail)}</li>")
            parts.append("</ul>")
        parts.append("<p>Ensuite : <a href='otb:step/appliquer'>appliquer une modification</a>, "
                     "<a href='otb:step/filtres'>régler les filtres</a> ou "
                     "<a href='otb:step/pid'>régler les PID par séries</a>.</p>")
        return "".join(parts)
