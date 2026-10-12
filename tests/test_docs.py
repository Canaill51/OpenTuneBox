"""Cohérence de la documentation avec le code (lancé à chaque `pytest`, donc sur chaque demande de fusion).

Ce que les tests ne peuvent pas juger (le sens du texte) est décrit dans la compétence
`.claude/skills/docs-sync` : quels documents relire selon le type de modification.
"""

import ast
import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src" / "opentunebox"


def _md_files() -> list[Path]:
    out = subprocess.run(["git", "ls-files", "*.md"], cwd=ROOT, capture_output=True, text=True).stdout.split()
    files = [ROOT / f for f in out]
    return files or sorted(ROOT.glob("*.md")) + sorted((ROOT / "docs").glob("*.md"))


MD_FILES = _md_files()
USER_DOCS = [ROOT / "README.md", ROOT / "docs" / "guide-utilisateur.md"]


def _slug(heading: str) -> str:
    """Ancre GitHub d'un titre : minuscules, ponctuation retirée, espaces → tirets."""
    return re.sub(r"[^\w\- ]", "", heading.strip().lower()).replace(" ", "-")


def _anchors(path: Path) -> set[str]:
    text = re.sub(r"```.*?```", "", path.read_text(encoding="utf-8"), flags=re.S)
    return {_slug(m.group(1)) for m in re.finditer(r"^#{1,6}\s+(.+)$", text, re.M)}


def _code_spans(text: str) -> list[str]:
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    return re.findall(r"`([^`\n]+)`", text)


def _rel(path: Path) -> str:
    return str(path.relative_to(ROOT))


@pytest.mark.parametrize("md", MD_FILES, ids=_rel)
def test_relative_links_exist(md):
    """Liens [texte](chemin#ancre) vers des fichiers du dépôt : le fichier et l'ancre existent."""
    text = re.sub(r"```.*?```", "", md.read_text(encoding="utf-8"), flags=re.S)
    broken = []
    for target in re.findall(r"\]\(([^)\s]+)\)", text):
        if target.startswith(("http://", "https://", "mailto:", "../../", "otb:")):
            continue  # pages GitHub (Releases…), liens externes, liens internes du guide (test_guide_links)
        file_part, _, anchor = target.partition("#")
        dest = (md.parent / file_part).resolve() if file_part else md
        if not dest.exists():
            broken.append(f"{target} (fichier absent)")
        elif anchor and dest.suffix == ".md" and anchor not in _anchors(dest):
            broken.append(f"{target} (titre absent)")
    assert not broken, f"{_rel(md)} : liens cassés : {broken}"


# Préfixes des chemins du dépôt cités entre `…` ; les chemins de gui/, analysis/… sont relatifs à src/opentunebox/.
REPO_PREFIXES = ("src/opentunebox/", "tests/", "scripts/", "packaging/", "docs/", ".github/", ".claude/")
PKG_DIRS = ("decoder/", "log/", "analysis/", "advisor/", "gui/")


def _symbol_defined(py: Path, name: str) -> bool:
    tree = ast.parse(py.read_text(encoding="utf-8"))
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and node.name == name:
            return True
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == name for t in node.targets):
            return True
    return False


@pytest.mark.parametrize("md", MD_FILES, ids=_rel)
def test_code_paths_and_symbols_exist(md):
    """Chemins cités (`packaging/…`, `gui/app.py:hard_exit`…) : le fichier et la fonction / classe existent."""
    missing = []
    for span in _code_spans(md.read_text(encoding="utf-8")):
        m = re.fullmatch(r"([\w./-]+?)(?::(\w+))?", span.strip())
        if not m or "/" not in m.group(1) or "*" in span:
            continue
        path, symbol = m.group(1).rstrip("/"), m.group(2)
        if path.startswith(REPO_PREFIXES):
            candidates = [ROOT / path]
        elif path.startswith(PKG_DIRS) or path.startswith(("tools/", "widgets/")):
            candidates = [SRC / path, SRC / "gui" / path]
        else:
            continue  # chemins du firmware Betaflight, noms de branches…
        found = next((c for c in candidates if c.exists()), None)
        if found is None:
            missing.append(span)
        elif symbol and found.suffix == ".py" and not _symbol_defined(found, symbol):
            missing.append(f"{span} (symbole absent)")
    assert not missing, f"{_rel(md)} : chemins ou symboles introuvables : {missing}"


def _constant(name: str):
    tree = ast.parse((SRC / "gui" / "main_window.py").read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == name for t in node.targets):
            return ast.literal_eval(node.value)
    raise AssertionError(f"{name} introuvable dans gui/main_window.py")


def _tool_labels() -> list[str]:
    """Libellés de tous les outils : barre d'outils (TOOLS) et outils ouverts ailleurs (OTHER_TOOLS)."""
    return [t[1] for t in _constant("TOOLS")] + [v[0] for v in _constant("OTHER_TOOLS").values()]


def test_every_tool_documented():
    """Chaque outil figure dans le tableau du README et dans un titre du guide."""
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    guide = (ROOT / "docs" / "guide-utilisateur.md").read_text(encoding="utf-8")
    guide_titles = re.findall(r"^### (.+)$", guide, re.M)
    missing = []
    for label in _tool_labels():
        if not re.search(rf"^\| {re.escape(label)} \|", readme, re.M):
            missing.append(f"README : ligne « | {label} | » du tableau des outils")
        if not any(t == label or t.endswith(f"({label})") for t in guide_titles):
            missing.append(f"guide : titre « ### … ({label}) »")
    assert not missing, missing


def test_guide_links():
    """Guide intégré : seul l'accueil est en Markdown ; chaque lien `otb:phase/…` / `otb:tool/…` mène quelque part."""
    from opentunebox.gui.guide import GUIDE_DIR, HOME_FILE, PHASE_KEYS
    from opentunebox.gui.tools import REGISTRY

    assert {p.name for p in GUIDE_DIR.glob("*.md")} == {HOME_FILE}
    broken = []
    for kind, key in re.findall(r"\]\(otb:(\w+)/(\w+)\)", (GUIDE_DIR / HOME_FILE).read_text(encoding="utf-8")):
        ok = key in PHASE_KEYS if kind == "phase" else kind == "tool" and (key in REGISTRY or key == "viewer")
        if not ok:
            broken.append(f"otb:{kind}/{key}")
    assert not broken, broken


def test_guide_steps_reference_real_tools_and_checks():
    """Parcours du guide : outils, parties du vol et points de contrôle cités existent."""
    import inspect

    from opentunebox.advisor import log_check
    from opentunebox.advisor.drone_card import DroneCard
    from opentunebox.advisor.tuning_path import phases
    from opentunebox.gui.tools import REGISTRY

    src = inspect.getsource(log_check)
    keys = set(re.findall(r'add\("(\w+)"', src)) | set(re.findall(r'Usage\("(\w+)"', src))
    bad = []
    for ph in phases(DroneCard("x")):
        for s in ph.steps:
            if s.tool and s.tool not in REGISTRY and s.tool != "viewer":
                bad.append(f"{s.key} : outil {s.tool}")
            if s.part not in (None, "hover", "moving"):
                bad.append(f"{s.key} : partie {s.part}")
            bad += [f"{s.key} : contrôle {k}" for k in (*s.checks, *s.usages) if k not in keys]
    assert not bad, bad


def test_every_module_documented():
    """Chaque module du programme est cité dans AGENTS.md (organisation) ou docs/architecture.md."""
    text = (ROOT / "AGENTS.md").read_text(encoding="utf-8") + (ROOT / "docs" / "architecture.md").read_text(encoding="utf-8")
    missing = [_rel(py) for py in sorted(SRC.rglob("*.py"))
               if not py.name.startswith("_") and py.stem not in text]
    assert not missing, f"modules absents de AGENTS.md et docs/architecture.md : {missing}"


def test_docs_index_complete():
    """docs/README.md liste chaque document de docs/."""
    index = (ROOT / "docs" / "README.md").read_text(encoding="utf-8")
    missing = [p.name for p in sorted((ROOT / "docs").glob("*.md")) if p.name != "README.md" and f"({p.name})" not in index]
    assert not missing, f"documents absents de docs/README.md : {missing}"


def _release_files() -> list[str]:
    workflow = (ROOT / ".github" / "workflows" / "construire.yml").read_text(encoding="utf-8")
    return re.findall(r"^\s+package: (\S+)$", workflow, re.M)


def test_download_files_documented():
    """Les fichiers publiés (construire.yml) sont ceux que décrivent le README et le guide."""
    published = _release_files()
    assert published, "aucun fichier « package: » dans construire.yml"
    for doc in USER_DOCS:
        text = doc.read_text(encoding="utf-8")
        cited = set(re.findall(r"OpenTuneBox-[\w.-]+?\.(?:exe|dmg|AppImage|zip|tar\.gz)", text))
        assert set(published) <= cited, f"{_rel(doc)} : fichiers publiés non décrits : {set(published) - cited}"
        assert cited <= set(published), f"{_rel(doc)} : fichiers qui ne sont plus publiés : {cited - set(published)}"


def test_cli_commands_documented():
    """Chaque commande de `opentunebox` (cli.py) figure dans le README."""
    commands = re.findall(r'add_parser\("(\w+)"', (SRC / "cli.py").read_text(encoding="utf-8"))
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    missing = [c for c in commands if f"opentunebox {c}" not in readme]
    assert not missing, f"commandes absentes du README : {missing}"


# Mots anglais très courants : un document qui en contient beaucoup n'est probablement pas en français.
_ENGLISH = re.compile(r"\b(the|and|with|from|when|which|only|are|is|of|in|to|by|for)\b", re.I)
_FRENCH = re.compile(r"\b(le|la|les|des|du|et|avec|pour|dans|sur|est|une|un|par|en)\b", re.I)


@pytest.mark.parametrize("md", [p for p in MD_FILES if ".github" not in p.parts], ids=_rel)
def test_documentation_in_french(md):
    """Règle du projet : documentation en français (le code, les formules et les noms restent tels quels)."""
    text = re.sub(r"```.*?```|`[^`\n]+`|\([^)]*\)|https?://\S+", " ", md.read_text(encoding="utf-8"), flags=re.S)
    en, fr = len(_ENGLISH.findall(text)), len(_FRENCH.findall(text))
    assert en <= max(5, fr // 3), f"{_rel(md)} semble rédigé en anglais ({en} mots anglais courants, {fr} français)"


# Registres numérotés (décision D-018) : décisions, leçons, travaux reportés.
REGISTERS = {
    "D": (ROOT / "docs" / "decisions.md", r"^### D-(\d{3}) "),
    "L": (ROOT / "docs" / "lecons.md", r"^### L-(\d{3}) "),
    "W": (ROOT / "docs" / "travaux-reportes.md", r"^- (?:~~)?\*\*W-(\d{3})\*\*"),
}


def _register_entries(prefix: str) -> list[str]:
    path, pattern = REGISTERS[prefix]
    text = re.sub(r"```.*?```", "", path.read_text(encoding="utf-8"), flags=re.S)
    return [f"{prefix}-{n}" for n in re.findall(pattern, text, re.M)]


@pytest.mark.parametrize("prefix", sorted(REGISTERS))
def test_register_ids_unique(prefix):
    """Chaque numéro n'est attribué qu'une fois, et l'index du registre liste exactement les entrées."""
    entries = _register_entries(prefix)
    assert entries, f"aucune entrée {prefix}- dans {_rel(REGISTERS[prefix][0])}"
    doubles = sorted({e for e in entries if entries.count(e) > 1})
    assert not doubles, f"numéros attribués deux fois : {doubles}"
    text = REGISTERS[prefix][0].read_text(encoding="utf-8")
    index = re.findall(rf"^\| ({prefix}-\d{{3}}) \|", text, re.M)
    assert sorted(index) == sorted(entries), (
        f"{_rel(REGISTERS[prefix][0])} : index ≠ entrées "
        f"(absents de l'index : {sorted(set(entries) - set(index))}, en trop : {sorted(set(index) - set(entries))})")


def test_register_ids_cited_exist():
    """Chaque D-xxx, L-xxx ou W-xxx cité dans la documentation, le code ou les tests existe dans son registre."""
    known = {e for p in REGISTERS for e in _register_entries(p)}
    tracked = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True).stdout.split()
    files = {ROOT / f for f in tracked if f.endswith((".md", ".py", ".yml"))} | set(MD_FILES)
    files |= {p for p, _ in REGISTERS.values()}
    unknown = []
    for f in sorted(files):
        if not f.exists():
            continue
        for ref in sorted(set(re.findall(r"\b[DLW]-\d{3}\b", f.read_text(encoding="utf-8")))):
            if ref not in known:
                unknown.append(f"{_rel(f)} : {ref}")
    assert not unknown, f"numéros cités qui n'existent pas : {unknown}"
