//! Parity with the Python prototype the filter was tuned as
//! (sc-grpc `.scratch/spectrum-search/tools/chat_filter.py`).
//!
//! Needs two local files, so it is ignored by default:
//! - `STARLUME_SPECTRUM_DUMP` — a staff search dump (JSONL of search hits);
//! - `STARLUME_SPECTRUM_EXPECTED` — the prototype's decisions on it (JSONL of
//!   `{index, id, keep, why}`).
//!
//! `cargo test -p mod-devtracker --test parity -- --ignored --nocapture`

use std::collections::HashMap;

use mod_devtracker::filter::{Reason, classify};
use serde_json::Value;

fn lines(var: &str) -> Vec<Value> {
    let path = std::env::var(var).unwrap_or_else(|_| panic!("{var} not set"));
    std::fs::read_to_string(path)
        .expect("readable")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json line"))
        .collect()
}

fn why(reason: &Reason) -> &'static str {
    match reason {
        Reason::Forum => "forum",
        Reason::Strong(_) => "strong",
        Reason::Weak(ws) if ws.len() == 1 => "weak1",
        Reason::Weak(_) => "weak2",
        Reason::Short => "short/emoji/mention",
        Reason::Moderation => "moderation",
        Reason::NoSignal => "no-signal",
    }
}

#[test]
#[ignore = "needs a local Spectrum dump"]
fn matches_the_prototype() {
    let expected: HashMap<(String, String), (bool, String)> = lines("STARLUME_SPECTRUM_EXPECTED")
        .into_iter()
        .map(|e| {
            let key = (
                e["index"].as_str().unwrap().to_owned(),
                e["id"].as_str().unwrap().to_owned(),
            );
            (
                key,
                (
                    e["keep"].as_bool().unwrap(),
                    e["why"].as_str().unwrap().to_owned(),
                ),
            )
        })
        .collect();
    let (mut total, mut kept, mut mismatches) = (0, 0, Vec::new());
    for hit in lines("STARLUME_SPECTRUM_DUMP") {
        let index = hit["_index"].as_str().unwrap();
        let id = hit["_id"].as_str().unwrap();
        let body = hit["_source"]["body"].as_str().unwrap_or("");
        let lobby = hit["details"]["lobby"]["name"].as_str();
        let v = classify(index == "tavern_message", body, lobby);
        total += 1;
        kept += usize::from(v.keep && index == "tavern_message");
        let (keep, why_py) = &expected[&(index.to_owned(), id.to_owned())];
        // The prototype labels a kept weak post by the count it needed, not
        // the count it had; compare kept/dropped plus the drop reason.
        let same = *keep == v.keep && (v.keep || why_py == why(&v.reason));
        if !same {
            mismatches.push(format!(
                "{index}/{id}: py {keep} {why_py}, rs {:?} | {}",
                v.reason,
                &body[..body.len().min(120)]
            ));
        }
    }
    println!(
        "{total} posts, {kept} chat kept, {} mismatches",
        mismatches.len()
    );
    for m in mismatches.iter().take(30) {
        println!("  {m}");
    }
    assert!(mismatches.is_empty());
}
