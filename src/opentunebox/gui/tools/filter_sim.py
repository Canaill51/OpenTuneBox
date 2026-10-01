"""Fenêtre « Simulateur de filtres » : effet de nouveaux réglages de filtres Betaflight.

Un seul log. On modifie les filtres (gyro, D, notchs, notch dynamique, filtre RPM) dans le panneau
de droite :
- à gauche, diagrammes de Bode des chaînes gyro et D (réglages du log en trait plein, réglages
  modifiés en tirets), calculés avec les formules exactes du firmware à la cadence PID, pour un gaz
  donné (LPF dynamiques) et une fréquence moteur donnée (notchs RPM) ; retards à 20/50/100 Hz ;
- à droite, spectres sur la période d'analyse : gyro avant filtres (journalisé), gyro filtré
  (journalisé) et gyro re-filtré par simulation avec les réglages modifiés ; idem pour le D.

Le D simulé : on dérive le gyro (−dg/dt), on le passe dans la chaîne D, puis on le met à l'échelle
du D journalisé par moindres carrés, l'échelle étant calée avec les réglages actuels (elle absorbe
Kd, D max, TPA). Le même facteur est appliqué au D simulé avec les réglages modifiés.
La simulation est un peu longue (≈ 1 s) : elle est faite dans compute(), en arrière-plan.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, field, replace

import numpy as np
import pyqtgraph as pg
from PySide6.QtCore import Qt
from PySide6.QtGui import QFont
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QComboBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QPlainTextEdit,
    QPushButton,
    QSlider,
    QSpinBox,
    QTabWidget,
    QVBoxLayout,
    QWidget,
)

from ...analysis.bf_filters import (
    chain_delay_ms,
    chain_response,
    dterm_chain,
    dyn_lpf_cutoff,
    gyro_chain,
    whatif_spectrum,
)
from ...analysis.motors import hover_motor_hz
from ...analysis.spectral import spectrum
from ...log.settings import LowpassSetting, Settings
from .. import theme
from ._step_common import style_plot, wrap_canvas
from .base import ToolWindow

LPF_TYPES = ("PT1", "BIQUAD", "PT2", "PT3")
LPF_MODES = ("désactivé", "statique", "dynamique")
COL_MOD = "#2f6fd6"
COL_PF = "#9a9a9a"
COL_SIM_CUR = "#f58231"
DELAY_FREQS = (20.0, 50.0, 100.0)


# ---------------------------------------------------------------- calcul (sans Qt)


def slice_log(log, sel: slice, margin_s: float = 0.5):
    """Copie du log réduite à `sel` (plus une marge avant, pour l'établissement des filtres).
    Renvoie (log_réduit, nombre d'échantillons de marge à ignorer)."""
    start = sel.start or 0
    stop = log.n if sel.stop is None else min(sel.stop, log.n)
    m = min(start, int(margin_s * log.fs))
    s = slice(start - m, stop)

    def cut(a):
        return None if a is None else a[..., s]

    names = ("t", "gyro", "gyro_unfilt", "setpoint", "rc_command", "p_term", "i_term", "d_term", "f_term",
             "throttle", "motors", "motor_hz", "debug", "vbat", "current", "acc", "mode_flags")
    return replace(log, **{k: cut(getattr(log, k)) for k in names}), m


@dataclass
class FilterSimResult:
    label: str
    color: str
    fs: float
    base: Settings
    mod: Settings
    freqs: np.ndarray = field(default_factory=lambda: np.zeros(0))
    # spectres PSD (dB) par axe : clé → liste de 3 tableaux (ou None)
    spectra: dict = field(default_factory=dict)
    d_scale: list = field(default_factory=list)
    has_unfilt: bool = True


def _lsq_gain(x: np.ndarray, y: np.ndarray) -> float:
    den = float(np.dot(x, x))
    return float(np.dot(x, y) / den) if den > 0 else 0.0


def simulate_filters(entry, mod: Settings, with_current: bool = True) -> FilterSimResult:
    """Spectres enregistrés et spectres « et si » avec les filtres `mod`.

    Méthode relative (bf_filters.whatif_spectrum) : spectre réellement enregistré × rapport des
    réponses des chaînes de filtres (modifiée / actuelle), évaluées à la vraie cadence PID fenêtre
    par fenêtre. Avec des réglages inchangés, la simulation retombe exactement sur le log.
    """
    log = entry.log
    base = log.settings
    sel = entry.sel
    fs = float(log.fs)
    res = FilterSimResult(entry.label, entry.color, fs, base, mod, has_unfilt=log.gyro_unfilt is not None)
    keys = ("gyro_pf", "gyro", "gyro_sim", "gyro_sim_cur", "d_pf", "d", "d_sim", "d_sim_cur")
    res.spectra = {k: [None, None, None] for k in keys}

    def psd(x):
        sp = spectrum(np.asarray(x[sel], dtype=float), fs, "psd", resolution_hz=1000.0 / 300.0)
        return sp.values

    for ax in range(3):
        w = whatif_spectrum(log.gyro[ax], log.gyro_unfilt[ax] if res.has_unfilt else None, log, sel, base, mod, "gyro")
        res.freqs = w.freqs
        res.spectra["gyro"][ax] = w.current_db
        res.spectra["gyro_sim"][ax] = w.modified_db
        if with_current:
            res.spectra["gyro_sim_cur"][ax] = w.current_db
        if res.has_unfilt:
            res.spectra["gyro_pf"][ax] = psd(log.gyro_unfilt[ax])
        if not np.any(log.d_term[ax]):
            res.d_scale.append(float("nan"))
            continue
        res.d_scale.append(1.0)
        wd = whatif_spectrum(log.d_term[ax], log.d_term_prefilter[ax], log, sel, base, mod, "dterm")
        res.spectra["d"][ax] = wd.current_db
        res.spectra["d_sim"][ax] = wd.modified_db
        if with_current:
            res.spectra["d_sim_cur"][ax] = wd.current_db
        res.spectra["d_pf"][ax] = psd(log.d_term_prefilter[ax])
    return res


def _type_code(t: str) -> int:
    return LPF_TYPES.index(t) if t in LPF_TYPES else 0


def cli_values(s: Settings) -> dict[str, str]:
    """Valeurs CLI Betaflight des réglages de filtres (noms vérifiés dans cli/settings.c, 4.4 → 2026.6)."""
    notches = list(s.gyro_notches[:2]) + [(0, 0)] * (2 - len(s.gyro_notches[:2]))
    out = {
        "gyro_lpf1_type": s.gyro_lpf1.type,
        "gyro_lpf1_static_hz": s.gyro_lpf1.static_hz,
        "gyro_lpf1_dyn_min_hz": s.gyro_lpf1.dyn_min_hz,
        "gyro_lpf1_dyn_max_hz": s.gyro_lpf1.dyn_max_hz,
        "gyro_lpf1_dyn_expo": s.gyro_lpf1.expo,
        "gyro_lpf2_type": s.gyro_lpf2.type,
        "gyro_lpf2_static_hz": s.gyro_lpf2.static_hz,
        "gyro_notch1_hz": notches[0][0],
        "gyro_notch1_cutoff": notches[0][1],
        "gyro_notch2_hz": notches[1][0],
        "gyro_notch2_cutoff": notches[1][1],
        "dterm_lpf1_type": s.dterm_lpf1.type,
        "dterm_lpf1_static_hz": s.dterm_lpf1.static_hz,
        "dterm_lpf1_dyn_min_hz": s.dterm_lpf1.dyn_min_hz,
        "dterm_lpf1_dyn_max_hz": s.dterm_lpf1.dyn_max_hz,
        "dterm_lpf1_dyn_expo": s.dterm_lpf1.expo,
        "dterm_lpf2_type": s.dterm_lpf2.type,
        "dterm_lpf2_static_hz": s.dterm_lpf2.static_hz,
        "dterm_notch_hz": s.dterm_notch[0],
        "dterm_notch_cutoff": s.dterm_notch[1],
        "dyn_notch_count": s.dyn_notch_count,
        "dyn_notch_q": s.dyn_notch_q,
        "dyn_notch_min_hz": s.dyn_notch_min_hz,
        "dyn_notch_max_hz": s.dyn_notch_max_hz,
        "rpm_filter_harmonics": s.rpm_harmonics,
        "rpm_filter_q": s.rpm_q,
        "rpm_filter_min_hz": s.rpm_min_hz,
        "rpm_filter_weights": ",".join(str(w) for w in (s.rpm_weights or [100])[:3]),
    }
    return {k: str(v) for k, v in out.items()}


def filter_cli(base: Settings, mod: Settings) -> list[str]:
    """Commandes CLI pour passer des filtres de `base` à ceux de `mod` (seulement ce qui change)."""
    a, b = cli_values(base), cli_values(mod)
    fam = base.family
    lines = [f"# OpenTuneBox — filtres modifiés (Betaflight {fam})"]
    changed = [k for k in b if a[k] != b[k]]
    if not changed:
        return lines + ["# aucun changement par rapport au log"]
    sl = base.sliders()
    gyro_keys = [k for k in changed if k.startswith("gyro_lpf")]
    d_keys = [k for k in changed if k.startswith("dterm_lpf")]
    if gyro_keys and sl.get("gyro_filter", 0) == 1:
        lines += ["# réglage manuel : on désactive le curseur de filtre gyro (comme le configurateur)",
                  "set simplified_gyro_filter = OFF"]
    if d_keys and sl.get("dterm_filter", 0) == 1:
        lines += ["# réglage manuel : on désactive le curseur de filtre D (comme le configurateur)",
                  "set simplified_dterm_filter = OFF"]
    for k in changed:
        if k == "rpm_filter_weights" and fam == "4.4":
            lines.append("# rpm_filter_weights n'existe qu'à partir de Betaflight 4.5 (ignoré)")
            continue
        lines.append(f"set {k} = {b[k]}")
    if fam == "4.4" and int(b["dyn_notch_count"]) > 5:
        lines.append("# attention : 5 notchs dynamiques au plus en 4.4")
    lines.append("save")
    return lines


# ---------------------------------------------------------------- fenêtre


class FilterSimWindow(ToolWindow):
    title = "Simulateur de filtres"
    multi_file = False

    def __init__(self, state, main_window=None) -> None:
        self._last_result = None
        self._base: Settings | None = None
        self._base_key = None
        self._loading = False
        self.ed: dict[str, QWidget] = {}
        super().__init__(state, main_window)
        _top, bottom = wrap_canvas(self)
        row = QHBoxLayout()
        self.cli = QPlainTextEdit()
        self.cli.setReadOnly(True)
        self.cli.setFont(QFont("monospace", 9))
        self.cli.setFixedHeight(120)
        col = QVBoxLayout()
        col.addWidget(QLabel("CLI des filtres modifiés :"))
        copy_btn = QPushButton("Copier")
        copy_btn.clicked.connect(self._copy_cli)
        col.addWidget(copy_btn)
        col.addStretch(1)
        row.addLayout(col)
        row.addWidget(self.cli, 1)
        self.delay_label = QLabel("")
        self.delay_label.setTextFormat(Qt.RichText)
        self.delay_label.setFont(QFont("monospace", 9))
        self.delay_label.setWordWrap(True)
        row.addWidget(self.delay_label, 1)
        bottom.addLayout(row)
        self.files.itemSelectionChanged.connect(self._file_changed)
        self._file_changed()

    # ------------------------------------------------------------ panneau
    def build_controls(self, layout: QVBoxLayout) -> None:
        tabs = QTabWidget()
        tabs.addTab(self._lpf_tab("g", with_notches=True), "Gyro")
        tabs.addTab(self._lpf_tab("d", with_notches=False), "D")
        tabs.addTab(self._notch_tab(), "Notch dyn / RPM")
        layout.addWidget(tabs)

        form = QFormLayout()
        self.throttle = QSlider(Qt.Horizontal)
        self.throttle.setRange(0, 100)
        self.throttle.setValue(50)
        self.throttle_lab = QLabel("50 %")
        trow = QHBoxLayout()
        trow.addWidget(self.throttle, 1)
        trow.addWidget(self.throttle_lab)
        form.addRow("Gaz (Bode)", trow)
        self.motor_hz = QSpinBox()
        self.motor_hz.setRange(20, 3000)
        self.motor_hz.setValue(200)
        self.motor_hz.setSuffix(" Hz")
        form.addRow("Moteur (Bode)", self.motor_hz)
        self.bode_dyn = QCheckBox("notchs dyn. (centres initiaux)")
        form.addRow(self.bode_dyn)
        self.show_cur = QCheckBox("simulation réglages actuels (référence)")
        self.show_cur.setChecked(True)
        self.show_cur.setToolTip("La simulation tourne à la cadence du log : comparez « simulé modifié » à "
                                 "« simulé actuel » plutôt qu'au gyro du log.")
        form.addRow(self.show_cur)
        self.throttle.valueChanged.connect(self._bode_options)
        self.motor_hz.valueChanged.connect(self._bode_options)
        self.bode_dyn.toggled.connect(self._bode_options)
        self.show_cur.toggled.connect(self.on_options_changed)
        layout.addLayout(form)

        btns = QHBoxLayout()
        reset = QPushButton("Réinitialiser")
        reset.clicked.connect(self._reset)
        apply_ = QPushButton("Appliquer")
        apply_.setStyleSheet("font-weight:600")
        apply_.clicked.connect(self.run)
        btns.addWidget(reset)
        btns.addWidget(apply_)
        layout.addLayout(btns)
        help_ = QLabel("Bode : trait plein = log, tirets = modifié. Spectres : « Appliquer » relance la "
                       "simulation du gyro (≈ 1 s).")
        help_.setWordWrap(True)
        help_.setStyleSheet("color:#777; font-size:8pt")
        layout.addWidget(help_)

    def _spin(self, key: str, lo: int, hi: int, suffix: str = "") -> QSpinBox:
        sp = QSpinBox()
        sp.setRange(lo, hi)
        if suffix:
            sp.setSuffix(suffix)
        sp.valueChanged.connect(self._editor_changed)
        self.ed[key] = sp
        return sp

    def _combo(self, key: str, items) -> QComboBox:
        cb = QComboBox()
        cb.addItems(list(items))
        cb.currentIndexChanged.connect(self._editor_changed)
        self.ed[key] = cb
        return cb

    def _lpf_tab(self, p: str, with_notches: bool) -> QWidget:
        w = QWidget()
        f = QFormLayout(w)
        f.setVerticalSpacing(3)
        f.addRow(QLabel("<b>LPF1</b>"))
        f.addRow("Type", self._combo(f"{p}1_type", LPF_TYPES))
        f.addRow("Mode", self._combo(f"{p}1_mode", LPF_MODES))
        f.addRow("Statique", self._spin(f"{p}1_static", 0, 1000, " Hz"))
        f.addRow("Dyn. min", self._spin(f"{p}1_min", 0, 1000, " Hz"))
        f.addRow("Dyn. max", self._spin(f"{p}1_max", 0, 1000, " Hz"))
        f.addRow("Expo", self._spin(f"{p}1_expo", 0, 10))
        f.addRow(QLabel("<b>LPF2</b> (0 = désactivé)"))
        f.addRow("Type", self._combo(f"{p}2_type", LPF_TYPES))
        f.addRow("Coupure", self._spin(f"{p}2_hz", 0, 1000, " Hz"))
        if with_notches:
            f.addRow(QLabel("<b>Notchs statiques</b> (centre / coupure)"))
            for i in (1, 2):
                r = QHBoxLayout()
                r.addWidget(self._spin(f"n{i}_hz", 0, 1000))
                r.addWidget(self._spin(f"n{i}_cut", 0, 1000))
                f.addRow(f"Notch {i}", r)
        else:
            f.addRow(QLabel("<b>Notch D</b> (centre / coupure)"))
            r = QHBoxLayout()
            r.addWidget(self._spin("dn_hz", 0, 1000))
            r.addWidget(self._spin("dn_cut", 0, 1000))
            f.addRow("Notch", r)
        return w

    def _notch_tab(self) -> QWidget:
        w = QWidget()
        f = QFormLayout(w)
        f.setVerticalSpacing(3)
        f.addRow(QLabel("<b>Notch dynamique</b>"))
        f.addRow("Nombre", self._spin("dyn_count", 0, 7))
        f.addRow("Q", self._spin("dyn_q", 1, 1000))
        f.addRow("Min", self._spin("dyn_min", 20, 250, " Hz"))
        f.addRow("Max", self._spin("dyn_max", 200, 1000, " Hz"))
        f.addRow(QLabel("<b>Filtre RPM</b>"))
        f.addRow("Harmoniques", self._spin("rpm_harm", 0, 3))
        f.addRow("Q", self._spin("rpm_q", 250, 3000))
        f.addRow("Min", self._spin("rpm_min", 30, 200, " Hz"))
        r = QHBoxLayout()
        for i in range(3):
            r.addWidget(self._spin(f"rpm_w{i}", 0, 100))
        f.addRow("Poids %", r)
        return w

    # ------------------------------------------------------------ éditeur ⇄ réglages
    def _file_changed(self) -> None:
        entries = self.files.selected_entries()
        if entries:
            self._ensure_base(entries[0])

    def _ensure_base(self, entry) -> None:
        key = (entry.label, id(entry.log))
        if key == self._base_key:
            return
        self._base_key = key
        self._base = entry.log.settings
        self._load_editor(self._base)
        hz = hover_motor_hz(entry.log)
        self.motor_hz.blockSignals(True)
        self.motor_hz.setValue(int(round(hz)) if hz else 200)
        self.motor_hz.blockSignals(False)
        self.ed["dyn_count"].setMaximum(5 if self._base.family == "4.4" else 7)
        self._editor_changed()

    def _load_editor(self, s: Settings) -> None:
        self._loading = True
        e = self.ed

        def lpf(p, lp: LowpassSetting):
            e[f"{p}1_type"].setCurrentIndex(_type_code(lp.type))
            e[f"{p}1_mode"].setCurrentIndex(2 if lp.dynamic else 1 if lp.static_hz > 0 else 0)
            e[f"{p}1_static"].setValue(lp.static_hz)
            e[f"{p}1_min"].setValue(lp.dyn_min_hz)
            e[f"{p}1_max"].setValue(lp.dyn_max_hz)
            e[f"{p}1_expo"].setValue(lp.expo)

        lpf("g", s.gyro_lpf1)
        lpf("d", s.dterm_lpf1)
        e["g2_type"].setCurrentIndex(_type_code(s.gyro_lpf2.type))
        e["g2_hz"].setValue(s.gyro_lpf2.static_hz)
        e["d2_type"].setCurrentIndex(_type_code(s.dterm_lpf2.type))
        e["d2_hz"].setValue(s.dterm_lpf2.static_hz)
        notches = list(s.gyro_notches[:2]) + [(0, 0)] * 2
        for i in (1, 2):
            e[f"n{i}_hz"].setValue(notches[i - 1][0])
            e[f"n{i}_cut"].setValue(notches[i - 1][1])
        e["dn_hz"].setValue(s.dterm_notch[0])
        e["dn_cut"].setValue(s.dterm_notch[1])
        e["dyn_count"].setValue(s.dyn_notch_count)
        e["dyn_q"].setValue(max(1, s.dyn_notch_q))
        e["dyn_min"].setValue(s.dyn_notch_min_hz or 100)
        e["dyn_max"].setValue(s.dyn_notch_max_hz or 600)
        e["rpm_harm"].setValue(s.rpm_harmonics)
        e["rpm_q"].setValue(s.rpm_q)
        e["rpm_min"].setValue(s.rpm_min_hz)
        w = list(s.rpm_weights or []) + [100] * 3
        for i in range(3):
            e[f"rpm_w{i}"].setValue(w[i])
        self._loading = False

    def modified_settings(self) -> Settings | None:
        """Réglages du log modifiés par l'éditeur (copie profonde, le log n'est pas touché)."""
        if self._base is None:
            return None
        e = self.ed
        s = copy.deepcopy(self._base)

        def lpf(p, lp: LowpassSetting) -> LowpassSetting:
            mode = e[f"{p}1_mode"].currentIndex()
            lp.type = LPF_TYPES[e[f"{p}1_type"].currentIndex()]
            lp.expo = e[f"{p}1_expo"].value()
            lp.dyn_max_hz = e[f"{p}1_max"].value()
            if mode == 0:
                lp.static_hz, lp.dyn_min_hz = 0, 0
            elif mode == 1:
                lp.static_hz, lp.dyn_min_hz = max(1, e[f"{p}1_static"].value()), 0
            else:
                lp.static_hz = e[f"{p}1_static"].value()
                lp.dyn_min_hz = max(1, e[f"{p}1_min"].value())
                lp.dyn_max_hz = max(lp.dyn_min_hz, lp.dyn_max_hz)
            return lp

        lpf("g", s.gyro_lpf1)
        lpf("d", s.dterm_lpf1)
        s.gyro_lpf2.type = LPF_TYPES[e["g2_type"].currentIndex()]
        s.gyro_lpf2.static_hz = e["g2_hz"].value()
        s.dterm_lpf2.type = LPF_TYPES[e["d2_type"].currentIndex()]
        s.dterm_lpf2.static_hz = e["d2_hz"].value()
        # les deux positions sont gardées (centre 0 = notch désactivé, ignoré par gyro_chain)
        s.gyro_notches = [(e[f"n{i}_hz"].value(), e[f"n{i}_cut"].value()) for i in (1, 2)]
        s.dterm_notch = (e["dn_hz"].value(), e["dn_cut"].value())
        s.dyn_notch_count = e["dyn_count"].value()
        s.dyn_notch_q = e["dyn_q"].value()
        s.dyn_notch_min_hz = e["dyn_min"].value()
        s.dyn_notch_max_hz = e["dyn_max"].value()
        s.rpm_harmonics = e["rpm_harm"].value()
        s.rpm_q = e["rpm_q"].value()
        s.rpm_min_hz = e["rpm_min"].value()
        s.rpm_weights = [e[f"rpm_w{i}"].value() for i in range(3)]
        return s

    def _editor_changed(self, *_a) -> None:
        if self._loading or self._base is None:
            return
        mod = self.modified_settings()
        self.cli.setPlainText("\n".join(filter_cli(self._base, mod)))
        res = self._last_result
        if res is not None and res.base is self._base:
            stale = cli_values(res.mod) != cli_values(mod)
            self.status.setText("Réglages modifiés : cliquez sur « Appliquer » pour simuler les spectres."
                                if stale else "")
        self._draw_bode()

    def _bode_options(self, *_a) -> None:
        self.throttle_lab.setText(f"{self.throttle.value()} %")
        self._draw_bode()

    def _reset(self) -> None:
        if self._base is not None:
            self._load_editor(self._base)
            self._editor_changed()

    # ------------------------------------------------------------ calcul
    def run(self) -> None:
        entries = self.files.selected_entries()
        if entries:
            self._ensure_base(entries[0])
        self._run_settings = self.modified_settings()
        super().run()

    def compute(self, entries) -> FilterSimResult:
        entry = entries[0]
        mod = getattr(self, "_run_settings", None)
        if mod is None or self._base is not entry.log.settings:
            # appel direct (tests) depuis le thread de l'interface
            self._ensure_base(entry)
            mod = self.modified_settings()
        return simulate_filters(entry, mod)

    # ------------------------------------------------------------ affichage
    def render(self, result: FilterSimResult) -> None:
        self._run_settings = None
        c = self.canvas
        c.clear()
        c.setBackground(theme.current().bg)
        self.bode = {}
        for row, name in enumerate(("gyro", "d")):
            title = "Gyro" if name == "gyro" else "D"
            mag = c.addPlot(row=row, col=0)
            ph = c.addPlot(row=row, col=1)
            spec = c.addPlot(row=row, col=2)
            for p in (mag, ph, spec):
                style_plot(p)
            mag.setTitle(f"Filtres {title} — gain", color=theme.current().fg)
            ph.setTitle(f"Filtres {title} — phase", color=theme.current().fg)
            mag.setLabel("left", "Gain (dB)")
            ph.setLabel("left", "Phase (°)")
            for p in (mag, ph):
                p.setLogMode(x=True, y=False)
                p.setLabel("bottom", "Fréquence (Hz)")
            ph.setXLink(mag)
            self.bode[name] = (mag, ph)
            self._draw_spectrum(spec, result, name)
        lay = c.ci.layout
        lay.setColumnStretchFactor(0, 2)
        lay.setColumnStretchFactor(1, 2)
        lay.setColumnStretchFactor(2, 3)
        self._draw_bode()

    def _draw_spectrum(self, p: pg.PlotItem, res: FilterSimResult, name: str) -> None:
        th = theme.current()
        axes = [a for a in self.axes()]
        lab = "+".join(theme.AXIS_NAMES[a][0] for a in axes)
        p.setTitle(f"Spectre {'gyro' if name == 'gyro' else 'D'} ({lab})", color=th.fg)
        p.setLabel("left", "PSD (dB)")
        p.setLabel("bottom", "Fréquence (Hz)")
        p.addLegend(offset=(-5, 5), labelTextColor=th.fg, brush=pg.mkBrush(th.bg))
        if name == "gyro":
            series = [("gyro_pf", "gyro avant filtres (log)", COL_PF, Qt.SolidLine, 1.5),
                      ("gyro", "gyro filtré (log)", th.fg, Qt.SolidLine, 2),
                      ("gyro_sim", "gyro simulé (modifié)", COL_MOD, Qt.DashLine, 2)]
            if self.show_cur.isChecked():
                series.append(("gyro_sim_cur", "gyro simulé (actuel)", COL_SIM_CUR, Qt.DotLine, 2))
        else:
            series = [("d_pf", "D avant filtres (log)", COL_PF, Qt.SolidLine, 1.5),
                      ("d", "D filtré (log)", th.fg, Qt.SolidLine, 2),
                      ("d_sim", "D simulé (modifié)", COL_MOD, Qt.DashLine, 2)]
            if self.show_cur.isChecked():
                series.append(("d_sim_cur", "D simulé (actuel)", COL_SIM_CUR, Qt.DotLine, 2))
        top = -np.inf
        for key, label, col, style, width in series:
            arrs = [res.spectra[key][a] for a in axes if res.spectra[key][a] is not None]
            if not arrs or res.freqs.size == 0:
                continue
            lin = np.mean([10 ** (np.asarray(a) / 10) for a in arrs], axis=0)
            db = 10 * np.log10(np.maximum(lin, 1e-12))
            p.plot(res.freqs, db, pen=pg.mkPen(col, width=width, style=style), name=label)
            top = max(top, float(np.max(db[res.freqs > 2])))
        if np.isfinite(top):
            p.setYRange(top - 70, top + 5, padding=0)
        p.setXRange(0, res.fs / 2, padding=0)
        if name == "gyro" and not res.has_unfilt:
            p.setTitle("Spectre gyro — pas de gyro non filtré dans ce log : pas de simulation", color="#d62728")

    def _chains(self, s: Settings):
        thr = self.throttle.value() / 100.0
        dyn = None if self.bode_dyn.isChecked() else []
        g = gyro_chain(s, thr, motor_hz=[float(self.motor_hz.value())], dyn_notch_hz=dyn)
        return g, dterm_chain(s, thr)

    def _draw_bode(self) -> None:
        if self._base is None or not getattr(self, "bode", None):
            return
        th = theme.current()
        mod = self.modified_settings()
        nyq = self._base.pid_rate_hz / 2
        f = np.geomspace(1.0, nyq * 0.999, 900)
        chains = {"cur": self._chains(self._base), "mod": self._chains(mod)}
        for i, name in enumerate(("gyro", "d")):
            mag, ph = self.bode[name]
            mag.clear()
            ph.clear()
            for k, col, style in (("cur", th.fg, Qt.SolidLine), ("mod", COL_MOD, Qt.DashLine)):
                h = chain_response(chains[k][i], f)
                db = 20 * np.log10(np.maximum(np.abs(h), 1e-4))
                deg = np.degrees(np.unwrap(np.angle(h)))
                pen = pg.mkPen(col, width=2, style=style)
                mag.plot(f, db, pen=pen)
                ph.plot(f, deg, pen=pen)
            mag.setYRange(-40, 3, padding=0)
            ph.setYRange(-270, 10, padding=0)
            mag.setXRange(np.log10(10), np.log10(nyq), padding=0)
            ticks = [(np.log10(v), f"{v:g}") for v in (10, 20, 50, 100, 200, 500, 1000, 2000, 5000) if v <= nyq]
            for p in (mag, ph):
                p.getAxis("bottom").setTicks([ticks, []])
        self.delay_label.setText(self._delay_html(chains))

    def _delay_html(self, chains) -> str:
        thr = self.throttle.value()

        def d(ch, fhz):
            return chain_delay_ms(ch, f_hz=fhz).phase_ms if ch else 0.0

        fg = theme.current().fg
        head = "".join(f"<th align='right'>&nbsp;&nbsp;{int(fz)} Hz</th>" for fz in DELAY_FREQS)
        rows = [f"<b>Retard des filtres (ms)</b> à {thr} % de gaz, moteur {self.motor_hz.value()} Hz "
                f"— actuel → <span style='color:{COL_MOD}'>modifié</span>",
                f"<table cellspacing='0' cellpadding='1'><tr><th></th>{head}</tr>"]
        lines = (("Gyro", lambda c: c[0]), ("D seul", lambda c: c[1]), ("Gyro + D", lambda c: c[0] + c[1]))
        for name, pick in lines:
            cells = []
            for fz in DELAY_FREQS:
                a, b = d(pick(chains["cur"]), fz), d(pick(chains["mod"]), fz)
                col = "#1a9a3a" if b < a - 0.005 else "#d62728" if b > a + 0.005 else fg
                cells.append(f"<td align='right'>&nbsp;&nbsp;{a:.2f} → <span style='color:{col}'>{b:.2f}</span></td>")
            rows.append(f"<tr><td>{name}</td>{''.join(cells)}</tr>")
        rows[-1] += "</table>"
        mod = self.modified_settings()
        for name, lp in (("gyro", mod.gyro_lpf1), ("D", mod.dterm_lpf1)):
            if lp.dynamic:
                rows.append(f"LPF1 {name} dynamique (modifié) à ce gaz : "
                            f"{dyn_lpf_cutoff(lp.dyn_min_hz, lp.dyn_max_hz, lp.expo, thr / 100):.0f} Hz")
        rows.append("<span style='color:#777'>D simulé = −dgyro/dt passé dans la chaîne D, mis à l'échelle "
                    "du D du log (moindres carrés, réglages actuels).</span>")
        return _join_rows(rows)

    def _copy_cli(self) -> None:
        QApplication.clipboard().setText(self.cli.toPlainText())
        self.status.setText("Commandes CLI copiées dans le presse-papiers.")


def _join_rows(rows: list[str]) -> str:
    """Assemble le texte des retards : lignes de tableau collées, autres lignes séparées par <br>."""
    out = ""
    for r in rows:
        if r.startswith("<tr>") or r.startswith("<table"):
            out += r
        else:
            out += ("" if not out or out.endswith("</table>") else "<br>") + r
    return out
