"""Fixtures communes à tous les tests : Qt hors écran, réglages de l'application isolés, logs synthétiques."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")  # avant tout import de PySide6

import pytest  # noqa: E402

from synthetic import make_flightlog  # noqa: E402


def pytest_configure(config):
    from PySide6.QtCore import QStandardPaths

    # QSettings et fiches drone écrits dans un dossier de test, jamais dans ceux de l'utilisateur
    QStandardPaths.setTestModeEnabled(True)


@pytest.fixture(scope="session")
def app():
    from PySide6.QtWidgets import QApplication

    return QApplication.instance() or QApplication([])


@pytest.fixture(scope="session")
def synthetic_log():
    """Vol synthétique d'un 5" en Betaflight 4.5 (voir `tests/synthetic.py`)."""
    return make_flightlog()


@pytest.fixture
def synthetic_state(app, synthetic_log):
    """État de l'application avec deux logs synthétiques : un 4.5 et un 2025.12 (convention D max)."""
    from opentunebox.gui.state import AppState

    st = AppState()
    st.add(synthetic_log)
    st.add(make_flightlog(name="synth2", seed=1, **{"Firmware revision": "Betaflight 2025.12.0 (ef567890) STM32H743",
                                                     "d_min": None, "d_max": "50,54,0"}))
    return st
