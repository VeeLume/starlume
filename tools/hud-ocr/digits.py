"""Prototype: segment the digits of the scan panel's number fields and see
how consistent the glyphs are. Fields are cut by fixed boxes found by the
Windows OCR run (panel at the same place in these shots, +-3 px)."""
import numpy as np
from PIL import Image

SHOTS = {
    "03-50-54": ("44047", None, "56.44", ["7.96%", "6.73%", "52.68%"], ["537", "593", "359"]),
    "03-51-51": ("13481", "0%", "17.64", ["8.92%", "10.22%", "57.39%"], ["537", "593", "359"]),
    "03-56-40": ("40742", "0%", "40.14", ["5.82%", "7.19%", "58.43%"], ["340", "742", "359"]),
    "03-57-06": ("13176", "0%", "18.19", ["6.84%", "11.50%", "72.34%"], ["537", "593", "359"]),
    "03-59-01": ("3440", "0%", "5.05", ["100.00%"], ["359"]),
    "04-01-05": ("2147", "0%", "2.83", ["6.20%", "8.11%", "55.46%"], ["537", "652", "359"]),
    "04-04-03": ("6393", "54%", "50.47", ["7.27%", "73.05%", "7.06%"], ["510", "298", "530"]),
}


def ink(a):
    """Text mask: the brightest channel well above the local background."""
    v = a.max(axis=2).astype(float)
    # background: median of the box's darker half
    bg = np.median(v)
    return v > max(bg + 60, 150)


def columns(mask, min_gap=1):
    """Split a field mask into glyphs by empty columns; trim rows."""
    cols = mask.any(axis=0)
    out, start = [], None
    for x, c in enumerate(list(cols) + [False]):
        if c and start is None:
            start = x
        elif not c and start is not None:
            out.append((start, x))
            start = None
    glyphs = []
    for x0, x1 in out:
        g = mask[:, x0:x1]
        rows = np.nonzero(g.any(axis=1))[0]
        glyphs.append((x0, x1, rows[0], rows[-1] + 1, g[rows[0] : rows[-1] + 1]))
    return glyphs


if __name__ == "__main__":
    for shot, (mass, *_rest) in SHOTS.items():
        im = np.asarray(Image.open(f"D:/Recordings/Gaming/Screenshot 2026-10-17 {shot}.png").convert("RGB")).astype(int)
        box = im[540:566, 2000:2100]
        m = ink(box)
        gl = columns(m)
        print(shot, mass, [(x1 - x0, y1 - y0) for x0, x1, y0, y1, _ in gl])
