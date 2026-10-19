"""Names: Windows OCR (the reader's own result, run2: OCR only) against
template matching with names rendered in Blender Pro Bold (the HUD's font,
exported from the game data).

The candidate set is every catalog ore name plus INERT MATERIALS. Each
name is rendered once (size and width calibrated on the reference capture)
and slid over the row's name band; the best NCC names the row. Two input
features are tried for the band: brightness ink (what the reader uses)
and raw brightness (no threshold: survives a bright background).
"""
import json
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

import train as T
from panel import load, ncc_map

FONT = "hudata/fonts/28_$Text1Bold_Blender Pro Bold.ttf"
NAMES = [l.split("\t")[1].strip().upper() for l in open("elements.tsv", encoding="utf-8") if l.strip()]
NAMES.append("INERT MATERIALS")
NAME_BAND = (64, -2, 205, 20)
TRUTH = json.load(open("truth.json", encoding="utf-8"))


def render(text, size, stretch=1.0, blur=0.6):
    f = ImageFont.truetype(FONT, size * 4)
    w = int(f.getlength(text)) + 16
    im = Image.new("L", (w, size * 6), 0)
    ImageDraw.Draw(im).text((8, size), text, font=f, fill=255)
    bbox = im.getbbox()
    im = im.crop(bbox)
    tw, th = im.size
    im = im.resize((max(2, round(tw / 4 * stretch)), max(2, round(th / 4))), Image.LANCZOS)
    if blur:
        im = im.filter(ImageFilter.GaussianBlur(blur))
    a = np.asarray(im).astype(float) / 255
    return np.pad(a, 1)


def bands(shot, feature):
    t = TRUTH[shot]
    a = load(shot)
    m = T.ink(a)
    spot = T.refine(m, T.locate_near(m, t["mass_label_box"]))
    img = m if feature == "ink" else a.max(axis=2) / 255.0
    tops = [top for top, bot in T.lines(m, spot, -8, 75, 180, 215)]
    r0 = tops[0] if tops and tops[0] < 205 else 193.0
    out = []
    for i, row in enumerate(t.get("rows", [])):
        dy = r0 - 6 + 22.3 * i
        b = (NAME_BAND[0], NAME_BAND[1] + dy, NAME_BAND[2], NAME_BAND[3] + dy)
        out.append((row.get("element", ""), T.band(img, spot, b)))
    return out


def best_name(band, templates, pad=False):
    scores = {}
    for name, tp in templates.items():
        if pad:
            # names sit left-aligned in their column: pad the template with
            # empty space to the band's width (less a little slack), so ink
            # past a short name's end counts against it
            width = band.shape[1] - 8
            if tp.shape[1] < width:
                tp = np.pad(tp, ((0, 0), (0, width - tp.shape[1])))
        if tp.shape[0] > band.shape[0] or tp.shape[1] > band.shape[1]:
            tp = tp[: band.shape[0], : band.shape[1]]
        scores[name] = float(ncc_map(band, tp).max())
    ranked = sorted(scores.items(), key=lambda kv: -kv[1])
    return ranked[0][0], ranked[0][1] - ranked[1][1]


def calibrate(feature):
    """Size, stretch and blur that best match the reference rows' names."""
    refs = [(n, b) for s in ("04-04-03", "03-51-51", "06-11-46") for n, b in bands(s, feature) if n]
    best = None
    for size in range(14, 22):
        for stretch in (0.9, 1.0, 1.1, 1.2):
            for blur in (0.0, 0.6, 1.0):
                sc = np.mean([ncc_map(b, render(n, size, stretch, blur)[: b.shape[0], : b.shape[1]]).max() for n, b in refs])
                if best is None or sc > best[0]:
                    best = (sc, size, stretch, blur)
    return best


if __name__ == "__main__":
    ocr = json.load(open("run2/readings.json", encoding="utf-8"))
    shots = [s for s in T.usable() + ["06-05-23", "06-05-32", "06-05-38", "06-09-00", "06-08-56"]
             if TRUTH.get(s, {}).get("mode") == "mining" and TRUTH[s].get("mass_label_box")]
    shots = sorted(set(shots))
    norm = lambda s: "".join(c for c in (s or "").upper() if c.isalpha())
    # OCR baseline (the reader's own OCR, before any model fill-in)
    ok = n = 0
    for s in shots:
        v = ocr.get(f"Screenshot 2026-10-17 {s}", {})
        for i, row in enumerate(TRUTH[s].get("rows", [])):
            el = row.get("element", "")
            if not el:
                continue
            rr = v.get("rows", [])[i] if i < len(v.get("rows", [])) else {}
            got = "INERT MATERIALS" if rr.get("inert") else (rr.get("element_name") or "")
            n += 1
            ok += norm(got) == norm(el)
    print(f"Windows OCR:                 {ok}/{n} ({100 * ok // n} %)")
    for feature in ("ink", "raw"):
        cal = calibrate(feature)
        templates = {nm: render(nm, cal[1], cal[2], cal[3]) for nm in NAMES}
        ok = n = 0
        misses = []
        for s in shots:
            for el, b in bands(s, feature):
                if not el:
                    continue
                got, margin = best_name(b, templates)
                n += 1
                if norm(got) == norm(el):
                    ok += 1
                else:
                    misses.append((s, el, got))
        print(f"rendered font, {feature:4} (size {cal[1]}, stretch {cal[2]}, blur {cal[3]}): {ok}/{n} ({100 * ok // n} %)")
        for m in misses:
            print("    miss", *m)
