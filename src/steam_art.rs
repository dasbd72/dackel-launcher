//! The images written for a game's Steam entry: the cover as capsules, a soft hero made from
//! it, and an icon that is either the cover cropped square or the icon inside the game's exe.

use crate::steam::write_grid;
use image::{DynamicImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Where a game's Steam icon comes from.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconSource {
    /// The middle of the cover, cropped square.
    #[default]
    Cover,
    /// The application icon stored in the game's exe.
    Exe,
}

const ICON_SIZE: u32 = 256;
const HERO_SIZE: (u32, u32) = (3840, 1240);

/// The icon in a Windows program: the first application icon in its resources.
pub fn exe_icon(exe: &[u8]) -> Result<DynamicImage, String> {
    use pelite::PeFile;
    let pe = PeFile::from_bytes(exe).map_err(|e| format!("not a Windows program: {e}"))?;
    let resources = pe
        .resources()
        .map_err(|_| "the program has no resources".to_owned())?;
    let (_, group) = resources
        .icons()
        .flatten()
        .next()
        .ok_or("the program has no icon")?;
    let mut ico = Vec::new();
    group.write(&mut ico).map_err(|e| e.to_string())?;
    image::load_from_memory_with_format(&ico, image::ImageFormat::Ico).map_err(|e| e.to_string())
}

/// The middle square of the cover, as the icon.
pub fn cover_icon(cover: &DynamicImage) -> DynamicImage {
    cover.resize_to_fill(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3)
}

/// A soft, wide version of the cover for the library page's background. Blurred small and
/// scaled up, which is smooth and fast.
pub fn cover_hero(cover: &DynamicImage) -> DynamicImage {
    let small = cover
        .resize_to_fill(HERO_SIZE.0 / 10, HERO_SIZE.1 / 10, FilterType::Triangle)
        .blur(8.0);
    DynamicImage::ImageRgba8(
        small
            .resize_exact(HERO_SIZE.0, HERO_SIZE.1, FilterType::CatmullRom)
            .to_rgba8(),
    )
}

/// Writes a game's images into Steam's grid folder: the cover as the wide and portrait
/// capsules, the hero and the icon. A request for the exe's icon that cannot be met (no exe,
/// no icon in it) uses the cover instead; the result says which one was written.
pub fn write_game_art(
    grid_dir: &Path,
    app_id: u32,
    cover: &Path,
    icon: IconSource,
    exe: Option<&Path>,
) -> io::Result<IconSource> {
    write_grid(cover, grid_dir, app_id)?;
    let image = image::open(cover).map_err(io::Error::other)?;
    save_fast(&cover_hero(&image), grid_dir, &format!("{app_id}_hero.png"))?;
    let from_exe = match (icon, exe) {
        (IconSource::Exe, Some(exe)) => std::fs::read(exe)
            .ok()
            .and_then(|bytes| exe_icon(&bytes).ok()),
        _ => None,
    };
    let (picture, used) = match from_exe {
        Some(picture) => (picture, IconSource::Exe),
        None => (cover_icon(&image), IconSource::Cover),
    };
    save(&picture, grid_dir, &format!("{app_id}_icon.png"))?;
    Ok(used)
}

fn save(picture: &DynamicImage, dir: &Path, name: &str) -> io::Result<()> {
    picture.save(dir.join(name)).map_err(io::Error::other)
}

/// Saves with the fastest PNG settings, for the large hero: it is soft, so it still shrinks well.
fn save_fast(picture: &DynamicImage, dir: &Path, name: &str) -> io::Result<()> {
    use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
    let file = std::io::BufWriter::new(std::fs::File::create(dir.join(name))?);
    let encoder = PngEncoder::new_with_quality(file, CompressionType::Fast, PngFilter::NoFilter);
    picture
        .write_with_encoder(encoder)
        .map_err(io::Error::other)
}
