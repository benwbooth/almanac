#!/usr/bin/env python3
"""Render icons/almanac.svg into the icon files the installers need.

Writes packaging/icons/almanac.ico (Windows), almanac.icns (macOS) and
almanac-512.png. Run it again after changing the SVG:

    pip install cairosvg pillow && python3 scripts/make-icons.py
"""
import io
import pathlib

import cairosvg
from PIL import Image

root = pathlib.Path(__file__).resolve().parent.parent
svg = (root / "icons" / "almanac.svg").read_bytes()
out = root / "packaging" / "icons"
out.mkdir(parents=True, exist_ok=True)

big = Image.open(io.BytesIO(cairosvg.svg2png(bytestring=svg, output_width=1024, output_height=1024))).convert("RGBA")
big.resize((512, 512), Image.LANCZOS).save(out / "almanac-512.png")
big.save(out / "almanac.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
big.save(out / "almanac.icns")
print("wrote", ", ".join(sorted(p.name for p in out.iterdir())))
