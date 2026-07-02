//! Regenerate `src/lib/bindings.ts` from the current Rust command surface
//! without booting the full Tauri app.
//!
//! Run from anywhere: `cargo run --bin export-bindings --features bindgen`
//! (the output path is anchored on this crate's manifest dir, not the cwd).

fn main() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/bindings.ts");
    starlume_lib::export_bindings(out.to_str().expect("utf-8 path"))
        .expect("failed to export bindings");
    println!("wrote {}", out.display());
}
