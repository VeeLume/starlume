//! IPC wiring: the single source-of-truth command list and the TypeScript
//! bindings export.

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::{auth, friends, groups, modules, notify, sc, settings};

/// Single source of truth for the IPC command list. Used both by `run()` at
/// app startup and by the `export-bindings` binary so the TypeScript file can
/// be regenerated without booting the full Tauri app.
pub fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        settings::get_settings,
        settings::update_settings,
        settings::list_grpc_features,
        modules::list_modules,
        notify::recent_notifications,
        sc::sc_status,
        sc::verify_rsi_account,
        auth::auth_status,
        auth::login_start,
        auth::fetch_profile,
        auth::logout,
        friends::list_friends,
        friends::create_friend_invite,
        friends::add_friend,
        friends::remove_friend,
        groups::list_groups,
        groups::create_group,
        groups::create_invite,
        groups::join_group,
        groups::leave_group,
    ])
}

/// Write `src/lib/bindings.ts` from the current Rust command surface.
/// Idempotent. Called from debug-build startup and the `export-bindings` bin.
pub fn export_bindings(out: &str) -> Result<(), specta_typescript::ExportError> {
    ipc_builder().export(Typescript::default(), out)
}
