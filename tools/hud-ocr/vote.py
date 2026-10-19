"""Locate the scan panel by anchor voting.

Each anchor (header, MASS block, COMPOSITION, bar frame) is searched over
the whole frame at a few scales, under the fused feature (brightness +
shadow outline). Its top candidates (non-max suppressed) each predict the
panel origin (the MASS label's top-left) and scale. Predictions within
12 px and 4 % scale of each other form a cluster; a cluster scores the sum
of its best candidate per anchor. The best cluster is the panel; how many
anchors agree is the confidence.
"""
import sys

import numpy as np

from anchors_eval import ANCHORS, MASS
from locate_eval import TRUTH, feats, half, load, ncc_full, resize

SCALES = np.arange(0.88, 1.13, 0.03)
import os
FEATURE = os.environ.get("FEATURE", "fused")
TOPK = 8


def peaks(r, k, sep=10):
    """Top-k local maxima of a score map (half-res coords)."""
    r = r.copy()
    out = []
    for _ in range(k):
        y, x = np.unravel_index(np.argmax(r), r.shape)
        v = r[y, x]
        if v <= 0.2:
            break
        out.append((float(v), x, y))
        r[max(0, y - sep): y + sep + 1, max(0, x - sep): x + sep + 1] = -1
    return out


def candidates(f, ref, feature="fused"):
    """{anchor: [(score, origin_x, origin_y, scale)]} in full-res px."""
    out = {}
    for a, (x0, y0, x1, y1) in ANCHORS.items():
        dx, dy = x0 - MASS[0], y0 - MASS[1]
        cands = []
        for sc in SCALES:
            rs = []
            for k in ("abs", "blackhat"):
                rs.append(ncc_full(half(f[k]), resize(ref[k][y0:y1, x0:x1], sc / 2)))
            if feature == "fused":
                r = (rs[0] + rs[1]) / 2
            elif feature == "max":
                # whichever cue the part shows: brightness, or (under
                # glare) the shadow outline
                r = np.maximum(rs[0], rs[1])
            else:
                r = rs[0 if feature == "abs" else 1]
            for v, x, y in peaks(r, TOPK):
                cands.append((v, x * 2 - dx * sc, y * 2 - dy * sc, sc))
        cands.sort(reverse=True)
        out[a] = cands[: TOPK * 2]
    return out


def vote(cands):
    """Best cluster: (score, n anchors, x, y, scale)."""
    flat = [(a, *c) for a, cs in cands.items() for c in cs]
    best = None
    for _, v0, x0, y0, s0 in flat:
        members = {}
        for a, v, x, y, s in flat:
            if abs(x - x0) <= 12 and abs(y - y0) <= 12 and abs(s - s0) <= 0.04:
                if a not in members or v > members[a][0]:
                    members[a] = (v, x, y, s)
        total = sum(m[0] for m in members.values())
        if best is None or total > best[0]:
            xs = np.mean([m[1] for m in members.values()])
            ys = np.mean([m[2] for m in members.values()])
            ss = np.mean([m[3] for m in members.values()])
            best = (total, len(members), xs, ys, ss)
    return best


if __name__ == "__main__":
    ref = feats(load("04-04-03"))
    shots = sys.argv[1:] or sorted(TRUTH)
    found = n = 0
    for s in shots:
        t = TRUTH[s]
        box = t.get("mass_label_box") if t.get("mode") == "mining" else None
        b = vote(candidates(feats(load(s)), ref, FEATURE)) or (0.0, 0, 0, 0, 1.0)
        if box:
            n += 1
            hit = abs(b[2] - box[0]) <= 14 and abs(b[3] - box[1]) <= 14
            found += hit
            tag = "ok" if hit else "XX"
        else:
            tag = "--"
        print(f"{s} {t.get('mode', '-'):7} {tag} score {b[0]:.2f} anchors {b[1]} at ({b[2]:.0f},{b[3]:.0f}) scale {b[4]:.2f}", flush=True)
    print(f"found {found}/{n}")
