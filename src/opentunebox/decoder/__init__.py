"""Décodeur Blackbox natif."""

import os
import sys

# Application « figée » (PyInstaller) : numba ne peut pas écrire son cache de compilation à côté du
# code ; on le place dans le dossier de cache de l'utilisateur (Linux, Windows, macOS).
if getattr(sys, "frozen", False) and "NUMBA_CACHE_DIR" not in os.environ:
    from platformdirs import user_cache_dir

    os.environ["NUMBA_CACHE_DIR"] = os.path.join(user_cache_dir("OpenTuneBox", "OpenTuneBox"), "numba")
