"""Train and cross-validate the scan-panel digit templates on truth.json.

Same pipeline as the Rust reader: soft ink, the label block located
(multi-scale NCC, here only near the labelled MASS box), field bands
resampled to the reference scale, format-constrained decoding."""
import json
import random
import sys

import numpy as np

import glyphs as G
from panel import REF, ink, load, ncc_map

TRUTH = json.load(open("truth.json"))
_REFINK = ink(load(REF))
ANCHOR = _REFINK[544:618, 1875:1950]
# COMPOSITION label: 132 reference px below the label block's top
COMP = _REFINK[676:696, 1874:2062]
COMP_DY = 132

FIELDS = {"mass": ((125, -4, 215, 22), "int"), "resist": ((125, 24, 190, 50), "int%"), "instab": ((125, 52, 200, 78), "dec")}
ROW0, PITCH = 186, 22
PCT, QUAL = (-8, 0, 75, 20), (205, 0, 250, 20)


def resize(t, s):
    from PIL import Image
    h, w = t.shape
    im = Image.fromarray((t * 255).astype(np.uint8)).resize((max(2, round(w * s)), max(2, round(h * s))), Image.BILINEAR)
    return np.asarray(im).astype(float) / 255


def locate_near(m, box):
    x0, y0 = box[0], box[1]
    best = (-1, 0, 0, 1)
    for s in np.arange(0.86, 1.16, 0.02):
        t = resize(ANCHOR, s)
        ys, xs = max(0, y0 - 16), max(0, x0 - 16)
        win = m[ys : ys + t.shape[0] + 32, xs : xs + t.shape[1] + 32]
        if win.shape[0] < t.shape[0] or win.shape[1] < t.shape[1]:
            continue
        r = ncc_map(win, t)
        y, x = np.unravel_index(np.argmax(r), r.shape)
        if r[y, x] > best[0]:
            best = (r[y, x], xs + x, ys + y, s)
    return best


def refine(m, spot):
    """Scale from the COMPOSITION label: far from the block, so the scale
    comes out to ~0.3 %. Returns (score, x, y, s) with the better scale."""
    sc, x, y, s = spot
    best = None
    for s2 in np.arange(s - 0.08, s + 0.081, 0.01):
        t = resize(COMP, s2)
        ys, xs = int(y + (COMP_DY - 14) * s2), int(x - 14 * s2)
        win = m[ys : ys + t.shape[0] + int(28 * s2), xs : xs + t.shape[1] + int(28 * s2)]
        if win.shape[0] < t.shape[0] or win.shape[1] < t.shape[1]:
            continue
        r = ncc_map(win, t)
        yy, xx = np.unravel_index(np.argmax(r), r.shape)
        if best is None or r[yy, xx] > best[0]:
            best = (r[yy, xx], xs + xx, ys + yy)
    if best is None or best[0] < 0.4:
        return spot
    s_new = (best[2] - y) / COMP_DY
    return (sc, x, y, s_new)


def lines(m, spot, x0, x1, y0, y1, min_h=7):
    """Text lines (top, bottom) in reference units by ink row profile."""
    _, x, y, s = spot
    a = m[int(y + y0 * s) : int(y + y1 * s), int(x + x0 * s) : int(x + x1 * s)]
    prof = a.sum(axis=1)
    thr = max(2.0, 0.08 * a.shape[1])
    out, start = [], None
    for i, v in enumerate(list(prof) + [0]):
        if v > thr and start is None:
            start = i
        elif v <= thr and start is not None:
            if i - start >= min_h * s:
                out.append((y0 + start / s, y0 + i / s))
            start = None
    return out


def band(m, spot, b):
    import os
    _, x, y, s = spot
    if os.environ.get("SAMPLE") == "nearest":
        # native pixels: 1-px holes (the 0's slit, the 8's two holes) survive
        l, t, r, bt = b
        gx, gy = np.meshgrid(np.arange(l, r), np.arange(t, bt))
        fx = np.clip(np.round(x + gx * s).astype(int), 0, m.shape[1] - 1)
        fy = np.clip(np.round(y + gy * s).astype(int), 0, m.shape[0] - 1)
        return m[fy, fx]
    l, t, r, bt = b
    gx, gy = np.meshgrid(np.arange(l, r), np.arange(t, bt))
    fx, fy = x + gx * s, y + gy * s
    x0, y0 = np.floor(fx).astype(int), np.floor(fy).astype(int)
    tx, ty = fx - x0, fy - y0
    H, W = m.shape
    x0c, y0c = np.clip(x0, 0, W - 2), np.clip(y0, 0, H - 2)
    return (m[y0c, x0c] * (1 - tx) * (1 - ty) + m[y0c, x0c + 1] * tx * (1 - ty)
            + m[y0c + 1, x0c] * (1 - tx) * ty + m[y0c + 1, x0c + 1] * tx * ty)


CACHE = {}


def samples(shot):
    """[(band, truth, fmt)] for one shot."""
    if shot in CACHE:
        return CACHE[shot]
    t = TRUTH[shot]
    a = load(shot)
    m = ink(a)
    spot = refine(m, locate_near(m, t["mass_label_box"]))
    import os
    mode = os.environ.get("BAND_MODE", "ink")
    if mode == "raw":
        # digits read from the raw brightest channel: holes stay darker
        # than strokes even inside the glow
        bm = a.max(axis=2) / 255.0
    elif mode == "gamma":
        bm = (np.clip((a.max(axis=2) - 160) / 95.0, 0, 1)) ** 3
    else:
        bm = m
    out = []
    for k, (b, fmt) in FIELDS.items():
        v = t.get(k)
        if v and "?" not in v:
            out.append((band(bm, spot, b), v, fmt))
    # First row: the first text line below the composition bar; the rest
    # follow at the panel's fixed pitch.
    tops = [top for top, bot in lines(m, spot, -8, 75, 180, 215)]
    r0 = tops[0] if tops and tops[0] < 205 else 193.0
    rows_at = [r0 + 22.3 * i for i in range(8)]
    for i, row in enumerate(t.get("rows", [])):
        if i >= len(rows_at):
            break
        dy = rows_at[i] - 6
        p, q = row.get("pct"), row.get("quality")
        if p and "?" not in p:
            out.append((band(bm, spot, (PCT[0], PCT[1] + dy, PCT[2], PCT[3] + dy)), p, "dec%"))
        if q and "?" not in q:
            out.append((band(bm, spot, (QUAL[0], QUAL[1] + dy, QUAL[2], QUAL[3] + dy)), q, "int"))
    CACHE[shot] = (spot[0], out)
    return CACHE[shot]


def usable():
    return [k for k, t in sorted(TRUTH.items())
            if t.get("mode") == "mining" and t.get("state") == "scanned" and t.get("mass_label_box")
            and "head tracking" not in t.get("conditions", [])]


def learn_clf(shots, T):
    data = [(b, v) for s in shots for b, v, _ in samples(s)[1]]
    patches = []
    for b, v in data:
        if set(v) <= set(T):
            for ch, g in G.align(b, v, T):
                if ch in G.DIG:
                    patches.append((ch, g))
                    # jitter: +-1 px shifts make it tolerant of placement
                    patches.append((ch, np.roll(g, 1, axis=1)))
                    patches.append((ch, np.roll(g, -1, axis=1)))
    return G.train_clf(patches)


def learn(shots):
    data = [(b, v) for s in shots for b, v, _ in samples(s)[1]]
    got = G.seeds(data)
    T = {ch: G.mean_template(gs) for ch, gs in got.items()}
    for _ in range(3):
        cut = {}
        for b, v in data:
            if set(v) <= set(T):
                for ch, g in G.align(b, v, T):
                    cut.setdefault(ch, []).append(g)
        for ch, gs in cut.items():
            T[ch] = np.mean(gs, axis=0)
    return T


if __name__ == "__main__":
    shots = usable()
    print(len(shots), "usable shots;", "anchor scores:",
          " ".join(f"{s}:{samples(s)[0]:.2f}" for s in shots))
    random.seed(1)
    folds = 5
    order = shots[:]
    random.shuffle(order)
    ok = tot = 0
    errs = []
    for f in range(folds):
        test = order[f::folds]
        train = [s for s in shots if s not in test]
        T = learn(train)
        W = learn_clf(train, T) if "--clf" in sys.argv else None
        for s in test:
            for b, v, fmt in samples(s)[1]:
                got = G.decode_fmt(b, T, fmt, clf=W)
                tot += 1
                ok += got == v
                if got != v:
                    errs.append((s, v, got))
    for e in errs:
        print("  ", *e)
    print(f"cross-validated: {ok}/{tot}")
    if "--save" in sys.argv:
        np.save("templates_all.npy", learn(shots), allow_pickle=True)
