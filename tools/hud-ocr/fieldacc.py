"""Per-field accuracy of reader runs against truth.json.
usage: fieldacc.py <run dir>..."""
import json
import sys

t = json.load(open("truth.json", encoding="utf-8"))
for run in sys.argv[1:]:
    r = json.load(open(f"{run}/readings.json", encoding="utf-8"))
    stats = {}
    wrong_fixes = []

    def add(k, ok):
        a = stats.setdefault(k, [0, 0])
        a[0] += ok
        a[1] += 1

    for key, v in r.items():
        s = key[22:]
        tr = t.get(s, {})
        if tr.get("mode") != "mining" or not tr.get("mass_label_box") or not v.get("panel"):
            continue
        for f, tk, fmt in (("mass", "mass", "{:.0f}"), ("resistance", "resist", "{:.0f}%"), ("instability", "instab", "{:.2f}")):
            lab = tr.get(tk)
            if lab and "?" not in lab:
                ok = v.get(f) is not None and fmt.format(v[f]) == lab
                add(f, ok)
                if not ok and f not in v.get("unsure", []):
                    add("wrong but not flagged", 1)
                else:
                    add("wrong but not flagged", 0)
        for i, row in enumerate(tr.get("rows", [])):
            rr = v["rows"][i] if i < len(v["rows"]) else {}
            p = row.get("pct")
            if p and "?" not in p:
                add("row %", rr.get("percent") is not None and f"{rr['percent']:.2f}%" == p)
            q = row.get("quality")
            if q and "?" not in q and "INERT" not in row.get("element", ""):
                add("quality", rr.get("quality") is not None and str(rr["quality"]) == q)
            el = row.get("element", "")
            if el:
                got = "INERT MATERIALS" if rr.get("inert") else (rr.get("element_name") or "")
                add("ore", "".join(c for c in got.upper() if c.isalpha()) == "".join(c for c in el if c.isalpha()))
        for n in v.get("notes", []):
            if "corrected" in n:
                wrong_fixes.append((s, n))
    print(run, {k: f"{a}/{b} ({100 * a // max(b, 1)}%)" for k, (a, b) in stats.items()})
    for s, n in wrong_fixes:
        print("   fix", s, n)
