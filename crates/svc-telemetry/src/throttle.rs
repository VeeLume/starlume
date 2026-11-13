//! When to tell the frontend: at once for a real change, at most once per
//! gap for fast-moving values.

use std::time::{Duration, Instant};

use crate::state::Change;

/// Decides, frame by frame, whether the snapshot is worth announcing.
#[derive(Debug)]
pub struct Throttle {
    gap: Duration,
    last: Option<Instant>,
    /// A minor change arrived inside the gap and has not been announced.
    pending: bool,
}

impl Throttle {
    pub fn new(gap: Duration) -> Self {
        Self {
            gap,
            last: None,
            pending: false,
        }
    }

    /// A frame changed the snapshot by `change` at `now`: announce it?
    ///
    /// [`Change::Major`] always goes out. [`Change::Minor`] goes out when the
    /// gap since the last announcement has passed, and is held otherwise; a
    /// held one goes out with the first later frame past the gap.
    pub fn offer(&mut self, change: Change, now: Instant) -> bool {
        match change {
            Change::Major => self.announce(now),
            Change::Minor => {
                self.pending = true;
                self.flush(now)
            }
            Change::None => self.flush(now),
        }
    }

    /// Announce a held minor change if the gap has passed. Also the call for
    /// a stream that went quiet (no further frame would release it).
    pub fn flush(&mut self, now: Instant) -> bool {
        let due = self
            .last
            .is_none_or(|last| now.duration_since(last) >= self.gap);
        if self.pending && due {
            self.announce(now)
        } else {
            false
        }
    }

    fn announce(&mut self, now: Instant) -> bool {
        self.last = Some(now);
        self.pending = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: Duration = Duration::from_secs(1);

    fn ms(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn a_fifteen_hertz_stream_of_minor_changes_goes_out_once_a_second() {
        let start = Instant::now();
        let mut t = Throttle::new(GAP);
        let sent = (0..45)
            .filter(|i| t.offer(Change::Minor, ms(start, i * 67)))
            .count();
        // 45 frames over three seconds: the first, then one per second.
        assert_eq!(sent, 3);
    }

    #[test]
    fn a_major_change_goes_out_at_once_and_restarts_the_gap() {
        let start = Instant::now();
        let mut t = Throttle::new(GAP);
        assert!(t.offer(Change::Minor, start));
        assert!(t.offer(Change::Major, ms(start, 10)));
        assert!(t.offer(Change::Major, ms(start, 20)));
        assert!(!t.offer(Change::Minor, ms(start, 900)));
        assert!(t.offer(Change::None, ms(start, 1020)));
    }

    #[test]
    fn unchanged_frames_announce_nothing() {
        let start = Instant::now();
        let mut t = Throttle::new(GAP);
        assert!(!t.offer(Change::None, start));
        assert!(!t.offer(Change::None, ms(start, 5000)));
    }

    #[test]
    fn a_held_change_is_flushed_when_the_stream_goes_quiet() {
        let start = Instant::now();
        let mut t = Throttle::new(GAP);
        assert!(t.offer(Change::Major, start));
        assert!(!t.offer(Change::Minor, ms(start, 100)));
        assert!(!t.flush(ms(start, 500)));
        assert!(t.flush(ms(start, 5000)));
        // Once.
        assert!(!t.flush(ms(start, 9000)));
    }
}
