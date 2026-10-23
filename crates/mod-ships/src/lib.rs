//! Ships module — loadouts: what each port of a ship holds, grouped by
//! system the way a pilot thinks of them, the ship's power network as the
//! engineering screen shows it, and what else fits a port.
//!
//! Reference data (vehicles, ports, stock loadouts, resource profiles,
//! component sheets) is svc-data's cooked [`svc_data::ShipsIndex`]
//! (CONTRIBUTING module rule 4); this crate is pure computation over it.
//! The shell runs it off the async runtime and mirrors the results into
//! IPC views.

pub mod candidates;
pub mod catalog;
pub mod crew;
pub mod home;
pub mod loadouts;
pub mod power;
pub mod routes;
pub mod state;
pub mod store;
pub mod tree;
pub mod view;

pub use candidates::{Candidate, Impact, Stat, candidates};
pub use catalog::{ShipRow, ships};
pub use crew::{Seats, seats};
pub use home::{Entitlement, Home, Owned, home};
pub use loadouts::{Draft, Loadouts, SavedLoadout};
pub use power::{Column, Mode, Power, PowerInput, Source, power};
pub use routes::{Place, Places, places};
pub use state::ShipsState;
pub use store::match_store;
pub use tree::{Slot, System, slots};
pub use view::{
    ArmorSummary, BoostLevel, Edit, Gun, HullPart, HullSummary, InsuranceSummary, LoadoutInput,
    QuantumSummary, ShieldSummary, ShipView, StorageSummary, Summary, Tour, Typed, ViewError,
    loadout, view,
};
