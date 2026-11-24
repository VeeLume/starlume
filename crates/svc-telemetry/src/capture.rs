//! Session captures: every telemetry frame of a session kept on disk, so
//! what the stream says can be studied after normal play (Valerie,
//! 2026-11-24: Rich Presence is reshaped from real captures).
//!
//! **Format.** One JSON line per frame, `{"ms": <unix ms>, "frame": …}`,
//! the [`Frame`] as `sc-dossier` models it: the same lines the gRPC lab's
//! captures hold. Fields the model drops are not kept.
//!
//! **Size.** Raw, a frame is about 2.4 KB and the stream sends about 15 a
//! second: 145 MB an hour. Compressed it is about 3 MB an hour (measured on
//! a 19-minute LIVE 4.10 capture: 47.1 MB of JSON, 1.0 MB gzipped), so the
//! frames are kept whole and compressed, nothing rounded or dropped.
//!
//! **Blocks.** Lines are gathered in memory and written as one gzip member
//! every [`BLOCK_EVERY`] or [`BLOCK_BYTES`], appended to the file: a crash
//! loses at most one block, and the members together read as one gzip
//! stream ([`read`]).

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use sc_dossier::telemetry::Frame;

/// The longest a frame waits in memory.
pub const BLOCK_EVERY: Duration = Duration::from_secs(30);
/// The most raw JSON held before a block is written (about 2 minutes of
/// frames at most).
pub const BLOCK_BYTES: usize = 4 << 20;

/// Appends one session's frames to a compressed capture file.
pub struct Recorder {
    path: PathBuf,
    pending: Vec<u8>,
    since: Instant,
    frames: u64,
}

impl Recorder {
    /// Record into `path`, after what it already holds (a stream that
    /// reconnects within one session continues its file).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Recorder {
            path: path.into(),
            pending: Vec::new(),
            since: Instant::now(),
            frames: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Frames taken since this recorder began.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Take one frame, received at `ms` (Unix milliseconds). Writes a block
    /// when one is due.
    pub fn push(&mut self, ms: i64, frame: &Frame) -> io::Result<()> {
        #[derive(serde::Serialize)]
        struct Line<'a> {
            ms: i64,
            frame: &'a Frame,
        }
        serde_json::to_writer(&mut self.pending, &Line { ms, frame }).map_err(io::Error::other)?;
        self.pending.push(b'\n');
        self.frames += 1;
        if self.pending.len() >= BLOCK_BYTES || self.since.elapsed() >= BLOCK_EVERY {
            self.flush()?;
        }
        Ok(())
    }

    /// Write what is held as one block.
    pub fn flush(&mut self) -> io::Result<()> {
        self.since = Instant::now();
        if self.pending.is_empty() {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let mut gz = GzEncoder::new(file, Compression::default());
        gz.write_all(&self.pending)?;
        gz.finish()?.sync_data()?;
        self.pending.clear();
        Ok(())
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Err(e) = self.flush() {
            eprintln!("telemetry capture {}: {e}", self.path.display());
        }
    }
}

/// Every frame of a capture, in order. A damaged last block (a crash while
/// writing it) ends the read early instead of failing it.
pub fn read(path: &Path) -> io::Result<Vec<(i64, Frame)>> {
    #[derive(serde::Deserialize)]
    struct Line {
        ms: i64,
        frame: Frame,
    }
    let mut out = Vec::new();
    for line in BufReader::new(MultiGzDecoder::new(File::open(path)?)).lines() {
        let Ok(line) = line else { break };
        if let Ok(l) = serde_json::from_str::<Line>(&line) {
            out.push((l.ms, l.frame));
        }
    }
    Ok(out)
}

/// Copy `from` into `to` compressed (a session's Game.log, beside its
/// frames).
pub fn gzip_file(from: &Path, to: &Path) -> io::Result<u64> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut src = File::open(from)?;
    let mut gz = GzEncoder::new(File::create(to)?, Compression::default());
    let n = io::copy(&mut src, &mut gz)?;
    gz.finish()?.sync_data()?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(balance: i64) -> Frame {
        serde_json::from_value(serde_json::json!({
            "vehicle": null, "weapons": null, "turret": null, "events": [],
            "actor": {
                "health": {"current": 100.0, "max": 100.0}, "oxygen": {"current": 1.0, "max": 1.0},
                "hunger": {"current": 100.0, "max": 100.0}, "thirst": {"current": 100.0, "max": 100.0},
                "body_temperature": {"current": 36.5, "max": 100.0}, "heart_rate": 60.0, "gravity": 1.0,
                "pressure": 1.0, "intoxication": 0, "external_temperature": 20.0, "radiation": 0.0,
                "breathable": true, "locomotion": "OnFoot", "crew_role": "None", "seat_name": "",
                "uec_balance": balance
            }
        }))
        .expect("a frame in the lab's capture shape")
    }

    #[test]
    fn blocks_append_and_read_back_as_one_capture() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.capture").join("telemetry.jsonl.gz");
        {
            let mut r = Recorder::new(&path);
            r.push(1, &frame(10)).unwrap();
            r.push(2, &frame(20)).unwrap();
            r.flush().unwrap();
            r.push(3, &frame(30)).unwrap();
        } // dropped: the last block is written
        // A reconnect within the session continues the file.
        Recorder::new(&path).push(4, &frame(40)).unwrap();
        let back = read(&path).unwrap();
        assert_eq!(
            back.iter().map(|(ms, _)| *ms).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(back[3].1, frame(40));
    }

    #[test]
    fn a_torn_last_block_keeps_what_came_before() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.jsonl.gz");
        drop({
            let mut r = Recorder::new(&path);
            r.push(1, &frame(10)).unwrap();
            r
        });
        let mut bytes = std::fs::read(&path).unwrap();
        let whole = bytes.len();
        drop({
            let mut r = Recorder::new(&path);
            r.push(2, &frame(20)).unwrap();
            r
        });
        bytes = std::fs::read(&path).unwrap();
        bytes.truncate(whole + (bytes.len() - whole) / 2);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read(&path).unwrap().len(), 1);
    }

    #[test]
    fn a_log_is_kept_compressed() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("Game.log");
        let text = "<2026-11-24T10:00:00.000Z> Log started\n".repeat(1000);
        std::fs::write(&log, &text).unwrap();
        let to = dir.path().join("c").join("Game.log.gz");
        assert_eq!(gzip_file(&log, &to).unwrap(), text.len() as u64);
        assert!(std::fs::metadata(&to).unwrap().len() < 2_000);
    }
}
