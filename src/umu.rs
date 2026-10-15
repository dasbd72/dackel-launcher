//! Finding `umu-run`. A read-only system such as SteamOS cannot install it, so the launcher
//! also looks beside its own binary, where the drive can carry umu's standalone build.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const NAME: &str = "umu-run";

/// The first usable `umu-run`: an explicit path (`$UMU_RUN`), the program's own folder,
/// the `PATH`, then `~/.local/bin`. Only executable files count.
pub fn locate(
    explicit: Option<&OsStr>,
    program_dir: Option<&Path>,
    path_var: Option<&OsString>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let explicit = explicit
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .filter(|p| is_executable(p));
    let beside = program_dir.map(|d| d.join(NAME));
    let on_path = path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|d| d.join(NAME));
    let local = home.map(|h| h.join(".local/bin").join(NAME));
    explicit.or_else(|| {
        beside
            .into_iter()
            .chain(on_path)
            .chain(local)
            .find(|p| is_executable(p))
    })
}

/// `locate` against this process's real environment.
pub fn locate_default() -> Option<PathBuf> {
    let program_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let home = std::env::var_os("HOME").map(PathBuf::from);
    locate(
        std::env::var_os("UMU_RUN").as_deref(),
        program_dir.as_deref(),
        std::env::var_os("PATH").as_ref(),
        home.as_deref(),
    )
}

/// What to show when no `umu-run` was found.
pub fn missing_message() -> String {
    "umu-run not found. Put it beside dackel-launcher, in ~/.local/bin, or set UMU_RUN.".to_owned()
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
