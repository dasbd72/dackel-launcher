//! Blurred copies of cover images, for the "hide NSFW covers" switch.
//!
//! The blur is made by shrinking the image to a thumbnail a few dozen pixels wide and
//! blurring that. Shown enlarged with smooth filtering it looks like a heavy blur, and
//! because so little of the picture survives, nothing in it can be made out.

use std::path::Path;

/// The longest side of the thumbnail, in pixels.
pub const MAX_SIDE: u32 = 40;
const SIGMA: f32 = 1.5;

pub struct Blurred {
    pub width: usize,
    pub height: usize,
    /// RGBA, 4 bytes per pixel, row by row.
    pub rgba: Vec<u8>,
}

/// A blurred thumbnail of the image at `path`. `None` when the file is missing or cannot
/// be decoded, so the caller shows a placeholder, never the real image.
pub fn blurred_thumbnail(path: &Path) -> Option<Blurred> {
    let small = image::open(path)
        .ok()?
        .thumbnail(MAX_SIDE, MAX_SIDE)
        .to_rgba8();
    let blurred = image::imageops::blur(&small, SIGMA);
    Some(Blurred {
        width: blurred.width() as usize,
        height: blurred.height() as usize,
        rgba: blurred.into_raw(),
    })
}
