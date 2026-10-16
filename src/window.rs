//! Window-mode decisions as pure functions of the environment and settings.

use crate::settings::Settings;

/// Sizes on screen offered in the dropdown. A size is the UI's zoom times the system's own
/// scale, so the same size looks alike on a desktop and on a Deck whose scalings multiply.
pub const SCALE_PRESETS: [f32; 7] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5];

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

/// The size on screen a machine starts at when the system's scale is known.
pub const DEFAULT_SIZE: f32 = 1.5;

fn usable(system: Option<f32>) -> Option<f32> {
    system.filter(|s| s.is_finite() && *s > 0.0)
}

/// The zoom that makes the UI `size` big on screen: the size divided by the system's scale.
/// With no (or a nonsense) system scale the size is the zoom.
pub fn zoom_for(size: f32, system: Option<f32>) -> f32 {
    let zoom = size / usable(system).unwrap_or(1.0);
    zoom.clamp(0.1, 4.0)
}

/// The size on screen a zoom gives, to two decimals; the inverse of `zoom_for`.
pub fn size_for(zoom: f32, system: Option<f32>) -> f32 {
    (zoom * usable(system).unwrap_or(1.0) * 100.0).round() / 100.0
}

/// The size to start at, and whether it should be saved. A saved size wins. An old saved
/// zoom becomes the size it gave on this system, saved so it is converted once. Otherwise
/// the default size, or, when the system's scale is unknown (so a size cannot be told from a
/// zoom), the old fallbacks: half size on the Deck, which draws too large at 1.0, else 1.0.
pub fn initial_size(
    saved: Option<f32>,
    legacy_zoom: Option<f32>,
    deck_like: bool,
    system: Option<f32>,
) -> (f32, bool) {
    if let Some(size) = saved {
        return (size, false);
    }
    match (legacy_zoom, usable(system)) {
        (Some(zoom), known) => (size_for(zoom, known), known.is_some()),
        (None, Some(_)) => (DEFAULT_SIZE, false),
        (None, None) => (if deck_like { 0.5 } else { 1.0 }, false),
    }
}

/// The scale dropdown's text: the size on screen and the system scale it is made from, so a
/// report of a too-large UI says which of the two is responsible. `system` is `None` when the
/// window system reported none, which must not read as a scale of 1.
pub fn scale_label(size: f32, system: Option<f32>) -> String {
    match system {
        Some(system) => format!("Size x{size:.2} (system x{system:.2})"),
        None => format!("Size x{size:.2} (system ?)"),
    }
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
    /// The size on screen the user wants; the zoom follows from it and the system's scale.
    size: f32,
    system: Option<f32>,
    /// The system's scale has been seen (or given up on), so the zoom can be trusted.
    synced: bool,
    frames_without_system: u32,
    minimize_on_launch: bool,
    commands: Vec<WindowCommand>,
    zoom_request: Option<f32>,
    minimized_by_us: bool,
    restore: Restore,
}

impl WindowState {
    pub fn new(fullscreen: bool, size: f32, minimize_on_launch: bool) -> Self {
        Self {
            fullscreen,
            size,
            system: None,
            synced: false,
            frames_without_system: 0,
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

    /// The size on screen the UI is set to.
    pub fn size(&self) -> f32 {
        self.size
    }

    pub fn system_scale(&self) -> Option<f32> {
        self.system
    }

    /// Whether the system's scale has been seen, so sizes can be turned into zooms.
    pub fn synced(&self) -> bool {
        self.synced
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

    /// Remembers a size on screen and asks for the window to be zoomed to it.
    pub fn set_size(&mut self, size: f32) {
        self.size = size;
        self.zoom_request = Some(zoom_for(size, self.system));
    }

    /// Called every frame with the system's scale. When it is first seen, or changes, the
    /// zoom is worked out again so the size on screen stays what it was. After a few frames
    /// without one it is given up on, and the size is used as the zoom. Returns true the
    /// first time the zoom can be trusted.
    pub fn see_system_scale(&mut self, system: Option<f32>) -> bool {
        const PATIENCE: u32 = 5;
        let system = usable(system);
        if system.is_none() && !self.synced {
            self.frames_without_system += 1;
            if self.frames_without_system < PATIENCE {
                return false;
            }
        }
        let first = !self.synced;
        let changed = system != self.system;
        self.synced = true;
        self.system = system;
        if first || changed {
            self.zoom_request = Some(zoom_for(self.size, system));
        }
        first
    }

    /// The zoom to apply this frame, once.
    pub fn take_zoom_request(&mut self) -> Option<f32> {
        self.zoom_request.take()
    }

    /// Whether the window's zoom has moved off the one the size gives (the user zoomed with
    /// the keyboard), so the new size should be remembered. False until the system's scale
    /// is known, since the window's zoom is a stand-in until then.
    pub fn zoom_differs(&self, zoom: f32) -> bool {
        self.synced && (zoom - zoom_for(self.size, self.system)).abs() > 1e-3
    }

    /// The size on screen a zoom gives with the current system scale.
    pub fn size_of_zoom(&self, zoom: f32) -> f32 {
        size_for(zoom, self.system)
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
