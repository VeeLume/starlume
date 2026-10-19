"""Charge AND throttle over a recording, for studying how the player flies.

Charge: bar.Arc (separator calibrated per recording, per-frame shift).
Throttle: the "NN%" readout above LASER, read by matching every string
"0%".."100%" rendered in Audimat Mono Regular (the readout's font) against the
readout box, shifted with the frame's HUD shift. Orange text -> ink is the
red channel ramped high.

usage: series2.py <video> <sep.npz> <box x0,y0,x1,y1 at shift 0> <out.csv> [fps]
"""
import csv
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

import bar
from panel import ncc_map

FONT = "hudata/fonts/14_$Low-Reg_Audimat Mono.ttf"  # the throttle readout's font


def render(text, size):
    f = ImageFont.truetype(FONT, size * 4)
    w = int(f.getlength(text)) + 16
    im = Image.new("L", (w, size * 6), 0)
    ImageDraw.Draw(im).text((8, size), text, font=f, fill=255)
    im = im.crop(im.getbbox())
    tw, th = im.size
    im = im.resize((max(2, round(tw / 4)), max(2, round(th / 4))), Image.LANCZOS).filter(ImageFilter.GaussianBlur(0.7))
    return np.pad(np.asarray(im).astype(float) / 255, 2)


def ink(a):
    # orange HUD text: red well above blue (grey rock and snow are neutral)
    # and bright: warm sandy rock is orange-ish but much darker than the text
    r, b = a[..., 0].astype(float), a[..., 2].astype(float)
    return np.clip((r - b - 40) / 80, 0, 1) * np.clip((r - 200) / 40, 0, 1)


def calibrate(frame, box, known=None):
    """Font size whose renders best match the readout (known: its value)."""
    x0, y0, x1, y1 = box
    crop = ink(frame[y0:y1, x0:x1])
    best = None
    for size in range(18, 40):
        cands = [known] if known is not None else range(0, 101)
        for v in cands:
            t = render(f"{v}%", size)
            if t.shape[0] > crop.shape[0] or t.shape[1] > crop.shape[1]:
                continue
            s = ncc_map(crop, t).max()
            if best is None or s > best[0]:
                best = (s, size, v)
    return best


def reader(size):
    """Read "NN%": find the % sign, then score every "0%".."100%" with its
    own % lined up on it (a missing or extra digit shows as unmatched ink).
    Returns (value, score, margin to the runner-up)."""
    raw = {v: render(f"{v}%", size) for v in range(0, 101)}
    # one window for all: pad left (and to the tallest) so every candidate
    # covers the same pixels, right-aligned on the %
    H = max(t.shape[0] for t in raw.values())
    W = max(t.shape[1] for t in raw.values())
    tmpl = {v: np.pad(t, ((H - t.shape[0], 0), (W - t.shape[1], 0))) for v, t in raw.items()}
    pct = render("%", size)

    def ncc_at(img, t, x, y):
        h, w = t.shape
        if x < 0 or y < 0 or x + w > img.shape[1] or y + h > img.shape[0]:
            return -1.0
        a = img[y: y + h, x: x + w]
        a = a - a.mean()
        b = t - t.mean()
        d = np.sqrt((a * a).sum() * (b * b).sum())
        return float((a * b).sum() / d) if d > 1e-9 else 0.0

    def read(crop):
        if pct.shape[0] > crop.shape[0] or pct.shape[1] > crop.shape[1]:
            return None, 0.0, 0.0
        r = ncc_map(crop, pct)
        py, px = np.unravel_index(np.argmax(r), r.shape)
        right = px + pct.shape[1]
        scores = []
        for v, t in tmpl.items():
            x = right - t.shape[1]
            best = max(ncc_at(crop, t, x + dx, py + dy - (t.shape[0] - pct.shape[0]))
                       for dx in (-1, 0, 1) for dy in (-2, -1, 0, 1, 2))
            scores.append((best, v))
        scores.sort(reverse=True)
        return scores[0][1], scores[0][0], scores[0][0] - scores[1][0]

    return read


if __name__ == "__main__":
    video, npz, boxs, out = sys.argv[1:5]
    fps = float(sys.argv[5]) if len(sys.argv) > 5 else 4
    box = tuple(int(v) for v in boxs.split(","))
    arc = bar.Arc(npz)
    W, H = 2560, 1440
    X0, Y0, X1, Y1 = 1500, 460, 1900, 1140
    cmd = ["ffmpeg", "-v", "error", "-i", video, "-vf", f"fps={fps},crop={X1 - X0}:{Y1 - Y0}:{X0}:{Y0}",
           "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE)
    size = (X1 - X0) * (Y1 - Y0) * 3
    canvas = np.zeros((H, W, 3), int)
    rows, read, k = [], None, 0
    while True:
        buf = p.stdout.read(size)
        if len(buf) < size:
            break
        canvas[Y0:Y1, X0:X1] = np.frombuffer(buf, np.uint8).reshape(Y1 - Y0, X1 - X0, 3)
        s, y, x, dx, dy = arc.charge(canvas)
        bx0, by0, bx1, by1 = box[0] + dx, box[1] + dy, box[2] + dx, box[3] + dy
        crop = ink(canvas[by0 - 4: by1 + 4, bx0 - 6: bx1 + 6])
        if read is None:
            read = reader(35)
        v, sc, margin = read(crop)
        rows.append((round(k / fps, 2), round(s * 100, 1), v, round(sc, 2), round(margin, 3), dx, dy))
        k += 1
    with open(out, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["t", "charge", "throttle", "score", "margin", "dx", "dy"])
        w.writerows(rows)
    print(len(rows), "frames ->", out)
