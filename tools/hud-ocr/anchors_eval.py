"""Per-anchor detection: for each small static panel part, search the
whole frame and check whether its best match lands where it should (given
the labelled MASS box). Shows which parts survive glare.
"""
import numpy as np

from locate_eval import TRUTH, feats, half, load, ncc_full, resize

REF = "04-04-03"
MASS = (1875, 544)
ANCHORS = {
    "header": (1876, 468, 2082, 494),
    "block": (1875, 544, 1950, 618),
    "comp": (1874, 674, 2062, 698),
    "bar": (1868, 698, 2166, 725),
}

if __name__ == "__main__":
    ref = feats(load(REF))
    scales = np.arange(0.88, 1.13, 0.03)
    panel = [s for s in sorted(TRUTH) if TRUTH[s].get("mode") == "mining" and TRUTH[s].get("mass_label_box")]
    hits = {(a, k): 0 for a in ANCHORS for k in ("abs", "blackhat", "fused")}
    for s in panel:
        f = feats(load(s))
        bx, by = TRUTH[s]["mass_label_box"][:2]
        line = []
        for a, (x0, y0, x1, y1) in ANCHORS.items():
            dx, dy = x0 - MASS[0], y0 - MASS[1]
            ms = {}
            for k in ("abs", "blackhat"):
                img = half(f[k])
                ms[k] = [ncc_full(img, resize(ref[k][y0:y1, x0:x1], sc / 2)) for sc in scales]
            ms["fused"] = [(p + q) / 2 for p, q in zip(ms["abs"], ms["blackhat"])]
            cell = []
            for k, rs in ms.items():
                best = (-2, 0, 0, 1)
                for sc, r in zip(scales, rs):
                    y, x = np.unravel_index(np.argmax(r), r.shape)
                    if r[y, x] > best[0]:
                        best = (float(r[y, x]), x * 2, y * 2, sc)
                ex, ey = bx + dx * best[3], by + dy * best[3]
                hit = abs(best[1] - ex) <= 14 and abs(best[2] - ey) <= 14
                hits[(a, k)] += hit
                cell.append(("+" if hit else "-") + f"{best[0]:.2f}")
            line.append(f"{a}:{'/'.join(cell)}")
        print(s, "  ".join(line), flush=True)
    print()
    for a in ANCHORS:
        print(a, {k: f"{hits[(a, k)]}/{len(panel)}" for k in ("abs", "blackhat", "fused")})
