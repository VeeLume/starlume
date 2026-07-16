//! The `build_manifest.id` watcher — polls installs, applies settle logic,
//! and reports [`InstallEvent`]s once an update has quiesced.
//!
//! Design (from the mod-langpatch scoping, 2026-07-04):
//! - **Poll, don't fs-watch.** One [`scan`] is ~50ms of launcher-store
//!   reads; a 30s poll is negligible and immune to the games launcher's
//!   write patterns. **Stat-only — never parses** (docs/memory.md: parse
//!   spikes only on build change; `InstallChanged` is the trigger, not a
//!   timer).
//! - **Settle lives here, not in consumers.** During a patch download the
//!   manifest and `Data.p4k` churn for minutes; every consumer (svc-data
//!   invalidation, mod-langpatch re-patch) wants the *settled* state, so an
//!   event fires only after the staleness key AND the p4k signature have
//!   been quiet for [`WatchConfig::settle`].
//! - **Sync + std-thread** (workspace layering: no tokio in `svc-*`). The
//!   shell bridges the callback onto its bus.
//!
//! The decision core ([`WatchCore`]) is pure — observations in, events out,
//! time injected — so the settle rules are unit-tested without a filesystem
//! or a clock.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use crate::InstallInfo;

/// A settled change to the install landscape.
#[derive(Debug, Clone)]
pub enum InstallEvent {
    /// An install settled on a new build — or appeared (fresh channel
    /// download counts as a change from nothing).
    Changed(InstallInfo),
    /// An install disappeared from the scan (missing for two consecutive
    /// polls, so a single launcher-store hiccup doesn't fire it).
    Removed {
        /// Channel label as scanned (e.g. `"Live"`).
        channel: String,
    },
}

/// Watcher tuning.
#[derive(Debug, Clone, Copy)]
pub struct WatchConfig {
    /// How often to rescan. Each poll is ~50ms of launcher-store reads +
    /// one `Data.p4k` stat per install.
    pub poll_interval: Duration,
    /// How long the staleness key + p4k signature must be quiet before a
    /// change is considered settled.
    pub settle: Duration,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(30),
            settle: Duration::from_secs(60),
        }
    }
}

/// Cheap change signature for `Data.p4k` — the launcher rewrites it
/// incrementally during a patch, so (size, mtime) churn means "still
/// downloading".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct P4kSig {
    len: u64,
    modified: Option<SystemTime>,
}

fn p4k_sig(install_dir: &str) -> Option<P4kSig> {
    let meta = std::fs::metadata(Path::new(install_dir).join("Data.p4k")).ok()?;
    Some(P4kSig {
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

/// One poll's worth of facts about one install.
pub(crate) struct Observation {
    info: InstallInfo,
    sig: Option<P4kSig>,
}

struct ChannelState {
    /// Staleness key of the last emitted (or seeded) state.
    emitted_key: String,
    /// Most recently observed staleness key.
    last_key: String,
    last_sig: Option<P4kSig>,
    /// When the key or signature last changed — the settle timer's anchor.
    last_activity: Instant,
    /// Consecutive polls this channel was absent from the scan.
    missing_polls: u32,
    /// Latest observed info, carried into the `Changed` event.
    info: InstallInfo,
}

/// The pure settle state machine. Feed it one [`Observation`] batch per
/// poll; it returns the events that became due.
pub(crate) struct WatchCore {
    settle: Duration,
    channels: HashMap<String, ChannelState>,
}

impl WatchCore {
    pub(crate) fn new(settle: Duration) -> Self {
        Self {
            settle,
            channels: HashMap::new(),
        }
    }

    /// Adopt the current state without emitting — the state present at
    /// watcher start was already handled (startup warm, startup staleness
    /// checks).
    pub(crate) fn seed(&mut self, now: Instant, observations: Vec<Observation>) {
        for obs in observations {
            let key = obs.info.staleness_key();
            self.channels.insert(
                obs.info.channel.clone(),
                ChannelState {
                    emitted_key: key.clone(),
                    last_key: key,
                    last_sig: obs.sig,
                    last_activity: now,
                    missing_polls: 0,
                    info: obs.info,
                },
            );
        }
    }

    /// Process one poll. Emits `Changed` for every channel whose pending
    /// change has settled, `Removed` for channels gone two polls in a row.
    pub(crate) fn observe(
        &mut self,
        now: Instant,
        observations: Vec<Observation>,
    ) -> Vec<InstallEvent> {
        let mut events = Vec::new();
        let mut seen: Vec<String> = Vec::with_capacity(observations.len());

        for obs in observations {
            let key = obs.info.staleness_key();
            seen.push(obs.info.channel.clone());
            let state = self
                .channels
                .entry(obs.info.channel.clone())
                .or_insert_with(|| ChannelState {
                    // New channel: pending change from nothing — settles
                    // like any other change (a fresh download churns first).
                    emitted_key: String::new(),
                    last_key: key.clone(),
                    last_sig: obs.sig,
                    last_activity: now,
                    missing_polls: 0,
                    info: obs.info.clone(),
                });
            state.missing_polls = 0;
            if key != state.last_key || obs.sig != state.last_sig {
                state.last_key = key;
                state.last_sig = obs.sig;
                state.last_activity = now;
                state.info = obs.info;
            }
            // A rollback (key back to the emitted one) simply leaves
            // nothing pending — no event was sent mid-churn, none is now.
            if state.last_key != state.emitted_key
                && now.duration_since(state.last_activity) >= self.settle
            {
                state.emitted_key = state.last_key.clone();
                events.push(InstallEvent::Changed(state.info.clone()));
            }
        }

        self.channels.retain(|channel, state| {
            if seen.contains(channel) {
                return true;
            }
            state.missing_polls += 1;
            if state.missing_polls >= 2 {
                events.push(InstallEvent::Removed {
                    channel: channel.clone(),
                });
                false
            } else {
                true
            }
        });

        events
    }
}

/// Handle to a running watcher thread. [`WatchHandle::stop`] (or drop)
/// wakes and joins it.
pub struct WatchHandle {
    stop_tx: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WatchHandle {
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn the watcher thread. `on_event` is called from that thread for
/// every settled event — keep it cheap (the shell forwards onto its bus).
pub fn spawn(config: WatchConfig, on_event: impl Fn(InstallEvent) + Send + 'static) -> WatchHandle {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("starlume-install-watch".into())
        .spawn(move || {
            let mut core = WatchCore::new(config.settle);
            if let Some(obs) = observe_now() {
                core.seed(Instant::now(), obs);
            }
            loop {
                match stop_rx.recv_timeout(config.poll_interval) {
                    // Stop requested, or the handle was dropped.
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                // Scan hiccup (e.g. launcher store locked mid-update):
                // skip the tick rather than mis-reading it as "all
                // installs removed".
                let Some(obs) = observe_now() else { continue };
                for event in core.observe(Instant::now(), obs) {
                    tracing::info!(?event, "install watcher event");
                    on_event(event);
                }
            }
        })
        .expect("spawn install watcher thread");
    WatchHandle {
        stop_tx,
        thread: Some(thread),
    }
}

fn observe_now() -> Option<Vec<Observation>> {
    match crate::scan() {
        Ok(scan) => Some(
            scan.installs
                .into_iter()
                .map(|info| {
                    let sig = p4k_sig(&info.directory);
                    Observation { info, sig }
                })
                .collect(),
        ),
        Err(e) => {
            tracing::warn!("install watcher scan failed (skipping tick): {e:#}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(channel: &str, build_id: &str, version: &str) -> InstallInfo {
        InstallInfo {
            channel: channel.into(),
            platform: "prod".into(),
            directory: format!("C:\\SC\\{channel}"),
            version: version.into(),
            build_id: build_id.into(),
        }
    }

    fn obs(channel: &str, build_id: &str, version: &str, sig: u64) -> Observation {
        Observation {
            info: info(channel, build_id, version),
            sig: Some(P4kSig {
                len: sig,
                modified: None,
            }),
        }
    }

    const SETTLE: Duration = Duration::from_secs(60);

    fn seeded(now: Instant) -> WatchCore {
        let mut core = WatchCore::new(SETTLE);
        core.seed(now, vec![obs("Live", "b1", "4.8.0-live.100", 10)]);
        core
    }

    #[test]
    fn steady_state_emits_nothing() {
        let t0 = Instant::now();
        let mut core = seeded(t0);
        for i in 1..10 {
            let events = core.observe(
                t0 + Duration::from_secs(30 * i),
                vec![obs("Live", "b1", "4.8.0-live.100", 10)],
            );
            assert!(events.is_empty());
        }
    }

    #[test]
    fn build_change_emits_only_after_settle() {
        let t0 = Instant::now();
        let mut core = seeded(t0);
        // Change lands at t+30 — not settled yet.
        let events = core.observe(
            t0 + Duration::from_secs(30),
            vec![obs("Live", "b2", "4.8.1-live.200", 20)],
        );
        assert!(events.is_empty());
        // t+60: only 30s quiet — still pending.
        let events = core.observe(
            t0 + Duration::from_secs(60),
            vec![obs("Live", "b2", "4.8.1-live.200", 20)],
        );
        assert!(events.is_empty());
        // t+90: 60s quiet — settled.
        let events = core.observe(
            t0 + Duration::from_secs(90),
            vec![obs("Live", "b2", "4.8.1-live.200", 20)],
        );
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], InstallEvent::Changed(i) if i.build_id == "b2"),
            "expected Changed(b2), got {events:?}"
        );
        // And only once.
        let events = core.observe(
            t0 + Duration::from_secs(120),
            vec![obs("Live", "b2", "4.8.1-live.200", 20)],
        );
        assert!(events.is_empty());
    }

    #[test]
    fn p4k_churn_defers_emission() {
        let t0 = Instant::now();
        let mut core = seeded(t0);
        // Manifest flips early but the p4k keeps growing each poll —
        // the settle anchor keeps moving.
        for i in 1..=5u64 {
            let events = core.observe(
                t0 + Duration::from_secs(30 * i),
                vec![obs("Live", "b2", "4.8.1-live.200", 20 + i)],
            );
            assert!(events.is_empty(), "poll {i} emitted during churn");
        }
        // Quiet for two polls (60s) after the last churn at t+150.
        let events = core.observe(
            t0 + Duration::from_secs(180),
            vec![obs("Live", "b2", "4.8.1-live.200", 25)],
        );
        assert!(events.is_empty()); // only 30s quiet
        let events = core.observe(
            t0 + Duration::from_secs(210),
            vec![obs("Live", "b2", "4.8.1-live.200", 25)],
        );
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn rollback_to_emitted_key_clears_pending() {
        let t0 = Instant::now();
        let mut core = seeded(t0);
        core.observe(
            t0 + Duration::from_secs(30),
            vec![obs("Live", "b2", "4.8.1-live.200", 20)],
        );
        // Launcher rolled back before the change settled.
        core.observe(
            t0 + Duration::from_secs(60),
            vec![obs("Live", "b1", "4.8.0-live.100", 10)],
        );
        let events = core.observe(
            t0 + Duration::from_secs(600),
            vec![obs("Live", "b1", "4.8.0-live.100", 10)],
        );
        assert!(events.is_empty());
    }

    #[test]
    fn new_channel_emits_after_settle() {
        let t0 = Instant::now();
        let mut core = seeded(t0);
        core.observe(
            t0 + Duration::from_secs(30),
            vec![
                obs("Live", "b1", "4.8.0-live.100", 10),
                obs("Ptu", "p1", "4.9.0-ptu.300", 5),
            ],
        );
        let events = core.observe(
            t0 + Duration::from_secs(90),
            vec![
                obs("Live", "b1", "4.8.0-live.100", 10),
                obs("Ptu", "p1", "4.9.0-ptu.300", 5),
            ],
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], InstallEvent::Changed(i) if i.channel == "Ptu"));
    }

    #[test]
    fn removed_after_two_missing_polls() {
        let t0 = Instant::now();
        let mut core = seeded(t0);
        let events = core.observe(t0 + Duration::from_secs(30), vec![]);
        assert!(events.is_empty(), "one missing poll must not remove");
        let events = core.observe(t0 + Duration::from_secs(60), vec![]);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], InstallEvent::Removed { channel } if channel == "Live"));
        // Reappearing later counts as a new install → settles → Changed.
        core.observe(
            t0 + Duration::from_secs(90),
            vec![obs("Live", "b1", "4.8.0-live.100", 10)],
        );
        let events = core.observe(
            t0 + Duration::from_secs(150),
            vec![obs("Live", "b1", "4.8.0-live.100", 10)],
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], InstallEvent::Changed(_)));
    }

    #[test]
    fn none_build_id_falls_back_to_version_label() {
        // The memory-note case: build_id is literally "None" on current
        // Live builds — the version label must drive change detection.
        let t0 = Instant::now();
        let mut core = WatchCore::new(SETTLE);
        core.seed(t0, vec![obs("Live", "None", "4.8.3-live.100", 10)]);
        core.observe(
            t0 + Duration::from_secs(30),
            vec![obs("Live", "None", "4.8.4-live.200", 20)],
        );
        let events = core.observe(
            t0 + Duration::from_secs(90),
            vec![obs("Live", "None", "4.8.4-live.200", 20)],
        );
        assert_eq!(events.len(), 1);
    }
}
