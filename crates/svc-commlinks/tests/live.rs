//! One real refresh against RSI's comm-link listing, into a temp cache.
//! Network, so ignored: `cargo test -p svc-commlinks --test live --
//! --ignored --nocapture`.

use svc_commlinks::CommLinksService;

#[tokio::test]
#[ignore = "reads RSI's website"]
async fn refreshes_from_rsi() {
    let dir = tempfile::tempdir().unwrap();
    let service = CommLinksService::new(dir.path().to_path_buf());
    let report = service.refresh("starlume-test").await.expect("refresh");
    assert!(report.first);
    let cache = service.cached().expect("cached");
    assert!(cache.items.len() >= 10, "two pages: {}", cache.items.len());
    for i in cache.items.iter().take(5) {
        println!(
            "{} {:?} {:?} {} — {}",
            i.id, i.kind, i.channel, i.age_text, i.title
        );
    }
    let again = service.refresh("starlume-test").await.expect("refresh");
    assert!(again.new_items.is_empty());
}
