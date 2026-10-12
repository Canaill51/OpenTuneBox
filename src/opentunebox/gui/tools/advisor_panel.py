"""Assistant de réglage : rapport de diagnostic argumenté pour un log."""

from __future__ import annotations

import html

import numpy as np
import pyqtgraph as pg
from PySide6.QtWidgets import QComboBox, QLabel, QTextBrowser

from ...advisor.profiles import PROFILES
from ...advisor.rules import Finding, analyze
from .. import theme
from .base import ToolWindow


def finding_html(f: Finding) -> str:
    """Un constat : étiquette de gravité (badge coloré de theme.SEVERITY_COLORS), titre, détail, puis les
    actions en liste."""
    color = theme.SEVERITY_COLORS.get(f.severity, theme.current().muted)
    out = (f"<p><span style='background:{color}; color:{theme.ON_ACCENT}; padding:1px 5px'>"
           f"&nbsp;{html.escape(f.severity.upper())}&nbsp;</span> "
           f"<b>{html.escape(f.title)}</b><br>{html.escape(f.detail)}</p>")
    if f.actions:
        out += "<ul>" + "".join(f"<li>{html.escape(a)}</li>" for a in f.actions) + "</ul>"
    return out


class AdvisorWindow(ToolWindow):
    title = "Assistant de réglage"
    multi_file = False

    def build_controls(self, layout) -> None:
        layout.addWidget(QLabel("Taille du drone"))
        self.profile_combo = QComboBox()
        self.profile_combo.addItem("Détection automatique", None)
        for key, p in PROFILES.items():
            self.profile_combo.addItem(p.label, key)
        self.profile_combo.currentIndexChanged.connect(lambda _i: self.run())
        layout.addWidget(self.profile_combo)
        help_ = QLabel("Les conseils portent sur la période d'analyse choisie dans le visualiseur. "
                       "Changez une seule chose à la fois, revolez, et comparez les logs.")
        help_.setWordWrap(True)
        help_.setStyleSheet(theme.muted_style())
        layout.addWidget(help_)

    def __init__(self, state, main_window=None) -> None:
        super().__init__(state, main_window)
        # le rapport texte occupe la place principale ; les courbes en dessous
        self.report = QTextBrowser()
        self.report.setOpenExternalLinks(False)
        splitter = self.canvas.parentWidget()
        splitter.insertWidget(0, self.report)
        splitter.setSizes([780, 420, 280])

    def snapshot_options(self) -> dict:
        return {"profile": self.profile_combo.currentData()}

    def compute(self, entries):
        e = entries[0]
        key = self.run_options.get("profile")  # lu par snapshot_options : pas de widget depuis ce thread
        prof = PROFILES[key] if key else None
        return e, analyze(e.log, e.sel, prof)

    def render(self, result) -> None:
        e, rep = result
        parts = [
            f"<h2 style='color:{e.color}'>{html.escape(e.label)}</h2>",
            f"<p><b>Drone :</b> {html.escape(rep.profile.label)} <i>({html.escape(rep.profile_reason)})</i><br>"
            f"<b>Période analysée :</b> {rep.period[0]:.1f} – {rep.period[1]:.1f} s · "
            f"<b>Firmware :</b> {html.escape(e.log.settings.firmware)}</p>",
        ]
        order = ["Enregistrement", "Vol", "Réponse", "Bruit", "Filtres", "Moteurs", "Mécanique", "Réglage"]
        findings = rep.sorted()
        for cat in order + sorted({f.category for f in findings} - set(order)):
            items = [f for f in findings if f.category == cat]
            if not items:
                continue
            parts.append(f"<h3>{html.escape(cat)}</h3>")
            parts.extend(finding_html(f) for f in items)
        parts.append(f"<hr><p style='color:{theme.current().muted}'>Diagnostic automatique à partir des mesures du log : "
                     "à confirmer par le ressenti en vol et la température des moteurs.</p>")
        self.report.setHtml("".join(parts))

        c = self.canvas
        c.clear()
        for row, st in enumerate(rep.steps):
            p = c.addPlot(row=row, col=0, title=f"{st.axis} — réponse indicielle")
            p.showGrid(x=True, y=True, alpha=0.2)
            p.setYRange(0, 1.5)
            p.addLine(y=1.0, pen=pg.mkPen(theme.current().fg, style=pg.QtCore.Qt.DashLine))
            r = st.result
            if r.n:
                p.plot(r.t_ms, r.mean, pen=pg.mkPen(e.color, width=2))
                lo, hi = r.mean - r.std, r.mean + r.std
                fill = pg.FillBetweenItem(pg.PlotDataItem(r.t_ms, lo), pg.PlotDataItem(r.t_ms, hi),
                                          brush=pg.mkBrush(pg.mkColor(e.color).red(), pg.mkColor(e.color).green(),
                                                           pg.mkColor(e.color).blue(), 40))
                p.addItem(fill)
                m = r.metrics
                txt = pg.TextItem(f"pic {m.peak:.2f} · latence {m.latency_ms:.1f} ms · n={r.n}",
                                  color=theme.current().fg, anchor=(1, 0))
                txt.setPos(500, 1.48)
                p.addItem(txt)
            else:
                p.addItem(pg.TextItem("données insuffisantes", color=theme.current().fg))
            if row == 2:
                p.setLabel("bottom", "Temps (ms)")
        _ = np  # numpy utilisé indirectement par pyqtgraph
