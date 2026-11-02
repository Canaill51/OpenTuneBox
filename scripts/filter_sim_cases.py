#!/usr/bin/env python3
"""Cas de test du Simulateur de filtres (`gui/tools/filter_sim.py`) pour `rust/otb-gui/src/tools/filter_sim_calc.rs` (6-4c).

Usage : filter_sim_cases.py [SORTIE.json]   (défaut : rust/otb-gui/tests/data/filter_sim.json)

- `cli` : `cli_values` et `filter_cli` pour trois versions (4.4, 4.5, 2025.12) et quatre combinaisons de curseurs simplifiés
  (gyro / D ON / OFF), un seul champ changé à la fois sur chaque famille de réglages, plus quelques changements multiples ; notchs
  dynamiques au-delà de 5 ; réglages sans notch ni poids RPM.
- `editor` : une vraie `FilterSimWindow` (Qt hors écran) : champs de `_load_editor` (bornes des `QSpinBox` comprises), puis
  `modified_settings` après quelques modifications des champs.
- `sim` : `simulate_filters` sur `tests/data/jeno5_5s.bbl` (réglages inchangés, g2_hz = 250, réglages variés, sans courbe « actuel »),
  courbes de Bode et retards de `_draw_bode` / `_delay_html` (chaînes de `_chains`, sans HTML).

Un flottant s'écrit en hexadécimal (`hx`), un tableau float64 dans la table `inputs` (`ref`). Les réglages des cas sont l'en-tête brut
(`raw`) : Rust les lit par `parse_settings` (déjà prouvé), les filtres modifiés sont écrits champ par champ (`enc_filters`).
"""
import copy
import os
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

import json  # noqa: E402

import numpy as np  # noqa: E402
from bf_filters_cases import hx  # noqa: E402
from flight_stats_cases import INPUTS, ref  # noqa: E402

from opentunebox.analysis.bf_filters import chain_delay_ms, chain_response, dyn_lpf_cutoff  # noqa: E402
from opentunebox.gui.tools import filter_sim  # noqa: E402
from opentunebox.log.settings import parse_settings  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
LOG = ROOT / "tests/data/jeno5_5s.bbl"
DEFAULT_OUT = ROOT / "rust/otb-gui/tests/data/filter_sim.json"

ED_KEYS = ["g1_type", "g1_mode", "g1_static", "g1_min", "g1_max", "g1_expo", "g2_type", "g2_hz", "n1_hz", "n1_cut", "n2_hz",
           "n2_cut", "d1_type", "d1_mode", "d1_static", "d1_min", "d1_max", "d1_expo", "d2_type", "d2_hz", "dn_hz", "dn_cut",
           "dyn_count", "dyn_q", "dyn_min", "dyn_max", "rpm_harm", "rpm_q", "rpm_min", "rpm_w0", "rpm_w1", "rpm_w2"]


def enc_filters(s) -> dict:
    """Tous les réglages de filtres d'un `Settings`, sous une forme comparable champ à champ."""
    lp = lambda x: [x.type, x.static_hz, x.dyn_min_hz, x.dyn_max_hz, x.expo]  # noqa: E731
    return {"gyro_lpf1": lp(s.gyro_lpf1), "gyro_lpf2": lp(s.gyro_lpf2), "dterm_lpf1": lp(s.dterm_lpf1),
            "dterm_lpf2": lp(s.dterm_lpf2), "gyro_notches": [list(n) for n in s.gyro_notches],
            "dterm_notch": list(s.dterm_notch), "dyn": [s.dyn_notch_count, s.dyn_notch_q, s.dyn_notch_min_hz, s.dyn_notch_max_hz],
            "rpm": [s.rpm_harmonics, s.rpm_q, s.rpm_min_hz], "rpm_weights": list(s.rpm_weights)}


def header(version: str, simp_gyro: str, simp_dterm: str, **extra) -> dict:
    raw = {"Firmware revision": f"Betaflight {version} (abc1234) STM32F7X2", "looptime": "125", "pid_process_denom": "2",
           "gyro_lpf1_type": "0", "gyro_lpf1_static_hz": "0", "gyro_lpf1_dyn_hz": "250,500", "gyro_lpf1_dyn_expo": "5",
           "gyro_lpf2_type": "0", "gyro_lpf2_static_hz": "500", "gyro_notch_hz": "260", "gyro_notch_cutoff": "200",
           "dterm_lpf1_type": "1", "dterm_lpf1_static_hz": "0", "dterm_lpf1_dyn_hz": "80,150", "dterm_lpf1_dyn_expo": "5",
           "dterm_lpf2_type": "0", "dterm_lpf2_static_hz": "150", "dterm_notch_hz": "0", "dterm_notch_cutoff": "0",
           "dyn_notch_count": "3", "dyn_notch_q": "300", "dyn_notch_min_hz": "100", "dyn_notch_max_hz": "600",
           "rpm_filter_harmonics": "3", "rpm_filter_weights": "100,50,70", "rpm_filter_q": "500", "rpm_filter_min_hz": "100",
           "simplified_gyro_filter": simp_gyro, "simplified_dterm_filter": simp_dterm}
    raw.update(extra)
    return raw


def mutations():
    """(nom, fonction) : un seul champ de réglage change par fonction."""
    out = []

    def add(name, fn):
        out.append((name, fn))

    for attr in ("gyro_lpf1", "gyro_lpf2", "dterm_lpf1", "dterm_lpf2"):
        add(f"{attr}.type", lambda s, a=attr: setattr(getattr(s, a), "type", "PT3"))
        add(f"{attr}.type2", lambda s, a=attr: setattr(getattr(s, a), "type", "BIQUAD"))
        add(f"{attr}.static", lambda s, a=attr: setattr(getattr(s, a), "static_hz", getattr(s, a).static_hz + 37))
        add(f"{attr}.static0", lambda s, a=attr: setattr(getattr(s, a), "static_hz", 0))
        add(f"{attr}.min", lambda s, a=attr: setattr(getattr(s, a), "dyn_min_hz", 123))
        add(f"{attr}.max", lambda s, a=attr: setattr(getattr(s, a), "dyn_max_hz", 777))
        add(f"{attr}.expo", lambda s, a=attr: setattr(getattr(s, a), "expo", 9))
    add("gyro_notches.1", lambda s: setattr(s, "gyro_notches", [(300, 150)]))
    add("gyro_notches.2", lambda s: setattr(s, "gyro_notches", [(260, 200), (400, 300)]))
    add("gyro_notches.none", lambda s: setattr(s, "gyro_notches", []))
    add("gyro_notches.pad", lambda s: setattr(s, "gyro_notches", [(260, 200), (0, 0)]))  # même CLI que l'original
    add("gyro_notches.cut", lambda s: setattr(s, "gyro_notches", [(260, 100)]))
    add("dterm_notch.hz", lambda s: setattr(s, "dterm_notch", (200, 100)))
    add("dterm_notch.cut", lambda s: setattr(s, "dterm_notch", (0, 50)))
    for v in (0, 1, 5, 6, 7):
        add(f"dyn_count={v}", lambda s, v=v: setattr(s, "dyn_notch_count", v))
    add("dyn_q", lambda s: setattr(s, "dyn_notch_q", 450))
    add("dyn_min", lambda s: setattr(s, "dyn_notch_min_hz", 90))
    add("dyn_max", lambda s: setattr(s, "dyn_notch_max_hz", 650))
    add("rpm_harm", lambda s: setattr(s, "rpm_harmonics", 2))
    add("rpm_q", lambda s: setattr(s, "rpm_q", 600))
    add("rpm_min", lambda s: setattr(s, "rpm_min_hz", 80))
    add("rpm_w.change", lambda s: setattr(s, "rpm_weights", [100, 50, 0]))
    add("rpm_w.short", lambda s: setattr(s, "rpm_weights", [80]))
    add("rpm_w.empty", lambda s: setattr(s, "rpm_weights", []))
    add("rpm_w.four", lambda s: setattr(s, "rpm_weights", [100, 50, 70, 25]))  # le 4e n'est pas écrit : aucun changement
    add("unchanged", lambda s: None)

    def multi(s):
        s.gyro_lpf2.static_hz = 300
        s.dterm_lpf1.static_hz = 90
        s.dyn_notch_count = 6
        s.rpm_weights = [100, 50, 0]

    add("multi", multi)

    def multi_gyro_only(s):
        s.gyro_lpf1.type = "PT2"
        s.gyro_notches = []

    add("multi_gyro", multi_gyro_only)
    return out


def case_cli():
    bases, cases = [], []
    for version in ("4.4.3", "4.5.5", "2025.12.1"):
        for sg, sd in (("1", "1"), ("0", "0"), ("1", "0"), ("0", "1")):
            bases.append(header(version, sg, sd))
    # sans notch ni poids RPM ni curseurs simplifiés
    bases.append(header("4.5.2", "1", "1", gyro_notch_hz="0", rpm_filter_weights="", rpm_filter_harmonics="0"))
    nosl = header("4.5.2", "1", "1")
    del nosl["simplified_gyro_filter"], nosl["simplified_dterm_filter"]
    bases.append(nosl)
    muts = mutations()
    for bi, raw in enumerate(bases):
        base = parse_settings(raw)
        for name, fn in muts:
            mod = copy.deepcopy(base)
            fn(mod)
            cases.append({"base": bi, "name": name, "mod": enc_filters(mod),
                          "values": [list(kv) for kv in filter_sim.cli_values(mod).items()],
                          "lines": filter_sim.filter_cli(base, mod)})
    return {"bases": bases, "base_values": [[list(kv) for kv in filter_sim.cli_values(parse_settings(r)).items()] for r in bases],
            "cases": cases}


def make_window(state):
    from opentunebox.gui.tools.filter_sim import FilterSimWindow

    win = FilterSimWindow(state)
    win.auto_run = False
    return win


def ed_values(win) -> dict:
    from PySide6.QtWidgets import QComboBox

    return {k: (w.currentIndex() if isinstance(w, QComboBox) else w.value()) for k, w in win.ed.items()}


def ed_set(win, key, v):
    from PySide6.QtWidgets import QComboBox

    w = win.ed[key]
    (w.setCurrentIndex if isinstance(w, QComboBox) else w.setValue)(v)


def load_base(win, s):
    """Comme `_ensure_base` dans une fenêtre neuve : champs chargés (maximum 7), puis maximum des notchs dynamiques."""
    win.ed["dyn_count"].setMaximum(7)
    win._base = s
    win._load_editor(s)
    win.ed["dyn_count"].setMaximum(5 if s.family == "4.4" else 7)
    win._editor_changed()


EDITS = [
    {},
    {"g2_hz": 250},
    {"g1_mode": 0}, {"g1_mode": 1, "g1_static": 0}, {"g1_mode": 1, "g1_static": 240, "g1_type": 2},
    {"g1_mode": 2, "g1_min": 0, "g1_max": 100}, {"g1_mode": 2, "g1_static": 50, "g1_min": 180, "g1_max": 120, "g1_expo": 3},
    {"d1_mode": 2, "d1_min": 60, "d1_max": 300, "d1_type": 3}, {"d1_mode": 1, "d1_static": 111}, {"d1_mode": 0},
    {"g2_type": 1, "d2_type": 2, "d2_hz": 0, "g2_hz": 0},
    {"n1_hz": 150, "n1_cut": 100, "n2_hz": 0, "n2_cut": 80}, {"n2_hz": 500, "n2_cut": 400}, {"dn_hz": 250, "dn_cut": 150},
    {"dyn_count": 6}, {"dyn_count": 7, "dyn_q": 1200}, {"dyn_count": 0}, {"dyn_q": 0, "dyn_min": 10, "dyn_max": 5000},
    {"rpm_harm": 1, "rpm_q": 100, "rpm_min": 250}, {"rpm_w0": 80, "rpm_w1": 0, "rpm_w2": 150},
    {"g2_hz": 5000, "n1_hz": 2000, "g1_expo": 50},
]


def case_editor(state):
    win = make_window(state)
    raws = [
        header("4.5.5", "1", "1"),
        header("4.4.3", "0", "1", dyn_notch_count="6"),
        header("2025.12.1", "1", "0", dyn_notch_count="7", gyro_lpf1_type="2", dterm_lpf2_type="3"),
        header("4.5.2", "1", "1", gyro_notch_hz="260,400,500", gyro_notch_cutoff="200,300,0", rpm_filter_weights="100"),
        header("4.5.2", "1", "1", dyn_notch_q="0", dyn_notch_min_hz="0", dyn_notch_max_hz="0", rpm_filter_q="100",
               rpm_filter_min_hz="20", gyro_lpf2_static_hz="1500", dterm_lpf1_dyn_expo="20", rpm_filter_weights="130,0,0"),
        header("4.5.2", "0", "0", gyro_lpf1_dyn_hz="0,0", gyro_lpf1_static_hz="200", dterm_lpf1_dyn_hz="0,0", gyro_notch_hz="0",
               rpm_filter_harmonics="0", rpm_filter_weights=""),
        header("4.5.2", "0", "0", gyro_lpf1_dyn_hz="100,60", gyro_lpf1_static_hz="200", dterm_notch_hz="220",
               dterm_notch_cutoff="150"),
    ]
    cases = []
    for raw in raws:
        base = parse_settings(raw)
        load_base(win, base)
        init = ed_values(win)
        item = {"raw": raw, "init": init, "init_mod": enc_filters(win.modified_settings()),
                "init_lines": filter_sim.filter_cli(base, win.modified_settings()), "edits": []}
        for ed in EDITS:
            load_base(win, base)
            for k, v in ed.items():
                ed_set(win, k, v)
            mod = win.modified_settings()
            item["edits"].append({"set": ed, "after": ed_values(win), "mod": enc_filters(mod),
                                  "lines": filter_sim.filter_cli(base, mod)})
        cases.append(item)
    return {"keys": ED_KEYS, "cases": cases}


def enc_result(res) -> dict:
    return {"fs": hx(res.fs), "has_unfilt": res.has_unfilt, "freqs": ref(np.asarray(res.freqs, float)),
            "d_scale": [hx(v) for v in res.d_scale],
            "spectra": {k: [None if a is None else ref(np.asarray(a, float)) for a in v] for k, v in res.spectra.items()}}


def case_sim(state, win):
    entry = state.logs[0]
    base = entry.log.settings
    load_base(win, base)
    mods = {"same": copy.deepcopy(base)}

    def edited(**ed):
        load_base(win, base)
        for k, v in ed.items():
            ed_set(win, k, v)
        return win.modified_settings()

    mods["g2_250"] = edited(g2_hz=250)
    mods["combo"] = edited(g1_mode=2, g1_min=100, g1_max=300, g1_expo=3, d1_mode=1, d1_static=120, dyn_count=4, dyn_q=250,
                           dyn_min=120, dyn_max=500, rpm_w2=0, n1_hz=300, n1_cut=150)
    mods["off"] = edited(g1_mode=0, g2_hz=0, d1_mode=0, d2_hz=0, dn_hz=0, dyn_count=0, rpm_harm=0)
    sel = entry.sel
    out = {"label": entry.label, "sel": [sel.start, sel.stop], "base": enc_filters(base),
           "mods": {k: enc_filters(m) for k, m in mods.items()}, "results": [], "bode": []}
    for name, with_cur in (("same", True), ("g2_250", True), ("combo", True), ("same", False), ("off", True)):
        res = filter_sim.simulate_filters(entry, mods[name], with_cur)
        out["results"].append({"mod": name, "with_current": with_cur, **enc_result(res)})
    for thr, mhz, dyn in ((50, 200, False), (80, 350, True), (20, 150, False)):
        win.throttle.setValue(thr)
        win.motor_hz.setValue(mhz)
        win.bode_dyn.setChecked(dyn)
        nyq = base.pid_rate_hz / 2
        f = np.geomspace(1.0, nyq * 0.999, 900)
        for name in ("g2_250", "combo", "off"):
            chains = {"cur": win._chains(base), "mod": win._chains(mods[name])}
            item = {"mod": name, "thr": thr, "motor_hz": mhz, "dyn": dyn, "freqs": ref(f), "curves": {}, "delays": [],
                    "dyn_cutoffs": []}
            for k in ("cur", "mod"):
                for i, nm in enumerate(("gyro", "d")):
                    h = chain_response(chains[k][i], f)
                    item["curves"][f"{k}_{nm}"] = [ref(20 * np.log10(np.maximum(np.abs(h), 1e-4))),
                                                   ref(np.degrees(np.unwrap(np.angle(h))))]

            def d(ch, fhz):
                return chain_delay_ms(ch, f_hz=fhz).phase_ms if ch else 0.0

            for line, pick in (("Gyro", lambda c: c[0]), ("D seul", lambda c: c[1]), ("Gyro + D", lambda c: c[0] + c[1])):
                item["delays"].append({"name": line, "cells": [[hx(d(pick(chains["cur"]), fz)), hx(d(pick(chains["mod"]), fz))]
                                                               for fz in filter_sim.DELAY_FREQS]})
            m = mods[name]
            for nm, lp in (("gyro", m.gyro_lpf1), ("D", m.dterm_lpf1)):
                if lp.dynamic:
                    item["dyn_cutoffs"].append([nm, hx(dyn_lpf_cutoff(lp.dyn_min_hz, lp.dyn_max_hz, lp.expo, thr / 100))])
            out["bode"].append(item)
    return out


def main() -> int:
    from opentunebox.gui.state import AppState
    from opentunebox.log.flightlog import load

    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_OUT
    from PySide6.QtWidgets import QApplication

    app = QApplication.instance() or QApplication([])  # noqa: F841 (doit vivre jusqu'à la fin)
    st = AppState()
    st.add(load(LOG)[0])
    out = {"cli": case_cli(), "editor": case_editor(st)}
    win = make_window(st)
    out["sim"] = case_sim(st, win)
    out["inputs"] = INPUTS
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out, separators=(",", ":")))
    print(len(out["cli"]["cases"]), "cas CLI,", len(out["editor"]["cases"]), "éditeurs,", len(INPUTS), "entrées,",
          out_path.stat().st_size, "octets")
    return 0


if __name__ == "__main__":
    code = 1
    try:
        code = main()
    finally:
        from opentunebox.gui.app import hard_exit

        hard_exit(code)
