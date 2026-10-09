"""Tests du visualiseur : période d'analyse réglable sur chaque graphique (Qt hors écran)."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")


import pytest  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from corpus_paths import LOGS  # noqa: E402
FINAL = LOGS / "Cedric/JeNo 5/FINAL.bbl"


@pytest.fixture(scope="module")
def app():
    return QApplication.instance() or QApplication([])


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
