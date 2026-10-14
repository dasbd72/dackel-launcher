//! Per-machine settings. They live in the user's config directory, never on the
//! portable drive, because the desktop and the Deck want different values.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "dlsite-launcher";
const FILE: &str = "settings.json";
const MIN_SCALE: f32 = 0.5;
const MAX_SCALE: f32 = 3.0;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// UI zoom; `None` means "use the default for this kind of machine".
    pub ui_scale: Option<f32>,
    /// Last fullscreen choice; `None` means "decide from the environment".
    pub fullscreen: Option<bool>,
}

/// Loads settings; a missing or unreadable file gives the defaults, so a bad
/// file can never stop the launcher from starting.
pub fn load(path: &Path) -> Settings {
    let mut settings: Settings = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    settings.ui_scale = settings
        .ui_scale
        .filter(|s| s.is_finite())
        .map(|s| s.clamp(MIN_SCALE, MAX_SCALE));
    settings
}

pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(io::Error::other)?;
    std::fs::write(path, json)
}

/// `$XDG_CONFIG_HOME/dlsite-launcher/settings.json`, else `~/.config/...`.
pub fn settings_path(xdg_config_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|p| !p.as_os_str().is_empty()) {
        Some(xdg) => xdg.to_path_buf(),
        None => home.filter(|p| !p.as_os_str().is_empty())?.join(".config"),
    };
    Some(base.join(APP_DIR).join(FILE))
}
