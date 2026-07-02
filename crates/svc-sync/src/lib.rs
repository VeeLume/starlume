//! Sync service — the client side of the Starlume server: authenticated
//! requests with the device token, outbox drain (local-write queue → server),
//! and opt-in reference-data upload.
//!
//! **Intentionally empty.** Lands on the web-tier timeline (migration step 6
//! in the design doc). The shell's `auth` module already owns obtaining and
//! storing the device token; this crate consumes it.
