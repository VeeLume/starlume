"""Export the reader's learned data for Rust as one binary file, scan.bin,
into the dev app's data dir (%APPDATA%\\starlume-dev\\mining). Never into the
repo: it is learned from the game's HUD font.

  magic b"SLHUD1"
  records, each:  u8 kind, u8 key, u8 feature, u16 w, u16 h, w*h u8
    kind 1  anchor template   key = anchor index (header, block, comp, bar)
                              feature = 0 brightness, 1 shadow
    kind 2  digit template    key = ascii char (mean template)
    kind 3  digit exemplar    key = ascii char
  Templates are at the reference HUD scale (2560x1440, 04-04-03).
"""
import os
import struct

import numpy as np

import knn as K
import train as T
from anchors_eval import ANCHORS, MASS
from locate_eval import TRUTH, feats, load, resize

# Not in the repo (learned from the game's HUD font): the dev app reads it here.
OUT = os.path.join(os.environ["APPDATA"], "starlume-dev", "mining", "scan.bin")
# glare captures for the shadow bank: (shot, its HUD scale)
GLARE = [("06-05-38", 1.07), ("06-08-56", 1.0), ("06-09-00", 0.99)]
NAMES = list(ANCHORS)


def u8(a):
    return np.clip(np.round(a * 255), 0, 255).astype(np.uint8)


def rec(kind, key, feature, a):
    h, w = a.shape
    return struct.pack("<BBBHH", kind, key, feature, w, h) + u8(a).tobytes()


def anchor_crops(shot, scale, feature_names):
    f = feats(load(shot))
    bx, by = MASS if shot == "04-04-03" else TRUTH[shot]["mass_label_box"][:2]
    out = []
    for i, (a, (x0, y0, x1, y1)) in enumerate(ANCHORS.items()):
        sx0 = int(round(bx + (x0 - MASS[0]) * scale))
        sy0 = int(round(by + (y0 - MASS[1]) * scale))
        sx1 = int(round(bx + (x1 - MASS[0]) * scale))
        sy1 = int(round(by + (y1 - MASS[1]) * scale))
        for fi, k in feature_names:
            crop = f[k][sy0:sy1, sx0:sx1]
            out.append((i, fi, resize(crop, 1 / scale) if scale != 1.0 else crop))
    return out


if __name__ == "__main__":
    blob = bytearray(b"SLHUD1")
    n = {1: 0, 2: 0, 3: 0}
    for i, fi, t in anchor_crops("04-04-03", 1.0, [(0, "abs"), (1, "blackhat")]):
        blob += rec(1, i, fi, t)
        n[1] += 1
    for shot, sc in GLARE:
        for i, fi, t in anchor_crops(shot, sc, [(1, "blackhat")]):
            blob += rec(1, i, fi, t)
            n[1] += 1
    shots = [s for s in T.usable() if T.samples(s)[0] >= 0.5]
    Tm = T.learn(shots)
    for ch, t in sorted(Tm.items()):
        blob += rec(2, ord(ch), 0, t)
        n[2] += 1
    for ch, gs in sorted(K.exemplars(shots, Tm).items()):
        if ch.isdigit():
            for g in gs:
                blob += rec(3, ord(ch), 0, g)
                n[3] += 1
    open(OUT, "wb").write(bytes(blob))
    print(f"wrote {OUT}: {len(blob)} bytes; anchors {n[1]}, templates {n[2]}, exemplars {n[3]}")
