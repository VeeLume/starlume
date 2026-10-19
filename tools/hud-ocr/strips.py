"""Side-by-side 4x crops of one region from several frames, with y ticks.
usage: strips.py <x0> <y0> <x1> <y1> <out.png> <frame.png>..."""
import sys
from PIL import Image, ImageDraw, ImageFont

x0, y0, x1, y1 = map(int, sys.argv[1:5])
out, frames = sys.argv[5], sys.argv[6:]
S, pad = 4, 40
w, h = (x1 - x0) * S, (y1 - y0) * S
can = Image.new("RGB", (pad + len(frames) * (w + 10), h + 30), (0, 0, 0))
d = ImageDraw.Draw(can)
font = ImageFont.truetype("arial.ttf", 14)
for y in range(y0, y1, 10):
    d.text((2, 30 + (y - y0) * S - 7), str(y), font=font, fill=(255, 0, 255))
for k, f in enumerate(frames):
    im = Image.open(f).convert("RGB").crop((x0, y0, x1, y1)).resize((w, h), Image.NEAREST)
    ox = pad + k * (w + 10)
    can.paste(im, (ox, 30))
    d.text((ox, 5), f.split("/")[-1], font=font, fill=(255, 255, 255))
    for y in range(y0, y1, 10):
        d.line((ox, 30 + (y - y0) * S, ox + 6, 30 + (y - y0) * S), fill=(255, 0, 255))
can.save(out)
