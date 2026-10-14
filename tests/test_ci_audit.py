"""Contrat du workflow d'audit des dépendances (`.github/workflows/audit.yml`, L-031).

pip-audit contrôle tout l'environnement où il tourne : installé à côté du projet, il audite aussi pip et ses
propres bibliothèques, et une faille étrangère au projet fait échouer la CI. Ces tests lisent le workflow
(pas de réseau) et refusent un retour à cette forme.
"""

import re
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "audit.yml"


def _commands() -> str:
    """Le texte du workflow sans ses commentaires (les commentaires citent les commandes à éviter)."""
    lines = WORKFLOW.read_text(encoding="utf-8").splitlines()
    return "\n".join(re.sub(r"\s#.*$", "", line) for line in lines if not line.lstrip().startswith("#"))


def test_pip_is_upgraded_before_the_audit():
    """pip fait partie de ce qui est audité : celui de l'image peut avoir une faille connue."""
    text = _commands()
    assert "pip install --upgrade pip" in text
    assert text.index("pip install --upgrade pip") < text.index("pip-audit")


def test_pip_audit_is_not_installed_in_the_audited_environment():
    """pip-audit s'installe dans son propre environnement (`python -m venv`), jamais par `python -m pip install`."""
    text = _commands()
    assert not re.search(r"python -m pip install[^\n]*pip-audit", text)
    assert re.search(r"python -m venv\s+\S+", text)


def test_pip_audit_targets_the_project_environment():
    """Depuis son propre environnement, pip-audit doit être dirigé sur celui du projet (`--path`)."""
    assert re.search(r"pip-audit\"?\s+[^\n]*--path\b", _commands())
