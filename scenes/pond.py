"""Pond scenes for git-koi.

The pond is drawn by scenes/pond.html, the same page as the interactive
explorer in docs/explorer.html, so what you see there is what the Action
draws. This script picks the scene for a date, draws the koi it needs with
git-koi, has headless Chrome render the page's 120 frames, and writes a GIF
whose outside is transparent, so the pond sits on a light or dark page.

    python3 pond.py plan [--date YYYY-MM-DD] [--scene NAME]
        prints the scene and koi body for that date (default: today, UTC)
    python3 pond.py draw --snapshot koi-snapshot.json --out koi.gif [options]
        draws the pond GIF (see --help for the options)
    python3 pond.py explorer --koi-dir DIR --out explorer.html
        builds the explorer page from DIR/<body>.gif for each body
"""
import argparse
import base64
import datetime
import html
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
TEMPLATE = HERE / "pond.html"
N = 120  # frames in a git-koi loop
DURATION = 70  # ms per frame, as git-koi
BODIES = ["white", "black", "asagi", "tea", "gold", "platinum"]

SEASONS = {12: "winter", 1: "winter", 2: "winter", 3: "spring", 4: "spring", 5: "spring",
           6: "summer", 7: "summer", 8: "summer", 9: "autumn", 10: "autumn", 11: "autumn"}
# the koi body that suits each season, used with body: auto
MATCH = {"spring": "asagi", "summer": "gold", "autumn": "tea", "winter": "black", "night": "platinum"}
CHOICES = {
    "theme": ["light", "dark"],
    "size": ["wide", "compact"],
    "garden": ["garden", "islands", "bank", "gravel", "deck", "deckdark", "deckold", "deckgrey", "deckwalnut", "mist"],
    "lights": ["reflect", "onefloat", "floating", "euro", "toro", "chinese", "string", "none"],
    "moon": ["lines", "round", "halo", "crescent", "glade", "none"],
    "drops": ["random", "fixed"],
    "weather": ["auto", "clear", "rain"],
    "depth": ["under", "top"],
    "koi": ["one", "three"],
}
SCENES = ["spring", "summer", "autumn", "winter"]


def plan(day):
    """The season for a date."""
    return SEASONS[day.month]


def weather(day, season):
    """Rain on about one day in seven, in any season but winter, which snows instead.

    The rule is plain arithmetic so the explorer page can apply the same one."""
    if season == "winter":
        return "clear"
    return "rain" if day_hash(day.year * 10000 + day.month * 100 + day.day) % 7 == 3 else "clear"


def day_hash(n):
    """Murmur3's 32-bit finaliser: scrambles the date so rain days fall in no pattern."""
    m = 0xFFFFFFFF
    h = n & m
    h ^= h >> 16
    h = (h * 0x85EBCA6B) & m
    h ^= h >> 13
    h = (h * 0xC2B2AE35) & m
    h ^= h >> 16
    return h


def trio(scene, theme):
    """The three bodies the page swims when koi is three (it picks them the same way)."""
    if theme == "dark":
        return ["platinum", "gold", "asagi"]
    return {"autumn": ["tea", "gold", "black"], "winter": ["black", "gold", "platinum"]}.get(scene, ["asagi", "platinum", "gold"])


def sheet(gif_path):
    """A git-koi GIF as one PNG sprite sheet of its 120 frames, 256 px each, as a data URL."""
    g = Image.open(gif_path)
    out = Image.new("RGBA", (256 * 12, 256 * 10), (0, 0, 0, 0))
    for i in range(min(g.n_frames, N)):
        g.seek(i)
        out.paste(g.convert("RGBA").resize((256, 256), Image.NEAREST), ((i % 12) * 256, (i // 12) * 256))
    buf = io.BytesIO()
    out.quantize(colors=255, method=Image.Quantize.FASTOCTREE).save(buf, "PNG", optimize=True)
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def page(fish, export=None):
    s = TEMPLATE.read_text()
    s = s.replace("__FISH__", json.dumps(fish))
    if export is not None:
        s = s.replace("<script>\nconst FISH", f"<script>window.KOI_EXPORT = {json.dumps(export)};</script>\n<script>\nconst FISH", 1)
    return s


def find_chrome():
    for name in [os.environ.get("CHROME"), "google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "chrome"]:
        if name and shutil.which(name):
            return shutil.which(name)
    for p in ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
              r"C:\Program Files\Google\Chrome\Application\chrome.exe"]:
        if Path(p).exists():
            return p
    sys.exit("pond.py needs Chrome or Chromium to draw the pond; set CHROME to its path")


def render_frames(html_text):
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "pond.html"
        path.write_text(html_text)
        out = subprocess.run(
            [find_chrome(), "--headless=new", "--no-sandbox", "--disable-gpu", "--hide-scrollbars",
             "--virtual-time-budget=120000", "--dump-dom", path.as_uri()],
            capture_output=True, text=True, timeout=600, check=False)
    m = re.search(r'<pre id="koi-frames">(.*?)</pre>', out.stdout, re.S)
    if not m:
        sys.exit("Chrome did not return the pond's frames:\n" + out.stderr[-2000:])
    frames = []
    for line in html.unescape(m.group(1)).split("\n"):
        frames.append(Image.open(io.BytesIO(base64.b64decode(line.split(",", 1)[1]))).convert("RGBA"))
    return frames


def write_gif(frames, out_path):
    """GIF transparency is one colour: paint the outside a key colour and mark that index transparent."""
    key = (255, 0, 255)
    w, h = frames[0].size
    rgb = []
    for f in frames:
        r = Image.new("RGB", (w, h), key)
        r.paste(f.convert("RGB"), (0, 0), f.getchannel("A").point(lambda v: 255 if v > 127 else 0))
        rgb.append(r)
    strip = Image.new("RGB", (w * 4, h))
    for i, f in enumerate(rgb[:: N // 4][:4]):
        strip.paste(f, (i * w, 0))
    pal = strip.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
    p = pal.getpalette()
    key_i = min(range(len(p) // 3), key=lambda i: sum((p[3 * i + k] - key[k]) ** 2 for k in range(3)))
    q = [f.quantize(palette=pal, dither=Image.Dither.NONE) for f in rgb]
    q[0].save(out_path, save_all=True, append_images=q[1:], duration=DURATION, loop=0,
              transparency=key_i, disposal=2, optimize=False)


def draw(a):
    day = a.date
    scene = a.scene if a.scene != "auto" else plan(day)
    if a.koi == "three":
        bodies = trio(scene, a.theme)
    else:
        body = a.body if a.body != "auto" else MATCH["night" if a.theme == "dark" else scene]
        bodies = [body]
    fish = {}
    with tempfile.TemporaryDirectory() as tmp:
        for b in dict.fromkeys(bodies):
            subprocess.run([a.git_koi, "render", a.snapshot, "--output-dir", f"{tmp}/{b}", "--body", b], check=True)
            fish[b] = sheet(f"{tmp}/{b}/koi.gif")
    wx = a.weather if a.weather != "auto" else weather(day, scene)
    state = {"size": a.size, "edge": a.garden, "scene": scene, "weather": wx, "depth": a.depth, "theme": a.theme,
             "fish": "three" if a.koi == "three" else bodies[0], "lamp": a.lights, "moon": a.moon,
             "drops": a.drops, "tag": ""}
    seed = day.year * 10000 + day.month * 100 + day.day
    write_gif(render_frames(page(fish, {"state": state, "daySeed": seed})), a.out)
    print(f"drew {a.out}: {scene}, {wx}, {a.theme}, {state['fish']}, {a.garden}, {a.size}")


def main():
    today = datetime.datetime.now(datetime.timezone.utc).date()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan")
    p.add_argument("--date", type=datetime.date.fromisoformat, default=today)
    p.add_argument("--scene", choices=["auto"] + SCENES, default="auto")
    d = sub.add_parser("draw")
    d.add_argument("--snapshot", required=True, help="git-koi collect output")
    d.add_argument("--out", required=True)
    d.add_argument("--git-koi", default="git-koi")
    d.add_argument("--date", type=datetime.date.fromisoformat, default=today)
    d.add_argument("--scene", choices=["auto"] + SCENES, default="auto")
    d.add_argument("--body", choices=["auto"] + BODIES, default="auto")
    for k, v in CHOICES.items():
        d.add_argument(f"--{k}", choices=v, default=v[0])
    e = sub.add_parser("explorer")
    e.add_argument("--koi-dir", required=True)
    e.add_argument("--out", required=True)
    a = ap.parse_args()
    if a.cmd == "plan":
        scene = a.scene if a.scene != "auto" else plan(a.date)
        print(f"scene={scene}")
        print(f"weather={weather(a.date, scene)}")
        print(f"body={MATCH[scene]}")
    elif a.cmd == "draw":
        draw(a)
    else:
        fish = {b: sheet(Path(a.koi_dir) / f"{b}.gif") for b in BODIES if (Path(a.koi_dir) / f"{b}.gif").exists()}
        Path(a.out).write_text(page(fish))


if __name__ == "__main__":
    main()
