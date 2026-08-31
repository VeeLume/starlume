//! langpatch module — shell orchestration + IPC surface over the
//! `mod-langpatch` engine.
//!
//! The module maintains an invariant per selected install (the scoping's
//! reframe): *the `global.ini` override on disk corresponds to (current
//! build, current patcher config, current language pack) — or it isn't
//! there at all.* This file is the machinery that restores the invariant
//! when an input changes:
//!
//! - **Triggers:** app startup, `InstallChanged` from the bus (both routed
//!   through [`spawn_warm_then_reconcile`] so the reconcile runs *after*
//!   svc-data's warm — one parse serves the catalogs and the derive), and
//!   config changes from the UI.
//! - **Write gates:** never while StarCitizen.exe runs (defers to a
//!   process-exit waiter); foreign files (SC Deutsch Launcher, manual
//!   edits) pause auto-patching with a "take over" action instead of a
//!   last-writer-wins war.
//! - **Degraded fallback:** if a re-patch can't complete and the override
//!   on disk is from an *older build*, the override comes off — vanilla
//!   beats an entire stale localization.
//!
//! Network: the community language pack is the module's ONLY network
//! surface. Its fetch passes `require_online()` and caches to disk, so
//! offline users keep re-patching with the last fetched copy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use mod_langpatch::{
    Fingerprint, LangpatchConfig, PatchPlan, PatchStateFile, PatcherConfig, builtin_patchers,
    cache_complete, derive_ops, merge, plan_for, sha256_bytes, sha256_file,
};
use svc_data::InstallRef;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::notify::{Notification, notify};
use crate::{AppState, data};

/// Fired after any reconcile/apply/remove — the frontend store refreshes
/// the overview on this. Payload-free.
pub const LANGPATCH_CHANGED_EVENT: &str = "langpatch:changed";

fn emit_changed(app: &AppHandle) {
    let _ = app.emit(LANGPATCH_CHANGED_EVENT, ());
}

fn langpatch_dir() -> PathBuf {
    app_kit::app_data_root().join("langpatch")
}

fn config_path() -> PathBuf {
    langpatch_dir().join("config.json")
}

fn load_config() -> LangpatchConfig {
    app_kit::load_json(&config_path())
}

fn save_config(config: &LangpatchConfig) -> Result<(), AppError> {
    app_kit::save_json(&config_path(), config)
        .map_err(|e| AppError::Internal(format!("saving langpatch config: {e}")))
}

fn module_enabled(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .enabled_modules
        .iter()
        .any(|m| m == "langpatch")
}

/// Per-channel user text overrides (the phase-2 editor's data; the LAYER
/// ships now — a hand-written file already works).
fn load_overrides(channel_key: &str) -> std::collections::BTreeMap<String, String> {
    app_kit::load_json(&langpatch_dir().join(format!("overrides-{channel_key}.json")))
}

// ── Game-running gate ───────────────────────────────────────────────────────

/// Is StarCitizen.exe running? Blocking (~10ms process enumeration) — call
/// via `spawn_blocking`.
fn sc_running_blocking() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes()
        .values()
        .any(|p| p.name().eq_ignore_ascii_case("StarCitizen.exe"))
}

async fn sc_running() -> bool {
    tokio::task::spawn_blocking(sc_running_blocking)
        .await
        .unwrap_or(false)
}

/// One process-exit waiter at a time: when a reconcile is blocked by a
/// running game, poll for exit and re-run once.
static EXIT_WAITER: AtomicBool = AtomicBool::new(false);

fn spawn_exit_waiter(app: &AppHandle) {
    if EXIT_WAITER.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // 30s poll, capped at 12h — the next trigger covers the rest.
        for _ in 0..1440 {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if !sc_running().await {
                break;
            }
        }
        EXIT_WAITER.store(false, Ordering::SeqCst);
        tracing::info!("SC exited — running deferred langpatch reconcile");
        reconcile_all(&app).await;
    });
}

// ── Language pack ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct PackMeta {
    source: String,
    sha256: String,
}

/// Resolve the configured language pack to (lines, content-hash).
///
/// URLs fetch behind `require_online()` and cache to
/// `langpatch/pack.ini` (+ meta); on fetch failure or offline mode the
/// cached copy serves as long as it came from the same source. Local paths
/// read directly. `None` (with a warning notification where it matters) →
/// patch proceeds without an overlay, matching sc-langpatch.
async fn resolve_pack(app: &AppHandle, config: &LangpatchConfig) -> Option<(Vec<String>, String)> {
    let source = config.language_pack.as_deref()?.trim().to_string();
    if source.is_empty() {
        return None;
    }

    let bytes = if source.starts_with("http://") || source.starts_with("https://") {
        fetch_or_cached_pack(app, &source).await
    } else {
        match std::fs::read(&source) {
            Ok(b) => Some(b),
            Err(e) => {
                warn_once(app, format!("Language pack unreadable: {e}"));
                None
            }
        }
    }?;

    match merge::decode_ini(&bytes) {
        Ok(lines) => Some((lines, sha256_bytes(&bytes))),
        Err(e) => {
            warn_once(app, format!("Language pack not decodable as INI: {e}"));
            None
        }
    }
}

async fn fetch_or_cached_pack(app: &AppHandle, url: &str) -> Option<Vec<u8>> {
    let cache_file = langpatch_dir().join("pack.ini");
    let meta_path = langpatch_dir().join("pack.meta.json");
    let cached = || -> Option<Vec<u8>> {
        let meta: PackMeta = app_kit::load_json(&meta_path);
        (meta.source == url)
            .then(|| std::fs::read(&cache_file).ok())
            .flatten()
    };

    // Online gate — INVARIANT (CLAUDE.md): offline means the cached copy
    // or nothing.
    if app.state::<AppState>().require_online().is_err() {
        return cached();
    }

    let fetch_url = rewrite_github_url(url);
    let response = async {
        let resp = reqwest::Client::new()
            .get(&fetch_url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;
        resp.bytes().await
    }
    .await;

    match response {
        Ok(bytes) => {
            let bytes = bytes.to_vec();
            let meta = PackMeta {
                source: url.to_string(),
                sha256: sha256_bytes(&bytes),
            };
            let _ = app_kit::atomic_write(&cache_file, &bytes);
            let _ = app_kit::save_json(&meta_path, &meta);
            Some(bytes)
        }
        Err(e) => {
            warn_once(
                app,
                format!("Language pack fetch failed ({e}); using cached copy if available"),
            );
            cached()
        }
    }
}

/// `github.com/u/r/blob/branch/file` → `raw.githubusercontent.com/u/r/branch/file`
/// (the sc-langpatch convenience — users paste the browser URL).
fn rewrite_github_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://github.com/")
        && rest.contains("/blob/")
    {
        return format!(
            "https://raw.githubusercontent.com/{}",
            rest.replacen("/blob/", "/", 1)
        );
    }
    url.to_string()
}

fn warn_once(app: &AppHandle, body: String) {
    // Session notification log dedupes visually; keep it simple here.
    notify(
        app,
        Notification::warning("Text patching")
            .with_body(body)
            .with_source("langpatch"),
    );
}

// ── Reconciliation ──────────────────────────────────────────────────────────

/// Startup / post-`InstallChanged` sequencing: svc-data warm first (one
/// parse serves catalogs *and* derive), then the langpatch reconcile.
pub fn spawn_warm_then_reconcile(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        data::run_startup_warm(&app).await;
        reconcile_all(&app).await;
    });
}

/// Reconcile every selected install (auto path — respects the module
/// enabled-set and the `auto_patch` switch; manual commands don't).
pub async fn reconcile_all(app: &AppHandle) {
    if !module_enabled(app) {
        return;
    }
    let config = load_config();
    if !config.auto_patch {
        return;
    }
    let installs = match data::refresh_installs(app).await {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("langpatch reconcile: install scan failed: {e}");
            return;
        }
    };
    for install in installs {
        if !config.channels.contains(&install.channel_key) {
            continue;
        }
        match reconcile_one(app, &config, &install, false).await {
            Ok(outcome) => tracing::info!(
                channel = %install.channel_key,
                ?outcome,
                "langpatch reconcile"
            ),
            Err(e) => {
                tracing::warn!(channel = %install.channel_key, "langpatch reconcile failed: {e:#}");
                notify(
                    app,
                    Notification::warning(format!(
                        "Text patch failed — {}",
                        data::display_channel(&install.channel_key)
                    ))
                    .with_body(format!("{e:#}"))
                    .with_source("langpatch"),
                );
            }
        }
    }
    emit_changed(app);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    UpToDate,
    Applied,
    /// Foreign file present — auto paused, user action required.
    ForeignPaused,
    /// Game running — deferred to the exit waiter.
    Deferred,
    /// Apply failed on a stale override → removed to vanilla.
    RemovedStale,
}

async fn reconcile_one(
    app: &AppHandle,
    config: &LangpatchConfig,
    install: &InstallRef,
    take_over: bool,
) -> anyhow::Result<Outcome> {
    let install_dir = install
        .p4k_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("p4k path has no parent dir"))?
        .to_path_buf();

    let pack = resolve_pack(app, config).await;
    let fingerprint = Fingerprint::new(
        &install.build_id,
        config,
        pack.as_ref().map(|(_, hash)| hash.clone()),
    );

    let data_dir = langpatch_dir();
    let mut state_file = PatchStateFile::load(&data_dir);
    let known = state_file.known_outputs();
    let entry = state_file.installs.get(&install.channel_key);
    let disk_sha = sha256_file(&merge::override_path(&install_dir));

    match plan_for(entry, &fingerprint, disk_sha.as_deref(), &known) {
        PatchPlan::UpToDate if !take_over => return Ok(Outcome::UpToDate),
        PatchPlan::Foreign if !take_over => {
            notify(
                app,
                Notification::warning(format!(
                    "Text patch paused — {}",
                    data::display_channel(&install.channel_key)
                ))
                .with_body(
                    "Another tool (or a manual edit) changed global.ini since the last patch. \
                     Automatic patching is paused for this install; use 'Take over' on the \
                     Text Patching page to overwrite it.",
                )
                .with_action("Open Text Patching", "/langpatch")
                .with_source("langpatch"),
            );
            return Ok(Outcome::ForeignPaused);
        }
        _ => {}
    }

    // Write gate: never touch the file while SC runs (it read the INI at
    // boot; writing now only risks fighting the launcher/verifier).
    if sc_running().await {
        spawn_exit_waiter(app);
        notify(
            app,
            Notification::info("Text patch queued")
                .with_body("Star Citizen is running; the patch applies when it exits.")
                .with_source("langpatch"),
        );
        return Ok(Outcome::Deferred);
    }

    // The override on disk is from an older build iff a previous applied
    // state exists with a different staleness key — the auto-remove-
    // fallback condition when the apply below fails.
    let stale_override_present =
        disk_sha.is_some() && entry.is_some_and(|e| e.staleness_key != fingerprint.staleness_key);

    match apply_install(app, config, install, &install_dir, pack).await {
        Ok(output_sha) => {
            state_file.installs.insert(
                install.channel_key.clone(),
                mod_langpatch::InstallPatchState {
                    staleness_key: fingerprint.staleness_key.clone(),
                    config_hash: fingerprint.config_hash.clone(),
                    pack_hash: fingerprint.pack_hash.clone(),
                    output_sha256: output_sha,
                    patched_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            state_file
                .save(&data_dir)
                .map_err(|e| anyhow::anyhow!("saving patch state: {e}"))?;
            notify(
                app,
                Notification::success(format!(
                    "Text patch applied — {}",
                    data::display_channel(&install.channel_key)
                ))
                .with_body(format!("{} enriched.", install.version))
                .with_source("langpatch"),
            );
            Ok(Outcome::Applied)
        }
        Err(e) if stale_override_present => {
            // Degraded fallback (2026-07-04 decision): vanilla beats an
            // entire old-build localization shadowing the new one.
            let install_dir = install_dir.clone();
            let _ = tokio::task::spawn_blocking(move || merge::remove_patch(&install_dir)).await;
            state_file.installs.remove(&install.channel_key);
            let _ = state_file.save(&data_dir);
            notify(
                app,
                Notification::warning(format!(
                    "Stale text patch removed — {}",
                    data::display_channel(&install.channel_key)
                ))
                .with_body(format!(
                    "Re-patching for {} failed ({e:#}); the old override was removed so the \
                     game shows vanilla text instead of outdated strings.",
                    install.version
                ))
                .with_source("langpatch"),
            );
            Ok(Outcome::RemovedStale)
        }
        Err(e) => Err(e),
    }
}

/// The apply pipeline: cooked data (only when the op-set cache misses) →
/// ops → base INI + pack overlay + ops + user overrides → written override.
/// Returns the sha256 of the written file.
async fn apply_install(
    app: &AppHandle,
    config: &LangpatchConfig,
    install: &InstallRef,
    install_dir: &Path,
    pack: Option<(Vec<String>, String)>,
) -> anyhow::Result<String> {
    let data_dir = langpatch_dir();
    let patchers = builtin_patchers();

    // Derive (cached per build+patcher+options). Loading CookedData is the
    // only potentially slow part — and only when the cache misses (fresh
    // build), where svc-data's own snapshot cache usually makes it cheap
    // because the warm ran first.
    let cooked = if cache_complete(
        &data_dir,
        &install.channel_key,
        &install.build_id,
        config,
        &patchers,
    ) {
        None
    } else {
        let service = app.state::<AppState>().data.clone();
        let install_for_load = install.clone();
        Some(tokio::task::spawn_blocking(move || service.load(&install_for_load, |_| {})).await??)
    };
    let ops = derive_ops(
        &data_dir,
        &install.channel_key,
        &install.build_id,
        cooked.as_deref(),
        config,
        &patchers,
    )?;

    let p4k_path = install.p4k_path.clone();
    let overrides = load_overrides(&install.channel_key);
    let install_dir = install_dir.to_path_buf();
    let channel_key = install.channel_key.clone();

    tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let base = svc_data::read_base_global_ini(&p4k_path)?;
        let mut lines = merge::decode_ini(&base)?;

        if let Some((pack_lines, _)) = &pack {
            let overlaid = merge::apply_language_pack(&mut lines, pack_lines);
            tracing::info!(channel = %channel_key, overlaid, "language pack overlaid");
        }

        let renames: Vec<mod_langpatch::KeyRename> = ops
            .iter()
            .flat_map(|p| p.ops.renames.iter().cloned())
            .collect();
        merge::apply_renames(&mut lines, &renames);

        // Ops stack per key in patcher-priority order (Replace resets the
        // running value; Prefix/Suffix compose — the sc-langpatch runner
        // semantics).
        let mut patch_map: HashMap<String, Vec<mod_langpatch::PatchOp>> = HashMap::new();
        for patcher_ops in &ops {
            for (key, op) in &patcher_ops.ops.patches {
                patch_map.entry(key.clone()).or_default().push(op.clone());
            }
        }
        let stats = merge::apply_patches(&mut lines, &patch_map);
        tracing::info!(
            channel = %channel_key,
            patched = stats.patched_lines,
            placeholders_skipped = stats.skipped_placeholders,
            missing = stats.missing_keys,
            "patches applied"
        );

        // User overrides: last, always win.
        if !overrides.is_empty() {
            let applied = merge::apply_user_overrides(&mut lines, &overrides);
            tracing::info!(channel = %channel_key, applied, "user overrides applied");
        }

        let bytes = merge::encode_utf8_bom(&lines);
        // Re-check the write gate right before touching the file.
        if sc_running_blocking() {
            anyhow::bail!("Star Citizen started during patching; aborted before writing");
        }
        merge::write_patch(&install_dir, &bytes)?;
        Ok(sha256_bytes(&bytes))
    })
    .await?
}

// ── IPC surface ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct LangpatchOverview {
    pub auto_patch: bool,
    pub channels: Vec<String>,
    pub language_pack: Option<String>,
    pub patchers: Vec<PatcherInfoView>,
    pub installs: Vec<LangpatchInstallView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct PatcherInfoView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    /// Overwrites community-pack text for its keys (UI warning badge).
    pub uses_replace_ops: bool,
    pub options: Vec<PatcherOptionView>,
    /// Chosen option values (option id → value); missing = default.
    pub values: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct PatcherOptionView {
    pub id: String,
    pub label: String,
    pub description: String,
    pub default: String,
    pub kind: OptionKindView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(tag = "type")]
pub enum OptionKindView {
    Bool,
    Choice { choices: Vec<ChoiceView> },
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ChoiceView {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct LangpatchInstallView {
    /// Display channel ("Live").
    pub channel: String,
    /// Lowercase key ("live").
    pub channel_key: String,
    pub version: String,
    /// In the user's patch set.
    pub selected: bool,
    /// `"up-to-date" | "stale" | "foreign" | "unpatched"`.
    pub state: String,
    pub patched_at: Option<String>,
}

/// The whole Text Patching page state in one call.
#[tauri::command]
#[specta::specta]
pub(crate) async fn langpatch_overview(app: AppHandle) -> Result<LangpatchOverview, AppError> {
    let config = load_config();
    let installs = data::refresh_installs(&app).await?;
    let state_file = PatchStateFile::load(&langpatch_dir());
    let known = state_file.known_outputs();

    // Status view avoids network: pack hash from the cached copy only.
    let pack_hash = config.language_pack.as_deref().and_then(|source| {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        if source.starts_with("http") {
            let meta: PackMeta = app_kit::load_json(&langpatch_dir().join("pack.meta.json"));
            (meta.source == source).then_some(meta.sha256)
        } else {
            std::fs::read(source).ok().map(|b| sha256_bytes(&b))
        }
    });

    let install_views = installs
        .iter()
        .map(|i| {
            let entry = state_file.installs.get(&i.channel_key);
            let fingerprint = Fingerprint::new(&i.build_id, &config, pack_hash.clone());
            let install_dir = i
                .p4k_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            let disk_sha = sha256_file(&merge::override_path(&install_dir));
            let state = match plan_for(entry, &fingerprint, disk_sha.as_deref(), &known) {
                PatchPlan::UpToDate => "up-to-date",
                PatchPlan::Foreign => "foreign",
                PatchPlan::Apply if entry.is_some() || disk_sha.is_some() => "stale",
                PatchPlan::Apply => "unpatched",
            };
            LangpatchInstallView {
                channel: data::display_channel(&i.channel_key),
                channel_key: i.channel_key.clone(),
                version: i.version.clone(),
                selected: config.channels.contains(&i.channel_key),
                state: state.into(),
                patched_at: entry.map(|e| e.patched_at.clone()),
            }
        })
        .collect();

    let patcher_views = builtin_patchers()
        .iter()
        .map(|p| PatcherInfoView {
            id: p.id().into(),
            name: p.name().into(),
            description: p.description().into(),
            enabled: config.patcher_enabled(p.as_ref()),
            uses_replace_ops: p.uses_replace_ops(),
            options: p
                .options()
                .into_iter()
                .map(|o| PatcherOptionView {
                    id: o.id,
                    label: o.label,
                    description: o.description,
                    default: o.default,
                    kind: match o.kind {
                        mod_langpatch::OptionKind::Bool => OptionKindView::Bool,
                        mod_langpatch::OptionKind::Choice { choices } => OptionKindView::Choice {
                            choices: choices
                                .into_iter()
                                .map(|c| ChoiceView {
                                    value: c.value,
                                    label: c.label,
                                })
                                .collect(),
                        },
                    },
                })
                .collect(),
            values: config.patcher_config(p.id()).options.into_iter().collect(),
        })
        .collect();

    Ok(LangpatchOverview {
        auto_patch: config.auto_patch,
        channels: config.channels,
        language_pack: config.language_pack,
        patchers: patcher_views,
        installs: install_views,
    })
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct LangpatchConfigUpdate {
    pub auto_patch: bool,
    pub channels: Vec<String>,
    pub language_pack: Option<String>,
    /// patcher id → (enabled, option values).
    pub patchers: HashMap<String, PatcherConfigUpdate>,
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct PatcherConfigUpdate {
    pub enabled: Option<bool>,
    pub options: HashMap<String, String>,
}

/// Save the module config and reconcile. Deselected channels that carry
/// our patch get it removed (leaving no orphaned overrides behind).
#[tauri::command]
#[specta::specta]
pub(crate) async fn langpatch_update_config(
    app: AppHandle,
    update: LangpatchConfigUpdate,
) -> Result<(), AppError> {
    let old = load_config();
    let config = LangpatchConfig {
        auto_patch: update.auto_patch,
        channels: update
            .channels
            .into_iter()
            .map(|c| c.to_ascii_lowercase())
            .collect(),
        language_pack: update.language_pack.filter(|s| !s.trim().is_empty()),
        patchers: update
            .patchers
            .into_iter()
            .map(|(id, p)| {
                (
                    id,
                    PatcherConfig {
                        enabled: p.enabled,
                        options: p.options.into_iter().collect(),
                    },
                )
            })
            .collect(),
    };
    save_config(&config)?;

    // Deselected channels: pull our patch off.
    for channel in old.channels.iter().filter(|c| !config.channels.contains(c)) {
        if let Err(e) = remove_channel(&app, channel).await {
            tracing::warn!(channel, "removing patch on deselect failed: {e:#}");
        }
    }

    let app_for_reconcile = app.clone();
    tauri::async_runtime::spawn(async move {
        reconcile_all(&app_for_reconcile).await;
    });
    emit_changed(&app);
    Ok(())
}

/// Manual apply — also the "take over" action for foreign files.
#[tauri::command]
#[specta::specta]
pub(crate) async fn langpatch_apply(app: AppHandle, channel: String) -> Result<(), AppError> {
    let config = load_config();
    let key = channel.to_ascii_lowercase();
    let installs = data::refresh_installs(&app).await?;
    let install = installs
        .into_iter()
        .find(|i| i.channel_key == key)
        .ok_or_else(|| AppError::Config(format!("no SC install found for channel '{channel}'")))?;
    reconcile_one(&app, &config, &install, true)
        .await
        .map_err(|e| AppError::Internal(format!("{e:#}")))?;
    emit_changed(&app);
    Ok(())
}

/// Remove the patch from one install (back to vanilla text).
#[tauri::command]
#[specta::specta]
pub(crate) async fn langpatch_remove(app: AppHandle, channel: String) -> Result<(), AppError> {
    remove_channel(&app, &channel.to_ascii_lowercase()).await?;
    emit_changed(&app);
    Ok(())
}

async fn remove_channel(app: &AppHandle, channel_key: &str) -> Result<(), AppError> {
    let installs = data::refresh_installs(app).await?;
    let Some(install) = installs.into_iter().find(|i| i.channel_key == channel_key) else {
        return Ok(());
    };
    let install_dir = install
        .p4k_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::Internal("p4k path has no parent dir".into()))?;
    tokio::task::spawn_blocking(move || merge::remove_patch(&install_dir))
        .await
        .map_err(|e| AppError::Internal(format!("remove task failed: {e}")))?
        .map_err(|e| AppError::Internal(format!("{e:#}")))?;

    let data_dir = langpatch_dir();
    let mut state_file = PatchStateFile::load(&data_dir);
    if state_file.installs.remove(channel_key).is_some() {
        let _ = state_file.save(&data_dir);
    }
    Ok(())
}
