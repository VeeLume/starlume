//! Drive Discord Rich Presence from a console: try what Starlume sends
//! without playing.
//!
//! ```text
//! cargo run -p svc-presence --example presence -- <application id>
//! ```
//!
//! The application id can also come from `STARLUME_DISCORD_CLIENT_ID`.
//! Discord must run on this PC. The presence stays while this runs: Discord
//! drops it when the connection closes, so `quit` (or Ctrl+C) clears it.
//!
//! Times in a moment below 10,000,000 are seconds ago, so a sample's timer
//! reads as it did in the session (`session_start: 4399` is 1:13:19 ago).
//! Type `help` for the commands.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use svc_presence::{Activity, Client, Moment, PlaceLevel, Share, compose};

const SAMPLES: &str = include_str!("../samples/moments.json");

const HELP: &str = "\
Moments
  samples            list the built-in moments (Valerie's sessions)
  <n>                send moment n
  load <file>        read moments from a JSON file (a list, or one moment)
  watch <file>       send the file's first moment, and again whenever it changes
  {…}                a moment as JSON on one line
What it shares (resends the current moment)
  place exact|system|off
  ship on|off        contracts on|off        button on|off
  share              print the current choices
Raw
  raw {…}            send an activity as Discord takes it, nothing composed
  clear              take the presence off the profile
  quit";

#[derive(Deserialize)]
struct Labelled {
    label: String,
    moment: Moment,
}

/// A file of moments: a labelled list (the samples' shape), or one moment.
#[derive(Deserialize)]
#[serde(untagged)]
enum MomentFile {
    List(Vec<Labelled>),
    One(Box<Moment>),
}

fn main() {
    let id = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("STARLUME_DISCORD_CLIENT_ID").ok())
        .unwrap_or_else(|| {
            eprintln!("usage: presence <discord application id>  (or STARLUME_DISCORD_CLIENT_ID)");
            std::process::exit(2);
        });
    let mut client = match Client::connect(&id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    println!(
        "Connected to Discord as {}. Type help.",
        client.user().unwrap_or("someone")
    );

    let mut moments = parse(SAMPLES).expect("the samples parse");
    let mut share = Share::default();
    let mut current: Option<Moment> = None;
    let mut watched: Option<(PathBuf, Option<SystemTime>)> = None;

    // Stdin on its own thread, so a watched file is checked while waiting.
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    list(&moments);
    prompt();

    loop {
        let line = match rx.recv_timeout(Duration::from_millis(700)) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some((path, seen)) = &mut watched {
                    let now = std::fs::metadata(&*path).and_then(|m| m.modified()).ok();
                    if now != *seen {
                        *seen = now;
                        match read(path) {
                            Ok(m) => {
                                println!("\n{} changed", path.display());
                                current = m.into_iter().next().map(|l| l.moment);
                                send_moment(&mut client, current.as_ref(), &share);
                            }
                            Err(e) => println!("\n{}: {e}", path.display()),
                        }
                        prompt();
                    }
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let line = line.trim();
        let (cmd, rest) = line
            .split_once(' ')
            .map_or((line, ""), |(c, r)| (c, r.trim()));
        match cmd {
            "" => {}
            "help" | "?" => println!("{HELP}"),
            "quit" | "exit" => break,
            "samples" => {
                moments = parse(SAMPLES).expect("the samples parse");
                list(&moments);
            }
            "load" => match read(&PathBuf::from(rest)) {
                Ok(m) => {
                    moments = m;
                    list(&moments);
                }
                Err(e) => println!("{e}"),
            },
            "watch" => {
                watched = Some((PathBuf::from(rest), None));
                println!("watching {rest}");
            }
            "place" => {
                share.place = match rest {
                    "exact" => PlaceLevel::Exact,
                    "system" => PlaceLevel::System,
                    "off" => PlaceLevel::Off,
                    _ => {
                        println!("place exact|system|off");
                        prompt();
                        continue;
                    }
                };
                send_moment(&mut client, current.as_ref(), &share);
            }
            "ship" | "contracts" | "button" => {
                let on = match rest {
                    "on" => true,
                    "off" => false,
                    _ => {
                        println!("{cmd} on|off");
                        prompt();
                        continue;
                    }
                };
                match cmd {
                    "ship" => share.ship = on,
                    "contracts" => share.contracts = on,
                    _ => share.button = on,
                }
                send_moment(&mut client, current.as_ref(), &share);
            }
            "share" => println!("{}", serde_json::to_string(&share).unwrap()),
            "raw" => match serde_json::from_str::<Activity>(rest) {
                Ok(a) => send(&mut client, &a),
                Err(e) => println!("not an activity: {e}"),
            },
            "clear" => match client.clear() {
                Ok(()) => println!("cleared"),
                Err(e) => println!("{e}"),
            },
            _ if line.starts_with('{') => match serde_json::from_str::<Moment>(line) {
                Ok(m) => {
                    current = Some(m);
                    send_moment(&mut client, current.as_ref(), &share);
                }
                Err(e) => println!("not a moment: {e}"),
            },
            _ => match line
                .parse::<usize>()
                .ok()
                .and_then(|n| moments.get(n.wrapping_sub(1)))
            {
                Some(l) => {
                    println!("{}", l.label);
                    current = Some(l.moment.clone());
                    send_moment(&mut client, current.as_ref(), &share);
                }
                None => println!("? (help lists the commands)"),
            },
        }
        prompt();
    }
    // Dropping the client closes the pipe; Discord clears the presence.
    let _ = client.clear();
}

fn parse(json: &str) -> Result<Vec<Labelled>, serde_json::Error> {
    Ok(match serde_json::from_str::<MomentFile>(json)? {
        MomentFile::List(l) => l,
        MomentFile::One(m) => vec![Labelled {
            label: "moment".into(),
            moment: *m,
        }],
    })
}

fn read(path: &PathBuf) -> Result<Vec<Labelled>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse(&text).map_err(|e| e.to_string())
}

fn list(moments: &[Labelled]) {
    for (i, l) in moments.iter().enumerate() {
        println!("  {}  {}", i + 1, l.label);
    }
}

fn send_moment(client: &mut Client, moment: Option<&Moment>, share: &Share) {
    let Some(m) = moment else {
        println!("no moment yet: pick one by its number");
        return;
    };
    send(client, &compose(&relative(m), share));
}

fn send(client: &mut Client, activity: &Activity) {
    println!("{}", serde_json::to_string_pretty(activity).unwrap());
    if let Err(e) = client.set_activity(activity) {
        println!("{e}");
    }
}

/// Times below 10,000,000 are seconds ago.
fn relative(m: &Moment) -> Moment {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let at = |t: i64| if t < 10_000_000 { now - t } else { t };
    let mut m = m.clone();
    m.session_start = m.session_start.map(at);
    if let Some(c) = &mut m.contract {
        c.started = c.started.map(at);
    }
    m
}

fn prompt() {
    print!("> ");
    let _ = std::io::stdout().flush();
}
