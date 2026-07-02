//! Data service — ONE DataCore parse + `HolotableSnapshot` cache per build,
//! shared by every module (this is what kills the 30s-per-app parse of the
//! separate-apps era).
//!
//! **Intentionally empty.** Carved out of Hearth's `sc_loader` (parse +
//! snapshot caching) — migration step 2 in the design doc. The sc-holotable
//! umbrella pin lands here when the carve-out happens, nowhere else.
