# CLAUDE.md

Starlume — the unified Star Citizen companion (Tauri 2 + Svelte 5 + Rust workspace).
Consolidation target for Hearth, sc-langpatch, sc-cargo-planner, and sc-fleetsync; those
migrate in as feature modules over time.

## Orientation

- `README.md` — architecture picture + the module rules. Read the rules before adding code.
- Design decisions, rationale, measurements, open work and ideas live in the maintainer's
  Firefly records (project `starlume`), not in this repo. Public design docs come later,
  once things are stable.
- Sibling repos this consolidates (read-only reference): [hearth](https://github.com/VeeLume/hearth)
  (the architecture donor — when in doubt, mirror its patterns),
  [sc-langpatch](https://github.com/VeeLume/sc-langpatch),
  [sc-cargo-planner](https://github.com/VeeLume/sc-cargo-planner), and sc-fleetsync.
  Local checkout paths are in `CLAUDE.local.md`.

## Workspace shape

- `crates/app-kit` — paths, atomic IO, JSON persistence. No Tauri, no tokio, no domain types.
- `crates/svc-*` — shared services. `svc-discovery` (install scan + RSI profiles) and
  `svc-data` (DCB parse + snapshot cache + reference catalogs incl. missions) are live;
  `svc-log` / `svc-sync` are doc-only stubs, filled by carving working code out of
  Hearth, **not** by writing speculative APIs. Reference catalogs are app framework,
  not modules — README module rule 4.
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

## Frontend

The component system is **@veelume/ui** (github.com/VeeLume/veelume-ui,
pinned by tag; ships source — dev builds exclude it from Vite's dependency
pre-bundler). The kit's rulebook (`packages/ui/CLAUDE.md` there) is binding
for what it covers: layers, the coupling contract, surfaces, URL-backed
browse. What stays Starlume's is below.

**Data-lifecycle rules** (SvelteKit re-mounts pages on every navigation; these
keep the app from refetching and re-spinning). Code comments cite them by name.

1. **Stores own data.** Anything that outlives one visit (installs, statuses,
   catalogs, friends, identity) lives in a store under
   `src/lib/state/*.svelte.ts`. Pages hold only ephemeral UI state. A page
   owning a long-lived dataset in local `$state` is a bug. Store shape:
   module-level `$state` + a read-only exported view + exported
   loader/mutator functions (`sc.svelte.ts` is the reference).
2. **Cache-first render.** Loaders return the cache and refresh in the
   background (stale-while-revalidate); pages render cached data
   synchronously. Spinners are for the genuine first load only.
3. **Startup hydration.** The root layout fires every cheap load at mount
   (settings → auth, SC scan, data statuses, default-channel catalog
   prefetch). Heavy work (the DCB cook) runs Rust-side and is visible as the
   `data:progress` banner in `Shell.Content`; a multi-minute parse must
   never read as a hang.
4. **Onboarding reads stores.** Wizard steps render from the store and
   refresh silently; they never gate on a fetch the shell already did.
5. **Backend pushes invalidation.** Rust emits `data:changed` after
   load/wipe/warm; stores also refresh on window focus (a suspended webview
   runs no JS, so focus is the catch-up point).
6. **Disposable frontend.** Stores must be rebuildable from Tauri commands at
   any time; nothing user-durable lives only in JS.
7. **URL-backed browse** (the kit's `createBrowseState`): search, facets, sort
   live in the URL. Expansion is page-local (`createExpansion`), never the URL.

**Styling.** `src/app.css` is the palette's source of truth; `src/theme.css`
bridges it onto the kit's token names. Components use **tokens only, no hex**
(tints via `--accent-fill*` or `color-mix` on `--accent`). Tailwind 4 **with
preflight** is layered; app.css is unlayered and wins, so control styling is
opt-in (`.btn` / `.input` / `.select`) — never add an element-level rule for
anything the kit renders. A pattern on 2+ pages goes to the kit when there is
one right answer (fix in kit, bump — never copy a kit part into the app), to
app.css when it is Starlume idiom, scoped `<style>` only for page-specific
layout. Catalogs are `Surface.Root` → `Surface.List` → `Expand.Row`; the
windowed list, never a pager. Game text goes through `RichText.svelte`, never
`{@html}`. Icons: lucide-svelte for chrome/nav, Unicode glyphs for inline data
marks. Three self-hosted faces (Lekton / IBM Plex Sans / JetBrains Mono) under
`static/fonts/` — **no webfont CDN**, since a runtime font fetch would violate
the online-policy invariant.

## Memory

Resident tray app — idle footprint is a product feature. Rules for caches and
long-lived state (code comments cite them by name):

- **No raw DataCore after the cook.** Parse → cook → drop; the raw pools never
  outlive the build step (enforced by the by-value cook signature).
- **Cooked data is evictable.** Every hide path evicts it beside the WebView2
  suspension; the next query reloads the on-disk snapshot. The disk snapshot
  is the product. (A lease model is deferred until a module must hold data
  while hidden.)
- **Parse only on build change** — never on a timer. `InstallChanged` and
  missing/stale snapshots are the only triggers; watchers are stat-only.
- **Disposable frontend** (frontend rule 6) — a suspended webview runs no JS,
  so anything the UI must show after a hide lives Rust-side (e.g. `NotifLog`).
- **Measure commit size**, not working set, across the app + all
  `msedgewebview2` children.

## Gotchas

- `src-tauri/icons/` are **placeholders copied from Hearth** — replace before any public
  release.
- The auth flow is wired desktop-side but the server doesn't exist yet; `login_start`
  errors politely without a configured server URL. Don't invent server endpoints — the
  contract lands together with `hearth-server`'s successor.
- Frontend dev server runs on port **1445** (Hearth uses 1420 — both apps run side by side
  in dev).

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes_tool` or `query_graph_tool` instead of Grep
- **Understanding impact**: `get_impact_radius_tool` instead of manually tracing imports
- **Code review**: `detect_changes_tool` + `get_review_context_tool` instead of reading entire files
- **Finding relationships**: `query_graph_tool` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview_tool` + `list_communities_tool`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
| ------ | ---------- |
| `detect_changes_tool` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context_tool` | Need source snippets for review — token-efficient |
| `get_impact_radius_tool` | Understanding blast radius of a change |
| `get_affected_flows_tool` | Finding which execution paths are impacted |
| `query_graph_tool` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes_tool` | Finding functions/classes by name or keyword |
| `get_architecture_overview_tool` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes_tool` for code review.
3. Use `get_affected_flows_tool` to understand impact.
4. Use `query_graph_tool` pattern="tests_for" to check coverage.
