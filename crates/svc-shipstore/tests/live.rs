//! One real read of RSI's pledge-store ship list. Network, so ignored:
//! `cargo test -p svc-shipstore --test live -- --ignored --nocapture`.
//! Leaves the list in `%TEMP%\starlume-shipstore-live.json`, where
//! mod-ships' real-data test measures the name join against it.

use svc_shipstore::{PledgeStore, ShipStoreCache};

#[tokio::test]
#[ignore = "reads RSI's website"]
async fn lists_the_store_ships() {
    let ships = PledgeStore::new("starlume-test")
        .ships()
        .await
        .expect("ships");
    assert!(ships.len() > 200, "{} ships", ships.len());
    let with_image = ships.iter().filter(|s| s.media.thumbnail.is_some()).count();
    println!("{} ships, {with_image} with an image", ships.len());
    for s in ships.iter().take(5) {
        println!("{} {} {:?}", s.id, s.name, s.media.thumbnail);
    }
    assert!(with_image * 10 >= ships.len() * 9);
    let cache = ShipStoreCache {
        ships,
        fetched_at: 1,
    };
    let path = std::env::temp_dir().join("starlume-shipstore-live.json");
    std::fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();
    println!("wrote {}", path.display());
}
