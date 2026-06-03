#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""Generate octiron art assets: a monospace bitmap-font atlas for the engine,
and per-game sprite PNGs. Run with uv (auto-installs Pillow):

    uv run tools/gen_assets.py

Sprites are drawn at 4x and downsampled with LANCZOS for clean edges.
"""
import os
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SS = 4  # supersampling factor

# ----------------------------------------------------------------------------
# helpers
# ----------------------------------------------------------------------------

def canvas(w, h):
    img = Image.new("RGBA", (w * SS, h * SS), (0, 0, 0, 0))
    return img, ImageDraw.Draw(img)

def finish(img, path):
    w, h = img.size
    out = img.resize((w // SS, h // SS), Image.LANCZOS)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    out.save(path)
    print(f"  {os.path.relpath(path, ROOT):42} {out.size[0]}x{out.size[1]}")

def s(*v):
    return tuple(x * SS for x in v) if len(v) > 1 else v[0] * SS

# ----------------------------------------------------------------------------
# font atlas (engine built-in)
# ----------------------------------------------------------------------------

def font_atlas():
    font_px = 26
    font = ImageFont.truetype(
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", font_px
    )
    ascent, descent = font.getmetrics()
    advance = round(font.getlength("M"))
    cw = advance + 2
    ch = ascent + descent + 2
    cols = 16
    first, last = 32, 127  # 95 printable glyphs, 96 cells
    rows = (last - first + cols) // cols
    atlas = Image.new("RGBA", (cw * cols, ch * rows), (0, 0, 0, 0))
    draw = ImageDraw.Draw(atlas)
    for i, code in enumerate(range(first, last)):
        col, row = i % cols, i // cols
        x = col * cw + (cw - advance) // 2
        y = row * ch + 1
        draw.text((x, y), chr(code), font=font, fill=(255, 255, 255, 255))
    path = os.path.join(ROOT, "engine", "assets", "font.png")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    atlas.save(path)
    print(f"  {os.path.relpath(path, ROOT):42} {atlas.size[0]}x{atlas.size[1]}  "
          f"cell={cw}x{ch} cols={cols} first={first}")
    return cw, ch

# ----------------------------------------------------------------------------
# sprites
# ----------------------------------------------------------------------------

def outline(draw, xy, fill, ol=(10, 12, 20, 255), w=1):
    draw.line(xy + [xy[0]], fill=ol, width=w * SS)

def ship():
    img, d = canvas(40, 30)
    body = [(20, 1), (4, 26), (20, 20), (36, 26)]
    d.polygon([s(*p) for p in body], fill=(90, 200, 255, 255))
    d.polygon([s(20, 4), s(11, 22), s(29, 22)], fill=(190, 240, 255, 255))
    d.ellipse([s(16, 9), s(24, 17)], fill=(30, 60, 110, 255))
    d.ellipse([s(17, 10), s(23, 16)], fill=(120, 220, 255, 255))
    # engine glow
    d.polygon([s(13, 26), s(20, 22), s(27, 26), s(20, 30)], fill=(255, 170, 60, 230))
    finish(img, os.path.join(ROOT, "examples/invaders/assets/ship.png"))

def alien(color, eye, path):
    img, d = canvas(34, 26)
    # blocky invader body
    d.rounded_rectangle([s(5, 6), s(29, 22)], radius=s(5), fill=color)
    d.rectangle([s(2, 12), s(7, 20)], fill=color)
    d.rectangle([s(27, 12), s(32, 20)], fill=color)
    d.rectangle([s(8, 22), s(13, 26)], fill=color)
    d.rectangle([s(21, 22), s(26, 26)], fill=color)
    # eyes
    for ex in (11, 21):
        d.ellipse([s(ex - 3, 11), s(ex + 3, 17)], fill=(255, 255, 255, 255))
        d.ellipse([s(ex - 2, 12), s(ex + 1, 16)], fill=eye)
    finish(img, os.path.join(ROOT, path))

def bullet():
    img, d = canvas(8, 18)
    d.rounded_rectangle([s(2, 1), s(6, 17)], radius=s(2), fill=(255, 240, 150, 255))
    d.rounded_rectangle([s(3, 3), s(5, 13)], radius=s(1), fill=(255, 255, 255, 255))
    finish(img, os.path.join(ROOT, "examples/invaders/assets/bullet.png"))

def bird():
    img, d = canvas(34, 28)
    d.ellipse([s(3, 4), s(31, 26)], fill=(255, 210, 70, 255))
    outline(d, [s(3, 15), s(17, 4), s(31, 15)], None)  # subtle top
    d.ellipse([s(5, 12), s(19, 24)], fill=(255, 230, 130, 255))  # belly
    # wing
    d.pieslice([s(9, 9), s(27, 25)], 20, 150, fill=(240, 180, 50, 255))
    # eye
    d.ellipse([s(20, 8), s(29, 17)], fill=(255, 255, 255, 255))
    d.ellipse([s(23, 10), s(28, 15)], fill=(30, 30, 40, 255))
    # beak
    d.polygon([s(29, 13), s(34, 15), s(29, 18)], fill=(255, 140, 40, 255))
    finish(img, os.path.join(ROOT, "examples/flappy/assets/bird.png"))

def pipe():
    img, d = canvas(48, 64)
    for x in range(48):
        t = abs(x - 24) / 24.0
        g = int(150 - 70 * t)
        d.line([s(x, 0), s(x, 64)], fill=(40, g + 40, 50, 255), width=SS)
    d.rectangle([s(0, 0), s(3, 64)], fill=(20, 90, 40, 255))
    d.rectangle([s(44, 0), s(47, 64)], fill=(20, 90, 40, 255))
    finish(img, os.path.join(ROOT, "examples/flappy/assets/pipe.png"))

def apple():
    img, d = canvas(26, 26)
    d.ellipse([s(3, 6), s(15, 24)], fill=(220, 50, 50, 255))
    d.ellipse([s(11, 6), s(23, 24)], fill=(230, 60, 55, 255))
    d.ellipse([s(6, 9), s(12, 16)], fill=(255, 170, 160, 200))  # highlight
    d.rectangle([s(12, 2), s(14, 8)], fill=(110, 70, 30, 255))  # stem
    d.ellipse([s(14, 2), s(22, 8)], fill=(80, 190, 80, 255))    # leaf
    finish(img, os.path.join(ROOT, "examples/snake/assets/apple.png"))

def brick():
    # light/neutral so per-row tint multiplies into a shaded colored brick
    img, d = canvas(52, 22)
    d.rounded_rectangle([s(1, 1), s(51, 21)], radius=s(3), fill=(225, 225, 230, 255))
    d.rounded_rectangle([s(1, 1), s(51, 7)], radius=s(3), fill=(255, 255, 255, 255))
    d.rectangle([s(3, 16), s(49, 20)], fill=(170, 170, 180, 255))
    finish(img, os.path.join(ROOT, "examples/breakout/assets/brick.png"))

def star():
    img, d = canvas(6, 6)
    d.ellipse([s(2, 2), s(4, 4)], fill=(255, 255, 255, 255))
    finish(img, os.path.join(ROOT, "examples/invaders/assets/star.png"))

# ----------------------------------------------------------------------------

if __name__ == "__main__":
    print("font:")
    font_atlas()
    print("sprites:")
    ship()
    alien((110, 220, 120, 255), (40, 120, 60, 255), "examples/invaders/assets/alien.png")
    alien((230, 120, 230, 255), (120, 40, 120, 255), "examples/invaders/assets/alien2.png")
    bullet()
    star()
    bird()
    pipe()
    apple()
    brick()
    print("done.")
