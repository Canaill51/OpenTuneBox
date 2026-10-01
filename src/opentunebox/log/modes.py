"""Noms des modes (boîtes) enregistrés dans flightModeFlags (= rcModeActivationMask du firmware)."""

from __future__ import annotations

import numpy as np

from .debug_tables import family

_44 = ["ARM", "ANGLE", "HORIZON", "MAG", "HEADFREE", "PASSTHRU", "FAILSAFE", "GPSRESCUE", "ANTIGRAVITY",
       "HEADADJ", "CAMSTAB", "BEEPERON", "LEDLOW", "CALIB", "OSD", "TELEMETRY", "SERVO1", "SERVO2", "SERVO3",
       "BLACKBOX", "AIRMODE", "3D", "FPVANGLEMIX", "BLACKBOXERASE", "CAMERA1", "CAMERA2", "CAMERA3",
       "FLIPOVERAFTERCRASH", "PREARM", "BEEPGPSCOUNT", "VTXPITMODE", "PARALYZE", "USER1", "USER2", "USER3",
       "USER4", "PIDAUDIO", "ACROTRAINER", "VTXCONTROLDISABLE", "LAUNCHCONTROL", "MSPOVERRIDE",
       "STICKCOMMANDDISABLE", "BEEPERMUTE", "READY"]
_2025 = ["ARM", "ANGLE", "HORIZON", "MAG", "ALTHOLD", "HEADFREE", "CHIRP", "PASSTHRU", "FAILSAFE", "POSHOLD",
         "GPSRESCUE", "ANTIGRAVITY", "HEADADJ", "CAMSTAB", "BEEPERON", "LEDLOW", "CALIB", "OSD", "TELEMETRY",
         "SERVO1", "SERVO2", "SERVO3", "BLACKBOX", "AIRMODE", "3D", "FPVANGLEMIX", "BLACKBOXERASE", "CAMERA1",
         "CAMERA2", "CAMERA3", "CRASHFLIP", "PREARM", "BEEPGPSCOUNT", "VTXPITMODE", "PARALYZE", "USER1",
         "USER2", "USER3", "USER4", "PIDAUDIO", "ACROTRAINER", "VTXCONTROLDISABLE", "LAUNCHCONTROL",
         "MSPOVERRIDE", "STICKCOMMANDDISABLE", "BEEPERMUTE", "READY", "LAPTIMERRESET"]
_2026 = _2025[:11] + ["AUTOPILOT"] + _2025[11:] + ["WPCAPTURE"]

TABLES = {"4.4": _44, "4.5": _44 + ["LAPTIMERRESET"], "2025.12": _2025, "2026.6": _2026}

# Modes affichés sous la ligne de temps (les autres sont des interrupteurs sans intérêt pour le réglage)
SHOWN = {"ARM", "ANGLE", "HORIZON", "ALTHOLD", "POSHOLD", "HEADFREE", "CHIRP", "PASSTHRU", "FAILSAFE",
         "GPSRESCUE", "AUTOPILOT", "AIRMODE", "3D", "FLIPOVERAFTERCRASH", "CRASHFLIP", "ACROTRAINER",
         "LAUNCHCONTROL", "PIDAUDIO"}


def mode_names(mask: int, version: tuple[int, ...], shown_only: bool = True) -> list[str]:
    table = TABLES[family(version)]
    out = []
    for i, name in enumerate(table):
        if mask >> i & 1 and (not shown_only or name in SHOWN):
            out.append(name)
    return out


def mode_changes(flags: np.ndarray | None, t: np.ndarray, version: tuple[int, ...]) -> list[tuple[float, str]]:
    """Liste (instant, « |ARM|ANGLE ») à chaque changement des modes affichés."""
    if flags is None or flags.size == 0:
        return []
    out = []
    last = None
    idx = np.concatenate([[0], np.nonzero(np.diff(flags))[0] + 1])
    for i in idx:
        names = mode_names(int(flags[i]), version)
        label = "|" + "|".join(names) if names else "|—"
        if label != last:
            out.append((float(t[i]), label))
            last = label
    return out
