"""Script de publication : préparation du CHANGELOG et extraction des notes de version."""
import importlib.util
from pathlib import Path

import pytest

_spec = importlib.util.spec_from_file_location("release", Path(__file__).parent.parent / "scripts" / "release.py")
release = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(release)

CHANGELOG = """# Journal des versions

## [Non publié]

### Ajouté
- Page de téléchargement.

## [0.1.0] — 2026-10-01

### Ajouté
- Première version.

[Non publié]: ../../compare/v0.1.0...HEAD
[0.1.0]: ../../releases/tag/v0.1.0
"""


def test_prepare_changelog_dates_section_and_links():
    text = release.prepare_changelog(CHANGELOG, "0.2.0", "2026-10-05")
    assert "## [Non publié]\n\n## [0.2.0] — 2026-10-05\n\n### Ajouté\n- Page de téléchargement." in text
    assert "[Non publié]: ../../compare/v0.2.0...HEAD\n[0.2.0]: ../../releases/tag/v0.2.0\n[0.1.0]:" in text
    assert release.changelog_section(text, "0.2.0") == "### Ajouté\n- Page de téléchargement."
    assert release.changelog_section(text, "0.1.0") == "### Ajouté\n- Première version."
    # La nouvelle section « Non publié » est vide.
    assert release.changelog_section(text, "Non publié") == ""


def test_prepare_changelog_refuses_existing_version():
    with pytest.raises(SystemExit):
        release.prepare_changelog(CHANGELOG, "0.1.0", "2026-10-05")


def test_changelog_section_missing():
    assert release.changelog_section(CHANGELOG, "9.9.9") is None


def test_real_changelog_has_current_version():
    """Le numéro du projet est le même partout et le CHANGELOG a une section pour lui."""
    py, init = release.project_versions()
    assert py == init
    assert release.changelog_section(release.CHANGELOG.read_text(encoding="utf-8"), py)
