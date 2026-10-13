"""Un vrai fichier Blackbox versionné, décodé sans corpus (L-025, W-009).

`tests/data/jeno5_5s.bbl` : les 5,1 premières secondes de la 1re session de `Cedric/JeNo 5/FINAL.bbl`
(Betaflight 2026.6.0-rc3, F405, 1618 Hz, 52 champs, eRPM, gyro non filtré, sans GPS), coupées sur une
frontière de trame I avec un événement de fin de log ajouté. Il exerce l'en-tête, les encodages et les
prédicteurs réels que `bbl_factory` n'écrit pas. Valeurs attendues relevées le 2026-10-12 avec notre
décodeur, égales à celles de `blackbox_decode` (PIDtoolbox) sur toutes les colonnes alignées sur `time`
(`scripts/validate_corpus.py` : statut `ok`), et identiques au début de la session complète.
"""

import hashlib
from pathlib import Path

import numpy as np
import pytest

from opentunebox.cli import main
from opentunebox.decoder.reader import decode_file, list_sessions
from opentunebox.log.flightlog import load

LOG = Path(__file__).resolve().parent / "data" / "jeno5_5s.bbl"
SHA256 = "71daf50b649ca763ebb7d880b678aa538493ab0fe4b6d75ac69cd58551b6683e"
FIRST_ROW = [0, 89946104, -8, 19, -15, 0, 0, 0, 6, 4, 0, 0, 0, 2, 3, -1, 1000, -2, 8, 0, 0, 2493, 72, 455, 2, -1, 5,
             1, -1, 2, 96, 128, 1999, 176, -447, 2242, 1, 197, 255, 0, 0, 0, 0, 0, 351, 158, 264, 221, 12, 12, 17, 13]
LAST_ROW = [16522, 95048448, 2, 8, -1, -2, 28, -1, 1, 0, 0, 3, 0, 13, -103, -1, 1220, 4, -49, 0, 224, 2409, 0, 498,
            3, -53, 0, -5, -42, 13, -443, 23, 2190, -871, 7311, 3116, 5, 216, 350, 0, 0, 0, 0, 0, 672, 484, 660, 498,
            980, 748, 943, 754]


def test_file_is_intact():
    """Le fichier binaire n'a pas été altéré (fins de ligne, encodage) : `.gitattributes` le marque `binary`."""
    assert LOG.stat().st_size == 365702
    assert hashlib.sha256(LOG.read_bytes()).hexdigest() == SHA256


def test_header_and_frames_decode_like_the_reference():
    (h,) = list_sessions(LOG)
    assert h.raw["Firmware revision"] == "Betaflight 2026.6.0-rc3 (f625a9996) STM32F405"
    assert h.raw["looptime"] == "312" and h.raw["pid_process_denom"] == "1" and h.raw["Data version"] == "2"
    assert h.raw["P interval"] == "2" and h.p_interval == (1, 2) and h.raw["I interval"] == "102"
    assert len(h.raw) == 190 and set(h.frames) == {"I", "P", "S"}
    assert h.frames["I"].count == h.frames["P"].count == 52 and h.frames["S"].count == 5

    (s,) = decode_file(LOG)
    assert s.main.shape == (8262, 52) and s.main_names[:2] == ["loopIteration", "time"]
    assert s.main_names[24:30] == [f"{n}[{i}]" for n in ("gyroADC", "gyroUnfilt") for i in range(3)]
    assert s.main_names[-4:] == [f"eRPM[{i}]" for i in range(4)]
    assert s.stats == {"corrupt_frames": 0, "desync_frames": 0, "skipped_iterations": 8261, "log_end_seen": True,
                       "bytes": 360103, "i_frames": 162, "p_frames": 8100}
    assert s.main[0].tolist() == FIRST_ROW and s.main[-1].tolist() == LAST_ROW
    assert int((s.main_kind == 0).sum()) == 162 and s.main_kind[0] == 0 and s.main_kind[-1] == 1
    its, t = s.main[:, 0], s.main[:, 1]
    assert np.all(np.diff(its) == 2) and np.all(np.diff(t) > 0)  # P interval 1/2 : une itération sur deux
    assert s.slow_names == ["flightModeFlags", "stateFlags", "failsafePhase", "rxSignalReceived", "rxFlightChannelsValid"]
    assert s.slow.shape == (8262, 5) and s.slow[0].tolist() == [0, 0, 0, 0, 0] and s.slow[-1].tolist() == [1, 0, 0, 1, 1]
    assert s.gps.shape[0] == 0
    assert s.events.tolist() == [[0, 1, 89779532, 0], [30, 2507, 1, 3], [255, 8262, 0, 0]]


def test_flightlog_physical_units_and_settings():
    (lg,) = load(LOG)
    assert lg.n == 8262 and lg.fs == pytest.approx(1618.12, abs=0.01) and lg.duration == pytest.approx(5.1023, abs=1e-3)
    assert np.all(np.diff(lg.t) > 0) and lg.stats["implausible_frames"] == 0
    s = lg.settings
    assert s.version == (2026, 6, 0) and s.debug_mode == "FFT_FREQ"
    assert [p.text() for p in s.pids] == ["58,72,39,39,156", "67,84,48,48,178", "58,72,0,0,156"]
    assert (s.gyro_lpf1.static_hz, s.gyro_lpf2.static_hz) == (0, 750)
    assert (s.dterm_lpf1.dyn_min_hz, s.dterm_lpf1.dyn_max_hz, s.rpm_harmonics) == (105, 210, 3)
    assert lg.gyro_unfilt is not None and lg.gyro.shape == lg.gyro_unfilt.shape == (3, 8262)
    assert float(np.abs(lg.gyro).max()) == 79.0 and float(np.abs(lg.setpoint).max()) == 78.0
    assert lg.n_motors == 4 and lg.motor_hz is not None and float(np.nanmedian(lg.motor_hz)) == pytest.approx(97.4, abs=0.1)
    assert (float(lg.throttle.min()), float(lg.throttle.max())) == (0.0, pytest.approx(50.9, abs=0.1))
    assert lg.vbat is not None and (float(lg.vbat.min()), float(lg.vbat.max())) == pytest.approx((21.98, 25.05), abs=1e-5)
    assert lg.debug.shape == (8, 8262)


def test_cli_info_and_report_on_a_real_log(capsys):
    assert main(["info", str(LOG)]) == 0
    out = capsys.readouterr().out
    assert "Betaflight 2026.6.0-rc3" in out and "8262 trames" in out and "1618 Hz" in out
    assert "R 58,72,39,39,156" in out
    assert main(["report", str(LOG)]) == 0
    out = capsys.readouterr().out
    assert "4 à 5 pouces" in out and "Traceback" not in out
