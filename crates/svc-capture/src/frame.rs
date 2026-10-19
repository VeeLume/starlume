use std::path::Path;

/// An RGBA image, row-major, 4 bytes a pixel.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Frame({}x{})", self.width, self.height)
    }
}

/// A pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("can't read the image: {0}")]
    Decode(String),
}

impl Frame {
    /// A screenshot file (PNG or JPEG).
    pub fn open(path: &Path) -> Result<Self, FrameError> {
        let img = image::open(path).map_err(|e| FrameError::Decode(e.to_string()))?;
        Ok(Self::from_image(img))
    }

    /// An encoded image in memory (PNG or JPEG).
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        let img = image::load_from_memory(bytes).map_err(|e| FrameError::Decode(e.to_string()))?;
        Ok(Self::from_image(img))
    }

    fn from_image(img: image::DynamicImage) -> Self {
        let rgba = img.into_rgba8();
        Self {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        }
    }

    /// The pixel at (x, y) as [r, g, b].
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    /// A copy of `r`, clamped to the frame.
    pub fn crop(&self, r: Rect) -> Frame {
        let x0 = r.x.min(self.width);
        let y0 = r.y.min(self.height);
        let x1 = (r.x + r.w).min(self.width);
        let y1 = (r.y + r.h).min(self.height);
        let (w, h) = (x1 - x0, y1 - y0);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in y0..y1 {
            let s = ((y * self.width + x0) * 4) as usize;
            rgba.extend_from_slice(&self.rgba[s..s + (w * 4) as usize]);
        }
        Frame {
            width: w,
            height: h,
            rgba,
        }
    }

    /// PNG bytes (debug views, saved captures).
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let img = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .expect("frame buffer matches its size");
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("PNG encoding into memory");
        out
    }
}
