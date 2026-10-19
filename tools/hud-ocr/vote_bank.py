"""Anchor voting with a template bank: each anchor has templates cut from
the reference capture and from glare captures (at their labelled spot,
rescaled to the reference scale), under brightness and shadow features.
A position's score is the best over the bank. Glare test shots are kept
out of the bank.

usage: vote_bank.py [bank shots...] -- [test shots...]
"""
import sys

import numpy as np

import vote as V
from anchors_eval import ANCHORS, MASS
from locate_eval import TRUTH, feats, half, load, ncc_full, resize


def bank_templates(sources):
    """{anchor: [(feature, template at ref scale)]}"""
    bank = {a: [] for a in ANCHORS}
    for src, scale in sources:
        f = feats(load(src))
        bx, by = (MASS if src == "04-04-03" else TRUTH[src]["mass_label_box"][:2])
        for a, (x0, y0, x1, y1) in ANCHORS.items():
            dx0, dy0, dx1, dy1 = x0 - MASS[0], y0 - MASS[1], x1 - MASS[0], y1 - MASS[1]
            sx0, sy0 = int(round(bx + dx0 * scale)), int(round(by + dy0 * scale))
            sx1, sy1 = int(round(bx + dx1 * scale)), int(round(by + dy1 * scale))
            for k in ("abs", "blackhat"):
                crop = f[k][sy0:sy1, sx0:sx1]
                t = resize(crop, 1 / scale)
                bank[a].append((k, t))
    return bank


def candidates(f, bank):
    out = {}
    halves = {k: half(f[k]) for k in ("abs", "blackhat")}
    for a, (x0, y0, x1, y1) in ANCHORS.items():
        dx, dy = x0 - MASS[0], y0 - MASS[1]
        cands = []
        for sc in V.SCALES:
            r = None
            for k, t in bank[a]:
                m = ncc_full(halves[k], resize(t, sc / 2))
                if r is None:
                    r = m
                else:
                    h, w = min(r.shape[0], m.shape[0]), min(r.shape[1], m.shape[1])
                    r = np.maximum(r[:h, :w], m[:h, :w])
            for v, x, y in V.peaks(r, V.TOPK):
                cands.append((v, x * 2 - dx * sc, y * 2 - dy * sc, sc))
        cands.sort(reverse=True)
        out[a] = cands[: V.TOPK * 2]
    return out


if __name__ == "__main__":
    args = sys.argv[1:]
    cut = args.index("--") if "--" in args else len(args)
    bank_shots, tests = args[:cut], args[cut + 1:] or sorted(TRUTH)
    # (shot, its HUD scale) -- scales from the earlier vote run
    known = {"04-04-03": 1.0, "06-05-38": 1.07, "06-08-56": 1.0, "06-09-00": 0.99}
    bank = bank_templates([("04-04-03", 1.0)] + [(s, known[s]) for s in bank_shots])
    found = n = 0
    for s in tests:
        t = TRUTH[s]
        box = t.get("mass_label_box") if t.get("mode") == "mining" else None
        b = V.vote(candidates(feats(load(s)), bank)) or (0.0, 0, 0, 0, 1.0)
        if box:
            n += 1
            hit = abs(b[2] - box[0]) <= 14 and abs(b[3] - box[1]) <= 14
            found += hit
            tag = "ok" if hit else "XX"
        else:
            tag = "--"
        print(f"{s} {t.get('mode', '-'):7} {tag} score {b[0]:.2f} anchors {b[1]} at ({b[2]:.0f},{b[3]:.0f}) scale {b[4]:.2f}", flush=True)
    print(f"found {found}/{n}")
