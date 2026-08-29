//! Contract tracker — doc-only stub (the svc-log / svc-sync pattern:
//! filled by building working features, **not** by writing speculative
//! APIs first).
//!
//! ## Scope (decided 2026-08-29 — `mod-contracts`, not `mod-cargo`)
//!
//! The cargo planner proved useful in real multi-contract hauling
//! (2026-08-22 play session), but its scope was wrong: the gRPC mission
//! fetch already returns *all* active contracts, and hauling is just the
//! contract type with panes. So this module is a **contract tracker**:
//!
//! - all contract types, per-objective progress;
//! - route planning as a contract-level concern (multi-contract routes);
//! - cargo packing as the hauling-specific view (the sc-cargo-planner
//!   port target — its overlay window comes along with it);
//! - the **rep-gated contract filter** (2026-08-27 probe): show only
//!   contracts you've actually unlocked, and how far off you are for the
//!   rest. `ContractPrerequisites.required_reputation[]` joins
//!   field-for-field against `ReputationService` rows; `standing_ids` is
//!   a **whitelist of band UUIDs, not a threshold**.
//!
//! ## Constraints carried in from the probes
//!
//! - **The server's eligibility flag is a dead end** (tested 2026-08-27:
//!   500/500 contracts came back `eligible: false`, location filters
//!   inert). The client-side rep join is the only route.
//! - **No-rep-row ≠ no rep**: 68/500 live contracts gate on orgs with no
//!   rep row at all — presumably the default/neutral band, unverified.
//!   Until verified in-game, the filter must NOT hide such rows by
//!   default; a filter that lies by omission is worse than no filter.
//! - Prerequisite whitelists leak band UUIDs above the current rank —
//!   ladders can be accumulated opportunistically.
//!
//! ## Module rules that bind here
//!
//! - Network access (contract fetch, rep fetch) goes through a service /
//!   the shell behind `AppState::require_grpc("<feature>")` — feature ids
//!   registered in `settings::GRPC_FEATURES` when the first real call
//!   lands. This module never talks to the network itself.
//! - Reference data (mission catalog) comes from svc-data; this module
//!   attaches personal state (tracked contracts, progress, routes) by
//!   GUID — catalogs stay fully functional with the module disabled.
