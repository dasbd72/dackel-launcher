//! Defaults every game's Wine prefix gets before its first launch, in the spirit of what Lutris
//! sets up: no desktop menu entries and no crash dialogs.
//!
//! The defaults are a `.reg` file imported once per prefix; a marker inside the prefix records
//! that, so Reset prefix brings them back with the new prefix.

use crate::launch::LaunchPlan;
use crate::library::META_DIR;
use crate::tuning::prefix_dir;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Bump when `REGISTRY_DEFAULTS` changes so existing prefixes get the new version too.
const VERSION: u32 = 1;

const REGISTRY_DEFAULTS: &str = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\Wine\DllOverrides]
"winemenubuilder.exe"=""

[HKEY_CURRENT_USER\Software\Wine\WineDbg]
"ShowCrashDialog"=dword:00000000
"#;

/// The file inside the prefix whose presence means the defaults were applied.
pub fn marker_path(game_dir: &Path) -> PathBuf {
    prefix_dir(game_dir).join(format!("dackel-defaults-{VERSION}"))
}

/// The `.reg` file the defaults are imported from.
pub fn registry_file(game_dir: &Path) -> PathBuf {
    game_dir.join(META_DIR).join("prefix-defaults.reg")
}

/// The registry text imported into every prefix.
pub fn registry_defaults() -> &'static str {
    REGISTRY_DEFAULTS
}

pub fn is_prepared(game_dir: &Path) -> bool {
    marker_path(game_dir).exists()
}

/// A path as Wine sees it: the `Z:` drive is the whole filesystem.
fn dos_path(path: &Path) -> String {
    format!("Z:{}", path.display().to_string().replace('/', "\\"))
}

/// If the game's prefix has not had the defaults applied, writes the `.reg` file and gives the
/// plan the setup step to run before the game (same program and environment).
pub fn prepare(game_dir: &Path, plan: &mut LaunchPlan) -> io::Result<()> {
    if is_prepared(game_dir) {
        return Ok(());
    }
    let reg = registry_file(game_dir);
    if let Some(dir) = reg.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&reg, REGISTRY_DEFAULTS)?;
    plan.setup = Some(vec![
        OsString::from("regedit"),
        OsString::from("/S"),
        OsString::from(dos_path(&reg)),
    ]);
    plan.marker = Some(marker_path(game_dir));
    Ok(())
}
