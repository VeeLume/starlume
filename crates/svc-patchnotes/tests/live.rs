//! One real refresh against RSI's public patch-note pages, into a temp
//! cache. Network, so ignored: `cargo test -p svc-patchnotes --test live --
//! --ignored --nocapture`.

use svc_patchnotes::{ARTICLE_WINDOW, PatchNotesService};

#[tokio::test]
#[ignore = "reads RSI's website"]
async fn refreshes_from_rsi() {
    let dir = tempfile::tempdir().unwrap();
    let service = PatchNotesService::new(dir.path().to_path_buf());
    let report = service.refresh("starlume-test").await.expect("refresh");
    assert!(report.first);
    assert!(report.failed.is_empty(), "failed: {:?}", report.failed);
    let cache = service.cached().expect("cached");
    assert_eq!(cache.articles.len(), ARTICLE_WINDOW);
    let (article, latest) = cache.latest().expect("a release");
    println!(
        "latest: {} {:?} {:?} build {:?} — {} — {:?} — fixes {:?}",
        latest.title,
        latest.date,
        latest.name,
        latest.build.as_ref().map(|b| &b.raw),
        latest.highlights.join(" · "),
        latest.build_info,
        latest.counts.bug_fixes,
    );
    println!(
        "article {} · {} releases parsed · {} listed",
        article.url,
        cache.releases().count(),
        cache.index.len()
    );
    // A second refresh reads only the newest article and finds nothing new.
    let again = service.refresh("starlume-test").await.expect("refresh");
    assert_eq!(again.articles_fetched, 1);
    assert!(again.new_releases.is_empty());
}
