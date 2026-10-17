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
    Area, AreaRock, ClassShares, GenerateError, GenerateRequest, Generated, MassBucket, OreOdds,
    Pick, Place, PlaceNode, PlaceSystem, RockClass, RockClasses, Sources, areas, by_share,
    class_table, generate, grade, place_tree,
};
pub use guide::{Category, ClimbStep, JumpRow, ThrottleGuide, category, throttle_guide};
pub use solve::{
    BandSegment, CrewRow, CrewShip, Effect, EntryInput, Eval, GadgetRow, KitRow, LaserInput,
    LoadoutInput, MassBand, Operator, Report, RockFacts, RockInput, SolveError, SupportRow,
    TotalRow, YourSetup, mass_band, solve,
};
pub use state::{MiningState, SavedLoadout};
