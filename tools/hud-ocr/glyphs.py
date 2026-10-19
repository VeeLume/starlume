"""Digit templates for the scan panel, learned from the captures' known
values, and a decoder (NCC + greedy non-max suppression).

1. Seeds: glyphs cut from fields whose column split is clean.
2. Forced alignment: for every field with a known value, place its
   characters in order (DP over x) with the current templates, cut them,
   and average per character.
3. Decode: slide every template over a field band; take the best matches
   above a threshold, no overlap, left to right.
Leave-one-shot-out accuracy is printed."""
import numpy as np

from digits import SHOTS, columns
from panel import MASS_BOX, REF, ink, load, locate, ncc_map

# Field bands relative to the MASS label's top-left.
FIELDS = {
    "mass": (125, -4, 215, 22),
    "res": (125, 24, 190, 50),
    "ins": (125, 52, 200, 78),
}
ROW0, ROW_PITCH = 186, 22
PCT = (-8, 0, 75, 20)
QUAL = (205, 0, 250, 20)
CHARS = "0123456789.%"


def fields_of(shot):
    """[(band image, truth string)] for one shot."""
    a = load(shot)
    m = ink(a)
    ref = ink(load(REF))
    x0, y0, x1, y1 = MASS_BOX
    x, y, _ = locate(a, ref[y0:y1, x0:x1])
    mass, res, ins, pcts, quals = SHOTS[shot]
    out = []
    for k, truth in (("mass", mass), ("res", res), ("ins", ins)):
        if truth is None:
            continue
        l, t, r, b = FIELDS[k]
        out.append((m[y + t : y + b, x + l : x + r], truth))
    for i, (p, q) in enumerate(zip(pcts, quals)):
        ry = y + ROW0 + i * ROW_PITCH
        l, t, r, b = PCT
        out.append((m[ry + t : ry + b, x + l : x + r], p))
        l, t, r, b = QUAL
        out.append((m[ry + t : ry + b, x + l : x + r], q))
    return out


H = 15  # window height: the glyph (12-13 px) plus a row of context


CELL = 10  # digits are monospaced: a narrow "1" still owns a full cell


def tight(band, x0, x1, ch="0"):
    if ch.isdigit():
        c = (x0 + x1) // 2
        x0, x1 = c - CELL // 2, c - CELL // 2 + CELL
    """The glyph between columns x0..x1 in a fixed-height window ending one
    row below its bottom, with a column of context each side."""
    g = band[:, x0:x1]
    ys = np.nonzero((g > 0.5).any(axis=1))[0]
    if not len(ys):
        return None
    bottom = ys[-1] + 2
    top = bottom - H
    if top < 0 or x0 < 1 or x1 + 1 > band.shape[1] or bottom > band.shape[0]:
        return None
    return band[top:bottom, x0 - 1 : x1 + 1]


def seeds(samples):
    """Cut glyphs where the column split matches the truth ('%' = 2 parts)."""
    got = {}
    for band, truth in samples:
        parts = columns(band > 0.5)
        want = len(truth) + truth.count("%")
        if len(parts) != want:
            continue
        i = 0
        for ch in truth:
            if ch == "%":
                a, b = parts[i], parts[i + 1]
                x0, x1 = a[0], b[1]
                i += 2
            else:
                x0, x1 = parts[i][0], parts[i][1]
                i += 1
            if x1 - x0 > (24 if ch == "%" else 10):
                continue
            t = tight(band, x0, x1, ch)
            if t is not None:
                got.setdefault(ch, []).append(t)
    return got


def mean_template(gs):
    h = int(np.median([g.shape[0] for g in gs]))
    w = int(np.median([g.shape[1] for g in gs]))
    acc = np.zeros((h, w))
    n = 0
    for g in gs:
        if abs(g.shape[0] - h) <= 1 and abs(g.shape[1] - w) <= 1:
            pad = np.zeros((h, w))
            hh, ww = min(h, g.shape[0]), min(w, g.shape[1])
            pad[:hh, :ww] = g[:hh, :ww]
            acc += pad
            n += 1
    return acc / max(n, 1)


def score_maps(band, templates):
    """Per char: best NCC over y for every x (and its y)."""
    maps = {}
    for ch, t in templates.items():
        th, tw = t.shape
        if band.shape[0] < th or band.shape[1] < tw:
            continue
        r = ncc_map(np.pad(band, ((1, 1), (0, 0))), t)
        maps[ch] = (r.max(axis=0), r.argmax(axis=0) - 1)
    return maps


def decode(band, templates, thr=0.6):
    maps = score_maps(band, templates)
    cands = []
    for ch, (s, _) in maps.items():
        w = templates[ch].shape[1]
        for x, v in enumerate(s):
            if v >= thr:
                cands.append((v, x, x + w, ch))
    cands.sort(reverse=True)
    taken = []
    for v, a, b, ch in cands:
        if all(b <= ta + 1 or a >= tb - 1 for _, ta, tb, _ in taken):
            taken.append((v, a, b, ch))
    taken.sort(key=lambda c: c[1])
    return "".join(c[3] for c in taken)


FORMATS = {
    # slots: (allowed chars, min count, max count)
    "int": [("0123456789", 1, 6)],
    "int%": [("0123456789", 1, 3), ("%", 1, 1)],
    "dec": [("0123456789", 1, 3), (".", 1, 1), ("0123456789", 2, 2)],
    "dec%": [("0123456789", 1, 3), (".", 1, 1), ("0123456789", 2, 2), ("%", 1, 1)],
}


def format_of(truth):
    if "." in truth:
        return "dec%" if truth.endswith("%") else "dec"
    return "int%" if truth.endswith("%") else "int"


DIG = "0123456789"


def zs(p):
    p = p.ravel() - p.mean()
    return p / (np.linalg.norm(p) + 1e-6)


def train_clf(patches, lam=1.0):
    """Ridge regression of z-scored digit patches onto one-hot classes."""
    X = np.array([zs(g) for _, g in patches])
    Y = np.array([[1.0 if c == d else 0.0 for d in DIG] for c, _ in patches])
    X1 = np.hstack([X, np.ones((len(X), 1))])
    W = np.linalg.solve(X1.T @ X1 + lam * np.eye(X1.shape[1]), X1.T @ Y)
    return W


def classify(W, g):
    o = np.append(zs(g), 1.0) @ W
    return o


def decode_fmt(band, templates, fmt, floor=0.25, bias=0.45, clf=None, beta=0.6):
    """Best string of the format: glyph placements left to right, each
    starting just after the previous one, scored by sum(NCC - bias). With a
    classifier, a digit's identity comes from it: at each digit position the
    generic digit score goes to the classified digit, the others pay."""
    maps = score_maps(band, templates)
    if clf is not None and all(d in maps for d in DIG):
        gen = np.max([maps[d][0] for d in DIG], axis=0)
        th, tw = templates["0"].shape
        padded = np.pad(band, ((1, 1), (0, 0)))
        outs = np.full((len(gen), len(DIG)), -9.0)
        for x in range(len(gen)):
            ybest = maps[DIG[int(np.argmax([maps[d][0][x] for d in DIG]))]][1][x] + 1
            g = padded[ybest : ybest + th, x : x + tw]
            if g.shape == (th, tw):
                outs[x] = classify(clf, g)
        for k, d in enumerate(DIG):
            pen = beta * (outs.max(axis=1) - outs[:, k])
            maps[d] = (gen - pen, maps[d][1])
    cands = []
    for ch, (sc, _) in maps.items():
        cols = np.nonzero(templates[ch].max(axis=0) > 0.5)[0]
        il, ir = cols[0], cols[-1]
        for x in range(len(sc)):
            v = sc[x]
            if v >= floor and v >= sc[max(0, x - 2) : x + 3].max():
                # (ink left, ink width, char, score)
                cands.append((x + il, ir - il + 1, ch, v))
    cands.sort()
    slots = FORMATS[fmt]
    # state: (slot, count) after placing a glyph; value, back-pointer
    best = {}
    for i, (x, w, ch, v) in enumerate(cands):
        for si, (chars, lo, hi) in enumerate(slots):
            if ch not in chars:
                continue
            for cnt in range(1, hi + 1):
                key = (i, si, cnt)
                val = None
                if si == 0 and cnt == 1:
                    val, bp = v - bias, None
                for j in range(i):
                    xj, wj, chj, vj = cands[j]
                    if not (xj + wj - 1 <= x <= xj + wj + 5):  # ink gap -1..5 px
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
                    best[key] = (val, bp)
    last = len(slots) - 1
    ends = [k for k in best if k[1] == last and slots[last][1] <= k[2] <= slots[last][2]]
    if not ends:
        return ""
    k = max(ends, key=lambda k: best[k][0])
    out = []
    while k is not None:
        out.append(cands[k[0]][2])
        k = best[k][1]
    return "".join(reversed(out))


def align(band, truth, templates):
    """Place truth's characters in order (DP), return cut glyphs."""
    maps = score_maps(band, templates)
    n, W = len(truth), band.shape[1]
    NEG = -1e9
    best = np.full((n, W), NEG)
    back = np.zeros((n, W), int)
    for i, ch in enumerate(truth):
        if ch not in maps:
            return []
        s = maps[ch][0]
        for x in range(len(s)):
            if i == 0:
                best[i, x] = s[x]
                continue
            pw = templates[truth[i - 1]].shape[1]
            lo, hi = max(0, x - pw - 6), max(0, x - pw + 3)
            prev = best[i - 1, lo:hi]
            if len(prev) and prev.max() > NEG:
                j = int(np.argmax(prev)) + lo
                best[i, x] = prev.max() + s[x]
                back[i, x] = j
    x = int(np.argmax(best[n - 1]))
    if best[n - 1, x] / n < 0.5:
        return []
    xs = [x]
    for i in range(n - 1, 0, -1):
        x = back[i, x]
        xs.append(x)
    xs.reverse()
    out = []
    for ch, x in zip(truth, xs):
        t = templates[ch]
        y = maps[ch][1][x]
        y = max(0, y)
        g = band[y : y + t.shape[0], x : x + t.shape[1]]
        if g.shape == t.shape:
            out.append((ch, g))
    return out


def learn(shots):
    samples = [s for shot in shots for s in fields_of(shot)]
    got = seeds(samples)
    templates = {ch: mean_template(gs) for ch, gs in got.items()}
    for _ in range(2):
        cut = {}
        for band, truth in samples:
            if not set(truth) <= set(templates):
                continue
            for ch, g in align(band, truth, templates):
                cut.setdefault(ch, []).append(g)
        for ch, gs in cut.items():
            templates[ch] = np.mean(gs, axis=0)
    return templates


if __name__ == "__main__":
    allshots = list(SHOTS)
    t = learn(allshots)
    print("chars:", "".join(sorted(t)), {c: t[c].shape for c in sorted(t)})
    ok = tot = 0
    for hold in allshots:
        tt = learn([s for s in allshots if s != hold])
        for band, truth in fields_of(hold):
            got = decode_fmt(band, tt, format_of(truth))
            tot += 1
            ok += got == truth
            if got != truth:
                print(f"  {hold}: want {truth!r:10} got {got!r}")
    print(f"leave-one-out: {ok}/{tot}")
    np.savez("templates.npz", **{("pct" if c == "%" else "dot" if c == "." else c): v for c, v in t.items()})
