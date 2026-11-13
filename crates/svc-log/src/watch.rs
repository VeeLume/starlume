//! The log watcher: one thread that follows whichever `Game.log` the game
//! is writing and keeps the current session's events.
//!
//! Shaped like `svc_discovery::watch`: sync + std-thread (no tokio in
//! `svc-*`), a pure-ish core ([`WatchCore::tick`]) the tests drive without
//! a thread or a clock, and a callback the shell bridges onto its bus.
//!
//! **What it costs while the game is not running** (the resident-app
//! rules, CLAUDE.md "Memory"): one `stat` per known install every
//! [`WatchConfig::idle_poll`], nothing else. No file is read, nothing is
//! parsed, no process is scanned. A log whose size or mtime moved is the
//! only thing that starts a read, and only the game moves it.
//!
//! **Which log.** Every install has its own `Game.log`; the active one is
//! the one that changes. At startup, with the game already running, it is
//! the most recently written one, replayed from its first line so the
//! session is whole.
//!
//! **When a session is over.** Cleanly: the log says so
//! ([`LogEventKind::SessionEnded`]). After a crash it just goes quiet, so a
//! live session that has written nothing for [`WatchConfig::quiet`] asks
//! `game_running` (a process scan, injected by the shell) and ends when the
//! answer is no. That scan runs only then.
//!
//! **What stays in memory.** The current session's events, capped at
//! [`WatchConfig::max_events`] (oldest dropped first). They are kept after
//! the session ends, until the next one starts, so "what did I just do"
//! still answers. With no session seen since the app started, nothing is
//! held: read the last log on demand with [`crate::read_log`].

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime};

use crate::event::{LogEvent, LogEventKind};
use crate::tailer::{LogTailer, log_head};

/// Watcher tuning.
#[derive(Debug, Clone, Copy)]
pub struct WatchConfig {
    /// Poll interval while a session is live.
    pub live_poll: Duration,
    /// Poll interval otherwise (stat-only).
    pub idle_poll: Duration,
    /// How long a live log may stay silent before the process is checked.
    pub quiet: Duration,
    /// Most events kept for the current session.
    pub max_events: usize,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            live_poll: Duration::from_secs(2),
            idle_poll: Duration::from_secs(5),
            quiet: Duration::from_secs(30),
            max_events: 5000,
        }
    }
}

/// What the watcher tells its owner, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchSignal {
    /// A session's log is being followed: the game started, or the app
    /// started while it ran. The session buffer was cleared.
    SessionStarted { log: PathBuf },
    /// New events of the session, oldest first.
    ///
    /// `replay` marks a batch read while catching up on a log that was
    /// already being written before the watcher followed it: the app
    /// started with the game running, or the watcher moved to another
    /// channel's log. Those events can be hours old. They belong in the
    /// session's history, but nothing may *react* to them (no
    /// notification, no timer, no presence update): check the flag, or the
    /// event's `ts`.
    Events { events: Vec<LogEvent>, replay: bool },
    /// The live session is over: the game quit, or its process is gone.
    SessionEnded,
}

/// The current session as the watcher holds it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SessionSnapshot {
    /// The game is running and its log is being followed.
    pub live: bool,
    /// The log these events came from; `None` when no session was seen
    /// since the app started.
    pub log: Option<String>,
    /// Oldest first.
    pub events: Vec<LogEvent>,
    /// Events dropped from the front to stay within the cap.
    pub dropped: u32,
}

#[derive(Debug, Default)]
struct Session {
    live: bool,
    log: Option<PathBuf>,
    events: VecDeque<LogEvent>,
    dropped: u32,
}

/// Cheap change signature of a log file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sig {
    len: u64,
    modified: Option<SystemTime>,
}

fn sig(path: &Path) -> Option<Sig> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Sig {
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

/// The log of a session that ended.
#[derive(Debug)]
struct Finished {
    path: PathBuf,
    /// Its first bytes: a new session replaces the file and changes them.
    head: Vec<u8>,
    /// The log itself said the game quit. Then nothing it still gains is a
    /// session, however long the process lingers.
    quit_seen: bool,
}

/// The decision core: one [`tick`](Self::tick) per poll, time injected.
pub struct WatchCore<S, R> {
    config: WatchConfig,
    /// The `Game.log` paths worth watching (one per install), asked every
    /// idle tick so a new install is picked up without a restart.
    sources: S,
    game_running: R,
    session: Arc<Mutex<Session>>,
    /// Last seen signature per candidate: refreshed every idle tick, and
    /// for the logs not being followed at every quiet check while live.
    seen: HashMap<PathBuf, Option<Sig>>,
    tailer: Option<LogTailer>,
    /// The log of the session that just ended: its last lines can still
    /// trickle in, and they are not a new session.
    finished: Option<Finished>,
    last_growth: Instant,
    started: bool,
}

impl<S, R> WatchCore<S, R>
where
    S: Fn() -> Vec<PathBuf>,
    R: Fn() -> bool,
{
    pub fn new(config: WatchConfig, sources: S, game_running: R) -> Self {
        Self {
            config,
            sources,
            game_running,
            session: Arc::default(),
            seen: HashMap::new(),
            tailer: None,
            finished: None,
            last_growth: Instant::now(),
            started: false,
        }
    }

    fn live(&self) -> bool {
        self.session.lock().unwrap().live
    }

    /// How long to sleep before the next tick.
    pub fn interval(&self) -> Duration {
        if self.live() {
            self.config.live_poll
        } else {
            self.config.idle_poll
        }
    }

    /// One poll. Idle: stat the candidates and attach to one that moved.
    /// Live: read what the log gained.
    pub fn tick(&mut self, now: Instant) -> Vec<WatchSignal> {
        let mut signals = Vec::new();
        if self.live() {
            self.tick_live(now, &mut signals);
        } else {
            self.tick_idle(now, &mut signals);
        }
        signals
    }

    fn tick_idle(&mut self, now: Instant, signals: &mut Vec<WatchSignal>) {
        let candidates = (self.sources)();
        // "First" is the first tick that has something to look at: the
        // shell may not know the installs yet when the thread starts.
        let first = !self.started && !candidates.is_empty();
        self.started |= first;

        let mut moved: Option<(PathBuf, Sig)> = None;
        let mut newest: Option<(PathBuf, Sig)> = None;
        for path in &candidates {
            let now_sig = sig(path);
            let before = self.seen.insert(path.clone(), now_sig);
            let Some(s) = now_sig else { continue };
            if newest.as_ref().is_none_or(|(_, n)| s.modified > n.modified) {
                newest = Some((path.clone(), s));
            }
            // A candidate first seen on a later tick (a new install) only
            // sets its baseline; a log that appears or changes has moved.
            if matches!(before, Some(b) if b != now_sig) && !self.is_finished_log(path) {
                moved = Some((path.clone(), s));
            }
        }
        self.seen.retain(|path, _| candidates.contains(path));

        // The app started while the game was already running: follow the
        // log written last. The one process scan of an idle watcher.
        // Hours of play may already be in it: a replay.
        if first && moved.is_none() && newest.is_some() && (self.game_running)() {
            if let Some((path, _)) = newest {
                self.attach(path, true, now, signals);
            }
        } else if let Some((path, _)) = moved {
            // A log that moved since the last idle tick is at most
            // `idle_poll` into its session, unless it is the finished log
            // picked up again.
            let resumed = self.finished.as_ref().is_some_and(|f| f.path == path);
            self.attach(path, resumed, now, signals);
        }
    }

    /// `path` moved, but is it still the log of the session that ended? The
    /// game writes a few more lines after it says it quits, and its process
    /// outlives that line by a moment. A new session replaces the file,
    /// which changes its head.
    ///
    /// The one way back in: the session was ended by the process check, not
    /// by the log (no quit line), and the game is running after all. Then
    /// the end was called too early and the log is followed again.
    fn is_finished_log(&self, path: &Path) -> bool {
        let Some(finished) = &self.finished else {
            return false;
        };
        finished.path == path
            && log_head(path).is_ok_and(|now| now == finished.head)
            && (finished.quit_seen || !(self.game_running)())
    }

    /// Follow `path` from its first line. `replay`: what is already in it
    /// may be old (see [`WatchSignal::Events`]).
    fn attach(
        &mut self,
        path: PathBuf,
        replay: bool,
        now: Instant,
        signals: &mut Vec<WatchSignal>,
    ) {
        let mut tailer = LogTailer::new(path.clone());
        let poll = match tailer.poll() {
            Ok(poll) => poll,
            Err(e) => {
                // Locked for a moment by the game's own rotation: the
                // changed signature is forgotten so the next tick retries.
                tracing::debug!(path = %path.display(), "Game.log not readable yet: {e}");
                self.seen.insert(path, None);
                return;
            }
        };
        self.tailer = Some(tailer);
        self.last_growth = now;
        self.begin_session(path, signals);
        // The log may already hold the whole session, its quit included.
        let quit = has_quit(&poll.events);
        self.push_events(poll.events, replay, signals);
        if quit {
            self.end_session(true, signals);
        }
    }

    fn begin_session(&mut self, log: PathBuf, signals: &mut Vec<WatchSignal>) {
        {
            let mut session = self.session.lock().unwrap();
            *session = Session {
                live: true,
                log: Some(log.clone()),
                ..Session::default()
            };
        }
        tracing::info!(log = %log.display(), "Game.log session started");
        signals.push(WatchSignal::SessionStarted { log });
    }

    /// `quit_seen`: the log said the game quit (else: its process is gone,
    /// or the watcher moved to another log).
    fn end_session(&mut self, quit_seen: bool, signals: &mut Vec<WatchSignal>) {
        self.session.lock().unwrap().live = false;
        // Back to stat-only: baseline every candidate as it is now, so the
        // finished log's last lines don't read as a new session.
        self.seen.clear();
        self.started = true;
        for path in (self.sources)() {
            let s = sig(&path);
            self.seen.insert(path, s);
        }
        self.finished = self.tailer.take().map(|tailer| Finished {
            path: tailer.path().to_owned(),
            head: tailer.head().to_owned(),
            quit_seen,
        });
        tracing::info!("Game.log session ended");
        signals.push(WatchSignal::SessionEnded);
    }

    fn tick_live(&mut self, now: Instant, signals: &mut Vec<WatchSignal>) {
        let Some(tailer) = self.tailer.as_mut() else {
            return self.end_session(false, signals);
        };
        let poll = match tailer.poll() {
            Ok(poll) => poll,
            Err(e) => {
                tracing::warn!("Game.log poll failed: {e}");
                Default::default()
            }
        };
        // Growth is bytes, not tracked events: most lines carry nothing
        // tracked, and a log that is being written is a running game.
        let grew = poll.restarted || poll.bytes > 0;
        if poll.restarted {
            // The game was restarted between two polls.
            let log = tailer.path().to_owned();
            signals.push(WatchSignal::SessionEnded);
            self.begin_session(log, signals);
        }
        let quit = has_quit(&poll.events);
        self.push_events(poll.events, false, signals);

        if quit {
            return self.end_session(true, signals);
        }
        if grew {
            self.last_growth = now;
        } else if now.duration_since(self.last_growth) >= self.config.quiet {
            // Not a byte for a while: a crash leaves no last line. A
            // running game writes all the time, so this is rare.
            self.last_growth = now;
            self.quiet_check(now, signals);
        }
    }

    /// The followed log went silent. Either another channel's game is the
    /// one running now (its log moved: follow that one), or the process is
    /// gone (the session is over), or the game is merely quiet.
    fn quiet_check(&mut self, now: Instant, signals: &mut Vec<WatchSignal>) {
        let followed = self.tailer.as_ref().map(|t| t.path().to_owned());
        let mut moved = None;
        for path in (self.sources)() {
            if Some(&path) == followed.as_ref() {
                continue;
            }
            let now_sig = sig(&path);
            let before = self.seen.insert(path.clone(), now_sig);
            if now_sig.is_some() && matches!(before, Some(b) if b != now_sig) {
                moved = Some(path);
            }
        }
        if let Some(path) = moved {
            // The process check would say "running" for the other game and
            // keep this dead log forever.
            self.end_session(false, signals);
            // It has been written for a while already: a replay.
            self.attach(path, true, now, signals);
        } else if !(self.game_running)() {
            self.end_session(false, signals);
        }
    }

    fn push_events(&mut self, events: Vec<LogEvent>, replay: bool, signals: &mut Vec<WatchSignal>) {
        if events.is_empty() {
            return;
        }
        {
            let mut session = self.session.lock().unwrap();
            session.events.extend(events.iter().cloned());
            let over = session.events.len().saturating_sub(self.config.max_events);
            if over > 0 {
                session.events.drain(..over);
                session.dropped = session.dropped.saturating_add(over as u32);
            }
        }
        signals.push(WatchSignal::Events { events, replay });
    }
}

fn has_quit(events: &[LogEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e.event, LogEventKind::SessionEnded { .. }))
}

/// Handle to a running watcher thread. [`LogWatch::stop`] (or drop) wakes
/// and joins it.
pub struct LogWatch {
    stop_tx: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
    session: Arc<Mutex<Session>>,
}

impl LogWatch {
    /// The current session: live or the last one seen. A copy; the events
    /// of a long session are a few hundred kilobytes at most.
    pub fn session(&self) -> SessionSnapshot {
        let session = self.session.lock().unwrap();
        SessionSnapshot {
            live: session.live,
            log: session.log.as_ref().map(|p| p.display().to_string()),
            events: session.events.iter().cloned().collect(),
            dropped: session.dropped,
        }
    }

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

impl Drop for LogWatch {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn the watcher thread.
///
/// - `sources`: the `Game.log` path of every install (see
///   [`crate::game_log_path`]); called on the watcher thread each idle
///   tick, so keep it a cheap read of state the shell already has.
/// - `game_running`: is the game's process up? Called rarely (see the
///   module docs); may block for a process scan.
/// - `on_signal`: called on the watcher thread for every signal; keep it
///   cheap (the shell forwards onto its bus).
pub fn spawn(
    config: WatchConfig,
    sources: impl Fn() -> Vec<PathBuf> + Send + 'static,
    game_running: impl Fn() -> bool + Send + 'static,
    on_signal: impl Fn(WatchSignal) + Send + 'static,
) -> LogWatch {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let mut core = WatchCore::new(config, sources, game_running);
    let session = core.session.clone();
    let thread = std::thread::Builder::new()
        .name("starlume-log-watch".into())
        .spawn(move || {
            loop {
                for signal in core.tick(Instant::now()) {
                    on_signal(signal);
                }
                match stop_rx.recv_timeout(core.interval()) {
                    // Stop requested, or the handle was dropped.
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        })
        .expect("spawn log watcher thread");
    LogWatch {
        stop_tx,
        thread: Some(thread),
        session,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const HEADER: &str = concat!(
        "<2026-11-13T16:10:48.019Z> BackupNameAttachment=\" Build(13165946) 13 Nov 26 (17 10 38)\"  -- used by backup system\n",
        "<2026-11-13T16:10:48.019Z> Log started on Fri Nov 13 16:10:48 2026\n",
    );
    const HEADER_NEXT: &str = concat!(
        "<2026-11-13T19:00:00.019Z> BackupNameAttachment=\" Build(13165946) 13 Nov 26 (20 00 00)\"  -- used by backup system\n",
        "<2026-11-13T19:00:00.019Z> Log started on Fri Nov 13 19:00:00 2026\n",
    );
    const JOIN: &str = "<2026-11-13T16:13:05.561Z> [Notice] <Join PU> address[34.156.32.60] port[64385] shard[pub_euw1b_13165946_100] locationId[-281470681677823] [Team_GameServices][GIM][Matchmaking]\n";
    const LOCATION: &str = "<2026-11-13T16:13:38.798Z> [Notice] <RequestLocationInventory> Player[VeeLume] requested inventory for Location[Nyx_Levski] [Team_CoreGameplayFeatures][Inventory]\n";
    const QUIT: &str = "<2026-11-13T18:52:46.490Z> [Notice] <SystemQuit> CSystem::Quit invoked with - cause=30016, reason=Quit via console command, exitCode=0, thread id=39820, main thread id=39820 [Team_Unknown][System]\n";

    struct Rig {
        _dir: tempfile::TempDir,
        live: PathBuf,
        ptu: PathBuf,
        running: Arc<AtomicBool>,
        scans: Arc<AtomicUsize>,
    }

    type Core = WatchCore<Box<dyn Fn() -> Vec<PathBuf>>, Box<dyn Fn() -> bool>>;

    fn rig(config: WatchConfig) -> (Rig, Core) {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("LIVE").join("Game.log");
        let ptu = dir.path().join("PTU").join("Game.log");
        std::fs::create_dir_all(live.parent().unwrap()).unwrap();
        std::fs::create_dir_all(ptu.parent().unwrap()).unwrap();
        let running = Arc::new(AtomicBool::new(false));
        let scans = Arc::new(AtomicUsize::new(0));
        let sources = {
            let (live, ptu) = (live.clone(), ptu.clone());
            Box::new(move || vec![live.clone(), ptu.clone()]) as Box<dyn Fn() -> Vec<PathBuf>>
        };
        let game_running = {
            let (running, scans) = (running.clone(), scans.clone());
            Box::new(move || {
                scans.fetch_add(1, Ordering::SeqCst);
                running.load(Ordering::SeqCst)
            }) as Box<dyn Fn() -> bool>
        };
        let core = WatchCore::new(config, sources, game_running);
        (
            Rig {
                _dir: dir,
                live,
                ptu,
                running,
                scans,
            },
            core,
        )
    }

    fn append(path: &Path, text: &str) {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }

    fn snapshot(core: &Core) -> (bool, usize) {
        let s = core.session.lock().unwrap();
        (s.live, s.events.len())
    }

    #[test]
    fn an_old_log_with_the_game_closed_is_never_read() {
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}{QUIT}")).unwrap();
        let t0 = Instant::now();
        for i in 0..5 {
            assert!(core.tick(t0 + Duration::from_secs(5 * i)).is_empty());
        }
        assert_eq!(snapshot(&core), (false, 0));
        assert_eq!(
            rig.scans.load(Ordering::SeqCst),
            1,
            "one process scan at startup, none after"
        );
    }

    #[test]
    fn installs_known_only_later_still_get_the_startup_check() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("Game.log");
        std::fs::write(&log, format!("{HEADER}{JOIN}")).unwrap();
        let known = Arc::new(AtomicBool::new(false));
        let sources = {
            let (known, log) = (known.clone(), log.clone());
            move || {
                if known.load(Ordering::SeqCst) {
                    vec![log.clone()]
                } else {
                    Vec::new()
                }
            }
        };
        let mut core = WatchCore::new(WatchConfig::default(), sources, || true);
        let t0 = Instant::now();
        assert!(core.tick(t0).is_empty());
        known.store(true, Ordering::SeqCst);
        let signals = core.tick(t0 + Duration::from_secs(5));
        assert!(matches!(signals[0], WatchSignal::SessionStarted { .. }));
    }

    #[test]
    fn starting_while_the_game_runs_replays_the_session() {
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);

        let signals = core.tick(Instant::now());
        assert_eq!(
            signals[0],
            WatchSignal::SessionStarted {
                log: rig.live.clone()
            }
        );
        assert!(
            matches!(&signals[1], WatchSignal::Events { events, replay: true } if events.len() == 2),
            "a session already under way is a replay"
        );
        assert_eq!(snapshot(&core), (true, 2));
    }

    #[test]
    fn a_log_that_starts_moving_begins_a_session_and_a_quit_ends_it() {
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}{QUIT}")).unwrap();
        let t0 = Instant::now();
        assert!(core.tick(t0).is_empty());

        // The game starts: a fresh log replaces the old one.
        std::fs::write(&rig.live, HEADER_NEXT).unwrap();
        let signals = core.tick(t0 + Duration::from_secs(5));
        assert_eq!(signals.len(), 2, "started + the header's event");
        assert!(
            matches!(signals[1], WatchSignal::Events { replay: false, .. }),
            "a game that just started is live, not a replay"
        );
        assert_eq!(snapshot(&core), (true, 1));
        assert_eq!(core.interval(), WatchConfig::default().live_poll);

        append(&rig.live, LOCATION);
        let signals = core.tick(t0 + Duration::from_secs(7));
        assert!(matches!(
            &signals[..],
            [WatchSignal::Events { events, replay: false }] if events.len() == 1
        ));

        append(&rig.live, QUIT);
        let signals = core.tick(t0 + Duration::from_secs(9));
        assert_eq!(signals.last(), Some(&WatchSignal::SessionEnded));
        // The finished session stays readable.
        assert_eq!(snapshot(&core), (false, 3));
        assert_eq!(core.interval(), WatchConfig::default().idle_poll);

        // The shutdown's last untracked lines are not a new session.
        append(
            &rig.live,
            "<2026-11-13T18:52:46.499Z> [CIG] CCIGBroker::FastShutdown\n",
        );
        assert!(core.tick(t0 + Duration::from_secs(14)).is_empty());
        assert!(core.tick(t0 + Duration::from_secs(19)).is_empty());

        // The next start replaces the log: that is one.
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}")).unwrap();
        let signals = core.tick(t0 + Duration::from_secs(24));
        assert!(matches!(signals[0], WatchSignal::SessionStarted { .. }));
        assert_eq!(snapshot(&core), (true, 2));
    }

    #[test]
    fn the_other_channels_log_is_followed_when_it_is_the_one_that_moves() {
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{QUIT}")).unwrap();
        let t0 = Instant::now();
        assert!(core.tick(t0).is_empty());

        // PTU is played for the first time: its log appears.
        std::fs::write(&rig.ptu, format!("{HEADER_NEXT}{JOIN}")).unwrap();
        let signals = core.tick(t0 + Duration::from_secs(5));
        assert_eq!(
            signals[0],
            WatchSignal::SessionStarted {
                log: rig.ptu.clone()
            }
        );
        append(&rig.ptu, LOCATION);
        core.tick(t0 + Duration::from_secs(7));
        assert_eq!(snapshot(&core), (true, 3));
    }

    #[test]
    fn a_crash_ends_the_session_once_the_process_is_gone() {
        let config = WatchConfig::default();
        let (rig, mut core) = rig(config);
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();
        core.tick(t0);
        let scans_after_start = rig.scans.load(Ordering::SeqCst);

        // Quiet, but still running: asked once per quiet period.
        assert!(core.tick(t0 + Duration::from_secs(10)).is_empty());
        assert_eq!(rig.scans.load(Ordering::SeqCst), scans_after_start);
        assert!(core.tick(t0 + config.quiet).is_empty());
        assert_eq!(rig.scans.load(Ordering::SeqCst), scans_after_start + 1);
        assert!(snapshot(&core).0);

        rig.running.store(false, Ordering::SeqCst);
        let signals = core.tick(t0 + config.quiet * 2);
        assert_eq!(signals, vec![WatchSignal::SessionEnded]);
        assert_eq!(snapshot(&core), (false, 2));
    }

    #[test]
    fn a_restart_between_two_polls_ends_one_session_and_starts_the_next() {
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}{LOCATION}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();
        core.tick(t0);
        assert_eq!(snapshot(&core), (true, 3));

        std::fs::write(&rig.live, HEADER_NEXT).unwrap();
        let signals = core.tick(t0 + Duration::from_secs(2));
        assert_eq!(signals[0], WatchSignal::SessionEnded);
        assert!(matches!(signals[1], WatchSignal::SessionStarted { .. }));
        assert_eq!(snapshot(&core), (true, 1), "only the new session's events");
    }

    #[test]
    fn the_session_buffer_is_capped_oldest_first() {
        let config = WatchConfig {
            max_events: 3,
            ..WatchConfig::default()
        };
        let (rig, mut core) = rig(config);
        std::fs::write(
            &rig.live,
            format!("{HEADER}{JOIN}{LOCATION}{LOCATION}{LOCATION}"),
        )
        .unwrap();
        rig.running.store(true, Ordering::SeqCst);
        core.tick(Instant::now());
        let session = core.session.lock().unwrap();
        assert_eq!(session.events.len(), 3);
        assert_eq!(session.dropped, 2);
        assert!(matches!(
            session.events[0].event,
            LogEventKind::LocationInventoryRequested { .. }
        ));
    }

    const SHUTDOWN_TAIL: &str = "<2026-11-13T18:52:46.499Z> [CIG] CCIGBroker::FastShutdown\n";
    const UNTRACKED: &str = "<2026-11-13T16:20:14.660Z> [Notice] <StatObjLoad 0x800 Format> 'data/objectcontainers/pu/loc/x.cgf' - File exists in P4K [Team_Graphics][CoreTech]\n";

    fn older(path: &Path) {
        let earlier = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
    }

    #[test]
    fn a_quit_is_final_while_the_process_is_still_alive() {
        // The live poll reads SystemQuit before the last shutdown lines, and
        // the process is still up at the next idle ticks.
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();
        core.tick(t0);
        let scans = rig.scans.load(Ordering::SeqCst);

        append(&rig.live, QUIT);
        let signals = core.tick(t0 + Duration::from_secs(2));
        assert_eq!(signals.last(), Some(&WatchSignal::SessionEnded));

        append(&rig.live, SHUTDOWN_TAIL);
        assert!(core.tick(t0 + Duration::from_secs(7)).is_empty());
        append(&rig.live, SHUTDOWN_TAIL);
        assert!(core.tick(t0 + Duration::from_secs(12)).is_empty());
        assert_eq!(snapshot(&core), (false, 3), "not replayed as a new session");
        assert_eq!(
            rig.scans.load(Ordering::SeqCst),
            scans,
            "a log that said quit needs no process scan"
        );
    }

    #[test]
    fn a_replay_that_already_holds_the_quit_ends_at_once() {
        // The app starts while the game is shutting down: the process is
        // there, the log is complete.
        let (rig, mut core) = rig(WatchConfig::default());
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}{QUIT}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();

        let signals = core.tick(t0);
        assert!(matches!(signals[0], WatchSignal::SessionStarted { .. }));
        assert!(matches!(
            &signals[1],
            WatchSignal::Events { events, replay: true } if events.len() == 3
        ));
        assert_eq!(signals[2], WatchSignal::SessionEnded);
        assert_eq!(snapshot(&core), (false, 3));

        append(&rig.live, SHUTDOWN_TAIL);
        assert!(core.tick(t0 + Duration::from_secs(5)).is_empty());
    }

    #[test]
    fn untracked_lines_are_growth_and_cost_no_process_scan() {
        let config = WatchConfig::default();
        let (rig, mut core) = rig(config);
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();
        core.tick(t0);
        let scans = rig.scans.load(Ordering::SeqCst);

        // Ten minutes of play in which nothing tracked happens.
        for i in 1..=300u64 {
            append(&rig.live, UNTRACKED);
            assert!(core.tick(t0 + config.live_poll * i as u32).is_empty());
        }
        assert_eq!(rig.scans.load(Ordering::SeqCst), scans);
        assert!(snapshot(&core).0);
    }

    #[test]
    fn after_a_crash_the_watcher_moves_to_the_channel_that_is_running() {
        let config = WatchConfig::default();
        let (rig, mut core) = rig(config);
        std::fs::write(&rig.ptu, format!("{HEADER}{QUIT}")).unwrap();
        older(&rig.ptu);
        std::fs::write(&rig.live, format!("{HEADER_NEXT}{JOIN}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();
        let signals = core.tick(t0);
        assert_eq!(
            signals[0],
            WatchSignal::SessionStarted {
                log: rig.live.clone()
            }
        );

        // LIVE crashes (no quit line); the player starts PTU instead. The
        // process check alone would say "running" and keep the dead log.
        std::fs::write(&rig.ptu, format!("{HEADER}{JOIN}{LOCATION}")).unwrap();
        let signals = core.tick(t0 + config.quiet);
        assert_eq!(signals[0], WatchSignal::SessionEnded);
        assert_eq!(
            signals[1],
            WatchSignal::SessionStarted {
                log: rig.ptu.clone()
            }
        );
        assert!(matches!(
            &signals[2],
            WatchSignal::Events { events, replay: true } if events.len() == 3
        ));
        assert_eq!(snapshot(&core), (true, 3));

        // And it keeps following PTU.
        append(&rig.ptu, LOCATION);
        let signals = core.tick(t0 + config.quiet + config.live_poll);
        assert!(matches!(
            &signals[..],
            [WatchSignal::Events { replay: false, .. }]
        ));
    }

    #[test]
    fn a_session_ended_too_early_is_followed_again_as_a_replay() {
        let config = WatchConfig::default();
        let (rig, mut core) = rig(config);
        std::fs::write(&rig.live, format!("{HEADER}{JOIN}")).unwrap();
        rig.running.store(true, Ordering::SeqCst);
        let t0 = Instant::now();
        core.tick(t0);

        // One process scan misses the game while its log is quiet.
        rig.running.store(false, Ordering::SeqCst);
        assert_eq!(
            core.tick(t0 + config.quiet),
            vec![WatchSignal::SessionEnded]
        );

        rig.running.store(true, Ordering::SeqCst);
        append(&rig.live, LOCATION);
        let signals = core.tick(t0 + config.quiet + config.idle_poll);
        assert!(matches!(signals[0], WatchSignal::SessionStarted { .. }));
        assert!(matches!(
            &signals[1],
            WatchSignal::Events { events, replay: true } if events.len() == 3
        ));
    }

    #[test]
    fn the_thread_stops_when_its_handle_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("Game.log");
        std::fs::write(&log, format!("{HEADER}{JOIN}")).unwrap();
        let (tx, rx) = mpsc::channel();
        let watch = spawn(
            WatchConfig::default(),
            move || vec![log.clone()],
            || true,
            move |signal| {
                let _ = tx.send(signal);
            },
        );
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            WatchSignal::SessionStarted { .. }
        ));
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            WatchSignal::Events { .. }
        ));
        let session = watch.session();
        assert!(session.live);
        assert_eq!(session.events.len(), 2);
        drop(watch); // joins; a hang here fails the test by timeout
    }
}
