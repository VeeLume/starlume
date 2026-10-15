//! Card images on RSI's media host — shared by the comm-links and the patch
//! notes (a patch-note article is a comm-link; its index card carries the
//! same thumbnail URL).
//!
//! A listing card gives the `heap_infobox` thumbnail URL
//! (`https://media.robertsspaceindustries.com/{code}/heap_infobox.{ext}`);
//! the same `{code}` serves other sizes under other stems. Images are
//! downloaded into a directory the shell lets the webview read, so the page
//! never fetches from RSI itself.
//!
//! # Online-policy contract
//!
//! [`sync`] is network: call it only after `AppState::require_online()`
//! passed (both services call it from inside their gated refresh).
//! Everything else here is pure or reads the local directory.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Spacing between image downloads (a CDN, but still paced).
pub const IMAGE_INTERVAL: Duration = Duration::from_millis(250);

/// Images larger than this are not stored (the wide size is ~200 KB).
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// The sizes RSI's media host serves a card image in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageSize {
    /// 165 × 165, ~20–35 KB — the listing's own thumbnail (`heap_infobox`).
    Thumb,
    /// 351 × 210, ~70 KB (`store_small`).
    Card,
    /// 648 × 366, ~200 KB (`slideshow`).
    Wide,
}

impl ImageSize {
    const ALL: [ImageSize; 3] = [ImageSize::Thumb, ImageSize::Card, ImageSize::Wide];

    /// The media host's name for the size (the file stem in its URLs).
    fn variant(self) -> &'static str {
        match self {
            ImageSize::Thumb => "heap_infobox",
            ImageSize::Card => "store_small",
            ImageSize::Wide => "slideshow",
        }
    }

    /// The size's name in local file names.
    fn slug(self) -> &'static str {
        match self {
            ImageSize::Thumb => "thumb",
            ImageSize::Card => "card",
            ImageSize::Wide => "wide",
        }
    }
}

/// One image to have on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantedImage {
    /// Local file name (`{id}-{size}.{ext}`).
    pub file: String,
    pub url: String,
}

impl WantedImage {
    /// Item `id`'s image in `size`, from its listing thumbnail URL. `None`
    /// for a thumbnail not on RSI's media host.
    pub fn new(id: u64, thumbnail: &str, size: ImageSize) -> Option<Self> {
        let (url, ext) = sized_url(thumbnail, size)?;
        Some(Self {
            file: file_name(id, size, &ext),
            url,
        })
    }
}

fn file_name(id: u64, size: ImageSize, ext: &str) -> String {
    format!("{id}-{}.{ext}", size.slug())
}

/// The URL of `size` for a listing thumbnail URL, and the extension. `None`
/// for any other host or shape.
fn sized_url(thumbnail: &str, size: ImageSize) -> Option<(String, String)> {
    let rest = thumbnail.strip_prefix("https://media.robertsspaceindustries.com/")?;
    let (code, file) = rest.split_once('/')?;
    let (_, ext) = file.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    let ok_code = !code.is_empty() && code.chars().all(|c| c.is_ascii_alphanumeric());
    if !ok_code || !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return None;
    }
    Some((
        format!(
            "https://media.robertsspaceindustries.com/{code}/{}.{ext}",
            size.variant()
        ),
        ext,
    ))
}

/// The best image on disk in `dir` for item `id`: `size`, else a smaller
/// one already there. No network.
pub fn best_on_disk(dir: &Path, id: u64, thumbnail: &str, size: ImageSize) -> Option<PathBuf> {
    ImageSize::ALL
        .iter()
        .rev()
        .filter(|s| **s <= size)
        .find_map(|s| {
            let (_, ext) = sized_url(thumbnail, *s)?;
            let path = dir.join(file_name(id, *s, &ext));
            path.is_file().then_some(path)
        })
}

/// Make `dir` hold exactly `wanted`: delete files nothing wants, download
/// the missing ones. A failed image is skipped (the page falls back to a
/// smaller size or none) and retried on the next call. Returns how many
/// were fetched. **Network** — see the module contract.
pub async fn sync(dir: &Path, wanted: &[WantedImage], user_agent: &str) -> usize {
    let keep: BTreeSet<&str> = wanted.iter().map(|w| w.file.as_str()).collect();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !keep.contains(name.to_string_lossy().as_ref()) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let missing: Vec<&WantedImage> = wanted
        .iter()
        .filter(|w| !dir.join(&w.file).is_file())
        .collect();
    if missing.is_empty() {
        return 0;
    }
    let client = match reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("card images: no HTTP client: {e}");
            return 0;
        }
    };
    let mut fetched = 0;
    for (i, image) in missing.into_iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(IMAGE_INTERVAL).await;
        }
        match fetch(&client, &image.url).await {
            Ok(bytes) => match app_kit::atomic_write(&dir.join(&image.file), &bytes) {
                Ok(()) => fetched += 1,
                Err(e) => tracing::warn!(file = %image.file, "card image not saved: {e}"),
            },
            Err(e) => tracing::info!(url = %image.url, "card image skipped: {e}"),
        }
    }
    fetched
}

async fn fetch(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let is_image = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.starts_with("image/"));
    if !is_image {
        return Err("not an image".into());
    }
    // The cap holds before anything is buffered: refuse on the declared
    // length, then read chunk by chunk and stop as soon as it is passed.
    if response
        .content_length()
        .is_some_and(|n| n > MAX_IMAGE_BYTES as u64)
    {
        return Err("too large".into());
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_IMAGE_BYTES {
            return Err("too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_urls_keep_code_and_extension() {
        let thumb = "https://media.robertsspaceindustries.com/dtl4zpmtu3s0a/heap_infobox.png";
        assert_eq!(
            sized_url(thumb, ImageSize::Card),
            Some((
                "https://media.robertsspaceindustries.com/dtl4zpmtu3s0a/store_small.png".into(),
                "png".into()
            ))
        );
        assert_eq!(
            sized_url("https://example.test/a/heap_infobox.jpg", ImageSize::Wide),
            None
        );
        assert_eq!(
            sized_url(
                "https://media.robertsspaceindustries.com/../x/heap_infobox.jpg",
                ImageSize::Wide
            ),
            None,
            "only a plain media code"
        );
        assert_eq!(
            sized_url(
                "https://media.robertsspaceindustries.com/abc/heap_infobox.svg",
                ImageSize::Wide
            ),
            None
        );
    }

    #[test]
    fn best_on_disk_falls_back_to_a_smaller_size() {
        let dir = tempfile::tempdir().unwrap();
        let thumb = "https://media.robertsspaceindustries.com/abc/heap_infobox.jpg";
        let d = dir.path();
        assert_eq!(best_on_disk(d, 7, thumb, ImageSize::Wide), None);
        std::fs::write(d.join("7-thumb.jpg"), b"x").unwrap();
        assert_eq!(
            best_on_disk(d, 7, thumb, ImageSize::Wide),
            Some(d.join("7-thumb.jpg"))
        );
        std::fs::write(d.join("7-wide.jpg"), b"x").unwrap();
        assert_eq!(
            best_on_disk(d, 7, thumb, ImageSize::Wide),
            Some(d.join("7-wide.jpg"))
        );
        assert_eq!(
            best_on_disk(d, 7, thumb, ImageSize::Thumb),
            Some(d.join("7-thumb.jpg")),
            "never larger than asked"
        );
    }
}
