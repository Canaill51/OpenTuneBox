"""Emplacement du corpus de vrais logs utilisé par les tests marqués `corpus`.

Par défaut `~/Documents/Drones/Logs` ; autre dossier : variable d'environnement `OTB_LOGS`.
Sans corpus, ces tests sont simplement ignorés (skip). Un log précis absent (supprimé, déplacé) fait
ignorer le test qui l'utilise, en le nommant : `pytest -rs` liste ces logs à remplacer.
"""

import os
from pathlib import Path

import pytest

LOGS = Path(os.environ.get("OTB_LOGS", Path.home() / "Documents" / "Drones" / "Logs"))


def corpus_log(rel: str) -> Path:
    """Chemin d'un log du corpus ; test ignoré (et log nommé) s'il n'existe plus."""
    path = LOGS / rel
    if not path.exists():
        pytest.skip(f"log du corpus absent : {rel}")
    return path
