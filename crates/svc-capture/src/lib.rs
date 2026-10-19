//! Capture service — the game window as a [`Frame`], and the tools to read
//! HUD panels on it.
//!
//! - [`capture_game`]: one frame of the Star Citizen window (Windows
//!   Graphics Capture), on demand only.
//! - [`Frame`]: an RGBA image, from a capture or a screenshot file.
//! - [`vision`]: grey feature images, resizing, rotation and whole-image
//!   template matching (NCC via FFT).
//! - [`gfxfont`]: the game's HUD fonts, read from its own `fonts_en.gfx`
//!   and rendered, for matching HUD text against exact glyphs.
//!
//! Modules read HUD panels through this service (module rule 1); what a
//! panel means stays in the module. Nothing here touches the network.

mod frame;
pub mod gfxfont;
pub mod vision;
mod window;

pub use frame::{Frame, FrameError, Rect};
pub use window::{CaptureError, capture_game};
