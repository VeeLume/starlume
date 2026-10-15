//! Request policy: the app-wide pacer and the throttle backoff.
//!
//! Spectrum rate-limits per IP **and per route** (~1 request/second); past it
//! the server answers `ErrThrottleLimit` (HTTP 200, not a 429) and the limit
//! clears within ~2 minutes. Both halves of the policy are therefore keyed by
//! endpoint. Pure state over an injected clock, so tests drive it in virtual
//! time.

use std::collections::HashMap;
use std::time::Duration;

use tokio::time::Instant;

/// Requests per second the pacer allows per endpoint — under the server's
/// ~1 req/s so clock jitter and a concurrent web session don't trip it.
pub const MAX_REQUESTS_PER_SECOND: f64 = 0.8;

/// Minimum spacing between two requests to one endpoint
/// (1 / [`MAX_REQUESTS_PER_SECOND`]).
pub const MIN_INTERVAL: Duration = Duration::from_millis(1250);

/// How long an endpoint stays shut after `ErrThrottleLimit`. The server clears
/// within ~120 s; the margin keeps the first retry from re-tripping it.
pub const THROTTLE_BACKOFF: Duration = Duration::from_secs(150);

/// Per-endpoint pacing slots and throttle backoffs. One per service — the
/// service is app-wide, so the pace is too.
#[derive(Debug, Default)]
pub(crate) struct Policy {
    /// Earliest instant the next request to each endpoint may go out.
    next_slot: HashMap<&'static str, Instant>,
    /// Active backoffs: until when (monotonic, for the check) and the same
    /// moment in Unix seconds (for surfacing across IPC).
    backoff: HashMap<&'static str, (Instant, i64)>,
}

impl Policy {
    /// Reserve the next request slot for `endpoint` and return how long to
    /// wait before sending. Reserving (not just reading) keeps concurrent
    /// callers spaced: each takes its own slot.
    pub(crate) fn reserve(&mut self, endpoint: &'static str, now: Instant) -> Duration {
        let slot = self
            .next_slot
            .get(endpoint)
            .copied()
            .filter(|slot| *slot > now)
            .unwrap_or(now);
        self.next_slot.insert(endpoint, slot + MIN_INTERVAL);
        slot - now
    }

    /// `Some(until)` (Unix seconds) while `endpoint` is backing off.
    pub(crate) fn backing_off(&self, endpoint: &'static str, now: Instant) -> Option<i64> {
        self.backoff
            .get(endpoint)
            .filter(|(until, _)| *until > now)
            .map(|(_, until_unix)| *until_unix)
    }

    /// Record a throttle on `endpoint`: shut it for [`THROTTLE_BACKOFF`].
    /// Returns the end of the backoff in Unix seconds.
    pub(crate) fn throttled(&mut self, endpoint: &'static str, now: Instant, now_unix: i64) -> i64 {
        let until_unix = now_unix + THROTTLE_BACKOFF.as_secs() as i64;
        self.backoff
            .insert(endpoint, (now + THROTTLE_BACKOFF, until_unix));
        until_unix
    }

    /// The latest end (Unix seconds) of any active backoff — what a status
    /// surface shows. `None` when nothing is backing off.
    pub(crate) fn latest_backoff(&self, now: Instant) -> Option<i64> {
        self.backoff
            .values()
            .filter(|(until, _)| *until > now)
            .map(|(_, until_unix)| *until_unix)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "/a";
    const B: &str = "/b";

    #[test]
    fn rate_is_point_eight_per_second() {
        assert_eq!(MIN_INTERVAL.as_secs_f64(), 1.0 / MAX_REQUESTS_PER_SECOND);
    }

    #[test]
    fn requests_to_one_endpoint_are_spaced() {
        let mut p = Policy::default();
        let t0 = Instant::now();
        // The first goes now; back-to-back callers each queue one interval
        // further out.
        assert_eq!(p.reserve(A, t0), Duration::ZERO);
        assert_eq!(p.reserve(A, t0), MIN_INTERVAL);
        assert_eq!(p.reserve(A, t0), MIN_INTERVAL * 2);
        // Once the queue has drained, the next request goes immediately.
        let later = t0 + MIN_INTERVAL * 10;
        assert_eq!(p.reserve(A, later), Duration::ZERO);
        // Partly drained: wait out the remainder only.
        let soon = later + Duration::from_millis(250);
        assert_eq!(p.reserve(A, soon), Duration::from_millis(1000));
    }

    #[test]
    fn endpoints_are_paced_independently() {
        let mut p = Policy::default();
        let t0 = Instant::now();
        assert_eq!(p.reserve(A, t0), Duration::ZERO);
        assert_eq!(p.reserve(B, t0), Duration::ZERO);
        assert_eq!(p.reserve(A, t0), MIN_INTERVAL);
    }

    #[test]
    fn a_throttle_shuts_its_endpoint_for_the_backoff() {
        let mut p = Policy::default();
        let t0 = Instant::now();
        assert_eq!(p.backing_off(A, t0), None);
        assert_eq!(p.latest_backoff(t0), None);

        let until = p.throttled(A, t0, 1_000);
        assert_eq!(until, 1_000 + THROTTLE_BACKOFF.as_secs() as i64);
        assert!(THROTTLE_BACKOFF >= Duration::from_secs(120));
        assert_eq!(p.backing_off(A, t0), Some(until));
        assert_eq!(p.backing_off(B, t0), None, "per endpoint");
        assert_eq!(p.latest_backoff(t0), Some(until));

        let almost = t0 + THROTTLE_BACKOFF - Duration::from_millis(1);
        assert_eq!(p.backing_off(A, almost), Some(until));
        let after = t0 + THROTTLE_BACKOFF;
        assert_eq!(p.backing_off(A, after), None);
        assert_eq!(p.latest_backoff(after), None);
    }
}
