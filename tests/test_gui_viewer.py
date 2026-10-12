"""Tests du visualiseur : période d'analyse réglable sur chaque graphique (Qt hors écran)."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")


import pytest  # noqa: E402

from corpus_paths import LOGS  # noqa: E402
FINAL = LOGS / "Cedric/JeNo 5/FINAL.bbl"



@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_period_region_on_every_plot(app):
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.gui.state import AppState
    from opentunebox.log.flightlog import load

    st = AppState()
    st.add(load(FINAL)[0])
    win = MainWindow(st)
    app.processEvents()
    # une zone par graphique : roll, pitch, yaw et moteurs
    assert len(win.regions) == 4
    t0, t1 = st.logs[0].period
    yaw = win.regions[2]
    # tirer la zone sur le graphique yaw (setRegion émettrait aussi le signal de relâchement)
    yaw.blockSignals(True)
    yaw.setRegion((t0 + 2.0, t1 - 2.0))
    yaw.blockSignals(False)
    yaw.sigRegionChanged.emit(yaw)
    assert win.regions[3].getRegion() == pytest.approx((t0 + 2.0, t1 - 2.0))  # les autres suivent en direct
    assert st.logs[0].period == (t0, t1)  # sans relancer les calculs avant le relâchement
    yaw.sigRegionChangeFinished.emit(yaw)
    assert st.logs[0].period == pytest.approx((t0 + 2.0, t1 - 2.0))
    for r in win.regions:
        assert r.getRegion() == pytest.approx((t0 + 2.0, t1 - 2.0))
        # info-bulles des bords, avec le temps en écriture française
        tips = sorted((ln.value(), ln.toolTip()) for ln in r.lines)
        assert tips[0][1].startswith("Début de la période d'analyse : ")
        assert tips[1][1].startswith("Fin de la période d'analyse : ")
        assert f"{t1 - 2.0:.2f}".replace(".", ",") + " s" in tips[1][1]
    win.close()
    win.deleteLater()
    app.processEvents()


class _Click:
    """Clic simulé tel que le transmet la scène pyqtgraph."""

    def __init__(self, pos):
        self._pos = pos

    def button(self):
        from PySide6.QtCore import Qt
        return Qt.LeftButton

    def double(self):
        return False

    def scenePos(self):
        return self._pos

    def accept(self):
        pass


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_measure_between_two_points(app):
    from PySide6.QtCore import QPointF

    from opentunebox.gui.main_window import MainWindow
    from opentunebox.gui.state import AppState
    from opentunebox.log.flightlog import load

    st = AppState()
    st.add(load(FINAL)[0])
    win = MainWindow(st)
    win.resize(1400, 900)
    app.processEvents()
    roll = win.axis_plots[0]
    roll.setXRange(10.0, 10.5, padding=0)
    app.processEvents()

    def click(plot, t):
        win._on_plot_clicked(_Click(plot.vb.mapViewToScene(QPointF(t, 0.0))))

    click(roll, 10.1)
    assert win._measure == []  # sans le mode mesure, un clic ne fait rien
    win.measure_btn.setChecked(True)
    click(roll, 10.1)
    click(win.axis_plots[2], 10.125)  # 2e point sur un autre axe
    a, b = win._measure
    assert b - a == pytest.approx(0.025, abs=0.002)
    assert win.measure_text() == f"Δt {(b - a) * 1000:.1f} ms · {1 / (b - a):.1f} Hz".replace(".", ",")
    assert all(ln.isVisible() and ln.value() == b for ln in win.measure_lines[1])
    win.measure_lines[1][1].setValue(a + 0.05)  # tirer le 2e repère sur pitch : tout suit
    assert win.measure_lines[1][2].value() == pytest.approx(a + 0.05)
    assert win.measure_text().startswith("Δt 50,0 ms · 20,0 Hz")
    click(roll, 10.3)  # 3e clic : nouvelle mesure
    assert len(win._measure) == 1 and not win.measure_lines[1][0].isVisible()
    win.measure_btn.setChecked(False)
    assert win._measure == [] and not win.measure_lines[0][0].isVisible()
    win.close()
    win.deleteLater()
    app.processEvents()


@pytest.mark.corpus
@pytest.mark.skipif(not FINAL.exists(), reason="log du corpus absent")
def test_toolbar_buttons_switch_pages(app):
    from opentunebox.gui.main_window import MainWindow
    from opentunebox.gui.state import AppState
    from opentunebox.log.flightlog import load

    st = AppState()
    st.add(load(FINAL)[0])
    win = MainWindow(st)
    checked = lambda: [k for k, a in win._tool_actions.items() if a.isChecked()]  # noqa: E731
    assert win.current_page() == "viewer" and checked() == ["viewer"]
    win.open_tool("step")
    win.open_tool("spectral")
    win.open_tool("step")  # déjà ouvert : on revient à sa page, sans en créer une autre
    assert win.pages.count() == 4 and win.current_page() == "step"  # visualiseur, guide, 2 outils and checked() == ["step"]
    win.open_tool("freq_time")  # ouvert depuis Spectre : le bouton Spectre reste mis en valeur
    assert checked() == ["spectral"]
    win.show_viewer()
    assert checked() == ["viewer"] and win.pages.currentWidget() is win.viewer_page
    t0, t1 = st.logs[0].period
    st.set_period(0, t0 + 1.0, t1 - 1.0)
    assert win.tool_windows["step"]._stale  # page cachée : recalcul repoussé à son affichage
    win.close()
    win.deleteLater()
    app.processEvents()
