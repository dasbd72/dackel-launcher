//! Window-mode decisions as pure functions of the environment and settings.

use crate::settings::Settings;

const SCALE_PRESETS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// True in a Steam Deck / Gamescope session. A desktop that merely has gamescope
/// configured (e.g. a `GAMESCOPECMD` variable) is not.
pub fn is_deck_like(get: impl Fn(&str) -> Option<String>) -> bool {
    let set = |key: &str| get(key).is_some_and(|v| !v.is_empty());
    set("GAMESCOPE_WAYLAND_DISPLAY")
        || set("SteamGamepadUI")
        || get("SteamDeck").as_deref() == Some("1")
        || get("XDG_CURRENT_DESKTOP").is_some_and(|d| d.eq_ignore_ascii_case("gamescope"))
}

pub fn default_ui_scale(deck_like: bool) -> f32 {
    if deck_like { 1.25 } else { 1.0 }
}

/// A saved choice wins; otherwise the Deck starts fullscreen and the desktop windowed.
pub fn initial_fullscreen(settings: &Settings, deck_like: bool) -> bool {
    settings.fullscreen.unwrap_or(deck_like)
}

/// The next preset above `current`, wrapping to the smallest.
pub fn next_scale(current: f32) -> f32 {
    SCALE_PRESETS
        .iter()
        .copied()
        .find(|&preset| preset > current + 1e-3)
        .unwrap_or(SCALE_PRESETS[0])
}
