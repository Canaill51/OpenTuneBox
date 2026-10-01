"""Infos de configuration : réglages des logs côte à côte, différences surlignées."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QBrush, QColor
from PySide6.QtWidgets import (
    QCheckBox,
    QGroupBox,
    QHBoxLayout,
    QLineEdit,
    QPushButton,
    QSplitter,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from ...log.settings import RATES_TYPES, Settings
from ..state import AppState
from .base import FileSelector


def _pid_rows(s: Settings) -> list[tuple[str, str]]:
    out = []
    for name, p in zip(("Roll", "Pitch", "Yaw"), s.pids):
        out.append((f"{name} P, I, D, Dmax, F", p.text()))
    return out


def _lp(lp) -> str:
    if lp.dynamic:
        return f"{lp.type} dynamique {lp.dyn_min_hz}–{lp.dyn_max_hz} Hz (expo {lp.expo})"
    if lp.static_hz:
        return f"{lp.type} {lp.static_hz} Hz"
    return "désactivé"


def _sections(s: Settings, log) -> list[tuple[str, list[tuple[str, str]]]]:
    g = s.get
    sl = s.sliders()
    rates_type = RATES_TYPES.get(s.get_int("rates_type", -1), g("rates_type", "?"))
    return [
        ("Général", [
            ("Firmware", s.firmware),
            ("Carte", s.board),
            ("Nom du drone", s.craft_name or "—"),
            ("Fréquence gyro / PID", f"{s.gyro_rate_hz:.0f} Hz / {s.pid_rate_hz:.0f} Hz"),
            ("Fréquence d'enregistrement", f"{log.fs:.0f} Hz"),
            ("Mode debug", s.debug_mode),
            ("Durée du log", f"{log.duration:.1f} s"),
        ]),
        ("PID", _pid_rows(s) + [
            ("D max : gain / avance", f"{g('d_max_gain', '—')} / {g('d_max_advance', '—')}"),
            ("Level (angle)", g("levelPID", "—") or "—"),
        ]),
        ("Curseurs simplifiés", [(k, str(v)) for k, v in sl.items()] or [("—", "absents")]),
        ("Filtres gyro", [
            ("LPF1", _lp(s.gyro_lpf1)),
            ("LPF2", _lp(s.gyro_lpf2)),
            ("Notchs statiques", ", ".join(f"{h} Hz (coupure {c})" for h, c in s.gyro_notches) or "aucun"),
            ("Notch dynamique", f"{s.dyn_notch_count} × Q{s.dyn_notch_q / 100:.1f}, {s.dyn_notch_min_hz}–{s.dyn_notch_max_hz} Hz"
             if s.dyn_notch_count else "désactivé"),
            ("Filtre RPM", f"{s.rpm_harmonics} harmoniques, Q{s.rpm_q / 100:.1f}, min {s.rpm_min_hz} Hz, "
                           f"poids {','.join(map(str, s.rpm_weights))} %, fondu {s.rpm_fade_hz} Hz"
             if s.rpm_harmonics else "désactivé"),
        ]),
        ("Filtres D", [
            ("LPF1", _lp(s.dterm_lpf1)),
            ("LPF2", _lp(s.dterm_lpf2)),
            ("Notch D", f"{s.dterm_notch[0]} Hz (coupure {s.dterm_notch[1]})" if s.dterm_notch[0] else "aucun"),
            ("Passe-bas P yaw", f"{s.yaw_lowpass_hz} Hz" if s.yaw_lowpass_hz else "—"),
        ]),
        ("Feedforward et lissage RC", [
            (k, g(k, "—")) for k in ("feedforward_transition", "feedforward_averaging", "feedforward_smooth_factor",
                                     "feedforward_jitter_factor", "feedforward_boost", "feedforward_max_rate_limit",
                                     "rc_smoothing_mode", "rc_smoothing_auto_factor", "rc_smoothing_setpoint_cutoff")
        ]),
        ("Terme I, TPA, anti-gravité", [
            (k, g(k, "—")) for k in ("iterm_relax", "iterm_relax_type", "iterm_relax_cutoff", "iterm_windup",
                                     "anti_gravity_gain", "anti_gravity_cutoff_hz", "tpa_mode", "tpa_rate",
                                     "tpa_breakpoint", "tpa_low_rate", "tpa_low_breakpoint", "throttle_boost")
        ]),
        ("Rates", [
            ("Type", str(rates_type)),
            ("rc_rates", g("rc_rates", "—")),
            ("rates (super / max)", g("rates", "—")),
            ("rc_expo", g("rc_expo", "—")),
            ("thr_mid / thr_expo", f"{g('thr_mid', '—')} / {g('thr_expo', '—')}"),
        ]),
        ("Moteurs", [
            ("Protocole / DShot bidirectionnel", f"{g('motor_pwm_protocol', '—')} / {'oui' if s.dshot_bidir else 'non'}"),
            ("Pôles moteur", str(s.motor_poles)),
            ("KV moteur", str(s.motor_kv) if s.motor_kv else "—"),
            ("Ralenti", g("motor_idle", g("dshot_idle_value", "—"))),
            ("Sortie moteurs (min, max)", g("motorOutput", "—")),
            ("Limite moteur / gaz", f"{g('motor_output_limit', '—')} % / {g('throttle_limit_percent', '—')} %"),
            ("Ralenti dynamique (min tr/min ×100)", g("dyn_idle_min_rpm", "—")),
            ("Linéarisation de poussée", g("thrust_linear", "—")),
        ]),
    ]


class SetupInfoWindow(QWidget):
    title = "Infos de configuration"

    def __init__(self, state: AppState, main_window=None) -> None:
        super().__init__(None, Qt.Window)
        self.state = state
        self.setWindowTitle(f"OpenTuneBox — {self.title}")
        self.resize(1200, 820)
        split = QSplitter(Qt.Horizontal, self)
        self.tree = QTreeWidget()
        self.tree.setAlternatingRowColors(True)
        split.addWidget(self.tree)
        panel = QWidget()
        pl = QVBoxLayout(panel)
        box = QGroupBox("Fichiers (max 10)")
        bl = QVBoxLayout(box)
        self.files = FileSelector(state)
        self.files.itemSelectionChanged.connect(self.refresh)
        bl.addWidget(self.files)
        pl.addWidget(box)
        self.only_diff = QCheckBox("Seulement les différences")
        self.only_diff.toggled.connect(self.refresh)
        pl.addWidget(self.only_diff)
        self.show_raw = QCheckBox("Tous les paramètres bruts")
        self.show_raw.toggled.connect(self.refresh)
        pl.addWidget(self.show_raw)
        self.filter = QLineEdit()
        self.filter.setPlaceholderText("Rechercher un paramètre…")
        self.filter.textChanged.connect(self.refresh)
        pl.addWidget(self.filter)
        row = QHBoxLayout()
        b = QPushButton("Exporter image")
        b.clicked.connect(self._save)
        row.addWidget(b)
        pl.addLayout(row)
        pl.addStretch(1)
        panel.setFixedWidth(260)
        split.addWidget(panel)
        split.setStretchFactor(0, 1)
        lay = QVBoxLayout(self)
        lay.addWidget(split)
        state.logs_changed.connect(self.refresh)
        self.refresh()

    def refresh(self) -> None:
        entries = self.files.selected_entries()
        self.tree.clear()
        self.tree.setColumnCount(1 + len(entries))
        self.tree.setHeaderLabels(["Paramètre", *[e.label for e in entries]])
        for i, e in enumerate(entries):
            self.tree.headerItem().setForeground(i + 1, QBrush(QColor(e.color)))
        if not entries:
            return
        needle = self.filter.text().strip().lower()
        per_file = [_sections(e.log.settings, e.log) for e in entries]
        names = [title for title, _ in per_file[0]]
        groups: list[tuple[str, list[tuple[str, list[str]]]]] = []
        for gi, title in enumerate(names):
            keys = [k for k, _ in per_file[0][gi][1]]
            rows = []
            for ri, k in enumerate(keys):
                vals = []
                for pf in per_file:
                    items = pf[gi][1]
                    vals.append(items[ri][1] if ri < len(items) else "—")
                rows.append((k, vals))
            groups.append((title, rows))
        if self.show_raw.isChecked():
            keys = sorted({k for e in entries for k in e.log.settings.raw})
            skip = ("Field ", "H ")
            rows = [(k, [e.log.settings.raw.get(k, "—") for e in entries]) for k in keys if not k.startswith(skip)]
            groups.append(("Tous les paramètres (en-tête brut)", rows))
        hl = QBrush(QColor(255, 200, 0, 90))
        for title, rows in groups:
            top = QTreeWidgetItem([title])
            f = top.font(0)
            f.setBold(True)
            top.setFont(0, f)
            n = 0
            for k, vals in rows:
                differs = len(set(vals)) > 1
                if self.only_diff.isChecked() and not differs:
                    continue
                if needle and needle not in k.lower() and not any(needle in str(v).lower() for v in vals):
                    continue
                it = QTreeWidgetItem([k, *[str(v) for v in vals]])
                if differs:
                    for c in range(len(vals) + 1):
                        it.setBackground(c, hl)
                top.addChild(it)
                n += 1
            if n:
                self.tree.addTopLevelItem(top)
                top.setExpanded(True)
        for c in range(self.tree.columnCount()):
            self.tree.resizeColumnToContents(c)

    def _save(self) -> None:
        from ..main_window import MainWindow

        MainWindow.save_figure(self)
