//! Timestamps: Unix milliseconds, UTC.
//!
//! svc-log hands every event its line's timestamp as logged (RFC 3339, UTC,
//! milliseconds); telemetry samples carry the arrival time. Both become
//! [`Millis`] here so they order and subtract.

/// Unix time in milliseconds, UTC.
pub type Millis = i64;

/// Days from 1970-01-01 to `y-m-d` in the proleptic Gregorian calendar
/// (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A log timestamp (`2026-11-13T16:13:05.561Z`) as Unix milliseconds.
/// `None` when it is not that shape; the fraction is optional and may have
/// any number of digits.
pub fn parse_ts(ts: &str) -> Option<Millis> {
    let ts = ts.strip_suffix('Z')?;
    let (date, time) = ts.split_once('T')?;
    let mut date = date.splitn(3, '-');
    let y: i64 = date.next()?.parse().ok()?;
    let m: i64 = date.next()?.parse().ok()?;
    let d: i64 = date.next()?.parse().ok()?;
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    let mut clock = clock.splitn(3, ':');
    let h: i64 = clock.next()?.parse().ok()?;
    let min: i64 = clock.next()?.parse().ok()?;
    let s: i64 = clock.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || min > 59 || s > 60 {
        return None;
    }
    if !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut ms = 0;
    for i in 0..3 {
        let digit = fraction
            .as_bytes()
            .get(i)
            .map_or(0, |b| i64::from(b - b'0'));
        ms = ms * 10 + digit;
    }
    Some((days_from_civil(y, m, d) * 86_400 + h * 3600 + min * 60 + s) * 1000 + ms)
}

/// The id of the session that started at `at`: its start time, in a form
/// that is also a file name (`2026-11-13T16-10-48.019Z`).
pub fn session_id(at: Millis) -> String {
    let secs = at.div_euclid(1000);
    let ms = at.rem_euclid(1000);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let day = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}.{ms:03}Z",
        day / 3600,
        day % 3600 / 60,
        day % 60
    )
}

/// Is `id` the shape [`session_id`] makes? The shell checks this before an
/// id from the webview becomes part of a file name.
pub fn is_session_id(id: &str) -> bool {
    let b = id.as_bytes();
    const SHAPE: &[u8] = b"0000-00-00T00-00-00.000Z";
    b.len() == SHAPE.len()
        && b.iter().zip(SHAPE).all(|(c, shape)| match shape {
            b'0' => c.is_ascii_digit(),
            other => c == other,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_timestamp_parses_to_unix_milliseconds() {
        assert_eq!(parse_ts("1970-01-01T00:00:00.000Z"), Some(0));
        // 2026-11-13T16:13:05.561Z, checked against `date -d`.
        assert_eq!(
            parse_ts("2026-11-13T16:13:05.561Z"),
            Some(1_794_586_385_561)
        );
        assert_eq!(
            parse_ts("2026-11-13T16:13:05Z"),
            Some(1_794_586_385_000),
            "the fraction is optional"
        );
        assert_eq!(parse_ts("2024-02-29T00:00:00.5Z"), Some(1_709_164_800_500));
    }

    #[test]
    fn what_is_not_a_timestamp_is_none() {
        for bad in [
            "",
            "2026-11-13 16:13:05",
            "2026-11-13T16:13:05.561",
            "2026-13-01T00:00:00.000Z",
            "2026-11-13T25:00:00.000Z",
            "2026-11-13T16:13:05.5x1Z",
        ] {
            assert_eq!(parse_ts(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_session_id_is_the_start_time_as_a_file_name() {
        let at = parse_ts("2026-11-13T16:10:48.019Z").unwrap();
        let id = session_id(at);
        assert_eq!(id, "2026-11-13T16-10-48.019Z");
        assert!(is_session_id(&id));
        assert_eq!(session_id(0), "1970-01-01T00-00-00.000Z");
        // Ids sort as their sessions do.
        assert!(session_id(at) < session_id(at + 1));
    }

    #[test]
    fn only_the_id_shape_passes() {
        for bad in [
            "",
            "..\\settings",
            "2026-11-13T16-10-48.019Z.json",
            "2026-11-13T16:10:48.019Z",
            "2026-11-13T16-10-48.01aZ",
        ] {
            assert!(!is_session_id(bad), "{bad}");
        }
    }
}
