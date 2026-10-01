"""Statistiques de vol et erreur PID (plusieurs logs comparés)."""

from __future__ import annotations

import numpy as np
import pyqtgraph as pg
from PySide6.QtWidgets import QLabel, QTextEdit

from ...analysis.stats import flight_stats, pid_error_stats
from .. import theme
from .base import ToolWindow


class FlightStatsWindow(ToolWindow):
    title = "Statistiques de vol"

    def build_controls(self, layout) -> None:
        layout.addWidget(QLabel("Résumé"))
        self.summary = QTextEdit()
        self.summary.setReadOnly(True)
        self.summary.setMinimumHeight(260)
        layout.addWidget(self.summary)

    def compute(self, entries):
        out = []
        for e in entries:
            out.append((e, flight_stats(e.log, e.sel), pid_error_stats(e.log, e.sel)))
        return out

    def render(self, result) -> None:
        c = self.canvas
        c.clear()
        axes = self.axes() or [0, 1, 2]
        n = len(result)
        width = 0.8 / max(1, n)

        # Ligne 1 : histogrammes des manches et des gaz
        for col, name in enumerate(("Roll", "Pitch", "Yaw", "Gaz")):
            p = c.addPlot(row=0, col=col, title=f"{name} (% du vol)")
            p.showGrid(x=True, y=True, alpha=0.2)
            for e, fs, _ in result:
                x, y = fs.stick_hist[name]
                p.plot(x, y, pen=pg.mkPen(e.color, width=2), stepMode=None)
            p.setLabel("bottom", "%" if name == "Gaz" else "déflexion %")

        # Ligne 2 : |gyro|, |P|, |I|, |D|, |F| moyens par axe (barres par fichier)
        terms = ("gyro", "P", "I", "D", "F")
        for col, ax in enumerate(axes[:3]):
            p = c.addPlot(row=1, col=col, title=f"{theme.AXIS_NAMES[ax]} : |valeurs| moyennes")
            p.showGrid(y=True, alpha=0.2)
            for k, (e, fs, _) in enumerate(result):
                xs = np.arange(len(terms)) + (k - (n - 1) / 2) * width
                means = [fs.terms[t][ax].mean for t in terms]
                sds = [fs.terms[t][ax].sd for t in terms]
                p.addItem(pg.BarGraphItem(x=xs, height=means, width=width * 0.9, brush=e.color))
                p.addItem(pg.ErrorBarItem(x=xs, y=np.array(means), top=np.array(sds) / 2, bottom=np.array(sds) / 2,
                                          beam=width * 0.3, pen=pg.mkPen(theme.current().fg)))
            p.getAxis("bottom").setTicks([[(i, t if t != "gyro" else "|gyro| °/s") for i, t in enumerate(terms)]])

        # moteurs
        p = c.addPlot(row=1, col=3, title="Moteurs (%)")
        p.showGrid(y=True, alpha=0.2)
        for k, (e, fs, _) in enumerate(result):
            nm = len(fs.motors)
            xs = np.arange(nm) + (k - (n - 1) / 2) * width
            p.addItem(pg.BarGraphItem(x=xs, height=[m.mean for m in fs.motors], width=width * 0.9, brush=e.color))
        p.getAxis("bottom").setTicks([[(i, f"M{i + 1}") for i in range(8)]])

        # Ligne 3 : erreur PID
        for col, ax in enumerate(axes[:3]):
            p = c.addPlot(row=2, col=col, title=f"{theme.AXIS_NAMES[ax]} : erreur PID")
            p.showGrid(x=True, y=True, alpha=0.2)
            p.setLabel("bottom", "erreur (°/s)")
            p.setLabel("left", "% des échantillons")
            for e, _, pe in result:
                x = 0.5 * (pe.edges[:-1] + pe.edges[1:])
                p.plot(x, pe.hist[ax], pen=pg.mkPen(e.color, width=2), name=f"σ={pe.sd[ax]:.1f}")
        p = c.addPlot(row=2, col=3, title="|erreur| vs manche")
        p.showGrid(x=True, y=True, alpha=0.2)
        p.setLabel("bottom", "déflexion %")
        p.setLabel("left", "°/s")
        styles = [pg.QtCore.Qt.SolidLine, pg.QtCore.Qt.DashLine, pg.QtCore.Qt.DotLine]
        for e, _, pe in result:
            for ax in axes[:3]:
                p.plot(pe.stick_bins, pe.err_vs_stick[ax], pen=pg.mkPen(e.color, width=2, style=styles[ax]),
                       symbol="o", symbolSize=4, symbolBrush=e.color)

        # Résumé texte
        lines = []
        for e, fs, pe in result:
            lines.append(f"<b style='color:{e.color}'>{e.label}</b> — {fs.duration_s:.1f} s analysées")
            lines.append(f"Gaz moyen {fs.throttle.mean:.0f} % (max {fs.throttle.max:.0f} %)")
            lines.append(f"Saturation moteur : {fs.motor_saturation_pct:.1f} % du temps")
            if np.isfinite(fs.motor_imbalance_pct):
                lines.append(f"Déséquilibre moteurs : {fs.motor_imbalance_pct:.1f} %")
            if fs.vbat is not None:
                lines.append(f"Batterie : moy. {fs.vbat.mean:.2f} V, min {fs.vbat_min:.2f} V")
            if fs.current is not None:
                lines.append(f"Courant : moy. {fs.current.mean:.1f} A, max {fs.current.max:.1f} A"
                             + (f", {fs.mah:.0f} mAh" if fs.mah else ""))
            lines.append("Erreur PID σ robuste (R/P/Y, |consigne| ≤ 100 °/s) : " + " / ".join(f"{v:.1f}" for v in pe.sd) + " °/s")
            lines.append("")
        self.summary.setHtml("<br>".join(lines))
