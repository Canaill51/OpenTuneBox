"""Tests des fenêtres Réponse indicielle, Curseurs PID et Simulateur de filtres (Qt hors écran)."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")


import numpy as np  # noqa: E402
import pytest  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from opentunebox.gui.tools import filter_sim  # noqa: E402
from opentunebox.gui.tools._step_common import segment_metrics  # noqa: E402
from opentunebox.log.settings import parse_settings  # noqa: E402

from corpus_paths import LOGS  # noqa: E402
D08 = LOGS / "1. PTB/BlackSummer/Damping 0.8.BFL"
D12 = LOGS / "1. PTB/BlackSummer/Damping 1.2.BFL"
FINAL = LOGS / "Cedric/JeNo 5/FINAL.bbl"


@pytest.fixture(scope="module")
def app():
    return QApplication.instance() or QApplication([])


def _close(app, win):
    win.close()
    win.deleteLater()
    app.processEvents()


# ---------------------------------------------------------------- sans log


def test_segment_metrics_matches_definition():
    t = np.arange(0, 501, 1.0)
    r1 = np.clip(t / 20.0, 0, 1)  # rampe : 50 % à 10 ms, pas de dépassement
    r2 = 1 - np.exp(-t / 10.0) * np.cos(t / 10.0)  # dépassement
    peak, lat = segment_metrics(t, np.vstack([r1, r2]))
    assert lat[0] == pytest.approx(10.0)
    assert peak[0] == pytest.approx(1.0)
    assert peak[1] > 1.0


def _settings(version="4.5.5", **extra):
    raw = {"Firmware revision": f"Betaflight {version} (abc) STM32F7X2", "looptime": "125",
           "pid_process_denom": "1", "gyro_lpf2_static_hz": "500", "rpm_filter_harmonics": "3",
           "rpm_filter_weights": "100,100,100", "simplified_gyro_filter": "1"}
    raw.update(extra)
    return parse_settings(raw)


def test_filter_cli_only_changes_and_versions():
    import copy

    base = _settings()
    assert filter_sim.filter_cli(base, copy.deepcopy(base))[-1].startswith("# aucun changement")
    mod = copy.deepcopy(base)
    mod.gyro_lpf2.static_hz = 300
    mod.rpm_weights = [100, 50, 0]
    lines = filter_sim.filter_cli(base, mod)
    assert "set gyro_lpf2_static_hz = 300" in lines
    assert "set simplified_gyro_filter = OFF" in lines  # le curseur de filtre écraserait le réglage
    assert "set rpm_filter_weights = 100,50,0" in lines
    assert lines[-1] == "save"
    # 4.4 : pas de rpm_filter_weights dans le CLI
    b44 = _settings("4.4.3")
    m44 = copy.deepcopy(b44)
    m44.rpm_weights = [100, 50, 0]
    assert not any(ln.startswith("set rpm_filter_weights") for ln in filter_sim.filter_cli(b44, m44))


# ---------------------------------------------------------------- logs réels


def _state(paths):
    from opentunebox.gui.state import AppState
    from opentunebox.log.flightlog import load

    st = AppState()
    for p in paths:
        st.add(load(p)[0])
    return st


@pytest.mark.corpus
@pytest.mark.skipif(not (D08.exists() and D12.exists()), reason="logs BlackSummer absents")
def test_step_window(app):
    from opentunebox.gui.tools.step_window import StepWindow

    st = _state([D08, D12])
    win = StepWindow(st)
    win.auto_run = False
    win.files.selectAll()
    entries = win.files.selected_entries()
    assert len(entries) == 2
    res = win.compute_sync(entries)
    win.render(res)
    # Damping 0.8 (moins de D) dépasse plus que Damping 1.2 en roulis
    p08, p12 = res[0].axes[0].res.metrics.peak, res[1].axes[0].res.metrics.peak
    assert p08 > p12
    assert res[0].axes[0].res.n > 100
    assert np.isfinite(res[0].axes[0].sp_delay_ms)
    # boucle ouverte mesurée à partir des termes P, I, D, F (mode angle : instrument = manche)
    from opentunebox.gui.tools.step_window import loop_marker, metric_value

    lp = res[0].axes[0].res.loop
    assert lp.measured and 30 < lp.phase_margin_deg < 60
    assert metric_value(res[0], 0, "phase_margin")[1] > 0  # barre d'erreur = écart-type jackknife
    assert loop_marker(res[0], 0, "phase_margin") == "mesurée"
    # toutes les options d'affichage se rendent sans erreur
    for i in range(win.metric1.count()):
        win.metric1.setCurrentIndex(i)
        win.render(res)
    win.raw.setChecked(True)
    win.sd.setChecked(True)
    win.expand.setCurrentIndex(2)
    win.xmax.setCurrentIndex(2)
    win.metric1.setCurrentIndex(list(win.metric1.itemData(i) for i in range(win.metric1.count())).index("ripple"))
    win.render(res)
    # diagnostic du réglage, toujours affiché : série Damping 0,6 → 1,0 (curseurs de l'en-tête), pic 1,25 → 1,03
    text = win.report.toPlainText()
    assert not win.side.isHidden()
    assert win.slider_canvas.ci.getItem(0, 0) is not None  # curseurs Betaflight sous le rapport
    assert "Équilibre P/D (série Damping)" in text and "Damping conseillé : 1,0" in text
    _close(app, win)


@pytest.mark.corpus
@pytest.mark.skipif(not (D08.exists() and FINAL.exists()), reason="logs absents")
def test_pid_slider_window(app):
    from opentunebox.gui.tools.pid_slider import PidSliderWindow

    st = _state([D08, FINAL])
    win = PidSliderWindow(st)
    win.auto_run = False
    win.files.selectAll()
    res = win.compute_sync(win.files.selected_entries())
    win.render(res)
    text = win.cli.toPlainText()
    assert "simplified_tuning apply" in text
    assert "set simplified_d_gain = 60" in text  # curseurs de Damping 0.8 lus dans l'en-tête
    # sans modification, les PID simulés sont ceux du fichier (aucun écart affiché)
    assert win.table.item(0, 2).text() == "17"
    win.sl_widgets["d_gain"][1].setValue(100)
    assert win.table.item(0, 2).text().startswith("30")  # D roulis par défaut en 2025.12
    assert "set simplified_d_gain = 100" in win.cli.toPlainText()
    _close(app, win)


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log FINAL absent")
def test_filter_sim_window(app):
    from opentunebox.gui.tools.filter_sim import FilterSimWindow

    st = _state([FINAL])
    win = FilterSimWindow(st)
    win.auto_run = False
    entries = win.files.selected_entries()
    assert len(entries) == 1
    res = win.compute_sync(entries)
    win.render(res)
    assert "aucun changement" in win.cli.toPlainText()
    g = res.spectra["gyro"][0]
    assert g is not None and np.all(np.isfinite(g))
    assert res.spectra["gyro_sim"][0] is not None
    # mêmes réglages : le D simulé (réglages actuels) colle au D du log après mise à l'échelle
    assert np.isfinite(res.d_scale[0]) and res.d_scale[0] > 0
    # filtre gyro LPF2 abaissé → moins de bruit au-dessus de 300 Hz dans la simulation
    win.ed["g2_hz"].setValue(250)
    assert "set gyro_lpf2_static_hz = 250" in win.cli.toPlainText()
    res2 = win.compute_sync(entries)
    win.render(res2)
    hf = res.freqs > 300
    assert np.mean(res2.spectra["gyro_sim"][0][hf]) < np.mean(res.spectra["gyro_sim"][0][hf])
    assert "Retard des filtres" in win.delay_label.text()
    _close(app, win)
