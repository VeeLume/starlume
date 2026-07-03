# Starlume

**The Star Citizen companion** — one desktop app for the machine-local side of SC tooling:
language patching, blueprint/mission tracking, cargo planning, hangar sync. Feature modules
share one install discovery, one DataCore parse, and one Game.log watcher instead of every
tool reimplementing them.

> Early scaffold. No feature modules are wired yet — this repo currently builds the app
> shell (window, tray, settings, auto-update, deep-link auth plumbing).

## Architecture

Shared services below, feature modules above, an event bus between them:

```
crates/
├── app-kit/        data-dir resolution, atomic IO, JSON persistence
├── server/         starlume-server: Discord auth + device tokens (grows into the v2 API)
├── svc-discovery/  SC install discovery, channel selection, build-change events
├── svc-data/       ONE DataCore parse + snapshot cache per build
├── svc-log/        ONE Game.log tailer broadcasting typed events
└── svc-sync/       server client (device-token auth, outbox, reference upload)
src-tauri/          shell: module registry, tray, settings, auth, updater
src/                SvelteKit frontend (static SPA)
```

### Module rules (do not break)

1. Modules never touch the filesystem / game / network directly — they consume `svc-*`
   services and react to bus events. No module-to-module dependencies.
2. The user picks modules in onboarding; modules not enabled stay invisible. The app must
   stay lightweight for someone who only wants one module.
3. New Star Citizen features land here as modules — not as new apps.

### Adding a module

A module lands in two halves sharing one id:

1. **Rust** — a `crates/mod-<id>` crate, registered in
   `src-tauri/src/modules.rs::registry()`; its commands join the list in
   `src-tauri/src/ipc.rs` (then regenerate bindings).
2. **Frontend** — `src/lib/modules/<id>/` exporting a `ModuleDescriptor`
   (routes under `src/routes/<id>/`, sidebar `nav` entries, optional
   `onboardingSteps`), added to `src/lib/modules/registry.ts`.

The onboarding picker, the sidebar, and the Settings module list all derive
from the registry — no wizard or layout edits needed. Module onboarding steps
appear automatically when the module is selected during setup.

## Build

```sh
pnpm install
just dev                  # dev with hot reload (pnpm tauri dev)
just build                # production build (NSIS installer)

just test                 # Rust tests
just check                # what CI runs: fmt + clippy + svelte-check
just bindings             # regenerate src/lib/bindings.ts after IPC changes
```

`just --list` shows everything (shared recipes come from `common.just`, synced
from the [template](https://github.com/VeeLume/template-rust) via `copier update`).

Prerequisites: Node.js LTS, pnpm, Rust (stable / edition 2024), just, copier.

## Server (Discord sign-in)

The desktop app's sign-in talks to `starlume-server`. To run it locally:

1. `just server-env`, then fill in the Discord application credentials in the
   root `.env` (creation steps + redirect URI are inside the file).
2. `just server` — listens on `http://localhost:5863`.
3. In the app: Settings → Account → Server URL → `http://localhost:5863` →
   Sign in with Discord.

Flow: app opens the browser at `/auth/desktop/start?nonce=…` → server runs
Discord OAuth (`identify` scope) → redirects to
`starlume://auth/callback?nonce&code` → app exchanges the one-time code for a
device token (`POST /auth/desktop/exchange`, token stored in the Windows
Credential Manager) → `GET /api/me` powers the sidebar profile. Tokens are
stored server-side as sha256 hashes, one row per device.

## Testing with two accounts

Community features (friend groups, later sharing) need two signed-in users to
test against each other. Dev profile mode (`STARLUME_PROFILE`, debug builds
only) runs a second instance side by side — own data dir
(`starlume-dev-<profile>`), own keyring slot, no single-instance lock, and a
**loopback auth callback** instead of the deep link (deep links are
machine-global and always land in the first instance).

1. `just server` and `just dev` — server + first instance; sign in normally
   (your main Discord account).
2. `just dev-alt` — second instance under profile `alt`, sharing the first
   instance's vite dev server.
3. In the second instance: Settings → same server URL → Sign in. Instead of
   opening a browser it shows + copies the sign-in URL — **paste it into a
   browser session where the second Discord account is logged in** (private
   window, second browser, or a browser profile). The browser session, not
   the Discord client, decides which account OAuth uses.
4. Both instances are now different users against the same local server —
   create a group in one, mint an invite, join from the other.

## License

MIT
