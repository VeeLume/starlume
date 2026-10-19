//! Mining captures — the "Rock scan" and "Signature scan" buttons.
//!
//! A button press grabs one frame of the game window (svc-capture), keeps
//! it under the module's data dir by kind (the action names the category,
//! so nobody sorts screenshots into folders), and reads what that kind
//! shows: the SCAN RESULTS panel for a rock scan (`mod_mining::read_scan`);
//! signature reading comes next. A screenshot file can be read the same
//! way, for anyone who'd rather not let Starlume capture the game.
//!
//! Names on the panel are matched against the names the game shows (its
//! active localisation) rendered in its own HUD font, both read from the
//! user's install. Local only: no network, so no online gate applies.
//! Capture and reading block (template search), so they run on the
//! blocking pool.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine;
use mod_mining::{NameSet, ScanReading, read_scan};
use svc_capture::Frame;
use tauri::AppHandle;

use crate::error::AppError;
use crate::mining::catalog_for;

/// Captures kept per kind; the oldest go first.
const KEEP: usize = 40;

/// The names the panel can show, rendered in the game's HUD font: built
/// once per install build and localisation file (a few hundred ms), kept
/// for the next scan. Key: p4k path, build id, localisation file and its
/// modification time (text patching rewrites it).
type NameKey = (PathBuf, String, Option<(PathBuf, std::time::SystemTime)>);
static NAMES: Mutex<Option<(NameKey, Arc<NameSet>)>> = Mutex::new(None);

fn name_set(
    install: &svc_data::InstallRef,
    catalog: &mod_mining::Catalog,
) -> Result<Arc<NameSet>, AppError> {
    let (locale, source) = svc_data::active_locale(&install.p4k_path)
        .map_err(|e| AppError::Config(format!("can't read the game's localisation: {e:#}")))?;
    let stamp = match &source {
        svc_data::LocaleSource::Override(p) => std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| (p.clone(), t)),
        svc_data::LocaleSource::Stock => None,
    };
    let key: NameKey = (install.p4k_path.clone(), install.build_id.clone(), stamp);
    let mut cache = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, set)) = cache.as_ref()
        && *k == key
    {
        return Ok(set.clone());
    }
    let movie = svc_data::read_p4k_file(&install.p4k_path, "ui/fonts/shared/fonts_en.gfx")
        .map_err(|e| AppError::Config(format!("can't read the game's HUD fonts: {e:#}")))?;
    let font = svc_capture::gfxfont::find_font(&movie, "Blender Pro Bold")
        .map_err(|e| AppError::Config(format!("can't read the game's HUD font: {e}")))?;
    let set = Arc::new(NameSet::build(&font, catalog, &locale));
    *cache = Some((key, set.clone()));
    Ok(set)
}

/// What a capture is of.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptureKind {
    /// Mining mode, the SCAN RESULTS panel in view.
    Rock,
    /// Scan mode, a signature pinged.
    Signature,
}

impl CaptureKind {
    fn dir(self) -> &'static str {
        match self {
            Self::Rock => "rock",
            Self::Signature => "signature",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanRowView {
    pub percent: Option<f32>,
    pub name: Option<String>,
    /// Catalog element id; `null` for inert material or an unknown name.
    pub element: Option<String>,
    pub inert: bool,
    pub quality: Option<u16>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanReadingView {
    /// The panel was found (and how well its labels matched, 0..1).
    pub found: bool,
    pub match_score: f32,
    pub rock_name: Option<String>,
    pub mass: Option<f32>,
    pub resistance: Option<f32>,
    pub instability: Option<f32>,
    pub difficulty: Option<String>,
    pub rows: Vec<ScanRowView>,
    /// Which values the panel showed, when the model check placed them:
    /// `bare` (no beam on the rock) or `beam` (your loadout's beam).
    pub basis: Option<String>,
    /// The panel was read tilted by this much (head tracking), degrees.
    pub angle: f32,
    /// Checks that passed, in words.
    pub verified: Vec<String>,
    /// Fields no check confirmed ("mass", "row 2 %", …).
    pub unsure: Vec<String>,
    /// What couldn't be read or was corrected.
    pub notes: Vec<String>,
}

impl From<ScanReading> for ScanReadingView {
    fn from(r: ScanReading) -> Self {
        Self {
            found: r.panel.is_some(),
            match_score: r.panel.map_or(0.0, |p| p.score),
            rock_name: r.rock_name,
            mass: r.mass,
            resistance: r.resistance,
            instability: r.instability,
            difficulty: r.difficulty,
            rows: r
                .rows
                .into_iter()
                .map(|w| ScanRowView {
                    percent: w.percent,
                    name: w.name,
                    element: w.element,
                    inert: w.inert,
                    quality: w.quality,
                })
                .collect(),
            basis: r.basis.map(|b| match b {
                mod_mining::Basis::Bare => "bare".into(),
                mod_mining::Basis::Beam => "beam".into(),
            }),
            angle: r.panel.map_or(0.0, |p| p.angle),
            verified: r.verified,
            unsure: r.unsure,
            notes: r.notes,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureView {
    pub kind: CaptureKind,
    /// The image read (a kept capture, or the picked file).
    pub file: String,
    /// The read panel as a PNG data URL, to check the reading against.
    pub preview: Option<String>,
    /// A rock scan's reading.
    pub rock: Option<ScanReadingView>,
    /// Why there is nothing to read yet (signature reading comes next).
    pub note: Option<String>,
}

/// The HUD reader's learned data: built on the user's machine (it comes
/// from the game's HUD font, so it isn't shipped), read from here.
fn scan_data_path() -> PathBuf {
    app_kit::app_data_root().join("mining").join("scan.bin")
}

/// Whether the HUD reader has its data. Release builds offer Read scan only
/// then: without it a reading finds nothing (building it in-app is open).
#[tauri::command]
#[specta::specta]
pub(crate) fn mining_scan_ready() -> bool {
    scan_data_path().is_file()
}

fn captures_dir(kind: CaptureKind) -> PathBuf {
    app_kit::app_data_root()
        .join("mining")
        .join("captures")
        .join(kind.dir())
}

/// Keep `frame` as the newest capture of `kind`; drop the oldest past KEEP.
fn keep(kind: CaptureKind, frame: &Frame) -> Result<PathBuf, AppError> {
    let dir = captures_dir(kind);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "{}.png",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    ));
    app_kit::atomic_write(&path, &frame.to_png())?;
    let mut all: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    all.sort();
    if all.len() > KEEP {
        for old in &all[..all.len() - KEEP] {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(path)
}

fn data_url(frame: &Frame) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(frame.to_png())
    )
}

/// Read what `kind` shows in `frame`.
fn read(
    kind: CaptureKind,
    frame: &Frame,
    file: &Path,
    catalog: &mod_mining::Catalog,
    names: &NameSet,
) -> CaptureView {
    match kind {
        CaptureKind::Rock => {
            // The calculator's loadout lets the model check recognise values
            // with your beam on.
            let loadout = crate::mining::load_state().current;
            let loaded = mod_mining::load_scan_data(&scan_data_path());
            let mut reading = read_scan(frame, catalog, names, Some(&loadout));
            if let Err(e) = loaded {
                reading
                    .notes
                    .push(format!("the HUD reader's data couldn't be read: {e}"));
            }
            let preview = reading
                .panel
                .map(|spot| data_url(&mod_mining::panel_crop(frame, &spot)));
            CaptureView {
                kind,
                file: file.display().to_string(),
                preview,
                rock: Some(reading.into()),
                note: None,
            }
        }
        CaptureKind::Signature => CaptureView {
            kind,
            file: file.display().to_string(),
            preview: None,
            rock: None,
            note: Some("Kept for calibration — reading signatures comes next.".into()),
        },
    }
}

/// Capture the game window now and read it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_capture(
    app: AppHandle,
    channel: String,
    kind: CaptureKind,
) -> Result<CaptureView, AppError> {
    let catalog = catalog_for(&app, &channel).await?;
    let install = crate::data::install_for(&app, &channel).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let frame = svc_capture::capture_game().map_err(|e| AppError::Config(e.to_string()))?;
        let file = keep(kind, &frame)?;
        let names = name_set(&install, &catalog)?;
        Ok(read(kind, &frame, &file, &catalog, &names))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Read a screenshot file as a capture of `kind` (it stays where it is).
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_read_screenshot(
    app: AppHandle,
    channel: String,
    kind: CaptureKind,
    path: String,
) -> Result<CaptureView, AppError> {
    let catalog = catalog_for(&app, &channel).await?;
    let install = crate::data::install_for(&app, &channel).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let file = PathBuf::from(path);
        let frame = Frame::open(&file).map_err(|e| AppError::Config(e.to_string()))?;
        let names = name_set(&install, &catalog)?;
        Ok(read(kind, &frame, &file, &catalog, &names))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
