"""Isolate digit classification from decoding.

Glyphs are placed by forced alignment against the labelled value (so the
positions are right), then cut out of three inputs and classified with a
leave-one-shot-out ridge classifier and with 1-NN:

  band   the thresholded (238..252), resampled band the reader uses now
  raw    native pixels, brightest channel, normalised per glyph (no
         threshold: a hole at 190 under strokes at 250 stays visible)
  raw2x  raw at 2x nearest (same information, finer placement jitter)
"""
import random
from collections import Counter, defaultdict

import numpy as np

import glyphs as G
import train as T
from panel import load

DIG = "0123456789"
W, H = 12, 16  # native window around a glyph's centre


def align_pos(band, truth, Tm):
    """Forced alignment: (char, x in band px, y in band px) per glyph."""
    maps = G.score_maps(band, Tm)
    n, Wd = len(truth), band.shape[1]
    NEG = -1e9
    best = np.full((n, Wd), NEG)
    back = np.zeros((n, Wd), int)
    for i, ch in enumerate(truth):
        if ch not in maps:
            return []
        s = maps[ch][0]
        for x in range(len(s)):
            if i == 0:
                best[i, x] = s[x]
                continue
            pw = Tm[truth[i - 1]].shape[1]
            lo, hi = max(0, x - pw - 6), max(0, x - pw + 3)
            prev = best[i - 1, lo:hi]
            if len(prev) and prev.max() > NEG:
                best[i, x] = prev.max() + s[x]
                back[i, x] = int(np.argmax(prev)) + lo
    x = int(np.argmax(best[n - 1]))
    if best[n - 1, x] / n < 0.5:
        return []
    xs = [x]
    for i in range(n - 1, 0, -1):
        x = back[i, x]
        xs.append(x)
    xs.reverse()
    return [(ch, x, max(0, maps[ch][1][x])) for ch, x in zip(truth, xs)]


def glyphs_of(shot, Tm):
    """[(char, band patch, raw native patch)] for one shot's digits."""
    t = T.TRUTH[shot]
    a = load(shot)
    raw = a.max(axis=2).astype(float)
    m = T.ink(a)
    spot = T.refine(m, T.locate_near(m, t["mass_label_box"]))
    _, sx, sy, s = spot
    out = []
    fields = []
    for k, (b, fmt) in T.FIELDS.items():
        v = t.get(k)
        if v and "?" not in v:
            fields.append((b, v))
    tops = [top for top, bot in T.lines(m, spot, -8, 75, 180, 215)]
    r0 = tops[0] if tops and tops[0] < 205 else 193.0
    for i, row in enumerate(t.get("rows", [])):
        dy = r0 - 6 + 22.3 * i
        for key, bb in (("pct", T.PCT), ("quality", T.QUAL)):
            v = row.get(key)
            if v and "?" not in v:
                fields.append(((bb[0], bb[1] + dy, bb[2], bb[3] + dy), v))
    for b, v in fields:
        band = T.band(m, spot, b)
        for ch, x, y in align_pos(band, v, Tm):
            if ch not in DIG:
                continue
            tpl = Tm[ch]
            g = band[y: y + tpl.shape[0], x: x + tpl.shape[1]]
            if g.shape != tpl.shape:
                continue
            # glyph centre in frame pixels
            cx = sx + (b[0] + x + tpl.shape[1] / 2) * s
            cy = sy + (b[1] + y + tpl.shape[0] / 2) * s
            x0, y0 = int(round(cx - W / 2)), int(round(cy - H / 2))
            rp = raw[y0: y0 + H, x0: x0 + W]
            if rp.shape != (H, W):
                continue
            out.append((ch, g, rp))
    return out


def norm(p):
    p = p.astype(float).ravel()
    p = p - p.mean()
    return p / (np.linalg.norm(p) + 1e-6)


def jitter(p):
    yield p
    for dy, dx in ((0, 1), (0, -1), (1, 0), (-1, 0)):
        yield np.roll(np.roll(p, dy, axis=0), dx, axis=1)


def ridge(train, lam=3.0):
    X = np.array([np.append(norm(p), 1) for _, p in train])
    Y = np.array([[c == d for d in DIG] for c, _ in train], float)
    return np.linalg.solve(X.T @ X + lam * np.eye(X.shape[1]), X.T @ Y)


def evaluate(by_shot, which):
    shots = list(by_shot)
    ok_r = ok_n = tot = 0
    conf = Counter()
    for hold in shots:
        train = [(c, q) for s in shots if s != hold for c, *ps in by_shot[s] for q in jitter(ps[which])]
        Wt = ridge(train)
        Xt = np.array([norm(q) for _, q in train])
        ct = [c for c, _ in train]
        for c, *ps in by_shot[hold]:
            p = ps[which]
            pr = DIG[int(np.argmax(np.append(norm(p), 1) @ Wt))]
            nn = ct[int(np.argmax(Xt @ norm(p)))]
            tot += 1
            ok_r += pr == c
            ok_n += nn == c
            if pr != c:
                conf[(c, pr)] += 1
    return ok_r, ok_n, tot, conf


if __name__ == "__main__":
    shots = [s for s in T.usable() if T.samples(s)[0] >= 0.5]
    Tm = T.learn(shots)
    by_shot = {s: glyphs_of(s, Tm) for s in shots}
    print("digits:", sum(len(v) for v in by_shot.values()),
          dict(sorted(Counter(c for v in by_shot.values() for c, *_ in v).items())))
    for name, which in (("band (thresholded, resampled)", 0), ("raw native pixels", 1)):
        r, n, tot, conf = evaluate(by_shot, which)
        print(f"{name:32} ridge {r}/{tot} ({100*r/tot:.0f} %)   1-NN {n}/{tot} ({100*n/tot:.0f} %)   top confusions {conf.most_common(5)}")
