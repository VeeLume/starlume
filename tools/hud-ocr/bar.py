"""Charge-arc geometry on the Golem HUD, after the user's description:

    [window bar, fills right to left] | slim separator | charge track | (laser power bar)

The HUD is projected, so its geometry differs between recordings (head
position). The separator is traced once per recording on a clean frame
(sep.py -> sep.npz); within a recording it only sways a few px, so each
frame is registered by the shift (dx, dy) that puts the most warm line
pixels under the path's top part (always over the overcrack window). The
charge is the highest point of the bright fill in the track right of the
separator, climbing from the bottom end and bridging the short gaps where
window edges cross the bar. Positions are shares of arc length from the
bottom end."""
import numpy as np


def score(f):
    r, g, b = f[..., 0], f[..., 1], f[..., 2]
    return np.where((r > 150) & (r - b > 60), r + g, 0)


def bright(px):
    r, g, b = px[..., 0], px[..., 1], px[..., 2]
    # The fill is yellow (g >= r); the separator's cream (r above g, b high)
    # must not count. Inside the window the fill pulses dimmer, but stays
    # saturated (low b), unlike the window's tint.
    yellow = g >= r - 8
    lit = (r > 180) & (g > 180) & (b < 150)
    dim = (r > 140) & (g > 140) & (b < 0.3 * g)
    return yellow & (lit | dim)


class Arc:
    def __init__(self, npz):
        s = np.load(npz)
        self.top, self.bottom = int(s["top"]), int(s["bottom"])
        self.ys = np.arange(self.bottom, self.top - 1, -1.0)
        self.xs = np.polyval(s["p"], self.ys)
        self.along = np.r_[0, np.cumsum(np.hypot(np.diff(self.xs), np.diff(self.ys)))]
        self.total = self.along[-1]

    def register(self, f):
        s = score(f)
        sel = (self.ys >= self.top + 6) & (self.ys <= self.top + 140)
        best = (-1, 0, 0)
        for dy in range(-8, 9):
            ys = (self.ys[sel] + dy).astype(int)
            for dx in range(-14, 15):
                v = s[ys, np.round(self.xs[sel] + dx).astype(int)].sum()
                if v > best[0]:
                    best = (v, dx, dy)
        return best[1], best[2]

    def share_at_y(self, y, dy=0):
        return self.along[int(np.argmin(abs(self.ys + dy - y)))] / self.total

    def charge(self, f, near=(3, 20), need=3, gap=20):
        """(share, y, x, dx, dy) of the charge top; y None when empty."""
        dx, dy = self.register(f)
        last = None
        for i, (x, y) in enumerate(zip(self.xs + dx, self.ys + dy)):
            xi = np.arange(int(round(x)) + near[0], int(round(x)) + near[1])
            if bright(f[int(y), xi]).sum() >= need:
                if last is None and self.along[i] > 25:
                    break
                if last is not None and self.along[i] - self.along[last] > gap:
                    break
                last = i
        if last is None:
            return 0.0, None, None, dx, dy
        return self.along[last] / self.total, self.ys[last] + dy, self.xs[last] + dx, dx, dy
