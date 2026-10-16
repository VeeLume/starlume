//! Mining module — the calculator: a scanned rock against a loadout, with
//! the verdict, our difficulty, the charge bar, and what would do better
//! (gadget, module swaps, a support Prospector).
//!
//! Reference data (rulesets, elements, lasers, modules, gadgets) is svc-data's
//! cooked [`svc_data::MiningIndex`] (CONTRIBUTING module rule 4); this crate
//! holds the calculator's choices about it ([`catalog`]) and the solver
//! ([`solve`]), pure computation over sc-mining's maths. The shell runs it
//! off the async runtime and mirrors the results into IPC views.

pub mod catalog;
pub mod generate;
pub mod guide;
pub mod solve;
pub mod state;

pub use catalog::{Catalog, LaserItem, Named, SHIP_RULESET};
pub use generate::{
    Area, AreaRock, ClassShares, GenerateError, GenerateRequest, Generated, OreOdds, Pick, Place,
    PlaceNode, PlaceSystem, RockClass, Sources, areas, generate, place_tree,
};
pub use guide::{Category, ClimbStep, JumpRow, ThrottleGuide, category, throttle_guide};
pub use solve::{
    CrewRow, CrewShip, EntryInput, Eval, GadgetRow, KitRow, LaserInput, LoadoutInput, Report,
    RockFacts, RockInput, SolveError, SupportRow, YourSetup, solve,
};
pub use state::{MiningState, SavedLoadout};
