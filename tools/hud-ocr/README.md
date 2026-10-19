# hud-ocr — prototype tooling for the mining HUD reader

Python (numpy, scipy, Pillow) used to develop `crates/mod-mining/src/hud` and to
generate its learned data, `crates/mod-mining/src/hud/scan.bin`. Not shipped.

Run every script **from the data folder** (`D:\Recordings\Gaming\starlume-ocr`, kept
out of the repo: it holds the labelled captures' truth, the game's extracted HUD
fonts — commercial, never commit them — and per-recording calibrations). Python puts
this folder on the import path, so `python <repo>\tools\hud-ocr\<script>.py` works.
The 2026-10-17 screenshots and recordings are read from `D:\Recordings\Gaming`.

## Regenerate scan.bin

```bash
cd /d/Recordings/Gaming/starlume-ocr
INK_LO=238 INK_HI=252 python /e/vscode/rust/starlume/tools/hud-ocr/genbin.py
```

Anchor bank (reference + glare captures), mean glyph templates and digit exemplars,
learned from `truth.json`. `INK_LO/INK_HI` must match the Rust reader's brightness ink.

## Measure the reader

```bash
STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
  cargo run -p mod-mining --release --example scan_read -- "D:\Recordings\Gaming" readings/runN.json
python annotate_scan.py readings/runN.json truth.json "D:/Recordings/Gaming" sheets/runN
python fieldacc.py <folder holding readings.json>
```

## The scripts

| Script | What |
|---|---|
| `panel.py`, `digits.py`, `glyphs.py`, `train.py` | ink, panel bands, template learning, cross-validated digit decoding |
| `knn.py`, `nn_decode.py`, `classify_exp.py`, `holes.py` | digit classification experiments (nearest example wins) |
| `locate_eval.py`, `anchors_eval.py`, `vote.py`, `vote_bank.py` | panel locating: features, anchors, voting, the glare template bank |
| `contrast.py` | brightness vs shadow (black-hat) vs top-hat ink, side by side |
| `names_cmp.py` | names: Windows OCR vs the HUD font rendered and matched |
| `genbin.py` | writes `scan.bin` |
| `annotate_scan.py`, `fieldacc.py` | review sheets and per-field accuracy against `truth.json` |
| `sep.py`, `bar.py` | charge bar: separator calibration per recording, charge reading |
| `series2.py`, `plot2.py` | charge + throttle readout over a recording (player behaviour) |
| `overlay.py`, `grid.py`, `strips.py` | debug views |
