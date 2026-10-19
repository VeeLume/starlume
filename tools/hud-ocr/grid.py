"""8x zoom of a frame region with a coordinate grid every 5 px.
usage: grid.py <frame.png> <x0> <y0> <x1> <y1> <out.png>"""
import sys
from PIL import Image, ImageDraw, ImageFont

src, x0, y0, x1, y1, out = sys.argv[1], *map(int, sys.argv[2:6]), sys.argv[6]
S = 8
im = Image.open(src).convert("RGB").crop((x0, y0, x1, y1)).resize(((x1 - x0) * S, (y1 - y0) * S), Image.NEAREST)
pad = 50
can = Image.new("RGB", (im.width + pad, im.height + pad), (0, 0, 0))
can.paste(im, (pad, pad))
d = ImageDraw.Draw(can)
font = ImageFont.truetype("arial.ttf", 14)
for x in range(x0, x1):
    if x % 5 == 0:
        X = pad + (x - x0) * S
        d.line((X, pad, X, can.height), fill=(0, 255, 255) if x % 10 == 0 else (0, 110, 110))
        if x % 10 == 0:
            d.text((X - 14, 5), str(x), font=font, fill=(0, 255, 255))
for y in range(y0, y1):
    if y % 5 == 0:
        Y = pad + (y - y0) * S
        d.line((pad, Y, can.width, Y), fill=(255, 0, 255) if y % 10 == 0 else (110, 0, 110))
        if y % 10 == 0:
            d.text((2, Y - 8), str(y), font=font, fill=(255, 0, 255))
can.save(out)
