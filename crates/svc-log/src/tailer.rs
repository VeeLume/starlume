//! Reading a `Game.log` into [`LogEvent`]s: the incremental, poll-based
//! tailer and the whole-file replay built on it.
//!
//! Carved from Hearth's `sensors::tailer`. What it keeps from there:
//! - **Poll, don't fs-watch.** A file another process appends to is the
//!   case Windows change notifications handle least reliably; a stat per
//!   poll is cheap and robust.
//! - **Raw `\n` splitting with a lossy decode per line.** The log holds
//!   stray non-UTF-8 bytes (Latin-1 chat text, the odd binary run); a
//!   `lines()` reader stops at the first one and silently drops everything
//!   after it.
//! - **A trailing partial line is carried** until its newline arrives.
//!
//! What is new here:
//! - **A new session is recognised by the file's first bytes**, not only by
//!   a shrink. The game starts every log with a line naming the build and
//!   the start time to the second, so a replaced log is caught even when it
//!   has already outgrown the old one between two polls.
//! - **Bounded reads.** A first poll over a long session (logs of 100 MB
//!   exist) goes through in [`CHUNK`]-sized pieces.
//! - **Multi-line notifications are joined** before parsing
//!   ([`RecordJoiner`]).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::event::LogEvent;
use crate::parse;

/// Read size per step. Keeps a full replay's memory flat.
const CHUNK: usize = 256 * 1024;
/// How much of the file's start identifies the session.
const HEAD: usize = 96;
/// A notification's text is a few lines at most; give up on a record that
/// never closes rather than swallowing the log.
const MAX_JOINED_LINES: usize = 8;

/// Joins a HUD notification whose text contains line breaks into one
/// record. The log writes each text line as its own log line, with the
/// same timestamp and no severity:
///
/// ```text
/// <2026-11-13T17:36:45.782Z> [Notice] <SHUDEvent_OnNotification> Added notification "Objective Complete: Go to Glaciem Ring Cluster
/// <2026-11-13T17:36:45.782Z> : " [48] to queue. New queue size: 1, MissionId: […], ObjectiveId: […] […]
/// ```
///
/// The record is complete at the line holding the closing `" [`.
#[derive(Debug, Default)]
pub struct RecordJoiner {
    open: Option<(String, usize)>,
}

impl RecordJoiner {
    /// Feed one line (without its newline); returns the event of the
    /// record it completes, if that record is one.
    pub fn feed(&mut self, line: &str) -> Option<LogEvent> {
        const OPEN: &str = "<SHUDEvent_OnNotification> Added notification \"";
        const CLOSE: &str = "\" [";

        if let Some((mut record, lines)) = self.open.take() {
            // A continuation repeats the timestamp; anything else means the
            // record was cut short (a restart mid-write): parse this line
            // on its own.
            let Some((_, rest)) = parse::split_ts(line) else {
                return parse::parse_record(line);
            };
            record.push('\n');
            record.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            if rest.contains(CLOSE) {
                return parse::parse_record(&record);
            }
            if lines < MAX_JOINED_LINES {
                self.open = Some((record, lines + 1));
            }
            return None;
        }

        if let Some(at) = line.find(OPEN)
            && !line[at + OPEN.len()..].contains(CLOSE)
        {
            self.open = Some((line.to_owned(), 1));
            return None;
        }
        parse::parse_record(line)
    }
}

/// What one [`LogTailer::poll`] found.
#[derive(Debug, Default, PartialEq)]
pub struct Poll {
    /// The file was replaced or truncated since the last poll: a new game
    /// session. `events` are that new session's, from its first line.
    pub restarted: bool,
    /// Bytes read by this poll: the log's growth, tracked lines or not.
    pub bytes: u64,
    pub events: Vec<LogEvent>,
}

/// Incremental `Game.log` reader. Holds a byte offset and the trailing
/// partial line, so repeated [`poll`](Self::poll)s see each appended line
/// exactly once.
#[derive(Debug)]
pub struct LogTailer {
    path: PathBuf,
    /// Bytes consumed so far.
    offset: u64,
    /// The first bytes of the file as last seen: the session's identity.
    head: Vec<u8>,
    /// Bytes after the last newline, awaiting the rest of their line.
    pending: Vec<u8>,
    joiner: RecordJoiner,
}

impl LogTailer {
    /// Start at the beginning of the file, so the first
    /// [`poll`](Self::poll) replays the whole current log.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            head: Vec::new(),
            pending: Vec::new(),
            joiner: RecordJoiner::default(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The first bytes of the log as last read: they name the build and
    /// the start time, so they identify the session.
    pub fn head(&self) -> &[u8] {
        &self.head
    }

    /// Read what was appended since the last poll. Nothing appended, or no
    /// file yet, is an empty [`Poll`], at the cost of one `open` + `stat`.
    pub fn poll(&mut self) -> io::Result<Poll> {
        let mut file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Poll::default()),
            Err(e) => return Err(e),
        };
        let len = file.metadata()?.len();
        if len == self.offset {
            return Ok(Poll::default());
        }

        let mut poll = Poll::default();
        let head = read_head(&mut file)?;
        if len < self.offset || !head.starts_with(&self.head) {
            // Truncated, or a different log under the same name.
            poll.restarted = self.offset > 0;
            self.offset = 0;
            self.pending.clear();
            self.joiner = RecordJoiner::default();
        }
        self.head = head;

        file.seek(SeekFrom::Start(self.offset))?;
        let mut remaining = len - self.offset;
        let mut chunk = vec![0u8; CHUNK.min(remaining as usize)];
        while remaining > 0 {
            let want = chunk.len().min(remaining as usize);
            let got = file.read(&mut chunk[..want])?;
            if got == 0 {
                break; // shrank under us; the next poll sorts it out
            }
            remaining -= got as u64;
            self.offset += got as u64;
            poll.bytes += got as u64;
            self.consume(&chunk[..got], &mut poll.events);
        }
        Ok(poll)
    }

    /// Split `bytes` (after whatever is pending) into lines and parse them.
    fn consume(&mut self, bytes: &[u8], events: &mut Vec<LogEvent>) {
        let mut rest = bytes;
        while let Some(nl) = rest.iter().position(|b| *b == b'\n') {
            let (line, tail) = rest.split_at(nl);
            rest = &tail[1..];
            let event = if self.pending.is_empty() {
                self.parse_line(line)
            } else {
                self.pending.extend_from_slice(line);
                let whole = std::mem::take(&mut self.pending);
                self.parse_line(&whole)
            };
            events.extend(event);
        }
        self.pending.extend_from_slice(rest);
    }

    fn parse_line(&mut self, line: &[u8]) -> Option<LogEvent> {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        // Nothing tracked fits in a line this short, and most lines are
        // long: decode only what could matter.
        if line.len() < 24 {
            return None;
        }
        self.joiner.feed(&String::from_utf8_lossy(line))
    }
}

/// The session identity of the log at `path` (see [`LogTailer::head`]),
/// without reading further.
pub fn log_head(path: &Path) -> io::Result<Vec<u8>> {
    read_head(&mut File::open(path)?)
}

fn read_head(file: &mut File) -> io::Result<Vec<u8>> {
    let mut head = vec![0u8; HEAD];
    file.seek(SeekFrom::Start(0))?;
    let mut filled = 0;
    while filled < HEAD {
        let got = file.read(&mut head[filled..])?;
        if got == 0 {
            break;
        }
        filled += got;
    }
    head.truncate(filled);
    Ok(head)
}

/// Every event of one log file, in order: a finished session from
/// `logbackups`, or the current `Game.log` so far. A missing file reads as
/// no events.
pub fn read_log(path: &Path) -> io::Result<Vec<LogEvent>> {
    Ok(LogTailer::new(path.to_owned()).poll()?.events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{LogEventKind, ObjectiveChange};
    use std::io::Write;

    const HEADER_A: &str = concat!(
        "<2026-11-12T00:07:13.187Z> BackupNameAttachment=\" Build(12913470) 12 Nov 26 (01 07 04)\"  -- used by backup system\n",
        "<2026-11-12T00:07:13.188Z> Log started on Thu Nov 12 00:07:13 2026\n",
    );
    const HEADER_B: &str = concat!(
        "<2026-11-13T16:10:48.019Z> BackupNameAttachment=\" Build(13165946) 13 Nov 26 (17 10 38)\"  -- used by backup system\n",
        "<2026-11-13T16:10:48.019Z> Log started on Fri Nov 13 16:10:48 2026\n",
    );
    const JOIN: &str = "<2026-11-13T16:13:05.561Z> [Notice] <Join PU> address[34.156.32.60] port[64385] shard[pub_euw1b_13165946_100] locationId[-281470681677823] [Team_GameServices][GIM][Matchmaking]\n";
    const LOCATION: &str = "<2026-11-13T16:13:38.798Z> [Notice] <RequestLocationInventory> Player[VeeLume] requested inventory for Location[Nyx_Levski] [Team_CoreGameplayFeatures][Inventory]\n";

    fn kinds(events: &[LogEvent]) -> Vec<&LogEventKind> {
        events.iter().map(|e| &e.event).collect()
    }

    fn append(path: &Path, text: &[u8]) {
        let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        f.write_all(text).unwrap();
    }

    #[test]
    fn reads_appended_lines_once_and_holds_back_a_partial_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        std::fs::write(&path, format!("{HEADER_B}{JOIN}")).unwrap();

        let mut tailer = LogTailer::new(path.clone());
        let first = tailer.poll().unwrap();
        assert!(!first.restarted, "the first read is not a restart");
        assert_eq!(first.events.len(), 2, "session start + shard join");
        assert_eq!(tailer.poll().unwrap(), Poll::default(), "nothing new");

        let (start, end) = LOCATION.split_at(60);
        append(&path, start.as_bytes());
        assert!(tailer.poll().unwrap().events.is_empty(), "line incomplete");
        append(&path, end.as_bytes());
        let third = tailer.poll().unwrap();
        assert_eq!(
            kinds(&third.events),
            vec![&LogEventKind::LocationInventoryRequested {
                location: "Nyx_Levski".into()
            }]
        );
    }

    #[test]
    fn a_truncated_log_restarts_from_its_first_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        std::fs::write(&path, format!("{HEADER_A}{JOIN}{LOCATION}")).unwrap();
        let mut tailer = LogTailer::new(path.clone());
        assert_eq!(tailer.poll().unwrap().events.len(), 3);

        std::fs::write(&path, HEADER_B).unwrap();
        let poll = tailer.poll().unwrap();
        assert!(poll.restarted);
        assert_eq!(kinds(&poll.events), vec![&LogEventKind::SessionStarted]);
    }

    #[test]
    fn a_replaced_log_that_already_outgrew_the_old_one_still_restarts() {
        // The game rotates Game.log into logbackups and writes a new one.
        // A short old session and a busy startup make the new file longer
        // than the old offset by the next poll: only its head tells.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        std::fs::write(&path, HEADER_A).unwrap();
        let mut tailer = LogTailer::new(path.clone());
        assert_eq!(tailer.poll().unwrap().events.len(), 1);

        std::fs::write(&path, format!("{HEADER_B}{JOIN}{LOCATION}")).unwrap();
        let poll = tailer.poll().unwrap();
        assert!(poll.restarted);
        assert_eq!(
            poll.events.len(),
            3,
            "the new session is read from its start"
        );
    }

    #[test]
    fn a_log_caught_while_its_first_line_was_being_written_is_not_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        let (start, end) = HEADER_B.split_at(40);
        std::fs::write(&path, start).unwrap();
        let mut tailer = LogTailer::new(path.clone());
        assert!(tailer.poll().unwrap().events.is_empty());

        append(&path, end.as_bytes());
        let poll = tailer.poll().unwrap();
        assert!(!poll.restarted);
        assert_eq!(kinds(&poll.events), vec![&LogEventKind::SessionStarted]);
    }

    #[test]
    fn a_missing_file_is_empty_and_appears_later() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        let mut tailer = LogTailer::new(path.clone());
        assert_eq!(tailer.poll().unwrap(), Poll::default());
        std::fs::write(&path, HEADER_B).unwrap();
        let poll = tailer.poll().unwrap();
        assert!(!poll.restarted);
        assert_eq!(poll.events.len(), 1);
    }

    #[test]
    fn non_utf8_bytes_garble_one_line_not_the_rest() {
        // Hearth's regression: Latin-1 chat text before a tracked line.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        let mut bytes = HEADER_B.as_bytes().to_vec();
        bytes.extend_from_slice(b"<2026-11-13T16:12:00.000Z> chat: Regel ist ung\xFCltig\n");
        bytes.extend_from_slice(JOIN.as_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_log(&path).unwrap().len(), 2);
    }

    #[test]
    fn crlf_line_endings_parse_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        std::fs::write(&path, format!("{HEADER_B}{JOIN}").replace('\n', "\r\n")).unwrap();
        let events = read_log(&path).unwrap();
        assert_eq!(
            kinds(&events)[1],
            &LogEventKind::ShardJoined {
                shard: "pub_euw1b_13165946_100".into()
            }
        );
    }

    #[test]
    fn a_log_longer_than_one_read_is_read_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Game.log");
        let mut text = String::from(HEADER_B);
        let filler = "<2026-11-13T16:12:00.000Z> [Notice] <StatObjLoad 0x800 Format> 'data/objectcontainers/pu/loc/x.cgf' - File exists in P4K [Team_Graphics][CoreTech]\n";
        let mut expected = 1;
        while text.len() < CHUNK * 2 + 1000 {
            for _ in 0..200 {
                text.push_str(filler);
            }
            text.push_str(LOCATION);
            expected += 1;
        }
        std::fs::write(&path, &text).unwrap();
        assert_eq!(read_log(&path).unwrap().len(), expected);
    }

    #[test]
    fn a_multi_line_notification_is_one_event_and_its_echo_none() {
        // Verbatim from a 4.10.3 log: the notification, its indented echo
        // (also two lines), and the first lifecycle line.
        let text = concat!(
            "<2026-11-13T17:36:45.782Z> [Notice] <SHUDEvent_OnNotification> Added notification \"Objective Complete: Go to Glaciem Ring Cluster\n",
            "<2026-11-13T17:36:45.782Z> : \" [48] to queue. New queue size: 1, MissionId: [1d803b44-738f-4f97-bcec-2641e38e93b9], ObjectiveId: [fb517a56-add8-9741-ac83-855782ca88dd] [Team_CoreGameplayFeatures][Missions][Comms]\n",
            "<2026-11-13T17:36:45.782Z>    \"Objective Complete: Go to Glaciem Ring Cluster\n",
            "<2026-11-13T17:36:45.782Z> : \" [48]\n",
            "<2026-11-13T17:36:45.786Z> [Notice] <UpdateNotificationItem> Notification \"Objective Complete: Go to Glaciem Ring Cluster\n",
            "<2026-11-13T17:36:45.786Z> : \" [48], Action: Next [Team_CoreGameplayFeatures][Missions][Comms]\n",
        );
        let mut joiner = RecordJoiner::default();
        let events: Vec<LogEvent> = text.lines().filter_map(|l| joiner.feed(l)).collect();
        assert_eq!(
            events,
            vec![LogEvent {
                ts: "2026-11-13T17:36:45.782Z".into(),
                event: LogEventKind::Objective {
                    change: ObjectiveChange::Completed,
                    mission_id: Some("1d803b44-738f-4f97-bcec-2641e38e93b9".into()),
                    objective_id: Some("fb517a56-add8-9741-ac83-855782ca88dd".into()),
                    text: "Go to Glaciem Ring Cluster".into(),
                },
            }]
        );
    }

    #[test]
    fn a_notification_that_never_closes_does_not_swallow_the_log() {
        let mut joiner = RecordJoiner::default();
        let open = "<2026-11-13T17:36:45.782Z> [Notice] <SHUDEvent_OnNotification> Added notification \"Joined hangar queue.";
        assert_eq!(joiner.feed(open), None);
        for _ in 0..MAX_JOINED_LINES + 1 {
            joiner.feed("<2026-11-13T17:36:45.782Z> more text");
        }
        assert!(joiner.feed(JOIN.trim_end()).is_some());
    }

    /// Replays the maintainer's real log when present: a smoke test that
    /// the parsers hold on a whole session, and a timing.
    /// `cargo test -p svc-log real_log -- --ignored --nocapture`
    #[test]
    #[ignore = "reads a real Game.log; set STARLUME_REAL_GAME_LOG or use the default path"]
    fn real_log_replays() {
        let path = std::env::var("STARLUME_REAL_GAME_LOG")
            .unwrap_or_else(|_| r"C:\Games\StarCitizen\LIVE\Game.log".into());
        let started = std::time::Instant::now();
        let events = read_log(Path::new(&path)).unwrap();
        println!("{} events in {:?}", events.len(), started.elapsed());
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for e in &events {
            let name = format!("{:?}", e.event);
            let name = name
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap()
                .to_owned();
            *counts.entry(name).or_default() += 1;
        }
        println!("{counts:#?}");
        assert!(
            events
                .iter()
                .any(|e| e.event == LogEventKind::SessionStarted)
        );
    }
}
