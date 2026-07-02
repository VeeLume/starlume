//! Log service — the ONE `Game.log` tailer for the whole app: rotation /
//! truncation / `logbackups` handling, typed event enum, broadcast to
//! subscribed modules (tokio broadcast channel).
//!
//! **Intentionally empty.** Sources to merge in migration step 2:
//! Hearth's `sensors` (blueprint receipts, lifecycle) and sc-cargo-planner's
//! parser (mission/objective state machine — the `(mission_id, objective_id)`
//! join-key lesson lives there). One tailer, many event consumers.
