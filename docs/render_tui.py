#!/usr/bin/env python3
"""Render capture_demo JSON frames to SVG + PNG (no Pillow; uses rsvg-convert).

Refresh tutorial shots from the repo root after a UI change:

    cargo run --offline --example capture_demo -- docs/images
    python3 docs/render_tui.py docs/images

See docs/README.md for the scene list. Commit PNGs only.
"""

from __future__ import annotations

import json
import subprocess
import sys
import xml.sax.saxutils as xml
from pathlib import Path

CELL_W = 11
CELL_H = 20
PAD = 16
FONT = "JetBrainsMono Nerd Font, Liberation Mono, monospace"
SIZE = 13


def hex_ok(value: str) -> str:
    v = value.strip()
    if v.startswith("#") and len(v) == 7:
        return v
    return "#2A2D31"


def render_svg(data: dict) -> str:
    w, h = int(data["width"]), int(data["height"])
    width = w * CELL_W + PAD * 2
    height = h * CELL_H + PAD * 2
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="#0F1114"/>',
        f'<g font-family="{FONT}" font-size="{SIZE}" xml:space="preserve">',
    ]
    for cell in data["cells"]:
        x = PAD + int(cell["x"]) * CELL_W
        y = PAD + int(cell["y"]) * CELL_H
        bg = hex_ok(cell["bg"])
        fg = hex_ok(cell["fg"])
        parts.append(
            f'<rect x="{x}" y="{y}" width="{CELL_W}" height="{CELL_H}" fill="{bg}"/>'
        )
        ch = cell.get("ch") or " "
        if ch != " ":
            weight = ' font-weight="700"' if cell.get("bold") else ""
            text_y = y + CELL_H - 4
            parts.append(
                f'<text x="{x}" y="{text_y}" fill="{fg}"{weight}>{xml.escape(ch)}</text>'
            )
    parts.append("</g></svg>")
    return "\n".join(parts)


def main() -> None:
    folder = Path(sys.argv[1] if len(sys.argv) > 1 else "docs/images")
    for path in sorted(folder.glob("demo-*.json")):
        data = json.loads(path.read_text())
        svg_path = path.with_suffix(".svg")
        png_path = path.with_suffix(".png")
        svg_path.write_text(render_svg(data), encoding="utf-8")
        subprocess.run(
            [
                "rsvg-convert",
                "-o",
                str(png_path),
                str(svg_path),
            ],
            check=True,
        )
        print(f"wrote {png_path}")


if __name__ == "__main__":
    main()
