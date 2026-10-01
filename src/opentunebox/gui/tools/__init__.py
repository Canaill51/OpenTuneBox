"""Registre des fenêtres d'outils."""

from __future__ import annotations

import importlib

# clé → (module, classe)
REGISTRY = {
    "spectral": ("spectral_window", "SpectralWindow"),
    "freq_throttle": ("freq_throttle", "FreqThrottleWindow"),
    "freq_time": ("freq_time", "FreqTimeWindow"),
    "step": ("step_window", "StepWindow"),
    "sliders": ("pid_slider", "PidSliderWindow"),
    "filter_sim": ("filter_sim", "FilterSimWindow"),
    "setup": ("setup_info", "SetupInfoWindow"),
    "stats": ("flight_stats", "FlightStatsWindow"),
    "advisor": ("advisor_panel", "AdvisorWindow"),
}


def create_tool(key: str, state, main_window):
    mod_name, cls_name = REGISTRY[key]
    try:
        mod = importlib.import_module(f"{__name__}.{mod_name}")
    except ModuleNotFoundError as e:
        if e.name and e.name.endswith(mod_name):
            return None
        raise
    return getattr(mod, cls_name)(state, main_window)
