//! What to draw for a game's cover: the real image, or a blurred stand-in.

use dackel_launcher::blur::blurred_thumbnail;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How many blurred covers may be made in one frame, so enabling blur on a big library
/// does not stall the window. The rest appear over the next few frames.
const PER_FRAME: usize = 3;

/// How a cover is shown right now.
#[derive(Clone)]
pub enum CoverView {
    /// The real image.
    Normal,
    /// Blur is on. `None` means the blurred copy is not ready (or could not be made):
    /// draw a plain box, never the real image.
    Blurred(Option<TextureHandle>),
}

#[derive(Default)]
pub struct BlurCache {
    /// `None` records a cover that could not be decoded, so it is not retried every frame.
    textures: HashMap<PathBuf, Option<TextureHandle>>,
    budget: usize,
}

impl BlurCache {
    pub fn new_frame(&mut self) {
        self.budget = PER_FRAME;
    }

    /// Forgets every blurred copy, e.g. after covers were re-downloaded.
    pub fn clear(&mut self) {
        self.textures.clear();
    }

    /// Decides how to draw the cover at `cover` (`None` when the game has no cover file).
    pub fn cover_view(
        &mut self,
        ctx: &egui::Context,
        blur: bool,
        cover: Option<&Path>,
    ) -> CoverView {
        let Some(path) = cover.filter(|_| blur) else {
            return CoverView::Normal;
        };
        if let Some(cached) = self.textures.get(path) {
            return CoverView::Blurred(cached.clone());
        }
        if self.budget == 0 {
            ctx.request_repaint();
            return CoverView::Blurred(None);
        }
        self.budget -= 1;
        let texture = blurred_thumbnail(path).map(|b| {
            let image = ColorImage::from_rgba_unmultiplied([b.width, b.height], &b.rgba);
            // Linear filtering smooths the enlarged thumbnail, which is what makes it a blur.
            ctx.load_texture(
                format!("blur:{}", path.display()),
                image,
                TextureOptions::LINEAR,
            )
        });
        self.textures.insert(path.to_path_buf(), texture.clone());
        CoverView::Blurred(texture)
    }
}
