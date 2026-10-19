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
pub mod hud;
mod lagrange;
pub mod player;
pub mod solve;
pub mod state;

pub use catalog::{Catalog, LaserItem, Named, SHIP_RULESET};
pub use generate::{
    Area, AreaRock, ClassShares, Comfort, ComfortBar, GenerateError, GenerateRequest, Generated,
    MassBucket, MassRange, OreOdds, Pick, Place, PlaceGroup, PlaceNode, PlaceSystem, RockClass,
    RockClasses, Sources, areas, by_share, class_table, comfort, generate, grade, grade_share,
    place_tree,
};
pub use guide::{Category, ClimbStep, JumpRow, ThrottleGuide, category, throttle_guide};
pub use hud::{
    Basis, NameKind, NameSet, PanelSpot, ScanReading, ScanRow, load_scan_data, panel_crop,
    read_scan, scan_data_loaded,
};
pub use solve::{
    BackOff, BandSegment, Blast, CrewRow, CrewShip, Effect, EntryInput, Eval, GadgetRow, KitRow,
    LaserInput, LoadoutInput, MassBand, Operator, RangeAdvice, Report, RockFacts, RockInput,
    ScannerValues, SolveError, Superseded, SupportRow, TotalRow, YourSetup, compare, mass_band,
    modified_for, scanner_values, solve,
};
pub use state::{MiningState, SavedLoadout};
