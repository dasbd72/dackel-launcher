//! Window-mode decisions as pure functions of the environment and settings.

use crate::settings::Settings;

/// UI scales offered in the dropdown.
pub const SCALE_PRESETS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75];

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

/// The Deck's Gaming Mode draws the UI too large at 1.0, so it starts at half size. The scale
/// multiplies the system's own scale (see `scale_label`), and a saved choice wins over this.
pub fn default_ui_scale(deck_like: bool) -> f32 {
    if deck_like { 0.5 } else { 1.0 }
}

/// The scale dropdown's text: the launcher's own scale and the system scale it multiplies, so
/// a report of a too-large UI says which of the two is responsible.
pub fn scale_label(scale: f32, system: f32) -> String {
    format!("UI x{scale:.2} (system x{system:.2})")
}

/// A saved choice wins; otherwise the Deck starts fullscreen and the desktop windowed.
pub fn initial_fullscreen(settings: &Settings, deck_like: bool) -> bool {
    settings.fullscreen.unwrap_or(deck_like)
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

/// A window change decided during a frame, sent to the window when the frame ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    Fullscreen(bool),
    Minimize,
}

/// What the launcher knows about its own window: fullscreen, UI scale, and whether a game
/// launch minimized it (so only then is it brought back when the game exits).
#[derive(Debug)]
pub struct WindowState {
    fullscreen: bool,
    scale: f32,
    minimize_on_launch: bool,
    commands: Vec<WindowCommand>,
    zoom_request: Option<f32>,
    minimized_by_us: bool,
    restore: Restore,
}

impl WindowState {
    pub fn new(fullscreen: bool, scale: f32, minimize_on_launch: bool) -> Self {
        Self {
            fullscreen,
            scale,
            minimize_on_launch,
            commands: Vec::new(),
            zoom_request: None,
            minimized_by_us: false,
            restore: Restore::default(),
        }
    }

    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn minimized_by_us(&self) -> bool {
        self.minimized_by_us
    }

    /// Flips fullscreen, asks the window to follow, and returns the new value.
    pub fn toggle_fullscreen(&mut self) -> bool {
        self.fullscreen = !self.fullscreen;
        self.commands
            .push(WindowCommand::Fullscreen(self.fullscreen));
        self.fullscreen
    }

    /// Remembers a UI scale and asks for the window to be zoomed to it.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.zoom_request = Some(scale);
    }

    /// The zoom to apply this frame, once.
    pub fn take_zoom_request(&mut self) -> Option<f32> {
        self.zoom_request.take()
    }

    /// Whether the window's zoom has moved off the remembered scale (the user zoomed with
    /// the keyboard), so the new value should be remembered.
    pub fn zoom_differs(&self, zoom: f32) -> bool {
        (zoom - self.scale).abs() > 1e-3
    }

    /// A game just started: hand the screen to it, if this machine does that.
    pub fn game_started(&mut self) {
        if self.minimize_on_launch {
            self.commands.push(WindowCommand::Minimize);
            self.minimized_by_us = true;
        }
    }

    /// The window changes queued since the last call, in order.
    pub fn drain_commands(&mut self) -> Vec<WindowCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Called every frame with whether a game was and is running. After the last game
    /// exits it retries bringing the launcher back, but only if a launch minimized it.
    /// `None` means nothing is waiting; otherwise the step to take now. `minimized` is
    /// asked only when a step is due.
    pub fn after_game_poll(
        &mut self,
        now_ms: u64,
        was_running: bool,
        running: bool,
        minimized: impl FnOnce() -> bool,
    ) -> Option<RestoreStep> {
        if was_running && !running && self.minimized_by_us {
            self.restore.arm(now_ms);
        }
        if !(self.minimized_by_us && !running) {
            return None;
        }
        let step = self.restore.poll(now_ms, minimized());
        if step == RestoreStep::Done {
            self.minimized_by_us = false;
        }
        Some(step)
    }
}
