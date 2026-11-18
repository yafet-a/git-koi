"""Builds docs/explorer.html, the interactive pond explorer.

The explorer is scenes/pond.html with koi frames embedded. It draws the pond
in the browser by the same rules as src/pond.rs, which draws it for the Action.

    python3 scenes/build_explorer.py KOI_DIR docs/explorer.html

KOI_DIR holds one koi.gif per body, as DIR/<body>.gif, from `git-koi render`.
Needs Pillow.
"""
import base64
import io
import json
import sys
from pathlib import Path

from PIL import Image

BODIES = ["white", "black", "asagi", "tea", "gold", "platinum"]


def sheet(gif_path):
    """A git-koi GIF as one PNG sprite sheet of its frames, 256 px each, as a data URL."""
    gif = Image.open(gif_path)
    out = Image.new("RGBA", (256 * 12, 256 * 10), (0, 0, 0, 0))
    for index in range(min(gif.n_frames, 120)):
        gif.seek(index)
        out.paste(gif.convert("RGBA").resize((256, 256), Image.NEAREST), ((index % 12) * 256, (index // 12) * 256))
    buffer = io.BytesIO()
    out.quantize(colors=255, method=Image.Quantize.FASTOCTREE).save(buffer, "PNG", optimize=True)
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


def main():
    koi_dir, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    fish = {body: sheet(koi_dir / f"{body}.gif") for body in BODIES if (koi_dir / f"{body}.gif").exists()}
    template = (Path(__file__).resolve().parent / "pond.html").read_text()
    out_path.write_text(template.replace("__FISH__", json.dumps(fish)))


if __name__ == "__main__":
    main()
