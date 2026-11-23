//! Discord Rich Presence: the shell side of `svc-presence`. Reads the live
//! session (the Session Log module's), names what it names from the game
//! data, and keeps the Discord app's activity in step (decision 01a2085d).
//!
//! **Gates.** Discord publishes what it is handed, so this is a network
//! call: it needs `online_enabled` and its own switch
//! (`AppSettings::presence`, off by default), and it only runs while a game
//! session is live, which needs the Session Log module and one of its
//! sources (Game.log or telemetry). Any gate closing clears the activity
//! and closes the pipe.
//!
//! **One worker.** The pipe is blocking, so it lives on a thread of its own,
//! woken by [`nudge`] (a session change, a settings change) and by its own
//! timers: an activity changes on Discord at most every [`MIN_GAP`]
//! (Discord allows about five in twenty seconds), and a Discord app that is
//! not running is looked for again every [`RETRY`]. Nothing runs before the
//! first nudge.
//!
//! **Memory** (CLAUDE.md "Memory"). Names are resolved once per internal
//! name and kept (a handful of strings per session). Resolving loads the
//! cooked data when it is not in memory; while the window is hidden it is
//! evicted again right after, so playing with Starlume in the tray does not
//! keep the game data resident ("Cooked data is evictable").

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mod_session::{EntryKind, MissionOutcome, Session};
use svc_log::ObjectiveChange;
use svc_presence::{Activity, Client, Contract, Doing, Moment, Place, Ship, compose};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::session_names::{SessionNameQuery, default_channel, resolve};

/// The Discord application Starlume shows as (its name is what "Playing"
/// says). Not a secret: every Rich Presence client ships its id.
pub(crate) const APPLICATION_ID: &str = "1574389120380182688";

/// Emitted when what Discord shows, or why it shows nothing, changed. The
/// Settings page re-reads [`presence_state`].
pub(crate) const PRESENCE_CHANGED_EVENT: &str = "presence:changed";

/// The least time between two activities sent.
const MIN_GAP: Duration = Duration::from_secs(15);
/// How often a Discord app that was not running is looked for again.
const RETRY: Duration = Duration::from_secs(30);
/// The worker's wake-up when nothing is pending.
const IDLE: Duration = Duration::from_secs(60);

// ── What the session says ───────────────────────────────────────────────────

/// What the live session says about right now, by the game's internal names.
#[derive(Debug, Clone, Default, PartialEq)]
struct Facts {
    /// A shard was joined: past the loading screens.
    in_shard: bool,
    /// The last place arrived at.
    location: Option<String>,
    /// In quantum travel: where to.
    quantum_to: Option<String>,
    /// The vehicle seated in, its class.
    vehicle: Option<String>,
    /// The newest mission still running.
    contract: Option<Running>,
    /// Missions running.
    running: usize,
    /// Missions completed this session.
    done: usize,
    /// The session's start, Unix seconds.
    started: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Running {
    /// The title, without the tags Text Patching adds.
    title: String,
    /// Its current objective.
    objective: Option<String>,
    objectives_done: usize,
    /// When it was taken, Unix seconds.
    accepted: i64,
}

/// A quantum trip whose arrival was never seen counts as under way only
/// this long.
const QUANTUM_FOR: i64 = 10 * 60 * 1000;

fn facts(s: &Session, now_ms: i64) -> Facts {
    let mut f = Facts {
        in_shard: !s.shards.is_empty(),
        started: s.started_at / 1000,
        ..Facts::default()
    };
    // Running missions by id, in the order taken.
    let mut running: Vec<(String, Running)> = Vec::new();
    let mut quantum: Option<(String, i64)> = None;
    for e in &s.timeline {
        match &e.kind {
            EntryKind::ShardJoined { .. } => f.in_shard = true,
            EntryKind::Location { location } => {
                f.location = Some(location.clone());
                quantum = None;
            }
            EntryKind::VehicleEntered { class_name, .. } => f.vehicle = Some(class_name.clone()),
            EntryKind::VehicleLeft { .. } => {
                f.vehicle = None;
                quantum = None;
            }
            EntryKind::Died | EntryKind::Respawned => {
                f.vehicle = None;
                quantum = None;
            }
            EntryKind::QuantumTravel { to, arrived_at, .. } => {
                quantum = match arrived_at {
                    None => Some((to.clone(), e.at)),
                    Some(_) => None,
                };
            }
            EntryKind::MissionAccepted { mission_id, title } => {
                let id = mission_id.clone().unwrap_or_else(|| title.clone());
                running.retain(|(r, _)| *r != id);
                running.push((
                    id,
                    Running {
                        title: untagged(title),
                        accepted: e.at / 1000,
                        ..Running::default()
                    },
                ));
            }
            EntryKind::Objective {
                change,
                mission_id: Some(id),
                text,
            } => {
                if let Some((_, r)) = running.iter_mut().find(|(r, _)| r == id) {
                    match change {
                        ObjectiveChange::New => r.objective = Some(text.clone()),
                        ObjectiveChange::Completed => {
                            r.objectives_done += 1;
                            if r.objective.as_deref() == Some(text.as_str()) {
                                r.objective = None;
                            }
                        }
                        ObjectiveChange::Withdrawn => {
                            if r.objective.as_deref() == Some(text.as_str()) {
                                r.objective = None;
                            }
                        }
                    }
                }
            }
            EntryKind::MissionEnded {
                mission_id,
                title,
                outcome,
                ..
            } => {
                if *outcome == MissionOutcome::Completed {
                    f.done += 1;
                }
                let key = mission_id.clone().or_else(|| title.clone());
                if let Some(key) = key {
                    let plain = untagged(&key);
                    running.retain(|(id, r)| *id != key && r.title != plain);
                }
            }
            _ => {}
        }
    }
    f.quantum_to = quantum
        .filter(|(_, at)| now_ms - at < QUANTUM_FOR)
        .map(|(to, _)| to);
    f.running = running.len();
    f.contract = running.pop().map(|(_, r)| r);
    f
}

/// A title without the tags Text Patching adds at its end (`[Illegal]`,
/// `[CS Risk]`, `[~]`).
fn untagged(title: &str) -> String {
    let mut t = title.trim();
    while t.ends_with(']') {
        match t.rfind('[') {
            Some(at) if at > 0 => t = t[..at].trim_end(),
            _ => break,
        }
    }
    t.to_owned()
}

// ── Names ───────────────────────────────────────────────────────────────────

/// What a contract's catalog entry says of it.
#[derive(Debug, Clone, Default, PartialEq)]
struct ContractLook {
    kind: Option<String>,
    faction: Option<String>,
}

/// Names for internal names, from the game data of one channel; kept, so
/// the game data is read once per name.
#[derive(Default)]
struct Names {
    channel: Option<String>,
    places: HashMap<String, Place>,
    vehicles: HashMap<String, Option<Ship>>,
    contracts: HashMap<String, ContractLook>,
}

impl Names {
    fn misses(&self, f: &Facts) -> bool {
        f.location
            .iter()
            .chain(&f.quantum_to)
            .any(|p| !self.places.contains_key(p))
            || f.vehicle
                .as_ref()
                .is_some_and(|v| !self.vehicles.contains_key(v))
            || f.contract
                .as_ref()
                .is_some_and(|c| !self.contracts.contains_key(&c.title))
    }

    /// Resolve what `f` names and this does not know yet.
    fn fill(&mut self, app: &AppHandle, channel: Option<&str>, f: &Facts) {
        if self.channel.as_deref() != channel {
            *self = Names {
                channel: channel.map(str::to_owned),
                ..Names::default()
            };
        }
        if !self.misses(f) {
            return;
        }
        let channel = match channel {
            Some(c) => Some(c.to_owned()),
            None => tauri::async_runtime::block_on(default_channel(app)),
        };
        let Some(channel) = channel else { return };
        let Ok(cooked) = tauri::async_runtime::block_on(crate::data::cooked_for(app, &channel))
        else {
            // No game data: nothing is kept, so it is asked again once there is.
            return;
        };
        let places: Vec<String> = f.location.iter().chain(&f.quantum_to).cloned().collect();
        let names = resolve(
            &SessionNameQuery {
                channel: Some(channel.clone()),
                places: places.clone(),
                destinations: places,
                ..SessionNameQuery::default()
            },
            Some(&cooked),
        );
        for (internal, n) in names.places.iter().chain(&names.destinations) {
            // A place the cook knows beats a cleaned destination.
            if self.places.get(internal).is_some_and(|_| !n.resolved) {
                continue;
            }
            self.places.insert(
                internal.clone(),
                Place {
                    exact: match n.parents.first() {
                        Some(p) => format!("{}, {p}", n.name),
                        None => n.name.clone(),
                    },
                    system: n.parents.last().cloned().unwrap_or_default(),
                },
            );
        }
        if let Some(class) = &f.vehicle {
            let ship = ship_look(app, &cooked, class);
            self.vehicles.insert(class.clone(), ship);
        }
        if let Some(c) = &f.contract {
            self.contracts.insert(
                c.title.clone(),
                contract_look(&catalog_titles(&cooked.missions), &c.title),
            );
        }
        release_if_hidden(app);
    }

    fn moment(&self, f: &Facts) -> Moment {
        let doing = if !f.in_shard {
            Doing::Loading
        } else if f.quantum_to.is_some() {
            Doing::Quantum
        } else if f.vehicle.is_some() {
            Doing::Flying
        } else {
            Doing::OnFoot
        };
        let place = f
            .quantum_to
            .as_ref()
            .or(f.location.as_ref())
            .filter(|_| f.in_shard)
            .and_then(|p| self.places.get(p).cloned());
        Moment {
            doing,
            place,
            ship: f
                .vehicle
                .as_ref()
                .and_then(|v| self.vehicles.get(v).cloned().flatten()),
            contract: f.contract.as_ref().map(|c| {
                let look = self.contracts.get(&c.title).cloned().unwrap_or_default();
                Contract {
                    kind: look.kind,
                    faction: look.faction,
                    title: c.title.clone(),
                    objective: c.objective.clone(),
                    progress: progress(f, c),
                    started: Some(c.accepted),
                }
            }),
            session_start: Some(f.started),
        }
    }
}

/// Progress that names no place: other contracts running, contracts done,
/// this contract's objectives done.
fn progress(f: &Facts, c: &Running) -> Option<String> {
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    if f.running > 1 {
        Some(format!(
            "+{} more {}",
            f.running - 1,
            if f.running == 2 {
                "contract"
            } else {
                "contracts"
            }
        ))
    } else if f.done > 0 {
        Some(format!("{} done", plural(f.done, "contract", "contracts")))
    } else if c.objectives_done > 0 {
        Some(format!(
            "{} done",
            plural(c.objectives_done, "objective", "objectives")
        ))
    } else {
        None
    }
}

/// The ship a vehicle class is: its name, its career, its store picture.
fn ship_look(
    app: &AppHandle,
    cooked: &std::sync::Arc<svc_data::CookedData>,
    class: &str,
) -> Option<Ship> {
    let rows = crate::ships::ship_rows(cooked);
    let base = mod_session::names::vehicle_base(class);
    let row = rows
        .iter()
        .find(|r| r.record_name.eq_ignore_ascii_case(base))?;
    let picture = app
        .state::<AppState>()
        .shipstore
        .cached()
        .and_then(|store| {
            let ids: Vec<(u32, &str)> = store
                .ships
                .iter()
                .map(|s| (s.id, s.name.as_str()))
                .collect();
            let id = *mod_ships::match_store(&rows, &ids).get(&row.guid)?;
            let ship = store.ships.iter().find(|s| s.id == id)?;
            ship.media
                .slideshow
                .clone()
                .or_else(|| ship.media.thumbnail.clone())
        });
    Some(Ship {
        short: short_name(
            &row.name,
            row.manufacturer.as_deref(),
            row.manufacturer_code.as_deref(),
        ),
        name: row.name.clone(),
        picture,
        career: row.career.clone(),
    })
}

/// A ship's name without its maker in front: "RSI Galaxy" → "Galaxy".
fn short_name(name: &str, maker: Option<&str>, code: Option<&str>) -> String {
    let first = maker.and_then(|m| m.split_whitespace().next());
    for prefix in [maker, code, first].into_iter().flatten() {
        if let Some(rest) = name.strip_prefix(prefix).and_then(|r| r.strip_prefix(' '))
            && !rest.trim().is_empty()
        {
            return rest.trim().to_owned();
        }
    }
    name.to_owned()
}

/// A running mission's type and faction from the catalog, by its title. The
/// HUD's title is not always the catalog's (a chain step is titled by its
/// chain, "Jorrit Dossier: Retrieve Security Data", the catalog says
/// "Jorrit Dossier: Updated Security Data"; placeholders like "[TargetName]"
/// are filled in), so a title matches whole, else by the part before its
/// colon or a placeholder. What all matches agree on is kept.
fn contract_look<'a>(missions: &[CatalogTitle<'a>], title: &str) -> ContractLook {
    let stem = |t: &str| -> String {
        let t = t.split(':').next().unwrap_or(t);
        t.split('[').next().unwrap_or(t).trim().to_lowercase()
    };
    let exact: Vec<&CatalogTitle> = missions
        .iter()
        .filter(|m| m.title.eq_ignore_ascii_case(title))
        .collect();
    let matches = if exact.is_empty() {
        let want = stem(title);
        missions
            .iter()
            .filter(|m| {
                (m.title.contains(':') || m.title.contains('['))
                    && !want.is_empty()
                    && stem(m.title) == want
            })
            .collect()
    } else {
        exact
    };
    let agreed = |of: fn(&CatalogTitle<'a>) -> Option<&'a str>| -> Option<String> {
        let first = of(matches.first()?)?;
        matches
            .iter()
            .all(|m| of(m) == Some(first))
            .then(|| first.to_owned())
    };
    ContractLook {
        kind: agreed(|m| m.kind),
        faction: agreed(|m| m.faction),
    }
}

/// A catalog mission as [`contract_look`] reads it.
struct CatalogTitle<'a> {
    title: &'a str,
    kind: Option<&'a str>,
    faction: Option<&'a str>,
}

fn catalog_titles(missions: &[svc_data::MissionEntry]) -> Vec<CatalogTitle<'_>> {
    missions
        .iter()
        .filter_map(|m| {
            Some(CatalogTitle {
                title: m.title.as_deref()?,
                kind: m.category.as_ref().and_then(|c| c.name.as_deref()),
                faction: m.faction.as_ref().and_then(|f| f.name.as_deref()),
            })
        })
        .collect()
}

/// While the window is hidden, nothing else wants the cooked data: let it
/// go again ("Cooked data is evictable").
fn release_if_hidden(app: &AppHandle) {
    let shown = app
        .get_webview_window(crate::window::MAIN)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !shown {
        app.state::<AppState>().data.evict();
    }
}

static NAMES: LazyLock<Mutex<Names>> = LazyLock::new(Mutex::default);

/// What the live session shows, as `settings` shares it; `None` when no
/// session is live.
fn wanted(app: &AppHandle, settings: &crate::settings::PresenceSettings) -> Option<Activity> {
    let session = crate::session::live(app)?;
    let f = facts(&session, chrono::Utc::now().timestamp_millis());
    let mut names = NAMES.lock().unwrap();
    names.fill(app, session.channel.as_deref(), &f);
    Some(compose(&names.moment(&f), &settings.share()))
}

// ── The worker ──────────────────────────────────────────────────────────────

/// Why Discord shows what it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PresenceStatus {
    /// The switch is off.
    #[default]
    Off,
    /// Online features are off.
    Offline,
    /// The Session Log module is off, or neither of its sources is on.
    NoSession,
    /// No game session is live.
    NotPlaying,
    /// The Discord app is not running (looked for again every 30 s).
    NoDiscord,
    /// Discord shows the activity.
    Shown,
}

/// What Discord shows now, for Settings.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, specta::Type)]
pub struct PresenceView {
    pub status: PresenceStatus,
    /// What Discord shows; while off, what it would show (the preview).
    pub activity: Option<Activity>,
    /// The Discord user the app is signed in as, while connected.
    pub user: Option<String>,
}

static VIEW: LazyLock<Mutex<PresenceView>> = LazyLock::new(Mutex::default);
static WAKE: OnceLock<mpsc::Sender<()>> = OnceLock::new();

/// Something the activity depends on changed. Starts the worker on first
/// use; never blocks.
pub(crate) fn nudge(app: &AppHandle) {
    let tx = WAKE.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("presence".into())
            .spawn(move || Worker::default().run(&app, &rx));
        if let Err(e) = spawned {
            tracing::warn!("the Rich Presence worker did not start: {e}");
        }
        tx
    });
    let _ = tx.send(());
}

#[derive(Default)]
struct Worker {
    client: Option<Client>,
    sent: Option<Activity>,
    sent_at: Option<Instant>,
    /// A wanted activity held back by [`MIN_GAP`].
    held: bool,
    /// When to look for the Discord app again.
    retry_at: Option<Instant>,
}

impl Worker {
    fn run(mut self, app: &AppHandle, rx: &mpsc::Receiver<()>) {
        // A nudge or a timer, until the app goes away.
        while let Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(self.wait()) {
            while rx.try_recv().is_ok() {}
            self.tick(app);
        }
    }

    fn wait(&self) -> Duration {
        let now = Instant::now();
        let held = self
            .held
            .then(|| {
                self.sent_at
                    .map(|at| (at + MIN_GAP).saturating_duration_since(now))
            })
            .flatten();
        let retry = self.retry_at.map(|at| at.saturating_duration_since(now));
        [held, retry, Some(IDLE)]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(IDLE)
    }

    fn tick(&mut self, app: &AppHandle) {
        let (settings, online, sources, module) = {
            let s = app.state::<AppState>().settings.lock().unwrap().clone();
            (
                s.presence.clone(),
                s.online_enabled,
                s.gamelog_enabled || s.telemetry_enabled,
                s.enabled_modules
                    .iter()
                    .any(|m| m == crate::session::MODULE_ID),
            )
        };
        let status = if !settings.enabled {
            PresenceStatus::Off
        } else if !online {
            PresenceStatus::Offline
        } else if !module || !sources {
            PresenceStatus::NoSession
        } else {
            PresenceStatus::NotPlaying
        };
        let wanted = if status == PresenceStatus::NotPlaying {
            wanted(app, &settings)
        } else {
            None
        };
        let status = match &wanted {
            None => {
                self.close();
                status
            }
            Some(activity) => self.show(activity),
        };
        let view = PresenceView {
            status,
            activity: wanted,
            user: self
                .client
                .as_ref()
                .and_then(|c| c.user())
                .map(str::to_owned),
        };
        let changed = *VIEW.lock().unwrap() != view;
        if changed {
            *VIEW.lock().unwrap() = view;
            let _ = app.emit(PRESENCE_CHANGED_EVENT, ());
        }
    }

    /// Show `activity`, at most every [`MIN_GAP`].
    fn show(&mut self, activity: &Activity) -> PresenceStatus {
        if self.sent.as_ref() == Some(activity) && self.client.is_some() {
            self.held = false;
            return PresenceStatus::Shown;
        }
        if self.sent_at.is_some_and(|at| at.elapsed() < MIN_GAP) {
            self.held = true;
            return if self.client.is_some() {
                PresenceStatus::Shown
            } else {
                PresenceStatus::NoDiscord
            };
        }
        self.held = false;
        if self.client.is_none() {
            if self.retry_at.is_some_and(|at| Instant::now() < at) {
                return PresenceStatus::NoDiscord;
            }
            match Client::connect(APPLICATION_ID) {
                Ok(c) => {
                    self.client = Some(c);
                    self.retry_at = None;
                }
                Err(e) => {
                    tracing::debug!("Rich Presence: {e}");
                    self.retry_at = Some(Instant::now() + RETRY);
                    return PresenceStatus::NoDiscord;
                }
            }
        }
        let client = self.client.as_mut().expect("connected above");
        match client.set_activity(activity) {
            Ok(()) => {
                self.sent = Some(activity.clone());
                self.sent_at = Some(Instant::now());
                PresenceStatus::Shown
            }
            Err(e) => {
                // Discord quit, or refused it: start over on the next try.
                tracing::warn!("Rich Presence: {e}");
                self.client = None;
                self.sent = None;
                self.retry_at = Some(Instant::now() + RETRY);
                PresenceStatus::NoDiscord
            }
        }
    }

    /// Take the activity off and close the pipe.
    fn close(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.clear();
        }
        self.sent = None;
        self.held = false;
        self.retry_at = None;
    }
}

// ── IPC ─────────────────────────────────────────────────────────────────────

/// What Discord shows now and why; while the switch is off, what it would
/// show, for the preview.
#[tauri::command]
#[specta::specta]
pub(crate) async fn presence_state(app: AppHandle) -> PresenceView {
    nudge(&app);
    let mut view = VIEW.lock().unwrap().clone();
    if view.activity.is_none() && view.status != PresenceStatus::Shown {
        let settings = app
            .state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .presence
            .clone();
        view.activity = tokio::task::spawn_blocking(move || wanted(&app, &settings))
            .await
            .ok()
            .flatten();
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_session::Entry;

    fn at(ms: i64, kind: EntryKind) -> Entry {
        Entry {
            at: ms,
            kind,
            ..Entry::default()
        }
    }

    fn session(timeline: Vec<Entry>) -> Session {
        Session {
            started_at: 1_000_000,
            live: true,
            shards: Vec::new(),
            timeline,
            ..Session::default()
        }
    }

    fn accepted(ms: i64, id: &str, title: &str) -> Entry {
        at(
            ms,
            EntryKind::MissionAccepted {
                mission_id: Some(id.into()),
                title: title.into(),
            },
        )
    }

    #[test]
    fn before_a_shard_is_loading() {
        let f = facts(&session(vec![]), 2_000_000);
        assert!(!f.in_shard);
        assert_eq!(Names::default().moment(&f).doing, Doing::Loading);
        assert_eq!(f.started, 1000);
    }

    #[test]
    fn a_ship_entered_and_left() {
        let s = session(vec![
            at(
                1,
                EntryKind::ShardJoined {
                    shard: "pub".into(),
                },
            ),
            at(
                2,
                EntryKind::VehicleEntered {
                    class_name: "RSI_Galaxy".into(),
                    name: "g".into(),
                },
            ),
        ]);
        assert_eq!(facts(&s, 3).vehicle.as_deref(), Some("RSI_Galaxy"));
        let mut s = s;
        s.timeline.push(at(
            3,
            EntryKind::VehicleLeft {
                class_name: "RSI_Galaxy".into(),
                name: "g".into(),
            },
        ));
        assert_eq!(facts(&s, 4).vehicle, None);
    }

    #[test]
    fn quantum_is_under_way_until_it_arrives_or_goes_stale() {
        let trip = |arrived: Option<i64>| {
            at(
                100,
                EntryKind::QuantumTravel {
                    from: None,
                    to: "OOC_Stanton_2c_Yela".into(),
                    vehicle: None,
                    arrived_at: arrived,
                },
            )
        };
        let s = session(vec![trip(None)]);
        assert_eq!(
            facts(&s, 200).quantum_to.as_deref(),
            Some("OOC_Stanton_2c_Yela")
        );
        assert_eq!(facts(&s, 100 + QUANTUM_FOR + 1).quantum_to, None);
        assert_eq!(facts(&session(vec![trip(Some(150))]), 200).quantum_to, None);
    }

    #[test]
    fn the_newest_running_contract_shows_with_the_others_counted() {
        let s = session(vec![
            accepted(10_000, "a", "Little Surprise for My Daughter"),
            accepted(20_000, "b", "Payback For A Bastard [~]"),
            at(
                30_000,
                EntryKind::Objective {
                    change: ObjectiveChange::New,
                    mission_id: Some("b".into()),
                    text: "Find him".into(),
                },
            ),
        ]);
        let f = facts(&s, 40_000);
        let c = f.contract.clone().unwrap();
        assert_eq!(c.title, "Payback For A Bastard");
        assert_eq!(c.objective.as_deref(), Some("Find him"));
        assert_eq!(c.accepted, 20);
        assert_eq!(f.running, 2);
        assert_eq!(progress(&f, &c).as_deref(), Some("+1 more contract"));
    }

    #[test]
    fn an_ended_contract_leaves_and_counts_when_done() {
        let s = session(vec![
            accepted(10_000, "a", "Deep space hit"),
            at(
                20_000,
                EntryKind::MissionEnded {
                    mission_id: Some("a".into()),
                    title: Some("Deep space hit".into()),
                    contract: None,
                    outcome: MissionOutcome::Completed,
                },
            ),
            accepted(30_000, "b", "Deep space hit"),
        ]);
        let f = facts(&s, 40_000);
        assert_eq!(f.running, 1);
        assert_eq!(f.done, 1);
        assert_eq!(
            progress(&f, f.contract.as_ref().unwrap()).as_deref(),
            Some("1 contract done")
        );
    }

    #[test]
    fn our_tags_come_off_the_title() {
        assert_eq!(untagged("Surprise Party [Illegal]"), "Surprise Party");
        assert_eq!(
            untagged("Block XenoThreat Expansion [CS Risk]"),
            "Block XenoThreat Expansion"
        );
        assert_eq!(
            untagged("Payback For A Bastard [~]"),
            "Payback For A Bastard"
        );
        assert_eq!(untagged("Deep space hit"), "Deep space hit");
    }

    #[test]
    fn the_maker_comes_off_the_ship_name() {
        assert_eq!(
            short_name("RSI Galaxy", Some("Roberts Space Industries"), Some("RSI")),
            "Galaxy"
        );
        assert_eq!(
            short_name("Drake Marauder", Some("Drake Interplanetary"), Some("DRAK")),
            "Marauder"
        );
        assert_eq!(
            short_name("Galaxy", Some("Roberts Space Industries"), Some("RSI")),
            "Galaxy"
        );
    }

    fn mission<'a>(title: &'a str, kind: &'a str, faction: Option<&'a str>) -> CatalogTitle<'a> {
        CatalogTitle {
            title,
            kind: Some(kind),
            faction,
        }
    }

    #[test]
    fn a_contract_matches_whole_or_by_its_chain() {
        let missions = vec![
            mission("Deep space hit", "Mercenary", Some("Headhunters")),
            mission(
                "Jorrit Dossier: Updated Security Data",
                "Investigation",
                None,
            ),
            mission(
                "Jorrit Dossier: Onyx Personnel Files",
                "Investigation",
                None,
            ),
            mission(
                "Missing: [TargetName]",
                "Investigation",
                Some("Citizens For Prosperity"),
            ),
        ];
        assert_eq!(
            contract_look(&missions, "Deep space hit"),
            ContractLook {
                kind: Some("Mercenary".into()),
                faction: Some("Headhunters".into())
            }
        );
        assert_eq!(
            contract_look(&missions, "Jorrit Dossier: Retrieve Security Data")
                .kind
                .as_deref(),
            Some("Investigation")
        );
        assert_eq!(
            contract_look(&missions, "Missing: Scott Akaboshi")
                .faction
                .as_deref(),
            Some("Citizens For Prosperity")
        );
        assert_eq!(
            contract_look(&missions, "Little Surprise for My Daughter"),
            ContractLook::default()
        );
    }
}
