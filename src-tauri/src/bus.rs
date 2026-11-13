//! The in-process event bus — services below, modules above, this between
//! them (the CONTRIBUTING architecture line). A tokio broadcast channel: producers
//! `send` and don't care who listens; consumers hold a `Receiver` from
//! [`AppState::bus`](crate::AppState) and react.
//!
//! Producers: the svc-discovery install watcher (see [`crate::watch`]) and
//! the svc-log Game.log watcher (see [`crate::gamelog`]).
//! First consumer: the shell itself (cache invalidation + re-warm). Future
//! consumers: mod-langpatch's re-patch, per the design doc —
//! "`mod-langpatch` subscribing to `InstallChanged` *is* the re-patch-before-
//! SC-starts feature".
//!
//! Events are **settled facts**, not raw observations — debounce/settle
//! happens at the producer (svc-discovery's watcher), so subscribers never
//! see a mid-download flap.

use tokio::sync::broadcast;

/// One bus event. Add variants as producers land; consumers ignore what
/// they don't handle.
#[derive(Debug, Clone)]
pub enum BusEvent {
    /// An SC install settled on a new build (or a new install appeared).
    InstallChanged(svc_discovery::InstallInfo),
    /// An SC install disappeared.
    InstallRemoved { channel: String },
    /// A game session began (`live`) or ended: the game started or quit,
    /// as its `Game.log` tells it.
    GameSession { live: bool },
    /// New `Game.log` events of the session, oldest first. One batch per
    /// poll, so a busy log cannot flood the channel.
    ///
    /// **`replay` batches are history, not news**: the app started while
    /// the game ran (or the watcher moved to another channel's log) and
    /// caught up on what was already written, possibly hours ago. A
    /// consumer that *does* something on an event (a notification, a timer,
    /// a presence update) must skip them; one that only keeps the session's
    /// record takes both.
    GameLog {
        events: std::sync::Arc<[svc_log::LogEvent]>,
        replay: bool,
    },
}

pub type Bus = broadcast::Sender<BusEvent>;

/// Fresh bus. Capacity is generous for events this rare; a lagged receiver
/// (`RecvError::Lagged`) should just resync from current state.
pub fn new_bus() -> Bus {
    broadcast::channel(32).0
}
