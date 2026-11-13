//! The current state the app keeps from the stream, and the reducer that
//! folds frames into it.

use sc_dossier::telemetry as t;
use serde::Serialize;

/// How many events the snapshot keeps (newest last).
pub const EVENTS_KEPT: usize = 16;

/// What the app knows from the stream right now. Small on purpose: it lives
/// Rust-side for as long as the opt-in is on (a suspended webview runs no
/// JS), and a consumer that needs more of a frame adds the field here.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Snapshot {
    /// aUEC, as last sent. Kept when the stream ends: the last known balance
    /// is still the best one.
    pub balance: Option<i64>,
    /// The vehicle the player is seated in; `None` on foot and when not
    /// connected.
    pub vehicle: Option<VehicleState>,
    /// The player character; `None` when not connected.
    pub actor: Option<ActorState>,
    /// The last [`EVENTS_KEPT`] events, newest last. The game's events are
    /// incomplete (the first seat entry of a session sends none).
    pub events: Vec<EventRecord>,
    /// When the last frame arrived, Unix seconds. Not a change by itself.
    pub updated_at: Option<i64>,
}

/// The vehicle the player is in.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VehicleState {
    /// The class key (`KRIG_S65_Stingray`): look the ship up by this.
    pub class_name: String,
    /// Entity name with an id suffix: tells two ships of one class apart.
    pub name: String,
    pub manufacturer: String,
    pub kind: t::VehicleKind,
    /// On the ground. `None` when the frame has no flight block.
    pub landed: Option<bool>,
    /// m/s. Wrong in quantum travel (the game's value freezes).
    pub speed: Option<f32>,
    /// 0–1.
    pub hydrogen_fuel: Option<f32>,
    /// m above the body's datum; `None` away from a body.
    pub altitude: Option<f32>,
    /// Degrees 0–360; `None` away from a body.
    pub heading: Option<f32>,
    /// Power plants on. `None` when the frame has no power block.
    pub powered: Option<bool>,
    /// `None` when not sent (LIVE 4.10.2 sends no door state).
    pub doors_open: Option<bool>,
    pub doors_locked: Option<bool>,
}

/// A value with its maximum.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Gauge {
    pub current: f32,
    pub max: f32,
}

/// The player character.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActorState {
    pub health: Gauge,
    pub oxygen: Gauge,
    pub hunger: Gauge,
    pub thirst: Gauge,
    /// °C.
    pub body_temperature: Gauge,
    pub locomotion: t::Locomotion,
    pub crew_role: t::CrewRole,
    /// Seat entity name; empty on foot.
    pub seat_name: String,
    pub breathable: bool,
    /// °C; -273.15 in vacuum.
    pub external_temperature: f32,
}

/// One event with the time its frame arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EventRecord {
    pub event: t::Event,
    /// Unix seconds.
    pub at: i64,
}

/// How much a frame changed the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Nothing the snapshot keeps moved.
    None,
    /// A fast-moving value moved (speed, fuel, altitude, a vital).
    Minor,
    /// Something a person would call a change: the balance, the vehicle, the
    /// seat, landed or not, power, doors, or an event.
    Major,
}

/// Folds frames into a [`Snapshot`].
#[derive(Debug, Default)]
pub struct Reducer {
    snapshot: Snapshot,
}

impl Reducer {
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Take a frame in. `now` is Unix seconds.
    pub fn apply(&mut self, frame: &t::Frame, now: i64) -> Change {
        let vehicle = frame.vehicle.as_ref().map(vehicle_state);
        let actor = frame.actor.as_ref().map(actor_state);
        // A frame without an actor says nothing about the balance.
        let balance = frame
            .actor
            .as_ref()
            .map(|a| a.uec_balance)
            .or(self.snapshot.balance);

        let major = !frame.events.is_empty()
            || balance != self.snapshot.balance
            || vehicle_key(&vehicle) != vehicle_key(&self.snapshot.vehicle)
            || actor_key(&actor) != actor_key(&self.snapshot.actor);
        let minor = vehicle != self.snapshot.vehicle || actor != self.snapshot.actor;

        self.snapshot.balance = balance;
        self.snapshot.vehicle = vehicle;
        self.snapshot.actor = actor;
        self.snapshot.updated_at = Some(now);
        for &event in &frame.events {
            self.snapshot.events.push(EventRecord { event, at: now });
        }
        let extra = self.snapshot.events.len().saturating_sub(EVENTS_KEPT);
        self.snapshot.events.drain(..extra);

        if major {
            Change::Major
        } else if minor {
            Change::Minor
        } else {
            Change::None
        }
    }

    /// The stream ended: what was live (vehicle, actor) is gone; the balance
    /// and the events stay as last known.
    pub fn disconnected(&mut self) -> Change {
        let had = self.snapshot.vehicle.is_some() || self.snapshot.actor.is_some();
        self.snapshot.vehicle = None;
        self.snapshot.actor = None;
        if had { Change::Major } else { Change::None }
    }
}

/// The part of a vehicle whose change is a [`Change::Major`].
type VehicleKey<'a> = Option<(
    &'a str,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
)>;

fn vehicle_key(v: &Option<VehicleState>) -> VehicleKey<'_> {
    v.as_ref().map(|v| {
        (
            v.name.as_str(),
            v.landed,
            v.powered,
            v.doors_open,
            v.doors_locked,
        )
    })
}

/// The part of the actor whose change is a [`Change::Major`].
fn actor_key(a: &Option<ActorState>) -> Option<(t::Locomotion, t::CrewRole, &str)> {
    a.as_ref()
        .map(|a| (a.locomotion, a.crew_role, a.seat_name.as_str()))
}

fn vehicle_state(v: &t::Vehicle) -> VehicleState {
    let instruments = v.flight.as_ref().map(|f| &f.instruments);
    VehicleState {
        class_name: v.class_name.clone(),
        name: v.name.clone(),
        manufacturer: v.manufacturer.clone(),
        kind: v.kind,
        landed: v.flight.as_ref().map(|f| f.landed),
        speed: instruments.map(|i| i.speed),
        hydrogen_fuel: instruments.map(|i| i.hydrogen_fuel),
        altitude: instruments.and_then(|i| i.altitude),
        heading: instruments.and_then(|i| i.heading),
        powered: v.power.as_ref().map(|p| p.powerplants),
        doors_open: v.doors.as_ref().and_then(|d| d.open),
        doors_locked: v.doors.as_ref().and_then(|d| d.locked),
    }
}

fn gauge(g: t::Gauge) -> Gauge {
    Gauge {
        current: g.current,
        max: g.max,
    }
}

fn actor_state(a: &t::Actor) -> ActorState {
    ActorState {
        health: gauge(a.health),
        oxygen: gauge(a.oxygen),
        hunger: gauge(a.hunger),
        thirst: gauge(a.thirst),
        body_temperature: gauge(a.body_temperature),
        locomotion: a.locomotion,
        crew_role: a.crew_role,
        seat_name: a.seat_name.clone(),
        breathable: a.breathable,
        external_temperature: a.external_temperature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame as the stream would deliver it. The library's types are
    /// `non_exhaustive`, so tests build them from JSON.
    fn frame(json: serde_json::Value) -> t::Frame {
        let mut full = serde_json::to_value(t::Frame::default()).unwrap();
        merge(&mut full, json);
        serde_json::from_value(full).unwrap()
    }

    fn merge(into: &mut serde_json::Value, from: serde_json::Value) {
        match (into, from) {
            (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                for (k, v) in b {
                    merge(a.entry(k).or_insert(serde_json::Value::Null), v);
                }
            }
            (a, b) => *a = b,
        }
    }

    fn actor(balance: i64, locomotion: &str) -> serde_json::Value {
        let mut a = serde_json::to_value(t::Actor::default()).unwrap();
        merge(
            &mut a,
            serde_json::json!({ "uec_balance": balance, "locomotion": locomotion }),
        );
        a
    }

    fn vehicle(name: &str, speed: f32, landed: bool) -> serde_json::Value {
        let mut v = serde_json::to_value(t::Vehicle::default()).unwrap();
        let mut flight = serde_json::to_value(t::Flight::default()).unwrap();
        merge(
            &mut flight,
            serde_json::json!({ "landed": landed, "instruments": { "speed": speed } }),
        );
        merge(
            &mut v,
            serde_json::json!({ "name": name, "class_name": "KRIG_S65_Stingray", "flight": flight }),
        );
        v
    }

    #[test]
    fn the_first_frame_is_a_major_change_and_a_repeat_is_none() {
        let mut r = Reducer::default();
        let f = frame(serde_json::json!({ "actor": actor(1500, "OnFoot") }));
        assert_eq!(r.apply(&f, 10), Change::Major);
        assert_eq!(r.snapshot().balance, Some(1500));
        assert_eq!(r.snapshot().updated_at, Some(10));
        assert_eq!(r.apply(&f, 11), Change::None);
        // The arrival time moves without being a change.
        assert_eq!(r.snapshot().updated_at, Some(11));
    }

    #[test]
    fn a_balance_change_is_major() {
        let mut r = Reducer::default();
        r.apply(
            &frame(serde_json::json!({ "actor": actor(1500, "OnFoot") })),
            1,
        );
        let paid = frame(serde_json::json!({ "actor": actor(900, "OnFoot") }));
        assert_eq!(r.apply(&paid, 2), Change::Major);
        assert_eq!(r.snapshot().balance, Some(900));
    }

    #[test]
    fn speed_is_minor_and_landing_is_major() {
        let mut r = Reducer::default();
        let seated = |speed, landed| {
            frame(serde_json::json!({
                "actor": actor(1, "Seated"),
                "vehicle": vehicle("Stingray_1", speed, landed),
            }))
        };
        assert_eq!(r.apply(&seated(0.0, true), 1), Change::Major);
        assert_eq!(r.apply(&seated(0.0, false), 2), Change::Major);
        assert_eq!(r.apply(&seated(120.0, false), 3), Change::Minor);
        assert_eq!(r.apply(&seated(120.0, false), 4), Change::None);
        let v = r.snapshot().vehicle.as_ref().unwrap();
        assert_eq!((v.speed, v.landed), (Some(120.0), Some(false)));
        assert_eq!(v.class_name, "KRIG_S65_Stingray");
    }

    #[test]
    fn leaving_the_seat_drops_the_vehicle() {
        let mut r = Reducer::default();
        r.apply(
            &frame(serde_json::json!({
                "actor": actor(1, "Seated"),
                "vehicle": vehicle("Stingray_1", 0.0, true),
            })),
            1,
        );
        let on_foot = frame(serde_json::json!({ "actor": actor(1, "OnFoot") }));
        assert_eq!(r.apply(&on_foot, 2), Change::Major);
        assert!(r.snapshot().vehicle.is_none());
    }

    #[test]
    fn events_are_kept_newest_last_and_capped() {
        let mut r = Reducer::default();
        let f = frame(serde_json::json!({ "events": ["EnterSeat", "LeaveSeat"] }));
        for at in 0..20 {
            assert_eq!(r.apply(&f, at), Change::Major);
        }
        let events = &r.snapshot().events;
        assert_eq!(events.len(), EVENTS_KEPT);
        assert_eq!(
            events.last(),
            Some(&EventRecord {
                event: t::Event::LeaveSeat,
                at: 19
            })
        );
        assert_eq!(events[0].at, 12);
    }

    #[test]
    fn a_frame_without_an_actor_keeps_the_balance() {
        let mut r = Reducer::default();
        r.apply(
            &frame(serde_json::json!({ "actor": actor(700, "OnFoot") })),
            1,
        );
        r.apply(&frame(serde_json::json!({})), 2);
        assert_eq!(r.snapshot().balance, Some(700));
        assert!(r.snapshot().actor.is_none());
    }

    #[test]
    fn a_disconnect_keeps_the_balance_and_the_events() {
        let mut r = Reducer::default();
        r.apply(
            &frame(serde_json::json!({ "actor": actor(700, "OnFoot"), "events": ["Spawn"] })),
            1,
        );
        assert_eq!(r.disconnected(), Change::Major);
        let s = r.snapshot();
        assert!(s.actor.is_none() && s.vehicle.is_none());
        assert_eq!(s.balance, Some(700));
        assert_eq!(s.events.len(), 1);
        // Nothing live left to drop.
        assert_eq!(r.disconnected(), Change::None);
    }
}
