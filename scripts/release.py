#!/usr/bin/env python3
"""Préparation et publication d'une version d'OpenTuneBox.

Usage :
  release.py prepare 0.2.0   met le numéro dans pyproject.toml et __init__.py, transforme la section
                             « Non publié » du CHANGELOG en « [0.2.0] — date du jour » et en rouvre une vide
  release.py notes v0.2.0    vérifie que l'étiquette correspond au numéro du projet et au CHANGELOG, puis
                             affiche la section de cette version (texte de la page Releases ; workflow construire.yml)
"""
import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PYPROJECT = ROOT / "pyproject.toml"
INIT = ROOT / "src" / "opentunebox" / "__init__.py"
CHANGELOG = ROOT / "CHANGELOG.md"

VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
PYPROJECT_RE = re.compile(r'^version = "([^"]+)"', re.M)
INIT_RE = re.compile(r'^__version__ = "([^"]+)"', re.M)


def project_versions() -> tuple[str, str]:
    """Numéros de version de pyproject.toml et de opentunebox/__init__.py."""
    return (PYPROJECT_RE.search(PYPROJECT.read_text(encoding="utf-8")).group(1),
            INIT_RE.search(INIT.read_text(encoding="utf-8")).group(1))


def changelog_section(text: str, version: str) -> str | None:
    """Corps de la section « ## [version] » du CHANGELOG (sans le titre), ou None si elle manque."""
    m = re.search(rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## \[|^\[[^\]]+\]: |\Z)", text, re.M | re.S)
    return m.group(1).strip() if m else None


def prepare_changelog(text: str, version: str, date: str) -> str:
    """Renomme « Non publié » en version datée, rouvre une section vide et met à jour les liens de bas de page."""
    if f"## [{version}]" in text:
        raise SystemExit(f"Le CHANGELOG contient déjà une section [{version}].")
    if "## [Non publié]" not in text:
        raise SystemExit("Section « ## [Non publié] » introuvable dans le CHANGELOG.")
    text = text.replace("## [Non publié]", f"## [Non publié]\n\n## [{version}] — {date}", 1)
    # Liens de bas de page : « Non publié » compare désormais à la nouvelle version.
    text = re.sub(r"^\[Non publié\]: \.\./\.\./compare/v\d+\.\d+\.\d+\.\.\.HEAD$",
                  f"[Non publié]: ../../compare/v{version}...HEAD\n[{version}]: ../../releases/tag/v{version}",
                  text, count=1, flags=re.M)
    return text


def prepare(version: str) -> None:
    if not VERSION_RE.match(version):
        raise SystemExit(f"Numéro invalide : {version!r} (attendu : 0.2.0, sans « v »).")
    old = project_versions()[0]
    CHANGELOG.write_text(prepare_changelog(CHANGELOG.read_text(encoding="utf-8"), version,
                                           datetime.date.today().isoformat()), encoding="utf-8")
    PYPROJECT.write_text(PYPROJECT_RE.sub(f'version = "{version}"', PYPROJECT.read_text(encoding="utf-8"), count=1),
                         encoding="utf-8")
    INIT.write_text(INIT_RE.sub(f'__version__ = "{version}"', INIT.read_text(encoding="utf-8"), count=1),
                    encoding="utf-8")
    print(f"Version {old} → {version} : pyproject.toml, src/opentunebox/__init__.py et CHANGELOG.md modifiés.")
    print("Relire le CHANGELOG, faire valider la demande de fusion, puis sur main :")
    print(f"  git tag v{version} && git push origin v{version}")


def notes(tag: str) -> None:
    version = tag.removeprefix("v")
    errors = [f"{name} indique {found}" for name, found in zip(("pyproject.toml", "__init__.py"), project_versions())
              if found != version]
    body = changelog_section(CHANGELOG.read_text(encoding="utf-8"), version)
    if not body:
        errors.append(f"le CHANGELOG n'a pas de section [{version}] (ou elle est vide)")
    if errors:
        raise SystemExit(f"Étiquette {tag} incohérente : " + " ; ".join(errors)
                         + f".\nLancer d'abord : python scripts/release.py prepare {version}")
    sys.stdout.reconfigure(encoding="utf-8")
    print(body)


def main(argv: list[str]) -> None:
    if len(argv) != 2 or argv[0] not in ("prepare", "notes"):
        raise SystemExit(__doc__)
    (prepare if argv[0] == "prepare" else notes)(argv[1])


if __name__ == "__main__":
    main(sys.argv[1:])
