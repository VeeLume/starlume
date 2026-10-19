"""Which feature finds the scan panel reliably? For every labelled shot,
search the whole frame (half resolution, scales 0.9..1.1) for the label
block under each feature and report: the score at the true spot, the best
score more than 25 px away (a false match), and whether the best match
overall is the true one. Shots without a panel report only the best score
(it should stay low).

Features (all on the brightest channel v):
  abs       ramp 238..252 (current)
  blackhat  closing(v, 5x5) - v: thin dark lines (the glyphs' shadow)
  grad      gradient magnitude (Sobel), clipped
  tophat    v - opening(v, 7x7): thin bright strokes
  both      tophat + blackhat (stroke and its shadow)
"""
import json
import sys

import numpy as np
from PIL import Image
from scipy import ndimage as ndi
from scipy.signal import fftconvolve

TRUTH = json.load(open("truth.json", encoding="utf-8"))
REF, BOX = "04-04-03", (1875, 544, 1950, 618)


def load(s):
    return np.asarray(Image.open(f"D:/Recordings/Gaming/Screenshot 2026-10-17 {s}.png").convert("RGB")).astype(np.float32)


def feats(a):
    v = a.max(axis=2)
    out = {"abs": np.clip((v - 238) / 14, 0, 1)}
    out["blackhat"] = np.clip((ndi.grey_closing(v, size=(5, 5)) - v) / 60, 0, 1)
    gx, gy = ndi.sobel(v, 1), ndi.sobel(v, 0)
    out["grad"] = np.clip(np.hypot(gx, gy) / 400, 0, 1)
    out["tophat"] = np.clip((v - ndi.grey_opening(v, size=(7, 7))) / 45, 0, 1)
    out["both"] = np.clip(out["tophat"] + out["blackhat"], 0, 1)
    return out


def half(img):
    h, w = img.shape[0] // 2 * 2, img.shape[1] // 2 * 2
    return img[:h, :w].reshape(h // 2, 2, w // 2, 2).mean(axis=(1, 3))


def ncc_full(img, t):
    """NCC of t over img (valid), via FFT and integral images."""
    th, tw = t.shape
    n = th * tw
    tz = t - t.mean()
    tn = np.sqrt((tz ** 2).sum())
    num = fftconvolve(img, tz[::-1, ::-1], mode="valid")
    c1 = np.cumsum(np.cumsum(np.pad(img, ((1, 0), (1, 0))), 0), 1)
    c2 = np.cumsum(np.cumsum(np.pad(img ** 2, ((1, 0), (1, 0))), 0), 1)

    def box(c):
        return c[th:, tw:] - c[:-th, tw:] - c[th:, :-tw] + c[:-th, :-tw]

    s1, s2 = box(c1), box(c2)
    var = s2 - s1 * s1 / n
    # A near-flat window (empty sky, saturated glare) has no shape to
    # match: score 0 instead of dividing by ~0.
    floor = 0.02 * (tz ** 2).sum()
    r = num / (np.sqrt(np.maximum(var, 1e-9)) * tn + 1e-9)
    return np.where(var > floor, np.clip(r, -1, 1), 0.0)


def resize(t, s):
    h, w = t.shape
    return np.asarray(Image.fromarray(t.astype(np.float32)).resize((max(2, round(w * s)), max(2, round(h * s))), Image.BILINEAR))


def search(img, tmpl):
    """Best (score, x, y, scale) and the score map maxima per position."""
    best = (-2, 0, 0, 1)
    maps = []
    for s in np.arange(0.88, 1.13, 0.03):
        t = resize(tmpl, s / 2)
        r = ncc_full(img, t)
        maps.append((s, r))
        y, x = np.unravel_index(np.argmax(r), r.shape)
        if r[y, x] > best[0]:
            best = (float(r[y, x]), x * 2, y * 2, s)
    return best, maps


if __name__ == "__main__":
    ref = feats(load(REF))
    x0, y0, x1, y1 = BOX
    tmpls = {k: half(v[y0:y1, x0:x1]) if False else v[y0:y1, x0:x1] for k, v in ref.items()}
    names = list(tmpls)
    shots = sys.argv[1:] or sorted(TRUTH)
    tally = {k: [0, 0] for k in names}
    print(f"{'shot':10} {'mode':7} " + " ".join(f"{k:>18}" for k in names))
    for s in shots:
        t = TRUTH.get(s, {})
        box = t.get("mass_label_box") if t.get("mode") == "mining" else None
        f = feats(load(s))
        cells = []
        for k in names:
            img = half(f[k])
            (sc, bx, by, bs), maps = search(img, tmpls[k])
            if box:
                tx, ty = box[0], box[1]
                true_sc = max(r[max(0, ty // 2 - 3): ty // 2 + 4, max(0, tx // 2 - 3): tx // 2 + 4].max() for _, r in maps)
                hit = abs(bx - tx) <= 12 and abs(by - ty) <= 12
                tally[k][0] += hit
                tally[k][1] += 1
                cells.append(f"{'ok' if hit else 'XX'} {true_sc:4.2f}/{sc:4.2f}")
            else:
                cells.append(f"   best {sc:4.2f}   ")
        print(f"{s:10} {t.get('mode', '-'):7} " + " ".join(f"{c:>18}" for c in cells), flush=True)
    print("found:", {k: f"{a}/{b}" for k, (a, b) in tally.items()})
