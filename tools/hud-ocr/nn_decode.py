"""Step 2 prototype: the format DP places glyphs (templates), then every
placed digit is re-identified by its nearest training example (best NCC
over exemplars, +-1 px). Cross-validated by shot like train.py."""
import random

import numpy as np

import glyphs as G
import holes as H
import knn as K
import train as T
from panel import ncc_map

DIG = "0123456789"


def nn_label(band, x, w, ex, Tm):
    """Best exemplar char for the glyph whose ink starts at x."""
    th, tw = Tm["0"].shape
    best = (-2, None)
    padded = np.pad(band, ((1, 1), (2, 2)))
    for ch in DIG:
        for g in ex.get(ch, []):
            # template window starts one context column left of the ink
            x0 = x - 1 - (tw - 2 - w) // 2 + 2
            win = padded[:, max(0, x0 - 2): x0 + tw + 2]
            if win.shape[1] < tw:
                continue
            v = ncc_map(win, g).max()
            if v > best[0]:
                best = (v, ch)
    return best[1]


def decode_nn(band, Tm, ex, fmt):
    out = []
    for x, w, ch in H.decode_pos(band, Tm, fmt):
        if ch in DIG:
            ch = nn_label(band, x, w, ex, Tm) or ch
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
        train = [s for s in shots if s not in test]
        Tm = T.learn(train)
        ex = K.exemplars(train, Tm)
        for s in test:
            for b, v, fmt in T.samples(s)[1]:
                tot += 1
                ok += G.decode_fmt(b, Tm, fmt) == v
                ok2 += decode_nn(b, Tm, ex, fmt) == v
    print(f"templates {ok}/{tot}   placement + nearest example {ok2}/{tot}")
