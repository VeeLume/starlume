//! Capture the Star Citizen window once and save it as a PNG.
//!
//! ```bash
//! cargo run -p svc-capture --example capture_probe -- out.png
//! ```

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "capture.png".into());
    let t = std::time::Instant::now();
    match svc_capture::capture_game() {
        Ok(frame) => {
            println!("{frame:?} in {:?}", t.elapsed());
            std::fs::write(&out, frame.to_png()).expect("write");
            println!("saved {out}");
        }
        Err(e) => println!("capture failed: {e}"),
    }
}
