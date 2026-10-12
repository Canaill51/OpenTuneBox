"""Intégration au système : variante claire / sombre du thème d'icônes et traduction française de Qt."""

import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import QCoreApplication  # noqa: E402
from PySide6.QtGui import QIcon  # noqa: E402

from opentunebox.gui import theme  # noqa: E402



def test_icon_theme_follows_light_and_dark(app, tmp_path, monkeypatch):
    for name in ("breeze", "breeze-dark"):
        (tmp_path / name).mkdir()
        (tmp_path / name / "index.theme").write_text("[Icon Theme]\nName=x\n")
    old_paths, old_name = QIcon.themeSearchPaths(), QIcon.themeName()
    QIcon.setThemeSearchPaths([str(tmp_path)])
    QIcon.setThemeName("breeze-dark")  # bureau sombre, comme celui de Cédric
    monkeypatch.setattr(theme, "_system_icon_theme", None)
    try:
        theme.apply("clair")
        assert QIcon.themeName() == "breeze"
        theme.apply("sombre")
        assert QIcon.themeName() == "breeze-dark"
    finally:
        QIcon.setThemeSearchPaths(old_paths)
        QIcon.setThemeName(old_name)
        theme.apply("clair")


def test_icon_theme_without_variant_is_kept(app, tmp_path, monkeypatch):
    (tmp_path / "Adwaita").mkdir()
    (tmp_path / "Adwaita" / "index.theme").write_text("[Icon Theme]\nName=x\n")
    old_paths, old_name = QIcon.themeSearchPaths(), QIcon.themeName()
    QIcon.setThemeSearchPaths([str(tmp_path)])
    QIcon.setThemeName("Adwaita")
    monkeypatch.setattr(theme, "_system_icon_theme", None)
    try:
        theme.apply("sombre")
        assert QIcon.themeName() == "Adwaita"
    finally:
        QIcon.setThemeSearchPaths(old_paths)
        QIcon.setThemeName(old_name)
        theme.apply("clair")


def test_qt_texts_in_french(app):
    theme.install_qt_translations(app)
    assert QCoreApplication.translate("QFileDialog", "&Open") == "&Ouvrir"
