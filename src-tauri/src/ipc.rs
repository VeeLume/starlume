//! IPC wiring: the single source-of-truth command list and the TypeScript
//! bindings export.

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::{
    activity, auth, data, devtracker, dossier, friends, groups, items, langpatch, modules, notify,
    sc, settings, window,
};

/// Single source of truth for the IPC command list. Used both by `run()` at
/// app startup and by the `export-bindings` binary so the TypeScript file can
/// be regenerated without booting the full Tauri app.
pub fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        settings::get_settings,
        settings::update_settings,
        settings::list_grpc_features,
        window::take_restore_route,
        window::update_check_due,
        dossier::blueprints_owned,
        dossier::blueprints_refresh,
        modules::list_modules,
        notify::recent_notifications,
        activity::current_activities,
        sc::sc_status,
        sc::verify_rsi_account,
        data::data_status,
        data::data_load,
        data::data_wipe,
        data::data_builds,
        data::data_delete_build,
        data::data_search_items,
        data::data_items_all,
        data::data_item_detail,
        data::data_resources,
        data::data_manufacturers,
        data::data_item_types,
        data::data_missions,
        langpatch::langpatch_overview,
        langpatch::langpatch_update_config,
        langpatch::langpatch_apply,
        langpatch::langpatch_remove,
        langpatch::langpatch_preview_sample,
        items::items_summary,
        items::items_refresh,
        items::items_finder,
        devtracker::devtracker_overview,
        devtracker::devtracker_authors,
        devtracker::devtracker_set_author_mode,
        devtracker::devtracker_reset_authors,
        devtracker::devtracker_set_notify_ops,
        devtracker::devtracker_mark_read,
        devtracker::devtracker_mark_all_read,
        devtracker::devtracker_open_post,
        devtracker::devtracker_refresh,
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
