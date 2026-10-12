"""Tests des fenêtres Analyse spectrale, Fréquence × gaz et Fréquence × temps (Qt hors écran).

Le calcul (`compute`) et l'affichage (`render`) sont appelés directement, sans thread, sur de vrais
logs ; `_busy` est forcé pour que les changements de réglages ne lancent pas de calcul en arrière-plan.
"""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")


import numpy as np  # noqa: E402
import pyqtgraph as pg  # noqa: E402
import pytest  # noqa: E402

from opentunebox.analysis import datasets  # noqa: E402
from opentunebox.analysis.spectral import PTB_PSD_OFFSET_DB, spectrum  # noqa: E402
from opentunebox.gui.tools import _spectral_common as sc  # noqa: E402

from corpus_paths import LOGS  # noqa: E402
BTFL = LOGS / "1. PTB/Apri/btfl_008.bbl"
FINAL = LOGS / "Cedric/JeNo 5/FINAL.bbl"

needs_logs = pytest.mark.skipif(not (BTFL.exists() and FINAL.exists()), reason="logs du corpus absents")



@pytest.fixture(scope="module")
def state(app):
    from opentunebox.gui.state import AppState
    from opentunebox.log.flightlog import load

    st = AppState()
    st.add(load(BTFL)[0])
    st.add(load(FINAL)[0])
    return st


def _open(state, cls, files=(0,)):
    win = cls(state, None)
    win._busy = True  # pas de calcul asynchrone pendant les tests
    win.files.clearSelection()
    for i in files:
        win.files.item(i).setSelected(True)
    return win


def _run(win):
    res = win.compute_sync(win.files.selected_entries())
    win._last_result = res
    win.render(res)
    return res


def _close(app, win):
    win.close()
    win.deleteLater()
    app.processEvents()


def _items(win, kind):
    return [it for it in win.canvas.scene().items() if isinstance(it, kind)]


# ---------------------------------------------------------------- sans log


def test_colormaps_and_helpers():
    for name in sc.COLORMAP_NAMES:
        lut = sc.colormap(name).getLookupTable(nPts=16)
        assert lut.shape[0] == 16
    assert sc.fmt_ms(1.25) == "1,25 ms"
    assert sc.fmt_ms(1.0) == "1 ms"
    assert sc.fmt_ms(float("nan")) == "—"
    assert sc.fmt3(0.07243) == "0,0724"
    t = np.arange(10_000) / 1000.0
    tt, yy = sc.decimate(t, np.ones_like(t), 1000)
    assert tt.size == yy.size <= 1000


# ---------------------------------------------------------------- avec logs


@pytest.mark.corpus
@needs_logs
def test_spectral_window(app, state):
    from opentunebox.gui.tools.spectral_window import SpectralWindow

    win = _open(state, SpectralWindow, files=(0, 1))
    try:
        assert win.selected_keys() == ["gyro", "gyro_prefilt"]
        res = _run(win)
        assert [f.entry.label for f in res.files] == ["btfl_008", "FINAL"]
        for fsp in res.files:
            assert len(fsp.spectra) == 6  # 2 jeux × 3 axes
            f, v = fsp.spectra[("gyro", 0)]
            assert f[-1] == pytest.approx(fsp.nyquist, rel=0.01)
            assert np.all(np.isfinite(v))
            # échelle de PIDtoolbox : densité physique − 6,5 dB, fenêtres de 2 s (0,5 Hz) par défaut
            log = fsp.entry.log
            ref = spectrum(datasets.get(log, "gyro")[0, fsp.entry.sel], log.fs, "psd", 0.5)
            assert np.allclose(v, ref.values - PTB_PSD_OFFSET_DB)
            assert np.isfinite(fsp.gyro_ms) and 0 < fsp.gyro_ms < 10
            assert len(fsp.motor_sorted) == 4 and fsp.motor_sorted[0].size > 100
            assert len(fsp.notch_sorted) >= 1
            mean, std = fsp.noise[("gyro_prefilt", 0)]
            assert mean.shape == (3,) and np.all(np.isfinite(mean))
            assert fsp.rpm_text.endswith(("Q500", "Q700"))
        plots = _items(win, pg.PlotItem)
        assert len(plots) == 6
        # mode « bruit moteur » : barres d'erreur ; axes réduits au roll
        win.right_mode.setCurrentIndex(1)
        assert _items(win, pg.ErrorBarItem)
        win.axis_boxes[1].setChecked(False)
        win.axis_boxes[2].setChecked(False)
        assert len(_items(win, pg.PlotItem)) == 2
        # options d'affichage seules (pas de recalcul)
        for i in range(win.harmonics.count()):
            win.harmonics.setCurrentIndex(i)
        win.notch_box.setChecked(False)
        # terme D : pas de yaw ; mode amplitude
        for i in range(win.ds_list.count()):
            it = win.ds_list.item(i)
            it.setSelected(it.text() in ("Terme D", "Terme D préfiltre"))
        win.psd_box.setChecked(False)
        assert win.ymin.value() == 0
        res = _run(win)
        assert res.mode == "amp"
        assert ("dterm", 2) not in res.files[0].spectra and ("dterm", 0) in res.files[0].spectra
    finally:
        _close(app, win)


@pytest.mark.corpus
@needs_logs
def test_freq_throttle_window(app, state):
    from opentunebox.gui.tools.freq_throttle import FreqThrottleWindow

    win = _open(state, FreqThrottleWindow, files=(0, 1))
    try:
        win.presets.setCurrentIndex(1)  # préréglage 1 : Gyro pf | Gyro | D pf | D, fichier A
        assert [c.currentData() for c in win.col_ds] == ["gyro_prefilt", "gyro", "dterm_prefilt", "dterm"]
        assert [c.currentData() for c in win.col_file] == [0, 0, 0, 0]
        res = _run(win)
        assert all(col is not None for col in res.columns)
        assert set(res.columns[0].maps) == {0, 1, 2}
        assert set(res.columns[3].maps) == {0, 1}  # pas de D en yaw
        ft = res.columns[0].maps[0]
        assert ft.z.shape == (ft.freqs.size, 100)
        mean, peak = ft.band_stats(100, 1000)
        assert np.isfinite(mean) and peak > mean > 0
        images = _items(win, pg.ImageItem)
        assert len([im for im in images if im.image is not None and im.image.ndim == 2]) >= 10
        # carte : fréquence en Y (0 → Nyquist), gaz en X (0 → 100)
        img = next(im for im in images if im.image is not None and im.image.shape[1] == 100)
        r = img.mapRectToParent(img.boundingRect())
        assert r.left() == pytest.approx(0) and r.right() == pytest.approx(100)
        assert r.bottom() == pytest.approx(ft.freqs[-1], abs=5)
        # préréglage 4 : A|A|B|B ; mode PSD ; < 100 Hz ; axe X en Hz moteur
        win.presets.setCurrentIndex(4)
        assert [c.currentData() for c in win.col_file] == [0, 0, 1, 1]
        win.psd_radio.setChecked(True)
        assert win.zmin.value() == -40 and win.col_scale[0].value() == 10
        win.col_sub[1].setChecked(True)
        win.xaxis.setCurrentIndex(2)
        res = _run(win)
        assert res.mode == "psd" and res.xaxis == "motor_hz"
        assert res.columns[2].entry.label == "FINAL"
        assert res.columns[0].x_range[1] > 100  # Hz moteur
        for name in sc.COLORMAP_NAMES:
            win.cmap.setCurrentIndex(sc.COLORMAP_NAMES.index(name))
        win._last_result = res
        win._auto_scale()
        assert win.col_scale[0].value() != 10  # adapté aux données
        # préréglage 6 avec 2 fichiers : colonnes C et D vides
        win.presets.setCurrentIndex(6)
        res = _run(win)
        assert res.columns[2] is None and res.columns[3] is None
    finally:
        _close(app, win)


@pytest.mark.corpus
@needs_logs
def test_freq_time_window(app, state):
    from opentunebox.gui.tools.freq_time import FreqTimeWindow

    win = _open(state, FreqTimeWindow, files=(1,))
    try:
        res = _run(win)
        assert res.entry.label == "FINAL" and res.key == "gyro_prefilt"
        sg = res.maps[0]
        assert sg.z.shape == (sg.freqs.size, sg.times.size)
        t0, t1 = res.entry.period
        assert t0 - 0.5 <= sg.times[0] and sg.times[-1] <= t1 + 0.5
        assert len(res.motor_hz) == 4 and res.motor_t.size <= 3000
        assert len(win.plots) == 3
        # curseur du lecteur : synchronisé dans les deux sens
        state.set_cursor(42.0)
        assert all(line.value() == pytest.approx(42.0) for line in win.cursor_lines)
        win.cursor_lines[1].setValue(50.0)
        assert state.cursor == pytest.approx(50.0)
        # options d'affichage
        win.sub100.setChecked(True)
        assert win.plots[0].vb.viewRange()[1][1] == pytest.approx(100.0)
        win.notch_box.setChecked(True)
        win.harmonics.setCurrentIndex(4)
        win.motors.boxes[0].setChecked(False)
        # terme D : seulement roll et pitch
        win.dataset.setCurrentIndex(win.dataset.findData("dterm"))
        win.time_step.setCurrentIndex(3)
        res = _run(win)
        assert set(res.maps) == {0, 1}
        assert len(win.plots) == 2
    finally:
        _close(app, win)


@pytest.mark.corpus
@needs_logs
def test_spectral_noise_diagnosis(app, state):
    from opentunebox.gui.tools.spectral_window import SpectralWindow

    win = _open(state, SpectralWindow)
    try:
        res = _run(win)
        assert res.files[0].diagnosis is None and win.report.isHidden()  # calculé seulement sur demande
        win.diag_box.setChecked(True)
        res = _run(win)
        nr = res.files[0].diagnosis
        assert [a.axis for a in nr.gyro] == ["Roll", "Pitch", "Yaw"] and len(nr.dterm) == 2
        assert all(a.reduction_db > 0 for a in nr.gyro)  # les filtres baissent le bruit
        assert nr.strategy
        assert not win.report.isHidden()
        text = win.report.toPlainText()
        for part in ("Conditions du log", "Gyro : bruit avant et après", "Filtre RPM", "Notch dynamique",
                     "Passe-bas du gyro", "Terme D :", "Passe-bas du D", "Stratégie de filtrage"):
            assert part in text
        win.diag_box.setChecked(False)  # masqué sans recalcul
        assert win.report.isHidden()
    finally:
        _close(app, win)
