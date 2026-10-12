"""Passage des valeurs brutes d'une session (DecodedSession) aux unités physiques (FlightLog), sans corpus."""

from pathlib import Path

import numpy as np

from opentunebox.decoder.header import LogHeader
from opentunebox.decoder.reader import DecodedSession
from opentunebox.log.flightlog import from_session


def make_session(columns: dict[str, list], header: dict[str, str] | None = None,
                 slow: dict[str, list] | None = None) -> DecodedSession:
    """Session décodée fabriquée à la main : `columns` = colonnes principales (loopIteration et time ajoutés
    s'ils manquent), `header` = lignes H brutes, `slow` = colonnes lentes (flightModeFlags…)."""
    cols = dict(columns)
    n = len(next(iter(cols.values())))
    cols.setdefault("loopIteration", list(range(n)))
    cols.setdefault("time", [1_000_000 + 500 * i for i in range(n)])  # 2 kHz
    names = ["loopIteration", "time"] + [k for k in cols if k not in ("loopIteration", "time")]
    main = np.array([cols[k] for k in names], dtype=np.int64).T
    slow = slow or {}
    slow_arr = np.array([slow[k] for k in slow], dtype=np.int64).T if slow else np.zeros((n, 0), dtype=np.int64)
    return DecodedSession(
        index=1, header=LogHeader(dict(header or {}), {}, 0, 0), main_names=names, main=main,
        main_kind=np.zeros(n, dtype=np.int64), slow_names=list(slow), slow=slow_arr,
        gps_names=[], gps=np.zeros((0, 0)), gps_at=np.zeros(0, dtype=np.int64), events=np.zeros((0, 4), dtype=np.int64),
    )


def test_high_resolution_scales_axes_and_rc_but_not_mixer_throttle():
    """`blackbox_high_resolution:1` : gyro, consigne des axes et les quatre rcCommand × 10 ; le gaz du mixeur
    (setpoint[3] = mixeur × 1000) ne change pas d'échelle (blackbox.c 4.4 → 2026.12, spec-betaflight.md §7)."""
    hr = {"gyroADC[0]": [1000] * 4, "setpoint[0]": [2000] * 4, "setpoint[3]": [500] * 4,
          "rcCommand[0]": [-3000] * 4, "rcCommand[3]": [15000] * 4}
    lg = from_session(Path("x.bbl"), make_session(hr, {"blackbox_high_resolution": "1"}))
    assert lg.gyro[0, 0] == 100.0 and lg.setpoint[0, 0] == 200.0
    assert lg.rc_command[0, 0] == -300.0 and lg.rc_command[3, 0] == 1500.0
    assert lg.throttle[0] == 50.0  # 500 / 1000 du mixeur = 50 %
    # sans haute résolution : mêmes grandeurs, valeurs brutes dix fois plus petites
    std = {k: [v[0] // 10 if k != "setpoint[3]" else v[0]] * 4 for k, v in hr.items()}
    lg = from_session(Path("x.bbl"), make_session(std, {}))
    assert lg.gyro[0, 0] == 100.0 and lg.setpoint[0, 0] == 200.0 and lg.rc_command[3, 0] == 1500.0
    assert lg.throttle[0] == 50.0


def test_high_resolution_throttle_fallback_from_rc_command():
    """Sans setpoint[3], le gaz vient de rcCommand[3], lui aussi × 10 en haute résolution."""
    lg = from_session(Path("x.bbl"), make_session({"gyroADC[0]": [0] * 4, "rcCommand[3]": [10000, 15000, 20000, 20000]},
                                                  {"blackbox_high_resolution": "1"}))
    assert lg.throttle.tolist() == [0.0, 50.0, 100.0, 100.0]


def test_throttle_falls_back_to_rc_command():
    """Sans setpoint[3] (anciens firmwares), le gaz vient de rcCommand[3] (1000..2000 → 0..100 %)."""
    lg = from_session(Path("x.bbl"), make_session({"gyroADC[0]": [0] * 4, "rcCommand[3]": [1000, 1250, 1500, 2000]}))
    assert lg.throttle.tolist() == [0.0, 25.0, 50.0, 100.0]
