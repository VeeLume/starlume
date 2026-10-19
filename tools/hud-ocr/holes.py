"""Topology check for the loop digits (0 3 5 6 8 9): after the template
decode, look at each such glyph's enclosed holes and side notches in the
ink band and override the template's pick when the shape is clear.

  8: two holes, one upper, one lower
  0: one hole across the middle (the slit)
  9: one upper hole, no lower hole
  6: one lower hole, no upper hole
  3 / 5: no hole; 3 opens left twice, 5 opens right above and left below
"""
import random
from collections import deque

import numpy as np

import glyphs as G
import train as T

LOOP = set("035689")


def decode_pos(band, templates, fmt, floor=0.25, bias=0.45):
    """decode_fmt, also returning each glyph's (x, y, char)."""
    maps = G.score_maps(band, templates)
    cands = []
    for ch, (sc, ys) in maps.items():
        cols = np.nonzero(templates[ch].max(axis=0) > 0.5)[0]
        il, ir = cols[0], cols[-1]
        for x in range(len(sc)):
            v = sc[x]
            if v >= floor and v >= sc[max(0, x - 2): x + 3].max():
                cands.append((x + il, ir - il + 1, ch, v, x, ys[x]))
    cands.sort()
    slots = G.FORMATS[fmt]
    best = {}
    for i, (x, w, ch, v, _, _) in enumerate(cands):
        for si, (chars, lo, hi) in enumerate(slots):
            if ch not in chars:
                continue
            for cnt in range(1, hi + 1):
                val = None
                bp = None
                if si == 0 and cnt == 1:
                    val, bp = v - bias, None
                for j in range(i):
                    xj, wj = cands[j][0], cands[j][1]
                    if not (xj + wj - 1 <= x <= xj + wj + 5):
                        continue
                    if cnt > 1:
                        prev = (j, si, cnt - 1)
                    else:
                        if si == 0:
                            continue
                        pl, ph = slots[si - 1][1], slots[si - 1][2]
                        prev = max(((j, si - 1, c) for c in range(pl, ph + 1) if (j, si - 1, c) in best),
                                   key=lambda k: best[k][0], default=None)
                        if prev is None:
                            continue
                    if prev in best and (val is None or best[prev][0] + v - bias > val):
                        val, bp = best[prev][0] + v - bias, prev
                if val is not None:
                    best[(i, si, cnt)] = (val, bp)
    last = len(slots) - 1
    ends = [k for k in best if k[1] == last and slots[last][1] <= k[2] <= slots[last][2]]
    if not ends:
        return []
    k = max(ends, key=lambda k: best[k][0])
    out = []
    while k is not None:
        c = cands[k[0]]
        out.append((c[0], c[1], c[2]))
        k = best[k][1]
    return list(reversed(out))


def shape(glyph, thr=0.35):
    """(holes, left notches, right notches) as lists of row centres 0..1."""
    ys, xs = np.nonzero(glyph > 0.5)
    if not len(ys):
        return None
    g = glyph[ys.min(): ys.max() + 1, xs.min(): xs.max() + 1]
    h, w = g.shape
    dark = g < thr
    seen = np.zeros_like(dark)
    holes, left, right = [], [], []
    for sy in range(h):
        for sx in range(w):
            if not dark[sy, sx] or seen[sy, sx]:
                continue
            q = deque([(sy, sx)])
            seen[sy, sx] = True
            cells = []
            while q:
                y, x = q.popleft()
                cells.append((y, x))
                for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    ny, nx = y + dy, x + dx
                    if 0 <= ny < h and 0 <= nx < w and dark[ny, nx] and not seen[ny, nx]:
                        seen[ny, nx] = True
                        q.append((ny, nx))
            cy = np.mean([c[0] for c in cells]) / max(h - 1, 1)
            touches_l = any(c[1] == 0 for c in cells)
            touches_r = any(c[1] == w - 1 for c in cells)
            touches_tb = any(c[0] in (0, h - 1) for c in cells)
            if not (touches_l or touches_r or touches_tb):
                holes.append(cy)
            elif touches_l and not touches_tb and not touches_r and 0.15 < cy < 0.85:
                left.append(cy)
            elif touches_r and not touches_tb and not touches_l and 0.15 < cy < 0.85:
                right.append(cy)
    return holes, left, right


def by_shape(glyph):
    s = shape(glyph)
    if s is None:
        return None
    holes, left, right = s
    up = [c for c in holes if c < 0.45]
    low = [c for c in holes if c > 0.55]
    mid = [c for c in holes if 0.3 <= c <= 0.7]
    if up and low:
        return "8"
    if len(holes) == 1 and mid and not left and not right:
        return "0"
    if up and not low and any(c > 0.5 for c in left):
        return "9"
    if low and not up and any(c < 0.5 for c in right):
        return "6"
    if not holes and len([c for c in left if c < 0.5]) and len([c for c in left if c > 0.5]):
        return "3"
    if not holes and any(c < 0.5 for c in right) and any(c > 0.5 for c in left):
        return "5"
    return None


def decode_checked(band, templates, fmt):
    placed = decode_pos(band, templates, fmt)
    out = []
    for x, w, ch in placed:
        if ch in LOOP:
            g = band[:, max(0, x - 1): x + w + 1]
            alt = by_shape(g)
            if alt is not None:
                ch = alt
        out.append(ch)
    return "".join(out)


if __name__ == "__main__":
    shots = [s for s in T.usable() if T.samples(s)[0] >= 0.5]
    random.seed(1)
    order = shots[:]
    random.shuffle(order)
    ok = ok2 = tot = 0
    for f in range(5):
        test = order[f::5]
        Tm = T.learn([s for s in shots if s not in test])
        for s in test:
            for b, v, fmt in T.samples(s)[1]:
                tot += 1
                ok += G.decode_fmt(b, Tm, fmt) == v
                got = decode_checked(b, Tm, fmt)
                ok2 += got == v
    print(f"templates {ok}/{tot}  with shape check {ok2}/{tot}")
