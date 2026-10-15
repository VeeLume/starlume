//! Live check against Spectrum (needs the RSI launcher session):
//! `cargo test -p svc-spectrum --test live_hotfix -- --ignored --nocapture`.

use svc_spectrum::SpectrumService;

#[tokio::test]
#[ignore = "reads Spectrum"]
async fn reads_hotfix_central() {
    let dir = tempfile::tempdir().unwrap();
    let service = SpectrumService::new(dir.path().to_path_buf());
    let (previous, cache) = service
        .refresh_hotfix("starlume-test")
        .await
        .expect("hotfix");
    assert!(previous.is_none());
    let central = cache.central.expect("a pinned Hotfix Central");
    println!(
        "{} ({}), edited {}",
        central.subject, central.url, central.edited_ms
    );
    println!("status: {:?}", central.status);
    for e in central.entries.iter().take(3) {
        println!(
            "- {:?} {:?} {:?} {}",
            e.date, e.channel, e.builds, e.headline
        );
        for i in e.items.iter().take(6) {
            println!("    {}{}", if i.bullet { "• " } else { "" }, i.text);
        }
    }
    assert!(service.cached_hotfix().is_some());
}
