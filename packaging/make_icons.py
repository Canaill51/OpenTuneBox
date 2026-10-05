"""Fabrique les icônes d'application à partir de src/opentunebox/gui/assets/opentunebox.svg.

Usage : python packaging/make_icons.py   (les fichiers produits sont versionnés ; relancer si le SVG change)
  packaging/opentunebox.icns         macOS (application .app et .dmg)
  packaging/linux/opentunebox.png    Linux (AppImage), 256 × 256
"""
import os
import struct
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
from PySide6.QtCore import QBuffer, QByteArray, QIODevice, Qt  # noqa: E402
from PySide6.QtGui import QGuiApplication, QImage, QPainter  # noqa: E402
from PySide6.QtSvg import QSvgRenderer  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "src" / "opentunebox" / "gui" / "assets" / "opentunebox.svg"
# Entrées PNG du format ICNS (code à 4 lettres → taille en pixels).
ICNS_TYPES = [(b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256), (b"ic09", 512),
              (b"ic10", 1024), (b"ic11", 32), (b"ic12", 64), (b"ic13", 256), (b"ic14", 512)]


def render_png(renderer: QSvgRenderer, size: int) -> bytes:
    img = QImage(size, size, QImage.Format.Format_ARGB32)
    img.fill(Qt.GlobalColor.transparent)
    painter = QPainter(img)
    renderer.render(painter)
    painter.end()
    data = QByteArray()
    buf = QBuffer(data)
    buf.open(QIODevice.OpenModeFlag.WriteOnly)
    img.save(buf, "PNG")
    return bytes(data)


def main() -> int:
    app = QGuiApplication(sys.argv)  # nécessaire au rendu, même hors écran
    renderer = QSvgRenderer(str(SVG), app)
    chunks = b"".join(t + struct.pack(">I", 8 + len(png)) + png
                      for t, png in ((t, render_png(renderer, n)) for t, n in ICNS_TYPES))
    (ROOT / "packaging" / "opentunebox.icns").write_bytes(b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks)
    (ROOT / "packaging" / "linux" / "opentunebox.png").write_bytes(render_png(renderer, 256))
    print("Icônes écrites : packaging/opentunebox.icns, packaging/linux/opentunebox.png", flush=True)
    os._exit(0)  # évite le nettoyage de fin de PySide6


if __name__ == "__main__":
    sys.exit(main())
