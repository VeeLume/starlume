"""Plot series2.csv: charge (yellow) and throttle (blue) over time, with the
crack window shaded. usage: plot2.py <out.png> <title>=<csv>=<win_lo>=<win_hi>..."""
import csv
import sys

from PIL import Image, ImageDraw, ImageFont

items = [a.split("=") for a in sys.argv[2:]]
PW, CH, M = 1100, 360, 55
font = ImageFont.truetype("arial.ttf", 15)
im = Image.new("RGB", (PW + 2 * M, len(items) * (CH + M) + M), (250, 250, 250))
d = ImageDraw.Draw(im)
for k, (title, path, lo, hi) in enumerate(items):
    rows = list(csv.DictReader(open(path)))
    t = [float(r["t"]) for r in rows]
    T = max(t)
    top = M + k * (CH + M)
    X = lambda v: M + v / T * PW
    Y = lambda pct: top + CH - pct / 100 * CH
    d.rectangle((M, Y(float(hi)), M + PW, Y(float(lo))), fill=(215, 240, 215))
    for pct in range(0, 101, 10):
        d.line((M, Y(pct), M + PW, Y(pct)), fill=(228, 228, 228))
        d.text((M - 28, Y(pct) - 8), str(pct), font=font, fill=(120, 120, 120))
    for s in range(0, int(T) + 1, 10):
        d.line((X(s), top, X(s), top + CH), fill=(235, 235, 235))
        d.text((X(s) - 6, top + CH + 4), str(s), font=font, fill=(120, 120, 120))
    d.line([(X(tt), Y(float(r["throttle"]))) for tt, r in zip(t, rows)], fill=(40, 110, 220), width=2)
    d.line([(X(tt), Y(float(r["charge"]))) for tt, r in zip(t, rows)], fill=(200, 150, 0), width=2)
    d.text((M, top - 20), f"{title}   charge (amber), throttle (blue), crack window (green band)", font=font, fill=(0, 0, 0))
im.save(sys.argv[1])
