"""Fenêtre « Curseurs PID » : positions des curseurs du réglage simplifié Betaflight et simulation.

En haut : position des 8 curseurs de chaque fichier sélectionné (lus dans l'en-tête, ou estimés à
partir des PID quand le réglage simplifié est désactivé) et PID actuels par axe.
En bas : éditeur « et si… » — on bouge les curseurs (de 5 en 5 %, comme le configurateur), on voit
aussitôt les PID que calculerait le firmware (même arithmétique que `simplified_tuning.c`),
l'écart avec les PID actuels, les fréquences des filtres, et les commandes CLI à coller.
"""

from __future__ import annotations

from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QFont
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QComboBox,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QPlainTextEdit,
    QPushButton,
    QSlider,
    QSpinBox,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from ...analysis.sliders import (
    PTB_CATEGORIES,
    SliderView,
    cli_commands,
    filters_from_sliders,
    pids_from_sliders,
    slider_view,
    sliders_from_pids,
)
from ...log.settings import AxisPID, Settings
from .. import theme
from ._step_common import CATEGORY_BF, draw_slider_plot, mode_text, wrap_canvas
from .base import ToolWindow

# (clé du curseur, libellé du configurateur Betaflight, explication courte)
SLIDERS = [
    ("d_gain", "Damping", "D Gains (et D max) : freine les dépassements, plus de bruit moteur."),
    ("pi_gain", "Tracking", "P & I Gains ensemble : colle mieux à la consigne, trop → oscillations."),
    ("ff_gain", "Stick Response", "FF Gains (feedforward) : réactivité aux mouvements de manche."),
    ("dmax_gain", "Dynamic Damping", "D Max : D renforcé seulement pendant les mouvements vifs."),
    ("i_gain", "Drift - Wobble", "I Gains : tient le cap (vent, gaz) ; trop → lente oscillation."),
    ("pitch_d", "Pitch Damping", "Pitch:Roll D : rapport D tangage / roulis."),
    ("pitch_pi", "Pitch Tracking", "Pitch:Roll P, I & FF : rapport P, I, F tangage / roulis."),
    ("master", "Master Multiplier", "Multiplie tous les gains (P, I, D, F) à la fois."),
]
PID_COLS = ("P", "I", "D", "Dmax", "F")


@dataclass
class FileSliders:
    label: str
    color: str
    settings: Settings
    view: SliderView
    mode: int

    @property
    def family(self) -> str:
        return self.settings.family

    @property
    def pids(self) -> list[AxisPID]:
        return self.settings.pids


def _snap(v: float, step: int = 5) -> int:
    return int(min(200, max(0, step * round(v / step))))


class PidSliderWindow(ToolWindow):
    title = "Curseurs PID"
    multi_file = True

    def __init__(self, state, main_window=None) -> None:
        self._last_result = None
        self._ref: FileSliders | None = None
        self._updating = False
        self._files: list[FileSliders] = []
        self.slider_plot = None
        super().__init__(state, main_window)
        for cb in self.axis_boxes:  # sans objet ici : les curseurs agissent sur tous les axes
            cb.hide()
        _top, bottom = wrap_canvas(self)
        bottom.addWidget(self._build_editor())
        self.canvas.setMinimumHeight(330)

    # ------------------------------------------------------------ éditeur « et si… »
    def build_controls(self, layout: QVBoxLayout) -> None:
        help_ = QLabel(
            "<b>Les curseurs Betaflight</b><br>"
            "• <b>Damping</b> = D<br>• <b>Tracking</b> = P et I<br>• <b>Stick Response</b> = F (feedforward)<br>"
            "• <b>Dynamic Damping</b> = D max<br>• <b>Drift - Wobble</b> = I<br>"
            "• <b>Pitch Damping / Pitch Tracking</b> = rapport tangage/roulis<br>• <b>Master Multiplier</b> = tout<br><br>"
            "Point plein : curseurs lus dans l'en-tête du log. Point creux : réglage simplifié désactivé, "
            "position estimée à partir des PID. Losange : votre simulation.")
        help_.setWordWrap(True)
        help_.setStyleSheet(theme.muted_style())
        layout.addWidget(help_)

    def _build_editor(self) -> QWidget:
        box = QGroupBox("Simulation « et si… »")
        outer = QHBoxLayout(box)

        left = QVBoxLayout()
        head = QHBoxLayout()
        self.ref_label = QLabel("Fichier : —")
        self.ref_label.setStyleSheet("font-weight:600")
        head.addWidget(self.ref_label, 1)
        head.addWidget(QLabel("Mode"))
        self.mode = QComboBox()
        self.mode.addItems(["OFF", "RP", "RPY"])
        self.mode.setCurrentIndex(2)
        self.mode.currentIndexChanged.connect(self._update_whatif)
        head.addWidget(self.mode)
        reset = QPushButton("Repartir du fichier")
        reset.clicked.connect(self._init_from_ref)
        head.addWidget(reset)
        left.addLayout(head)

        grid = QGridLayout()
        grid.setHorizontalSpacing(8)
        grid.setVerticalSpacing(2)
        self.sl_widgets: dict[str, tuple[QSlider, QSpinBox]] = {}
        rows = [(k, n, h, 0, 200) for k, n, h in SLIDERS] + [
            ("gyro_mult", "Filtre gyro", "Multiplie les coupures des filtres gyro (plus haut = moins de retard).", 10, 200),
            ("dterm_mult", "Filtre D", "Multiplie les coupures des filtres du D.", 10, 200),
        ]
        self.filter_on: dict[str, QCheckBox] = {}
        for r, (key, name, hint, lo, hi) in enumerate(rows):
            lab = QLabel(name)
            lab.setToolTip(hint)
            s = QSlider(Qt.Horizontal)
            s.setRange(lo // 5, hi // 5)  # pas de 5 %
            s.setMinimumWidth(180)
            s.setToolTip(hint)
            sp = QSpinBox()
            sp.setRange(lo, hi)
            sp.setSingleStep(5)
            sp.setSuffix(" %")
            s.valueChanged.connect(lambda v, sp=sp: sp.setValue(v * 5) if sp.value() != v * 5 else None)
            sp.valueChanged.connect(lambda v, s=s: (s.blockSignals(True), s.setValue(round(v / 5)),
                                                    s.blockSignals(False), self._update_whatif()))
            grid.addWidget(lab, r, 0)
            grid.addWidget(s, r, 1)
            grid.addWidget(sp, r, 2)
            if key in ("gyro_mult", "dterm_mult"):
                cb = QCheckBox("actif")
                cb.setToolTip("Curseur de filtre actif (simplified_gyro_filter / simplified_dterm_filter)")
                cb.toggled.connect(self._update_whatif)
                grid.addWidget(cb, r, 3)
                self.filter_on[key] = cb
            else:
                h = QLabel(hint)
                h.setStyleSheet(theme.muted_style())
                grid.addWidget(h, r, 3)
            self.sl_widgets[key] = (s, sp)
        grid.setColumnStretch(3, 1)
        left.addLayout(grid)
        outer.addLayout(left, 3)

        right = QVBoxLayout()
        self.table = QTableWidget(3, len(PID_COLS))
        self.table.setHorizontalHeaderLabels(list(PID_COLS))
        self.table.setVerticalHeaderLabels(list(theme.AXIS_NAMES))
        self.table.horizontalHeader().setSectionResizeMode(QHeaderView.Stretch)
        self.table.verticalHeader().setSectionResizeMode(QHeaderView.Stretch)
        self.table.setEditTriggers(QTableWidget.NoEditTriggers)
        self.table.setFixedHeight(120)
        right.addWidget(QLabel("PID résultants (écart avec le fichier) :"))
        right.addWidget(self.table)
        self.filters_label = QLabel("")
        self.filters_label.setTextFormat(Qt.RichText)
        right.addWidget(self.filters_label)
        cli_head = QHBoxLayout()
        cli_head.addWidget(QLabel("Commandes CLI Betaflight :"), 1)
        copy = QPushButton("Copier")
        copy.clicked.connect(self._copy_cli)
        cli_head.addWidget(copy)
        right.addLayout(cli_head)
        self.cli = QPlainTextEdit()
        self.cli.setReadOnly(True)
        self.cli.setFont(QFont("monospace", 9))
        self.cli.setMinimumHeight(110)
        right.addWidget(self.cli, 1)
        outer.addLayout(right, 2)
        return box

    # ------------------------------------------------------------ calcul / affichage
    def compute(self, entries) -> list[FileSliders]:
        out = []
        for e in entries:
            s = e.log.settings
            out.append(FileSliders(e.label, e.color, s, slider_view(s), int(s.sliders().get("mode", 0) or 0)))
        return out

    def render(self, result: list[FileSliders]) -> None:
        c = self.canvas
        c.clear()
        c.setBackground(theme.current().bg)
        self._files = result
        if not result:
            return
        self.slider_plot = c.addPlot(row=0, col=0)
        self.info = c.addLabel(self._info_html(result), row=0, col=1, justify="left")
        c.ci.layout.setAlignment(self.info, Qt.AlignTop | Qt.AlignLeft)
        c.ci.layout.setColumnStretchFactor(0, 3)
        c.ci.layout.setColumnStretchFactor(1, 2)
        ref = result[0]
        if self._ref is None or self._ref.label != ref.label or self._ref.settings is not ref.settings:
            self._ref = ref
            self._init_from_ref()
        else:
            self._ref = ref
            self._update_whatif()

    def _info_html(self, result: list[FileSliders]) -> str:
        fg = theme.current().fg
        rows = [f"<b style='color:{fg}'>PID actuels (P, I, D, Dmax, F)</b>"]
        for i, f in enumerate(result):
            src = "curseurs de l'en-tête" if f.view.from_header else "curseurs estimés"
            warn = "" if f.view.consistent else " — ⚠ PID modifiés à la main"
            rows.append(f"<span style='color:{f.color}; font-weight:600'>{i + 1}) {f.label}</span>"
                        f"<span style='color:{fg}'> &nbsp;BF {f.family}, mode {mode_text(f.mode)}, {src}{warn}</span>")
            pid = " &nbsp;|&nbsp; ".join(f"{a[0]} {p.text()}" for a, p in zip(theme.AXIS_NAMES, f.pids))
            rows.append(f"<span style='color:{f.color}'>&nbsp;&nbsp;&nbsp;{pid}</span>")
            pos = ", ".join(f"{CATEGORY_BF[k][0]} {v:.2f}" for k, v in f.view.values.items())
            rows.append(f"<span style='color:{fg}; font-size:8pt'>&nbsp;&nbsp;&nbsp;{pos}</span>")
        return "<br>".join(rows)

    # ------------------------------------------------------------ simulation
    def _init_from_ref(self) -> None:
        ref = self._ref
        if ref is None:
            return
        s = ref.settings
        sl = s.sliders()
        if ref.view.from_header:
            vals = {k: sl.get(k, 100) for k, *_ in SLIDERS}
            mode = ref.mode
        else:
            est = sliders_from_pids(s.pids, s.family)
            vals = {k: _snap(est[k]) for k, *_ in SLIDERS}
            mode = 2
        vals["gyro_mult"] = sl.get("gyro_mult", 100) if sl.get("gyro_mult", -1) >= 0 else 100
        vals["dterm_mult"] = sl.get("dterm_mult", 100) if sl.get("dterm_mult", -1) >= 0 else 100
        self._updating = True
        for key, v in vals.items():
            s_, sp = self.sl_widgets[key]
            sp.setValue(int(v))
            s_.blockSignals(True)
            s_.setValue(round(int(v) / 5))
            s_.blockSignals(False)
        self.mode.setCurrentIndex(max(0, min(2, mode)))
        self.filter_on["gyro_mult"].setChecked(sl.get("gyro_filter", 0) == 1)
        self.filter_on["dterm_mult"].setChecked(sl.get("dterm_filter", 0) == 1)
        self._updating = False
        self.ref_label.setText(f"Fichier : {ref.label}  (BF {ref.family})")
        self.ref_label.setStyleSheet(f"font-weight:600; color:{ref.color}")
        self._update_whatif()

    def whatif_sliders(self) -> dict:
        out = {k: self.sl_widgets[k][1].value() for k, *_ in SLIDERS}
        out["mode"] = self.mode.currentIndex()
        out["gyro_filter"] = int(self.filter_on["gyro_mult"].isChecked())
        out["dterm_filter"] = int(self.filter_on["dterm_mult"].isChecked())
        out["gyro_mult"] = self.sl_widgets["gyro_mult"][1].value()
        out["dterm_mult"] = self.sl_widgets["dterm_mult"][1].value()
        return out

    def _update_whatif(self, *_a) -> None:
        if self._updating or self._ref is None:
            return
        ref = self._ref
        sl = self.whatif_sliders()
        new = pids_from_sliders(sl, ref.family, base=ref.pids)
        fg = QColor(theme.current().fg)
        for ax in range(3):
            for col, name in enumerate(PID_COLS):
                v, v0 = getattr(new[ax], name), getattr(ref.pids[ax], name)
                d = v - v0
                it = QTableWidgetItem(f"{v}" + (f"  ({d:+d})" if d else ""))
                it.setTextAlignment(Qt.AlignCenter)
                it.setForeground(QColor(theme.accent("green")) if d > 0 else QColor(theme.accent("red")) if d < 0 else fg)
                self.table.setItem(ax, col, it)
        self.filters_label.setText(self._filters_html(sl))
        self.cli.setPlainText("\n".join(self._cli_lines(sl)))
        if getattr(self, "slider_plot", None) is not None and self._files:
            files = [(f.color, f.view) for f in self._files]
            extra = {cat: sl[key] / 100.0 for cat, key in PTB_CATEGORIES.items()}
            draw_slider_plot(self.slider_plot, files, extra, "◇ simulation")

    def _filters_html(self, sl: dict) -> str:
        s = self._ref.settings
        new = filters_from_sliders(sl["gyro_mult"], sl["dterm_mult"], s.family, s)

        def cur(lp, prefix):
            return {f"{prefix}_lpf1_dyn_min_hz": lp[0].dyn_min_hz, f"{prefix}_lpf1_dyn_max_hz": lp[0].dyn_max_hz,
                    f"{prefix}_lpf1_static_hz": lp[0].static_hz, f"{prefix}_lpf2_static_hz": lp[1].static_hz}

        now = {**cur((s.gyro_lpf1, s.gyro_lpf2), "gyro"), **cur((s.dterm_lpf1, s.dterm_lpf2), "dterm")}
        rows = []
        for prefix, name, key in (("gyro", "Gyro", "gyro_filter"), ("dterm", "D", "dterm_filter")):
            if not sl[key]:
                rows.append(f"<b>{name}</b> : curseur inactif → filtres inchangés")
                continue
            parts = []
            for k, lab in ((f"{prefix}_lpf1_dyn_min_hz", "LPF1 dyn min"), (f"{prefix}_lpf1_dyn_max_hz", "max"),
                           (f"{prefix}_lpf1_static_hz", "LPF1"), (f"{prefix}_lpf2_static_hz", "LPF2")):
                a, b = now[k], new[k]
                if a == 0 and b == 0:
                    continue
                if k.endswith("dyn_max_hz") and not (now[k.replace("max", "min")] or new[k.replace("max", "min")]):
                    continue  # LPF1 dynamique désactivé (min = 0) : le max n'a pas d'effet
                parts.append(f"{lab} {a}→<b>{b}</b>" if a != b else f"{lab} {a}")
            rows.append(f"<b>{name}</b> : " + (", ".join(parts) if parts else "aucun filtre actif") + " Hz")
        return "<br>".join(rows)

    def _cli_lines(self, sl: dict) -> list[str]:
        lines = [f"# OpenTuneBox — curseurs pour {self._ref.label} (Betaflight {self._ref.family})"]
        if sl["mode"] == 0:
            lines.append("# mode OFF : les curseurs PID ne seront pas appliqués")
        return lines + cli_commands(sl, self._ref.family)

    def _copy_cli(self) -> None:
        QApplication.clipboard().setText(self.cli.toPlainText())
        self.status.setText("Commandes CLI copiées dans le presse-papiers.")

