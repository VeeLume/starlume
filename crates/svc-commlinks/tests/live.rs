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
    // Images: the newest wide, the next CARDS as cards, all as thumbnails.
    let wanted = svc_commlinks::wanted_images(&cache.items);
    let on_disk = std::fs::read_dir(service.images_dir()).unwrap().count();
    println!("images: {on_disk} of {} wanted", wanted.len());
    assert!(on_disk * 10 >= wanted.len() * 9, "most images downloaded");
    assert!(service.image_path(&cache.items[0], 0).is_some());

    let again = service.refresh("starlume-test").await.expect("refresh");
    assert!(again.new_items.is_empty());
}
