# Memory strategy

Starlume is a resident companion — it sits in the tray for hours while the user plays.
Its idle footprint is a product feature: nobody keeps a helper app that eats 400 MB while
hidden. This doc is the plan for keeping that footprint small, ordered by leverage.
Status per lever: **implemented** / **planned** / **rejected**.

## Where the memory actually is

1. **WebView2** — the dominant idle cost. An Edge WebView with a loaded SPA holds roughly
   100–200 MB across its helper processes, even hidden. The Rust shell itself is a few MB.
2. **SC data (future, svc-data)** — the DataCore parse is the real spike: hundreds of MB
   transiently during a full parse (Hearth/bulkhead experience: ~30 s, deep recursion,
   bulkhead runs it on a 32 MB stack). The *cooked* indices (HolotableSnapshot) are far
   smaller (~12 MB on disk in SC 4.8) and reload in under a second.
3. **Module state** — bounded by design (session-memory stores, capped lists).

## Levers, by leverage

### 1. svc-data lifecycle policy (planned — the big one for us)

The rule that keeps the spike from becoming resident cost:

- **Never hold the raw DataCore after cooking.** Parse → build cooked indices → drop the
  parse. The raw bytes/pools must not outlive the build step.
- **Cooked indices are evictable.** `ProcessedSnapshot` loads are sub-second, so svc-data
  can drop its in-memory indices when idle (window hidden + no module holding a lease)
  and reload on demand. Lease model: modules `acquire()` the data they need; svc-data
  evicts when the lease count is zero for N minutes.
- **Parse spikes only on build change**, never on a timer — `InstallChanged` is the only
  trigger, and the snapshot cache makes re-launches cheap.

### 2. WebView suspension when hidden (planned)

WebView2 supports suspension (`CoreWebView2.TrySuspendAsync`) — the renderer pauses
timers/JS and Windows can reclaim most of its working set. Requirements: window hidden
(true in tray mode) + resume on show. Tauri doesn't expose this directly; it's reachable
via `WebviewWindow::with_webview` + the `webview2-com` controller on Windows. Wire it to
the same hide/show paths as the tray (`show_main_window` resumes). Verify: suspended
webviews still receive `emit`ed events only after resume — notifications raised while
suspended must land in the store on resume (the backend native-toast fallback already
covers user visibility while hidden).

### 3. Destroy & recreate the window (planned, further out — biggest webview win)

Closing the webview window entirely (not hiding) releases the whole WebView2 tree;
recreate it from config on tray → Show. Prerequisite that must become an architecture
rule before adopting this: **the frontend must be disposable** — every store hydrates
from the backend on mount, nothing user-visible lives only in JS. Current violation: the
notification center's session log lives in the frontend store; it would need a small
Rust-side ring buffer the store hydrates from. Adopt lever 2 first; only reach for this
if suspension's savings disappoint.

### 4. Browser arguments (evaluate)

`app.windows[].additionalBrowserArgs` in `tauri.conf.json` can pass WebView2/Chromium
flags. Candidates to measure (not adopt blindly): `--disable-gpu` for a tray-mostly app
(saves the GPU process, costs rendering smoothness), renderer process model flags.
Measure before shipping — some flags regress startup or break the updater dialogs.

### 5. Rejected / non-levers

- **`EmptyWorkingSet` / working-set trimming tricks** — makes Task Manager numbers look
  good without freeing commit; the memory pages right back in. Optics, not savings.
- **Replacing the WebView with native UI** — that's a different product (bulkhead exists).

## Measuring honestly

- Use **commit size** (Task Manager → Details → "Commit size" column, or Process
  Explorer's Private Bytes) across *all* Starlume + `msedgewebview2` child processes —
  "Memory" in the default view is working set and undercounts/overcounts wildly.
- Baseline to record when levers land: idle-visible, idle-hidden, during a DCB parse,
  after eviction. Keep the numbers in this doc.

## Current state (2026-07-03)

- Shell only, no SC data yet — footprint is essentially the WebView2 baseline.
- Implemented today: hide-to-tray on close (default) and on minimize (opt-in setting) —
  hiding is the *prerequisite* for levers 2/3, not a saving in itself (a hidden WebView
  keeps its memory until suspended/destroyed).
