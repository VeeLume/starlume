"""Prototype of the pixel scan-panel reader.

ink = brightest channel >= 235 (HUD text is saturated; snow and rock stay
below with the text's dark outline between). The panel is found by the
"MASS:" label (ink template, NCC); fields sit at fixed offsets from it.
Digits are read by NCC template matching with greedy non-max suppression.
"""
import numpy as np
from PIL import Image

from digits import SHOTS

REF = "04-04-03"
MASS_BOX = (1875, 544, 1945, 562)  # the "MASS:" label in REF


def load(shot):
    return np.asarray(Image.open(f"D:/Recordings/Gaming/Screenshot 2026-10-17 {shot}.png").convert("RGB")).astype(int)


def ink(a):
    # Soft: the antialiased edge keeps the small glyphs' shapes apart.
    import os
    lo = float(os.environ.get("INK_LO", 190))
    hi = float(os.environ.get("INK_HI", 245))
    return np.clip((a.max(axis=2) - lo) / max(hi - lo, 1), 0, 1)


def ncc_map(img, t):
    """Normalised cross-correlation of template t over img (valid area)."""
    from numpy.lib.stride_tricks import sliding_window_view

    th, tw = t.shape
    win = sliding_window_view(img, (th, tw))
    tz = t - t.mean()
    tn = np.sqrt((tz**2).sum())
    wm = win.mean(axis=(2, 3), keepdims=True)
    wz = win - wm
    num = (wz * tz).sum(axis=(2, 3))
    den = np.sqrt((wz**2).sum(axis=(2, 3))) * tn
    return np.where(den > 1e-6, num / np.maximum(den, 1e-6), 0)


def locate(a, tmpl):
    m = ink(a[400:700, 1780:2060])
    r = ncc_map(m, tmpl)
    y, x = np.unravel_index(np.argmax(r), r.shape)
    return 1780 + x, 400 + y, r.max()


if __name__ == "__main__":
    ref = ink(load(REF))
    x0, y0, x1, y1 = MASS_BOX
    tmpl = ref[y0:y1, x0:x1]
    for shot in SHOTS:
        x, y, score = locate(load(shot), tmpl)
        print(shot, "MASS at", x, y, "dx", x - x0, "dy", y - y0, f"ncc {score:.2f}")
