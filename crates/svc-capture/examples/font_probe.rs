//! Render text with a font from the game's fonts_en.gfx to a PNG.
//!
//! ```bash
//! cargo run -p svc-capture --example font_probe -- <fonts_en.gfx> "<font name>" <px> "<text>" out.png
//! ```

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let movie = std::fs::read(&a[0]).expect("movie");
    let font = svc_capture::gfxfont::find_font(&movie, &a[1]).expect("font");
    let px: f32 = a[2].parse().expect("px");
    let g = font.render(&a[3], px, 1.0);
    let png: Vec<u8> = g.v.iter().map(|v| (v * 255.0) as u8).collect();
    image::GrayImage::from_raw(g.w as u32, g.h as u32, png)
        .unwrap()
        .save(&a[4])
        .unwrap();
    println!("{}x{} -> {}", g.w, g.h, a[4]);
}
