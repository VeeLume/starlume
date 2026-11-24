//! Presence service — Discord Rich Presence: what a moment of play shows on
//! a Discord profile, and the client that hands it to the Discord app.
//!
//! - [`compose`] turns a [`Moment`] (what you are doing, where, in which
//!   ship, on which contract) and the user's [`Share`] choices into an
//!   [`Activity`]: Details says what, State says where, the ship is the
//!   large image, the contract lives in the small image's hover (decision
//!   01a2085d, 2026-11-23). Pure, so it is tested without Discord.
//! - [`Client`] speaks Discord's local IPC: the handshake, `SET_ACTIVITY`,
//!   and clearing it. Discord drops the activity when the connection
//!   closes, so a caller keeps one client for as long as it shows one.
//!
//! The `presence` example drives both from a console, so what Starlume
//! sends can be tried and adjusted without playing:
//! `cargo run -p svc-presence --example presence`.
//!
//! # Policy contract
//!
//! The pipe is local, but the Discord app sends the activity on to
//! Discord's servers: publishing game data is a network call. This crate
//! cannot see the settings, so the caller checks `AppState::require_online()`
//! and the presence opt-in before [`Client::connect`], and drops the client
//! when either turns off. The example is a developer tool and asks nobody.
//!
//! # Memory
//!
//! One pipe handle and the last activity sent; nothing else is held.

pub mod activity;
pub mod compose;
pub mod ipc;

pub use activity::{Activity, Assets, Button, LINE_MAX, Timestamps, cut};
pub use compose::{Contract, Doing, Moment, Place, PlaceLevel, Share, Ship, compose};
pub use ipc::{Client, Error};
