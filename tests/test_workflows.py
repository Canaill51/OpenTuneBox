"""Les workflows GitHub sont du YAML valide : un fichier invalide fait échouer l'exécution en 0 s, sans message utile
(constaté le 2026-10-29 : un nom d'étape contenant « : » non protégé par des guillemets)."""

import re
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


def test_build_workflow_installs_what_its_tests_import():
    """La construction lance des tests avant PyInstaller : scipy et numba (références des tests, D-032) doivent y
    être installés dès qu'un test lancé, `conftest.py` ou `synthetic.py` les importe (échec constaté en relecture)."""
    data = yaml.safe_load((WORKFLOWS[0].parent / "construire.yml").read_text(encoding="utf-8"))
    steps = [s for job in data["jobs"].values() for s in job.get("steps", [])]
    install = next(s["run"] for s in steps if s.get("name") == "Dépendances")
    pytest_run = next(s["run"] for s in steps if s.get("name") == "Tests rapides")
    tests = Path(__file__).parent
    files = [tests / "conftest.py", tests / "synthetic.py"] + [tests.parent / m for m in re.findall(r"tests/\w+\.py", pytest_run)]
    for lib in ("scipy", "numba"):
        uses = [f.name for f in files if re.search(rf"^\s*(import|from) {lib}\b", f.read_text(encoding="utf-8"), re.M)]
        assert not uses or lib in install, f"construire.yml : {lib} utilisé par {uses} mais non installé"
