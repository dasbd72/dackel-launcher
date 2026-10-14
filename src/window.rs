//! Window-mode decisions as pure functions of the environment and settings.

use crate::settings::Settings;

/// UI scales offered in the dropdown, and cycled through by the controller.
pub const SCALE_PRESETS: [f32; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// True in a Steam Deck / Gamescope session. A desktop that merely has gamescope
/// configured (e.g. a `GAMESCOPECMD` variable) is not.
pub fn is_deck_like(get: impl Fn(&str) -> Option<String>) -> bool {
    let set = |key: &str| get(key).is_some_and(|v| !v.is_empty());
    set("GAMESCOPE_WAYLAND_DISPLAY")
        || set("SteamGamepadUI")
        || get("SteamDeck").as_deref() == Some("1")
        || get("XDG_CURRENT_DESKTOP").is_some_and(|d| d.eq_ignore_ascii_case("gamescope"))
}

/// Mirrors winit's backend choice: Wayland whenever a Wayland display or socket is
/// present, else X11. (The session-type variable is not consulted, as winit ignores it.)
pub fn is_native_wayland(get: impl Fn(&str) -> Option<String>) -> bool {
    let set = |key: &str| get(key).is_some_and(|v| !v.is_empty());
    set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET")
}

/// Minimize hands the screen to the game on the Deck. Native Wayland has no request for an
/// app to un-minimize itself, so there the launcher stays put and the compositor brings it
/// back when the game's window closes.
pub fn should_minimize_on_launch(deck_like: bool, native_wayland: bool) -> bool {
    deck_like && !native_wayland
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreStep {
    Idle,
    Request,
    Done,
}

const RESTORE_FIRST_DELAY_MS: u64 = 300;
const RESTORE_INTERVAL_MS: u64 = 700;
const RESTORE_MAX_REQUESTS: u32 = 6;

/// Retries "bring the launcher back" after a game exits. A window manager may refuse
/// while the exiting game's window is still the active one, so the first ask waits a
/// moment and later asks repeat until the window is visible or the tries run out.
#[derive(Debug, Default)]
pub struct Restore {
    armed: bool,
    next_ms: u64,
    requests: u32,
}

impl Restore {
    pub fn arm(&mut self, now_ms: u64) {
        self.armed = true;
        self.next_ms = now_ms + RESTORE_FIRST_DELAY_MS;
        self.requests = 0;
    }

    pub fn poll(&mut self, now_ms: u64, still_minimized: bool) -> RestoreStep {
        if !self.armed {
            return RestoreStep::Idle;
        }
        if !still_minimized || self.requests >= RESTORE_MAX_REQUESTS && now_ms >= self.next_ms {
            self.armed = false;
            return RestoreStep::Done;
        }
        if now_ms < self.next_ms {
            return RestoreStep::Idle;
        }
        self.requests += 1;
        self.next_ms = now_ms + RESTORE_INTERVAL_MS;
        RestoreStep::Request
    }
}
