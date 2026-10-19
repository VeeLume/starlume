"""Contrast-based ink candidates for glare: render the panel of a few shots
under each transform side by side.

  abs      current: brightest channel ramped over 238..252
  tophat   brightest channel minus its grey opening (7x7): thin bright
           strokes over any smooth background, sun included
  shadow   tophat gated by the dark outline: a stroke pixel needs a pixel
           at least D darker within 2 px (the font's drop shadow)
"""
import json
import sys

import numpy as np
from PIL import Image
from scipy import ndimage as ndi

TRUTH = json.load(open("truth.json", encoding="utf-8"))


def v_of(a):
    return a.max(axis=2).astype(float)


def abs_ink(a):
    return np.clip((v_of(a) - 238) / 14, 0, 1)


def tophat(a, size=7, gain=45.0):
    v = v_of(a)
    th = v - ndi.grey_opening(v, size=(size, size))
    return np.clip(th / gain, 0, 1)


def shadow(a, size=7, gain=45.0, dark=25.0):
    v = v_of(a)
    th = v - ndi.grey_opening(v, size=(size, size))
    near_min = ndi.minimum_filter(v, size=5)
    edge = (v - near_min) >= dark
    return np.clip(th / gain, 0, 1) * edge


if __name__ == "__main__":
    shots = sys.argv[1:] or ["06-05-32", "06-05-23", "06-05-38", "06-09-00", "04-04-03"]
    tiles = []
    for s in shots:
        a = np.asarray(Image.open(f"D:/Recordings/Gaming/Screenshot 2026-10-17 {s}.png").convert("RGB")).astype(int)
        x, y = TRUTH[s]["mass_label_box"][:2]
        crop = a[y - 100 : y + 290, x - 30 : x + 290]
        row = [Image.fromarray(crop.astype(np.uint8))]
        for f in (abs_ink, tophat, shadow):
            row.append(Image.fromarray((f(crop) * 255).astype(np.uint8)).convert("RGB"))
        tiles.append(row)
    w, h = tiles[0][0].size
    can = Image.new("RGB", (w * 4 + 30, (h + 10) * len(tiles)), (40, 40, 40))
    for i, row in enumerate(tiles):
        for j, im in enumerate(row):
            can.paste(im, (j * (w + 10), i * (h + 10)))
    can.save("out/contrast.png")
    print("saved", can.size)
