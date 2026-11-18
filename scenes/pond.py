"""Pond scenes for git-koi.

Draws a looping pixel-art pond (spring blossom, summer lily pads, autumn
leaves, winter ice, rain, or a lamplit night) and swims the koi that git-koi
drew through it. Every moving thing is periodic in the koi's own 120-frame
loop, so the GIF has no seam.

    python3 pond.py plan [--date YYYY-MM-DD] [--scene NAME]
        prints the scene and body for that date (default: today, UTC)
    python3 pond.py render --koi koi.gif --scene autumn --out koi.gif
"""
import argparse
import datetime
import hashlib
import math
import random

from PIL import Image, ImageDraw

N = 120  # frames, same as git-koi
L = 128  # logical canvas
OUT = 256  # rendered size (README shows the koi at 256)
DUR = 70  # ms per frame, same as git-koi
TAU = 2 * math.pi

BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]


def hexc(h, a=255):
    h = h.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), a)


def water(t, tones, calm=1.0):
    """Dithered caustic water in 3-4 tones, periodic in t."""
    ph = TAU * t / N
    im = Image.new("RGB", (L, L))
    px = im.load()
    n = len(tones)
    for y in range(L):
        for x in range(L):
            v = (
                math.sin(0.11 * x + 0.07 * y + ph)
                + math.sin(-0.06 * x + 0.13 * y + 2 * ph)
                + 0.6 * math.sin(0.19 * x - 0.05 * y - ph)
            ) * calm
            v = (v + 2.6) / 5.2  # 0..1
            v += (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.35
            i = max(0, min(n - 1, int(v * n)))
            px[x, y] = tones[i][:3]
    return im


def ring(d, cx, cy, r, col):
    if r < 0.6:
        d.point((round(cx), round(cy)), fill=col)
        return
    d.ellipse((cx - r, cy - r * 0.75, cx + r, cy + r * 0.75), outline=col)


def ripples(d, t, seeds, period, max_r, col):
    for (sx, sy, off) in seeds:
        k = ((t + off) % period) / period
        a = int(col[3] * (1 - k))
        if a > 8:
            ring(d, sx, sy, 0.5 + k * max_r, col[:3] + (a,))


def sprite(d, x, y, rows, pal, alpha=255):
    for j, row in enumerate(rows):
        for i, ch in enumerate(row):
            if ch != ".":
                c = pal[ch]
                d.point((x + i, y + j), fill=c[:3] + (min(c[3], alpha),))


PETAL = [
    "..pp.",
    ".pppP",
    "ppPPp",
    ".pPp.",
]
MAPLE = [
    ".....a.....",
    "....aaa....",
    "..a.aaa.a..",
    "..aaaaaaa..",
    "a.aaaaaaa.a",
    "aaaaabaaaaa",
    ".aaaabaaaa.",
    "..aaabaaa..",
    "...aabaa...",
    ".....b.....",
    ".....b.....",
]


def rot(rows, k):
    for _ in range(k % 4):
        rows = ["".join(r[i] for r in rows[::-1]) for i in range(len(rows[0]))]
    return rows


LEAF = [rot(MAPLE, k) for k in range(4)]
LOTUS = [".w.w.", "wpwpw", ".pyp.", "wpwpw", ".w.w."]
LAMP = [
    "..kkk..",
    ".kLLLk.",
    ".kLLLk.",
    ".kLLLk.",
    "..kkk..",
    "...k...",
    "...k...",
    "...k...",
    "...k...",
    "...k...",
    "...k...",
    "..kkk..",
    ".kkkkk.",
]


def drift(i, t, rnd, vx, vy, sway=2.0):
    """Wrapping drift whose total travel over N frames is a whole number of laps."""
    x0, y0 = rnd.uniform(0, L), rnd.uniform(0, L)
    x = (x0 + vx * L * t / N + sway * math.sin(TAU * t / N * 2 + i)) % (L + 8) - 4
    y = (y0 + vy * L * t / N) % (L + 8) - 4
    return x, y


def fallers(d, t, n, seed, sprites, pals, ripple_col, fall=16, vx=0.10, vy=0.04):
    """Things that fall onto the water, ring it where they land, then float and fade.

    Each one runs a lifecycle of exactly N frames, so the loop has no seam."""
    rnd = random.Random(seed)
    for i in range(n):
        lx, ly = rnd.uniform(8, L - 16), rnd.uniform(8, L - 16)
        off = rnd.randrange(N)
        pal = pals[i % len(pals)]
        u = (t + off) % N
        shade = {k: (0, 0, 0, 60) for k in pal}
        if u < fall:  # still in the air: drawn up and to the right of its shadow
            h = (fall - u) / fall
            spr = sprites[i % len(sprites)]
            sprite(d, int(lx), int(ly), spr, shade, alpha=int(60 * (1 - h)))
            sprite(d, int(lx + 9 * h), int(ly - 14 * h), spr, pal)
            continue
        k = u - fall
        if k < 26:  # the ring where it landed
            r = 1 + k * 0.45
            ring(d, lx + 2, ly + 2, r, ripple_col[:3] + (int(ripple_col[3] * (1 - k / 26)),))
        x = lx + vx * k + 0.8 * math.sin(k * 0.08 + i)
        y = ly + vy * k
        fade = 255 if u < N - 18 else int(255 * (N - u) / 18)
        spr = sprites[(i + k // 40) % len(sprites)]
        sprite(d, int(x) + 1, int(y) + 1, spr, shade, alpha=min(60, fade))
        sprite(d, int(x), int(y), spr, pal, alpha=fade)


# ---------------------------------------------------------------- scenes


def spring(t):
    im = water(t, [hexc("#7cc6c0"), hexc("#94d4cc"), hexc("#b4e4dc"), hexc("#d2f1ea")])
    d = ImageDraw.Draw(im, "RGBA")
    # branch over the top-left corner, with its shadow on the water
    for dx, dy, col in [(3, 4, (40, 70, 70, 70)), (0, 0, None)]:
        c = col or hexc("#5a3b2e")
        d.line([(0 + dx, 10 + dy), (14 + dx, 16 + dy), (30 + dx, 14 + dy), (44 + dx, 20 + dy)], fill=c, width=2)
        d.line([(14 + dx, 16 + dy), (22 + dx, 26 + dy)], fill=c, width=1)
    rb = random.Random(7)
    for _ in range(26):
        bx, by = rb.choice([(rb.uniform(2, 44), rb.uniform(8, 22)), (rb.uniform(14, 24), rb.uniform(16, 28))])
        d.rectangle((bx, by, bx + 1, by + 1), fill=rb.choice([hexc("#f5a3bd"), hexc("#fbd0dd"), hexc("#ffffff")]))
    pals = [{"p": hexc("#f7b2c6"), "P": hexc("#e88aa6")}, {"p": hexc("#fbd3df"), "P": hexc("#f2a2ba")}]
    fallers(d, t, 14, 1, [PETAL, rot(PETAL, 1), rot(PETAL, 2), rot(PETAL, 3)], pals, (255, 255, 255, 235))
    return im, None


def lily(d, cx, cy, r, ang, fill, rim):
    pts = []
    for k in range(40):
        a = ang + 0.35 + (TAU - 0.7) * k / 39
        pts.append((cx + r * math.cos(a), cy + r * 0.8 * math.sin(a)))
    pts.append((cx, cy))
    d.polygon(pts, fill=fill, outline=rim)
    if rim:
        for v in range(5):
            a = ang + 0.9 + v * 1.1
            d.line((cx, cy, cx + (r - 2) * math.cos(a), cy + (r - 2) * 0.8 * math.sin(a)), fill=hexc("#5fae52"))


def summer(t):
    im = water(t, [hexc("#2f7c69"), hexc("#3e9479"), hexc("#55ab8b"), hexc("#7cc6a2")])
    d = ImageDraw.Draw(im, "RGBA")
    pal = {"g": hexc("#2e7d32"), "G": hexc("#5fb35f"), "w": hexc("#fff3f6"), "p": hexc("#f48fb1"), "y": hexc("#ffd54f")}
    for (x, y, r, ang) in [(14, 18, 11, 0.6), (104, 26, 9, 2.4), (100, 100, 13, 4.0), (20, 104, 10, 5.2), (62, 114, 8, 1.4)]:
        bob = math.sin(TAU * t / N + x) * 0.8
        lily(d, x + 2, y + 3 + bob, r, ang, (0, 40, 30, 70), None)
        lily(d, x, y + bob, r, ang, hexc("#3f8f3a"), hexc("#2a6b28"))
    sprite(d, 98, 96 + int(round(math.sin(TAU * t / N + 100) * 0.8)), LOTUS, pal)
    rnd = random.Random(3)
    for i in range(18):  # sun glints
        x, y, off = rnd.randrange(L), rnd.randrange(L), rnd.randrange(N)
        if (t + off) % 30 < 4:
            d.point((x, y), fill=(255, 255, 230, 230))
            if (t + off) % 30 == 1:
                d.point((x + 1, y), fill=(255, 255, 230, 140))
                d.point((x - 1, y), fill=(255, 255, 230, 140))
    return im, None


def autumn(t):
    im = water(t, [hexc("#24464f"), hexc("#2f5a63"), hexc("#3d6e76"), hexc("#4f838a")])
    d = ImageDraw.Draw(im, "RGBA")
    cols = ["#d9542b", "#e8862a", "#f2c14e", "#a83a2a", "#c96a2c"]
    pals = []
    for c in cols:
        a = hexc(c)
        pals.append({"a": a, "b": tuple(int(v * 0.65) for v in a[:3]) + (255,)})
    fallers(d, t, 9, 5, LEAF, pals, (225, 245, 245, 220), fall=20)
    return im, None


def winter(t):
    im = water(t, [hexc("#5f7c94"), hexc("#7592a8"), hexc("#90abbe"), hexc("#b3c8d6")], calm=0.6)
    d = ImageDraw.Draw(im, "RGBA")
    # ice creeping in from the edges
    rnd = random.Random(11)
    px = im.load()
    for y in range(L):
        for x in range(L):
            e = min(x, y, L - 1 - x, L - 1 - y)
            along = x if e in (y, L - 1 - y) else y
            depth = 5 + 4 * math.sin(along * 0.11 + (x < y)) + 2 * math.sin(along * 0.37)
            if e < depth - 1.2:
                px[x, y] = (232, 241, 248) if (x * 7 + y * 3) % 11 else (214, 228, 238)
            elif e < depth:
                px[x, y] = (255, 255, 255)
    for i in range(30):
        x, y = drift(i, t, rnd, 0, 1, sway=3)
        a = 200 if i % 3 else 255
        d.point((int(x), int(y)), fill=(255, 255, 255, a))
        if i % 4 == 0:
            d.point((int(x) + 1, int(y)), fill=(255, 255, 255, 140))
    return im, None


def big(d, x, y, rows, pal):
    for j, row in enumerate(rows):
        for i, ch in enumerate(row):
            if ch != ".":
                d.rectangle((x + 2 * i, y + 2 * j, x + 2 * i + 1, y + 2 * j + 1), fill=pal[ch])


def night(t):
    im = water(t, [hexc("#0b1424"), hexc("#111d33"), hexc("#182843"), hexc("#203453")])
    glow = Image.new("RGBA", (L, L), (0, 0, 0, 0))
    g = ImageDraw.Draw(glow)
    lamps = [(18, 2), (102, 2)]
    for n_, (lx, ly) in enumerate(lamps):
        flick = 0.85 + 0.15 * math.sin(TAU * t / N * 6 + n_ * 1.7)
        cx, cy = lx + 3, ly + 30
        for r in range(44, 0, -2):
            a = int(52 * flick * (1 - r / 44) ** 0.8)
            g.ellipse((cx - r * 1.2, cy - r, cx + r * 1.2, cy + r), fill=(255, 190, 100, a))
    im = Image.alpha_composite(im.convert("RGBA"), glow).convert("RGB")
    d = ImageDraw.Draw(im, "RGBA")
    ph = TAU * t / N
    for n_, (lx, ly) in enumerate(lamps):  # broken reflection streaks under each lamp
        for k in range(10):
            y = ly + 18 + k * 4
            w = 2 + int(2 * abs(math.sin(ph * 2 + k + n_)))
            x = lx + 3 + int(2 * math.sin(ph * 3 + k * 0.9 + n_))
            d.line((x - w, y, x + w, y), fill=(255, 214, 140, 235 - k * 16))
    # moon reflection
    mx, my = 64 + int(1.5 * math.sin(ph * 2)), 50
    for k in range(10):
        w = 9 - 1.6 * abs(k - 4.5)
        sx = int(1.5 * math.sin(ph * 3 + k))
        d.line((mx - w + sx, my + k * 2, mx + w + sx, my + k * 2), fill=(225, 232, 255, 130))
    # stone edge along the top, then the lamps on it
    for x in range(0, L, 6):
        d.rectangle((x, 0, x + 5, 5), fill=hexc("#2a2f3a"))
        d.line((x, 5, x + 5, 5), fill=hexc("#1a1e27"))
    for n_, (lx, ly) in enumerate(lamps):
        flick = int(40 * math.sin(TAU * t / N * 6 + n_ * 1.7))
        lit = (255, min(255, 214 + flick // 2), 120, 255)
        big(d, lx - 3, ly, LAMP, {"k": hexc("#0a0c12"), "L": lit})
    rnd = random.Random(9)
    for i in range(10):  # fireflies
        bx, by, off = rnd.uniform(8, 120), rnd.uniform(20, 120), rnd.randrange(N)
        x = bx + 4 * math.sin(ph * 2 + i)
        y = by + 3 * math.cos(ph * 3 + i)
        b = (math.sin(TAU * (t + off) / 40) + 1) / 2
        if b > 0.45:
            d.point((int(x), int(y)), fill=(214, 255, 120, int(255 * b)))
    return im, "night"


def rain(t):
    im = water(t, [hexc("#3d5560"), hexc("#4a6570"), hexc("#5b7782"), hexc("#728d97")], calm=0.7)
    d = ImageDraw.Draw(im, "RGBA")
    rnd = random.Random(13)
    seeds = [(rnd.uniform(4, 124), rnd.uniform(4, 124), rnd.randrange(30)) for _ in range(22)]
    ripples(d, t, seeds, 30, 10, (235, 248, 252, 235))
    for (sx, sy, off) in seeds:  # the drop landing
        k = (t + off) % 30
        if k in (28, 29):
            d.point((int(sx), int(sy) - (29 - k) * 3), fill=(240, 250, 255, 220))
    return im, "rain"


SCENES = {"spring": spring, "summer": summer, "autumn": autumn, "winter": winter, "night": night, "rain": rain}


# ---------------------------------------------------------------- compositing


def koi_frames(path, size):
    k = Image.open(path)
    out = []
    for i in range(k.n_frames):
        k.seek(i)
        out.append(k.convert("RGBA").resize((size, size), Image.NEAREST))
    return out


def shadow_of(fish, dx, dy, a=70):
    alpha = fish.getchannel("A").point(lambda v: a if v else 0)
    sh = Image.new("RGBA", fish.size, (0, 20, 30, 0))
    sh.putalpha(alpha)
    canvas = Image.new("RGBA", fish.size, (0, 0, 0, 0))
    canvas.alpha_composite(sh, (dx, dy))
    return canvas


def mood(fish, kind):
    if kind == "night":  # moonlit: darker and cooler, so the lamps carry the light
        r, g, b, a = fish.split()
        r = r.point(lambda v: int(v * 0.55))
        g = g.point(lambda v: int(v * 0.6))
        b = b.point(lambda v: int(v * 0.75))
        return Image.merge("RGBA", (r, g, b, a))
    if kind == "rain":
        r, g, b, a = fish.split()
        return Image.merge("RGBA", (r.point(lambda v: int(v * 0.85)), g.point(lambda v: int(v * 0.9)), b, a))
    return fish


def render(scene, fishes, out_path):
    """fishes: list of (frames, scale, (ox, oy), phase_offset, mirror)."""
    frames = []
    for t in range(N):
        bg, kind = SCENES[scene](t)
        bg = bg.resize((OUT, OUT), Image.NEAREST).convert("RGBA")
        for (fr, scale, (ox, oy), off, mirror) in fishes:
            f = fr[(t + off) % len(fr)]
            if scale != 1:
                sz = int(OUT * scale)
                f = f.resize((sz, sz), Image.NEAREST)
            if mirror:
                f = f.transpose(Image.FLIP_LEFT_RIGHT)
            f = mood(f, kind)
            bg.alpha_composite(shadow_of(f, 4, 6), (ox, oy))
            bg.alpha_composite(f, (ox, oy))
        frames.append(bg.convert("RGB"))
    # one shared palette so colours do not shimmer between frames
    strip = Image.new("RGB", (OUT * 4, OUT))
    for i, fi in enumerate(frames[:: N // 4][:4]):
        strip.paste(fi, (i * OUT, 0))
    pal = strip.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
    q = [f.quantize(palette=pal, dither=Image.Dither.NONE) for f in frames]
    q[0].save(out_path, save_all=True, append_images=q[1:], duration=DUR, loop=0, optimize=True, disposal=1)


# ---------------------------------------------------------------- choosing

SEASONS = {12: "winter", 1: "winter", 2: "winter", 3: "spring", 4: "spring", 5: "spring",
           6: "summer", 7: "summer", 8: "summer", 9: "autumn", 10: "autumn", 11: "autumn"}
# the koi body that suits each scene
BODIES = {"spring": "asagi", "summer": "gold", "autumn": "tea", "winter": "black", "rain": "gold", "night": "platinum"}


def plan(day):
    """Scene for a date: its season, or rain on about one day in seven outside summer."""
    scene = SEASONS[day.month]
    roll = hashlib.sha256(day.isoformat().encode()).digest()[0]
    if scene != "summer" and roll % 7 == 0:
        scene = "rain"
    return scene, BODIES[scene]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan")
    p.add_argument("--date", type=datetime.date.fromisoformat, default=datetime.datetime.now(datetime.timezone.utc).date())
    p.add_argument("--scene", choices=sorted(SCENES), help="use this scene instead of the date's")
    r = sub.add_parser("render")
    r.add_argument("--koi", required=True)
    r.add_argument("--scene", required=True, choices=sorted(SCENES))
    r.add_argument("--out", required=True)
    a = ap.parse_args()
    if a.cmd == "plan":
        scene, body = (a.scene, BODIES[a.scene]) if a.scene else plan(a.date)
        print(f"scene={scene}")
        print(f"body={body}")
    else:
        render(a.scene, [(koi_frames(a.koi, OUT), 1, (0, 0), 0, False)], a.out)


if __name__ == "__main__":
    main()
