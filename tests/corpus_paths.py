"""Emplacement du corpus de vrais logs utilisé par les tests marqués `corpus`.

Par défaut `~/Documents/Drones/Logs` ; autre dossier : variable d'environnement `OTB_LOGS`.
Sans corpus, ces tests sont simplement ignorés (skip).
"""

import os
from pathlib import Path

LOGS = Path(os.environ.get("OTB_LOGS", Path.home() / "Documents" / "Drones" / "Logs"))
