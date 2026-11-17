//! Plans: the goals you set and what is left to do for them.
//!
//! A plan is a goal, never a checklist: its steps and its progress are
//! derived from what the account holds, so nothing here is ticked by hand.
//! Crafting projects and loadouts live in the same list (one unified plan),
//! because they share materials: the [`ledger`] counts each stack once and
//! reserves it in plan order.
//!
//! The crate is pure. The shell gathers stock, recipes, shop offers and
//! routes from the services and hands them in as the small input types of
//! [`ledger`], [`steps`] and [`session`].

pub mod ledger;
mod model;
pub mod session;
pub mod steps;

pub use model::{CraftGoal, Goal, LoadoutGoal, PartSource, Plan, PlansState, ProjectGoal};
