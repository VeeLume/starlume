"""Overlay the registered separator path (cyan) and the charge top (pink)
on 2x crops of frames, side by side.
usage: overlay.py <out.png> <sep.npz>:<frame.png>..."""
import sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
import bar

X0, Y0, X1, Y1 = 1570, 520, 1760, 1110
S = 2
font = ImageFont.truetype("arial.ttf", 16)
items = [a.split(":", 1) for a in sys.argv[2:]]
w, h = (X1 - X0) * S, (Y1 - Y0) * S
can = Image.new("RGB", (len(items) * (w + 10), h + 30), (0, 0, 0))
for k, (npz, name) in enumerate(items):
    arc = bar.Arc(npz)
    full = Image.open(name).convert("RGB")
    f = np.asarray(full).astype(int)
    s, y, x, dx, dy = arc.charge(f)
    im = full.crop((X0, Y0, X1, Y1)).resize((w, h), Image.NEAREST)
    d = ImageDraw.Draw(im)
    for xx, yy in zip(arc.xs + dx, arc.ys + dy):
        d.point(((xx - X0) * S, (yy - Y0) * S), fill=(0, 255, 255))
    if y is not None:
        cx, cy = (x + 8 - X0) * S, (y - Y0) * S
        d.ellipse((cx - 12, cy - 12, cx + 12, cy + 12), outline=(255, 60, 220), width=3)
    ox = k * (w + 10)
    can.paste(im, (ox, 30))
    ImageDraw.Draw(can).text((ox, 5), f"{name.split('/')[-1]} {s * 100:.0f} % (dx {dx} dy {dy})", font=font, fill=(255, 255, 255))
can.save(sys.argv[1])
