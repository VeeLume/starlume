"""Annotation sheets for checking the scan-panel reader.

One PNG per screenshot: the panel as the reader found it (zoomed, with the
found label block outlined) beside a table of every field: what the app
read, the hand label (truth.json) and whether they agree.

  green  read == label
  red    read != label (or nothing read)
  grey   no label to compare (unreadable "?" or not labelled)

usage: annotate_scan.py <readings.json> <truth.json> <shots dir> <out dir>
"""
import json
import os
import sys

from PIL import Image, ImageDraw, ImageFont

readings = json.load(open(sys.argv[1], encoding="utf-8"))
truth = json.load(open(sys.argv[2], encoding="utf-8"))
shots, out = sys.argv[3], sys.argv[4]
os.makedirs(out, exist_ok=True)

F = ImageFont.truetype("arial.ttf", 22)
FB = ImageFont.truetype("arialbd.ttf", 24)
FS = ImageFont.truetype("arial.ttf", 18)
GREEN, RED, GREY, WHITE = (90, 210, 110), (240, 90, 80), (150, 150, 150), (235, 235, 235)


def fmt_num(v, kind):
    if v is None:
        return "—"
    if kind == "int":
        return str(int(round(v)))
    if kind == "pct0":
        return f"{v:.0f}%"
    if kind == "dec":
        return f"{v:.2f}"
    if kind == "pct2":
        return f"{v:.2f}%"
    return str(v)


def norm(s):
    return None if s is None else s.replace(",", "").strip().upper()


def verdict(read, label):
    if label is None or "?" in str(label):
        return GREY
    return GREEN if read is not None and norm(read) == norm(label) else RED


def name_match(read_name, label):
    if label is None:
        return GREY
    if read_name is None:
        return RED
    a = "".join(c for c in read_name.upper() if c.isalpha())
    b = "".join(c for c in label.upper() if c.isalpha())
    return GREEN if a == b else RED


summary = []
for key in sorted(readings):
    shot = key.replace("Screenshot 2026-10-17 ", "")
    r = readings[key]
    t = truth.get(shot, {})
    img = Image.open(os.path.join(shots, key + ".png")).convert("RGB")
    rect = r.get("panel_png") or r.get("panel_rect")
    if r.get("panel_png"):
        crop = Image.open(r["panel_png"]).convert("RGB")
        zoom = 820 / max(crop.height, 1)
        crop = crop.resize((int(crop.width * zoom), 820), Image.LANCZOS)
    elif rect:
        x, y, w, h = rect
        crop = img.crop((x, y, x + w, y + h))
        zoom = 820 / max(h, 1)
        crop = crop.resize((int(w * zoom), 820), Image.LANCZOS)
    elif t.get("signature_box"):
        # Scan mode: the signature readout at 4x, never a shrunk frame.
        x0, y0, x1, y1 = t["signature_box"]
        crop = img.crop((x0 - 80, y0 - 40, x1 + 80, y1 + 40))
        crop = crop.resize((crop.width * 4, crop.height * 4), Image.NEAREST)
    else:
        # No panel and no readout labelled: the right-hand HUD area at
        # native resolution (where the panel would be).
        crop = img.crop((1700, 380, 2300, 1000))
    rows_out = []  # (label, read, truth, colour)
    if not rect and t.get("mode") == "scan":
        rows_out.append(("signature", "(not read yet)", t.get("signature"), GREY))
    if rect:
        rows_out.append(("rock", r.get("rock_name") or "—", t.get("rock"), name_match(r.get("rock_name"), t.get("rock"))))
        for field, kind, tk in (("mass", "int", "mass"), ("resistance", "pct0", "resist"), ("instability", "dec", "instab")):
            v = fmt_num(r.get(field), kind) if r.get(field) is not None else None
            rows_out.append((field, v or "—", t.get(tk), verdict(v, t.get(tk))))
        basis = r.get("basis")
        beam = None if basis is None else basis == "beam"
        tb = t.get("beam")
        rows_out.append(("values", basis or "-", None if tb is None else ("beam" if tb else "bare"),
                         GREY if tb is None else (GREEN if beam == tb else RED)))
        rows_out.append(("difficulty", r.get("difficulty") or "—", t.get("difficulty"),
                         name_match(r.get("difficulty"), t.get("difficulty"))))
        trows = t.get("rows", [])
        for i in range(max(len(r.get("rows", [])), len(trows))):
            rr = r["rows"][i] if i < len(r.get("rows", [])) else {}
            tr = trows[i] if i < len(trows) else {}
            p = fmt_num(rr.get("percent"), "pct2") if rr.get("percent") is not None else None
            rows_out.append((f"row {i + 1} %", p or "—", tr.get("pct"), verdict(p, tr.get("pct"))))
            nm = rr.get("element_name") or ("INERT MATERIALS" if rr.get("inert") else rr.get("name"))
            rows_out.append((f"row {i + 1} ore", nm or "—", tr.get("element"), name_match(nm, tr.get("element"))))
            if not rr.get("inert") and "INERT" not in (tr.get("element") or ""):
                q = str(rr["quality"]) if rr.get("quality") is not None else None
                rows_out.append((f"row {i + 1} quality", q or "—", tr.get("quality"), verdict(q, tr.get("quality"))))
    good = sum(c == GREEN for *_, c in rows_out)
    bad = sum(c == RED for *_, c in rows_out)
    summary.append((shot, good, bad, t.get("mode"), r.get("panel")))

    W = crop.width + 760
    H = max(crop.height, 160 + 34 * len(rows_out) + 30 * (len(r.get("notes", [])) + len(r.get("verified", [])) + 1)) + 20
    can = Image.new("RGB", (W, H), (24, 24, 28))
    can.paste(crop, (0, 0))
    d = ImageDraw.Draw(can)
    ox = crop.width + 24
    spot = r.get("panel")
    head = f"{shot}   mode: {t.get('mode', '?')}"
    d.text((ox, 12), head, font=FB, fill=WHITE)
    sub = (f"panel: {spot['parts']} parts agree, score {spot['score']:.2f}, scale {spot['scale']:.3f}, tilt {spot['angle']:.0f} deg" if spot else "no panel found") + \
          f"   {r.get('ms', 0)} ms   right {good}, wrong {bad}"
    d.text((ox, 44), sub, font=FS, fill=GREY)
    cond = ", ".join(t.get("conditions", []))
    if cond:
        d.text((ox, 68), cond, font=FS, fill=GREY)
    y = 104
    d.text((ox, y), "field", font=FS, fill=GREY)
    d.text((ox + 190, y), "app read", font=FS, fill=GREY)
    d.text((ox + 460, y), "label", font=FS, fill=GREY)
    y += 28
    for name, read, lab, col in rows_out:
        d.text((ox, y), name, font=F, fill=WHITE)
        d.text((ox + 190, y), str(read), font=F, fill=col)
        d.text((ox + 460, y), "—" if lab is None else str(lab), font=F, fill=GREY)
        y += 34
    for v in r.get("verified", []):
        d.text((ox, y + 6), "checked: " + v, font=FS, fill=GREEN)
        y += 30
    if r.get("unsure"):
        d.text((ox, y + 6), "unsure: " + ", ".join(r["unsure"]), font=FS, fill=(230, 190, 90))
        y += 30
    for n in r.get("notes", []):
        d.text((ox, y + 6), "note: " + n, font=FS, fill=(230, 190, 90))
        y += 30
    can.save(os.path.join(out, f"{shot}.png"))

with open(os.path.join(out, "summary.txt"), "w", encoding="utf-8") as f:
    for shot, good, bad, mode, spot in summary:
        f.write(f"{shot}  {mode or '-':7}  right {good:2}  wrong {bad:2}  {'found' if spot else 'no panel'}\n")
    tg = sum(s[1] for s in summary)
    tb = sum(s[2] for s in summary)
    f.write(f"\nfields right {tg}, wrong {tb}\n")
print(open(os.path.join(out, "summary.txt"), encoding="utf-8").read())
