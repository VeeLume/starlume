"""Exemplar matching: every aligned training glyph is a template; a char's
score at x is its best exemplar's NCC. Cross-validated like train.py."""
import random
import sys

import numpy as np

import glyphs as G
import train as T
from panel import ncc_map


def exemplars(shots, Tm):
    ex = {}
    for s in shots:
        for b, v, _ in T.samples(s)[1]:
            if set(v) <= set(Tm):
                for ch, g in G.align(b, v, Tm):
                    ex.setdefault(ch, []).append(g)
    return ex


def score_maps_knn(band, Tm, ex):
    maps = {}
    padded = np.pad(band, ((1, 1), (0, 0)))
    for ch, gs in ex.items():
        th, tw = Tm[ch].shape
        if band.shape[1] < tw:
            continue
        best = None
        for g in gs:
            r = ncc_map(padded, g)
            m = r.max(axis=0)
            best = m if best is None else np.maximum(best, m)
        maps[ch] = (best, np.zeros(len(best), int))
    return maps


if __name__ == "__main__":
    shots = [s for s in T.usable() if T.samples(s)[0] >= 0.5]
    random.seed(1)
    order = shots[:]
    random.shuffle(order)
    ok = tot = 0
    for f in range(5):
        test = order[f::5]
        train = [s for s in shots if s not in test]
        Tm = T.learn(train)
        ex = exemplars(train, Tm)
        orig = G.score_maps
        G.score_maps = lambda band, templates: score_maps_knn(band, Tm, ex)
        for s in test:
            for b, v, fmt in T.samples(s)[1]:
                got = G.decode_fmt(b, Tm, fmt)
                tot += 1
                ok += got == v
        G.score_maps = orig
    print(f"knn cross-validated (good shots only): {ok}/{tot}")
