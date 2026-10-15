//! Spotting Steam's desktop layout: it turns the sticks into arrow keys, so the launcher
//! gets keyboard arrows and never a gamepad event. Steam sets `SteamGameId` for programs it
//! started itself, which is when the Gamepad layout should have applied.

/// What to tell the user.
pub const HINT: &str = "Steam Input looks like it is on the desktop layout. \
Start the launcher from its Steam shortcut and give the shortcut the Gamepad layout.";

const ARROWS_BEFORE_HINT: u32 = 3;

pub struct Watch {
    from_steam: bool,
    pad_seen: bool,
    arrows: u32,
    shown: bool,
}

impl Watch {
    pub fn new(from_steam: bool) -> Self {
        Watch {
            from_steam,
            pad_seen: false,
            arrows: 0,
            shown: false,
        }
    }

    /// Whether this process was started by Steam, from the real environment.
    pub fn from_env() -> Self {
        Self::new(std::env::var_os("SteamGameId").is_some())
    }

    /// A gamepad event arrived, so the layout passes the pad through.
    pub fn pad_seen(&mut self) {
        self.pad_seen = true;
    }

    /// An arrow key was pressed; returns the hint the first time it looks like Steam's
    /// desktop layout.
    pub fn arrow_pressed(&mut self) -> Option<&'static str> {
        if !self.from_steam || self.pad_seen || self.shown {
            return None;
        }
        self.arrows += 1;
        if self.arrows < ARROWS_BEFORE_HINT {
            return None;
        }
        self.shown = true;
        Some(HINT)
    }
}
