"""Les workflows GitHub sont du YAML valide : un fichier invalide fait échouer l'exécution en 0 s, sans message utile
(constaté le 2026-10-29 : un nom d'étape contenant « : » non protégé par des guillemets)."""

from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

WORKFLOWS = sorted((Path(__file__).parent.parent / ".github" / "workflows").glob("*.yml"))


@pytest.mark.parametrize("path", WORKFLOWS, ids=lambda p: p.name)
def test_workflow_is_valid_yaml(path):
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    assert data["jobs"], path.name
    for job in data["jobs"].values():
        for step in job.get("steps", []):
            assert "uses" in step or "run" in step, f"{path.name} : étape sans action ({step.get('name')})"
