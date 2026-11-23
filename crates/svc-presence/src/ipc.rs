//! The Discord app's local IPC: a named pipe on Windows
//! (`\\.\pipe\discord-ipc-N`), a Unix socket elsewhere, N from 0 to 9 (one
//! per running Discord client; the first that opens wins).
//!
//! Every message is a frame: an opcode and a payload length, both `u32`
//! little-endian, then that many bytes of JSON. The client sends a
//! handshake (`{"v":1,"client_id":…}`), the app answers `READY`; after that
//! each command is a frame with a `nonce` the answer echoes. The app may
//! ping at any time and wants a pong with the same payload.
//!
//! **Blocking.** Reads wait for the app's answer, which comes within
//! milliseconds while Discord runs; a caller that must never wait runs the
//! client on a thread of its own. The Windows pipe opened as a file has no
//! read timeout.

use std::io::{self, Read, Write};

use serde_json::{Value, json};

use crate::Activity;

/// Opcodes of the frame header.
mod op {
    pub const HANDSHAKE: u32 = 0;
    pub const FRAME: u32 = 1;
    pub const CLOSE: u32 = 2;
    pub const PING: u32 = 3;
    pub const PONG: u32 = 4;
}

/// A frame larger than this is not Discord talking: refuse it rather than
/// allocate it.
const FRAME_MAX: u32 = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No Discord app listens on any of the ten pipes.
    #[error("Discord is not running")]
    NotRunning,
    /// The app closed the connection: a wrong application id, or Discord
    /// quitting.
    #[error("Discord closed the connection ({code}): {message}")]
    Closed { code: i64, message: String },
    /// The app refused a command, e.g. a field it does not accept.
    #[error("Discord refused it ({code}): {message}")]
    Refused { code: i64, message: String },
    #[error("unexpected answer from Discord: {0}")]
    Protocol(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A connection to the Discord app, after its `READY`.
pub struct Client<T: Read + Write = Pipe> {
    io: T,
    nonce: u64,
    /// The Discord user the app is signed in as, from `READY`.
    user: Option<String>,
}

impl Client<Pipe> {
    /// Open the first Discord pipe and hand-shake as `client_id` (the
    /// Discord application's id: its name is what the profile says you
    /// are playing). The caller has passed the online gate (crate docs).
    pub fn connect(client_id: &str) -> Result<Self, Error> {
        Client::handshake(Pipe::open()?, client_id)
    }
}

impl<T: Read + Write> Client<T> {
    /// Hand-shake over an open transport: send the handshake, wait for
    /// `READY`.
    pub fn handshake(io: T, client_id: &str) -> Result<Self, Error> {
        let mut client = Client {
            io,
            nonce: 0,
            user: None,
        };
        client.send(op::HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))?;
        let ready = client.answer(|v| v.get("evt").and_then(Value::as_str) == Some("READY"))?;
        client.user = ready
            .pointer("/data/user/username")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(client)
    }

    /// The Discord user name the app is signed in as.
    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// Show `activity` on the profile, replacing what was there.
    pub fn set_activity(&mut self, activity: &Activity) -> Result<(), Error> {
        self.command(serde_json::to_value(activity).map_err(|e| Error::Protocol(e.to_string()))?)
    }

    /// Take the activity off the profile; the connection stays open.
    pub fn clear(&mut self) -> Result<(), Error> {
        self.command(Value::Null)
    }

    fn command(&mut self, activity: Value) -> Result<(), Error> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        self.send(
            op::FRAME,
            &json!({
                "cmd": "SET_ACTIVITY",
                "args": { "pid": std::process::id(), "activity": activity },
                "nonce": nonce,
            }),
        )?;
        let answer =
            self.answer(|v| v.get("nonce").and_then(Value::as_str) == Some(nonce.as_str()))?;
        if answer.get("evt").and_then(Value::as_str) == Some("ERROR") {
            return Err(Error::Refused {
                code: answer
                    .pointer("/data/code")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                message: answer
                    .pointer("/data/message")
                    .and_then(Value::as_str)
                    .unwrap_or("no message")
                    .to_owned(),
            });
        }
        Ok(())
    }

    /// Read frames until one `wanted` accepts: pings are answered, a close
    /// ends it, other frames (events nobody subscribed to) are skipped.
    fn answer(&mut self, wanted: impl Fn(&Value) -> bool) -> Result<Value, Error> {
        loop {
            let (code, body) = self.read()?;
            match code {
                op::FRAME if wanted(&body) => return Ok(body),
                op::FRAME | op::PONG => {}
                op::PING => self.send(op::PONG, &body)?,
                op::CLOSE => {
                    return Err(Error::Closed {
                        code: body.get("code").and_then(Value::as_i64).unwrap_or(0),
                        message: body
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("no message")
                            .to_owned(),
                    });
                }
                other => return Err(Error::Protocol(format!("opcode {other}"))),
            }
        }
    }

    fn send(&mut self, code: u32, body: &Value) -> Result<(), Error> {
        let bytes = encode(code, body);
        self.io.write_all(&bytes)?;
        self.io.flush()?;
        Ok(())
    }

    fn read(&mut self) -> Result<(u32, Value), Error> {
        let mut head = [0u8; 8];
        self.io.read_exact(&mut head)?;
        let code = u32::from_le_bytes(head[..4].try_into().unwrap());
        let len = u32::from_le_bytes(head[4..].try_into().unwrap());
        if len > FRAME_MAX {
            return Err(Error::Protocol(format!("a frame of {len} bytes")));
        }
        let mut body = vec![0u8; len as usize];
        self.io.read_exact(&mut body)?;
        let value = serde_json::from_slice(&body).map_err(|e| Error::Protocol(e.to_string()))?;
        Ok((code, value))
    }
}

/// One frame: header, then the JSON.
fn encode(code: u32, body: &Value) -> Vec<u8> {
    let json = body.to_string();
    let mut out = Vec::with_capacity(8 + json.len());
    out.extend_from_slice(&code.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(json.as_bytes());
    out
}

/// The connection to the Discord app.
pub struct Pipe(PipeInner);

#[cfg(windows)]
type PipeInner = std::fs::File;
#[cfg(unix)]
type PipeInner = std::os::unix::net::UnixStream;

impl Pipe {
    fn open() -> Result<Self, Error> {
        for n in 0..10 {
            #[cfg(windows)]
            let opened = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!(r"\\.\pipe\discord-ipc-{n}"));
            #[cfg(unix)]
            let opened = unix_dirs()
                .into_iter()
                .find_map(|dir| {
                    std::os::unix::net::UnixStream::connect(dir.join(format!("discord-ipc-{n}")))
                        .ok()
                })
                .ok_or(());
            if let Ok(inner) = opened {
                return Ok(Pipe(inner));
            }
        }
        Err(Error::NotRunning)
    }
}

/// Where the Discord app puts its socket: the runtime dir, the temp dirs,
/// and the Flatpak and Snap sandboxes inside them.
#[cfg(unix)]
fn unix_dirs() -> Vec<std::path::PathBuf> {
    let mut bases: Vec<std::path::PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .filter_map(|k| std::env::var_os(k).map(Into::into))
        .collect();
    bases.push("/tmp".into());
    bases
        .iter()
        .flat_map(|b| {
            [
                b.clone(),
                b.join("app/com.discordapp.Discord"),
                b.join("snap.discord"),
            ]
        })
        .collect()
}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A scripted Discord: what it will answer, and what it was sent.
    struct Fake {
        answers: Cursor<Vec<u8>>,
        sent: Vec<u8>,
    }

    impl Fake {
        fn new(frames: &[(u32, Value)]) -> Self {
            let answers = frames.iter().flat_map(|(c, v)| encode(*c, v)).collect();
            Fake {
                answers: Cursor::new(answers),
                sent: Vec::new(),
            }
        }
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.answers.read(buf)
        }
    }

    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.sent.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// The frames a client sent, decoded.
    fn sent(bytes: &[u8]) -> Vec<(u32, Value)> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let code = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            out.push((
                code,
                serde_json::from_slice(&bytes[at + 8..at + 8 + len]).unwrap(),
            ));
            at += 8 + len;
        }
        out
    }

    fn ready() -> (u32, Value) {
        (
            op::FRAME,
            json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1, "user": { "username": "veelume" } } }),
        )
    }

    #[test]
    fn the_handshake_waits_for_ready_and_answers_a_ping_on_the_way() {
        let fake = Fake::new(&[(op::PING, json!({ "n": 1 })), ready()]);
        let client = Client::handshake(fake, "123").unwrap();
        assert_eq!(client.user(), Some("veelume"));
        let frames = sent(&client.io.sent);
        assert_eq!(
            frames[0],
            (op::HANDSHAKE, json!({ "v": 1, "client_id": "123" }))
        );
        assert_eq!(frames[1], (op::PONG, json!({ "n": 1 })));
    }

    #[test]
    fn a_wrong_application_id_is_a_close_with_its_reason() {
        let fake = Fake::new(&[(
            op::CLOSE,
            json!({ "code": 4000, "message": "Invalid Client ID" }),
        )]);
        match Client::handshake(fake, "nope") {
            Err(Error::Closed { code, message }) => {
                assert_eq!((code, message.as_str()), (4000, "Invalid Client ID"))
            }
            other => panic!("{:?}", other.err()),
        }
    }

    #[test]
    fn set_activity_sends_it_with_our_pid_and_reads_its_own_answer() {
        let fake = Fake::new(&[
            ready(),
            (
                op::FRAME,
                json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": "1", "data": {} }),
            ),
        ]);
        let mut client = Client::handshake(fake, "123").unwrap();
        let activity = Activity {
            details: Some("On foot".into()),
            ..Activity::default()
        };
        client.set_activity(&activity).unwrap();
        let (code, body) = sent(&client.io.sent).pop().unwrap();
        assert_eq!(code, op::FRAME);
        assert_eq!(body["cmd"], "SET_ACTIVITY");
        assert_eq!(body["nonce"], "1");
        assert_eq!(body["args"]["pid"], std::process::id());
        assert_eq!(body["args"]["activity"]["details"], "On foot");
    }

    #[test]
    fn a_refused_activity_says_why() {
        let fake = Fake::new(&[
            ready(),
            (
                op::FRAME,
                json!({ "cmd": "SET_ACTIVITY", "evt": "ERROR", "nonce": "1", "data": { "code": 4000, "message": "child \"activity\" fails" } }),
            ),
        ]);
        let mut client = Client::handshake(fake, "123").unwrap();
        match client.set_activity(&Activity::default()) {
            Err(Error::Refused { code, .. }) => assert_eq!(code, 4000),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn clear_sends_a_null_activity() {
        let fake = Fake::new(&[
            ready(),
            (
                op::FRAME,
                json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": "1", "data": null }),
            ),
        ]);
        let mut client = Client::handshake(fake, "123").unwrap();
        client.clear().unwrap();
        let (_, body) = sent(&client.io.sent).pop().unwrap();
        assert_eq!(body["args"]["activity"], Value::Null);
    }
}
