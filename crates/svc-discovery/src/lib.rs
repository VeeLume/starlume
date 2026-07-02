//! Install service — SC install discovery, selected channel, and
//! build-change detection (`build_manifest.id` watch → `InstallChanged`).
//!
//! **Intentionally empty.** This is carved out of Hearth's `sc_loader`
//! discovery path (which wraps sc-holotable's `sc-discovery`) — migration
//! step 2 in the design doc. No API is invented here before the carve-out;
//! the first real commit moves working code, not speculation.
