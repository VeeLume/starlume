"""Trace the slim separator line of the charge arc on a clean frame (no
crack fill, low charge): red over the overcrack window, yellow over the
crack window, cream below. Row by row from the top, the brightest warm
pixel within 2 px of the previous one. Saves the smoothed path x(y) as a
polynomial plus the measured ends.
usage: sep.py <frame.png> <out.npz>"""
import sys
import numpy as np
from PIL import Image

f = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
r, g, b = f[..., 0], f[..., 1], f[..., 2]
warm = (r > 150) & (r - b > 60)
score = np.where(warm, r + g, 0)

# Start: the red line near its top, around (1596, 540).
ys, xs = [], []
y = 600
x = int(np.argmax(score[y, 1560:1680])) + 1560
for y in range(600, 400, -1):
    row = score[y, x - 2 : x + 3]
    if row.max() == 0:
        if score[y - 1, x - 3 : x + 4].max() == 0 and score[y - 2, x - 3 : x + 4].max() == 0:
            break
        continue
    x = x - 2 + int(np.argmax(row))
    ys.append(y)
    xs.append(x)
top = ys[-1]
ys, xs = ys[::-1], xs[::-1]
x = xs[-1]
miss = 0
for y in range(601, 1130):
    row = score[y, x - 2 : x + 3]
    if row.max() == 0:
        miss += 1
        if miss > 20:
            break
        if miss % 2 == 0 and len(xs) > 10:
            x += int(np.sign(xs[-1] - xs[-10]))
        continue
    miss = 0
    x = x - 2 + int(np.argmax(row))
    ys.append(y)
    xs.append(x)
bottom = ys[-1]
ys, xs = np.array(ys), np.array(xs)
p = np.polyfit(ys, xs, 5)
res = xs - np.polyval(p, ys)
print(f"top {top} bottom {bottom} rows {len(ys)} residual median {np.median(abs(res)):.2f} max {abs(res).max():.1f}")
for yy in range(top, bottom, 40):
    print(yy, round(np.polyval(p, yy), 1))
np.savez(sys.argv[2], p=p, top=top, bottom=bottom, ys=ys, xs=xs)
