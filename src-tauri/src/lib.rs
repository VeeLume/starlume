//! Tauri shell for Starlume — the desktop app crate.
//!
//! Thin by design: module wiring plus the public re-exports `main.rs` and the
//! `export-bindings` binary depend on.
//!
//! - [`lifecycle`] — plugin registration, tray, window behavior, `run()`.
//! - [`ipc`] — the single source-of-truth command list + bindings export.
//! - [`state`] — shared app state ([`state::AppState`]).
//! - [`settings`] — app-global preferences (JSON on disk via app-kit).
//! - [`auth`] — device-token auth against the Starlume server (deep-link
//!   callback + Windows Credential Manager storage).
//! - [`data`] — SC game-data commands over `svc-data` (parse/snapshot
//!   status, load, item/resource/manufacturer queries).
//! - [`bus`] — the in-process event bus (tokio broadcast; settled facts).
//! - [`watch`] — install-watcher wiring: svc-discovery's watcher → bus →
//!   shell reactions (cache invalidation, re-warm, notification).
//! - [`modules`] — the feature-module registry (empty until the first
//!   carve-out; the trait + enabled-set live here).
//! - [`notify`] — the global notification funnel (toast/center event +
//!   native-toast fallback when the window is hidden).
//! - [`error`] — the shared IPC error type.

mod auth;
mod bus;
mod data;
mod error;
mod friends;
mod groups;
mod ipc;
mod langpatch;
mod lifecycle;
mod modules;
pub mod notify;
mod sc;
mod settings;
mod state;
mod suspend;
mod watch;

pub use ipc::{export_bindings, ipc_builder};
pub use lifecycle::run;

pub(crate) use state::AppState;
