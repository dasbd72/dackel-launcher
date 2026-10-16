//! Icon buttons and small shared widgets.
//!
//! The icons are Phosphor Icons (MIT, see `assets/icons/LICENSE-phosphor.txt`), embedded as
//! SVG so they look the same on every machine and need no icon font. The files are white;
//! egui tints them to whatever color the button needs.

use eframe::egui::{self, Color32, Image, Pos2, Rect, Response, Sense, StrokeKind, Ui, Vec2, vec2};

/// Height shared by every button, text or icon, so rows of mixed buttons line up.
pub const BUTTON_HEIGHT: f32 = 34.0;
/// Size of an icon-only button.
pub const ICON_BUTTON: Vec2 = Vec2::new(46.0, BUTTON_HEIGHT);
const ICON_SIZE: f32 = 20.0;

const PLAY_GREEN: Color32 = Color32::from_rgb(76, 200, 118);
const QUIT_RED: Color32 = Color32::from_rgb(235, 110, 100);
const HEART_RED: Color32 = Color32::from_rgb(235, 72, 96);

const PLAY: &[u8] = include_bytes!("../assets/icons/play-fill.svg");
const HEART_FILL: &[u8] = include_bytes!("../assets/icons/heart-fill.svg");
const HEART: &[u8] = include_bytes!("../assets/icons/heart.svg");
const INFO: &[u8] = include_bytes!("../assets/icons/info.svg");
const CLOSE: &[u8] = include_bytes!("../assets/icons/x.svg");
const GEAR: &[u8] = include_bytes!("../assets/icons/gear.svg");
const RESCAN: &[u8] = include_bytes!("../assets/icons/arrows-clockwise.svg");
const FOLDER: &[u8] = include_bytes!("../assets/icons/folder-open.svg");
const FULLSCREEN: &[u8] = include_bytes!("../assets/icons/arrows-out.svg");
const EYE: &[u8] = include_bytes!("../assets/icons/eye.svg");
const EYE_SLASH: &[u8] = include_bytes!("../assets/icons/eye-slash.svg");
const POWER: &[u8] = include_bytes!("../assets/icons/power.svg");

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Heart {
        filled: bool,
    },
    Info,
    Close,
    Settings,
    Rescan,
    Libraries,
    Fullscreen,
    /// Blur is on: the images are hidden.
    Blur {
        on: bool,
    },
    Quit,
}

impl Icon {
    fn image(self) -> Image<'static> {
        let (uri, bytes) = match self {
            Icon::Play => ("bytes://icons/play-fill.svg", PLAY),
            Icon::Heart { filled: true } => ("bytes://icons/heart-fill.svg", HEART_FILL),
            Icon::Heart { filled: false } => ("bytes://icons/heart.svg", HEART),
            Icon::Info => ("bytes://icons/info.svg", INFO),
            Icon::Close => ("bytes://icons/x.svg", CLOSE),
            Icon::Settings => ("bytes://icons/gear.svg", GEAR),
            Icon::Rescan => ("bytes://icons/arrows-clockwise.svg", RESCAN),
            Icon::Libraries => ("bytes://icons/folder-open.svg", FOLDER),
            Icon::Fullscreen => ("bytes://icons/arrows-out.svg", FULLSCREEN),
            Icon::Blur { on: true } => ("bytes://icons/eye-slash.svg", EYE_SLASH),
            Icon::Blur { on: false } => ("bytes://icons/eye.svg", EYE),
            Icon::Quit => ("bytes://icons/power.svg", POWER),
        };
        Image::from_bytes(uri, bytes)
    }

    /// The color of the icon on a button; `ink` is the normal text color.
    fn color(self, enabled: bool, ink: Color32, weak: Color32) -> Color32 {
        match self {
            _ if !enabled => weak,
            Icon::Play => PLAY_GREEN,
            Icon::Heart { filled: true } => HEART_RED,
            Icon::Quit => QUIT_RED,
            _ => ink,
        }
    }
}

/// An icon-only button with a tooltip.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str, enabled: bool) -> Response {
    let (rect, response) = allocate(ui, ICON_BUTTON, enabled);
    paint_background(ui, rect, &response, enabled);
    paint_icon(ui, icon, rect.center(), enabled);
    response.on_hover_text(tip)
}

/// A button with an icon followed by a text label.
pub fn icon_text_button(ui: &mut Ui, icon: Icon, text: &str, enabled: bool) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let pad = 12.0;
    let gap = 8.0;
    let width = pad + ICON_SIZE + gap + galley.size().x + pad;
    let (rect, response) = allocate(ui, vec2(width, BUTTON_HEIGHT), enabled);
    paint_background(ui, rect, &response, enabled);
    let icon_center = Pos2::new(rect.left() + pad + ICON_SIZE / 2.0, rect.center().y);
    paint_icon(ui, icon, icon_center, enabled);
    let text_pos = Pos2::new(
        rect.left() + pad + ICON_SIZE + gap,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, color);
    response
}

/// Vertical padding inside a chip.
const CHIP_PAD_Y: i8 = 2;

/// A small rounded box with a background, for circle and tag names.
///
/// Painted and allocated by hand rather than with a `Frame`: a `Frame` is not wrapped by
/// `horizontal_wrapped`, so a row of chips ran past the right edge instead of onto a new line.
pub fn chip(ui: &mut Ui, text: &str) {
    let color = ui.visuals().text_color();
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        color,
    );
    let size = galley.size() + vec2(14.0, 2.0 * f32::from(CHIP_PAD_Y));
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, 4.0, ui.visuals().widgets.inactive.bg_fill);
    ui.painter()
        .galley(rect.min + vec2(7.0, f32::from(CHIP_PAD_Y)), galley, color);
}

fn allocate(ui: &mut Ui, size: Vec2, enabled: bool) -> (Rect, Response) {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    ui.allocate_exact_size(size, sense)
}

fn paint_background(ui: &Ui, rect: Rect, response: &Response, enabled: bool) {
    let visuals = if enabled {
        ui.style().interact(response)
    } else {
        &ui.visuals().widgets.noninteractive
    };
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
}

fn paint_icon(ui: &Ui, icon: Icon, center: Pos2, enabled: bool) {
    let color = icon.color(
        enabled,
        ui.visuals().text_color(),
        ui.visuals().weak_text_color(),
    );
    let rect = Rect::from_center_size(center, Vec2::splat(ICON_SIZE));
    icon.image().tint(color).paint_at(ui, rect);
}
