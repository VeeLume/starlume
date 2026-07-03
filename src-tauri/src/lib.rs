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
//! - [`modules`] — the feature-module registry (empty until the first
//!   carve-out; the trait + enabled-set live here).
//! - [`notify`] — the global notification funnel (toast/center event +
//!   native-toast fallback when the window is hidden).
//! - [`error`] — the shared IPC error type.

mod auth;
mod error;
mod ipc;
mod lifecycle;
mod modules;
pub mod notify;
mod settings;
mod state;

pub use ipc::{export_bindings, ipc_builder};
pub use lifecycle::run;

pub(crate) use state::AppState;
