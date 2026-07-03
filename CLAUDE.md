# CLAUDE.md

Starlume — the unified Star Citizen companion (Tauri 2 + Svelte 5 + Rust workspace).
Consolidation target for Hearth, sc-langpatch, sc-cargo-planner, and sc-fleetsync; those
migrate in as feature modules over time.

## Orientation

- `README.md` — architecture picture + the module rules. Read the rules before adding code.
- The design doc (target picture, per-project disposition, migration order, open decisions)
  lives in the maintainer's vault, not in this repo. `CLAUDE.local.md` (gitignored) carries
  the pointer on the maintainer's machine.
- Sibling repos this consolidates (read-only reference): [hearth](https://github.com/VeeLume/hearth)
  (the architecture donor — when in doubt, mirror its patterns),
  [sc-langpatch](https://github.com/VeeLume/sc-langpatch),
  [sc-cargo-planner](https://github.com/VeeLume/sc-cargo-planner), and sc-fleetsync.
  Local checkout paths are in `CLAUDE.local.md`.

## Workspace shape

- `crates/app-kit` — paths, atomic IO, JSON persistence. No Tauri, no tokio, no domain types.
- `crates/svc-*` — shared services (install / data / log / sync). Currently doc-only stubs;
  they get filled by carving working code out of Hearth, **not** by writing speculative APIs.
- `src-tauri` — the shell: lifecycle (plugins, tray, windows), settings, auth, module
  registry, typed IPC.
- `src` — SvelteKit static SPA (adapter-static, SSR off).

## Conventions

- **Typed IPC via tauri-specta.** Every command is listed once in `src-tauri/src/ipc.rs`;
  `src/lib/bindings.ts` is generated (debug startup regenerates it, or
  `cargo run --bin export-bindings --features bindgen`). Never hand-edit bindings.ts.
- **Plugin order in `lifecycle.rs` is load-bearing** — single-instance first; its
  `deep-link` feature forwards second-instance `starlume://` URLs to `on_open_url`.
- **Settings** are one JSON snapshot (`AppSettings`, `#[serde(default)]`) under
  `app_kit::app_data_root()`. Add fields, never rename/repurpose them. Side effects of a
  settings change (autostart registration) happen in `update_settings`.
- **Online policy gates — INVARIANT.** Every outbound network call, anywhere in the app
  (shell, `svc-*`, modules, frontend), passes a gate first: `AppState::require_online()`
  for any network call (Discord, RSI, server), and `AppState::require_grpc("<feature>")`
  for CIG game-services calls (ToS-grey; master opt-in + per-feature allow-list in
  `AppSettings::grpc_features`, feature ids registered in `settings::GRPC_FEATURES`).
  New network code without a gate call is a bug, not a style issue. Offline trumps
  everything: gRPC settings are preserved but inert while online is off.
  **Sole exception: update checks** (`src/lib/updater.ts`) — a legitimate app function
  with no ToS implications; offline-mode users still get (security-)fixed builds. Do not
  add further exceptions without the same level of justification, documented here.
- **Secrets** (the device token) go in the Windows Credential Manager via `keyring`,
  never in JSON.
- **Data dirs are build-namespaced**: debug → `%APPDATA%\starlume-dev`, release →
  `%APPDATA%\starlume`, `STARLUME_DATA_DIR` overrides — dev experiments must never touch
  release data.
- **Module rules** are in the README; treat them as invariants, not guidelines.
- Updater signing: private key is a GitHub secret (`TAURI_SIGNING_PRIVATE_KEY`), only the
  pubkey lives in `tauri.conf.json`. Never commit `*.key`.

## Memory

Resident tray app — idle footprint is a product feature. The strategy (svc-data
lifecycle/eviction rules, WebView2 suspension, disposable-frontend prerequisite for
window destroy/recreate, honest measurement) lives in [docs/memory.md](docs/memory.md).
Read it before adding caches or long-lived state.

## Gotchas

- `src-tauri/icons/` are **placeholders copied from Hearth** — replace before any public
  release.
- The auth flow is wired desktop-side but the server doesn't exist yet; `login_start`
  errors politely without a configured server URL. Don't invent server endpoints — the
  contract lands together with `hearth-server`'s successor (see the design doc).
- Frontend dev server runs on port **1430** (Hearth uses 1420 — both apps run side by side
  in dev).
