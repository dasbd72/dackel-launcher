use std::io;
use std::path::{Path, PathBuf};

const GE_PREFIX: &str = "GE-Proton";

/// Picks the GE-Proton directory for a launch, in priority order: the
/// per-machine override file, `$PROTONPATH`, the newest installed
/// `GE-Proton*` under `compat_dir`. `None` leaves the choice to umu.
pub fn resolve(
    config_file: &Path,
    env_protonpath: Option<&Path>,
    compat_dir: &Path,
) -> io::Result<Option<PathBuf>> {
    if let Some(path) = read_override(config_file)? {
        return Ok(Some(path));
    }
    if let Some(path) = env_protonpath {
        return Ok(Some(path.to_path_buf()));
    }
    newest_installed(compat_dir)
}

fn read_override(config_file: &Path) -> io::Result<Option<PathBuf>> {
    match std::fs::read_to_string(config_file) {
        Ok(text) => {
            let line = text.trim();
            Ok((!line.is_empty()).then(|| PathBuf::from(line)))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn newest_installed(compat_dir: &Path) -> io::Result<Option<PathBuf>> {
    let entries = match std::fs::read_dir(compat_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut best: Option<(Vec<u64>, PathBuf)> = None;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(GE_PREFIX) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let key = version_key(rest);
        if best.as_ref().is_none_or(|(k, _)| key > *k) {
            best = Some((key, entry.path()));
        }
    }
    Ok(best.map(|(_, path)| path))
}

/// Numeric fields of a version suffix, so `10-1` sorts after `9-27`.
fn version_key(suffix: &str) -> Vec<u64> {
    suffix
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}

/// Resolves the Proton for this machine from the real config dir, `$PROTONPATH`
/// and `~/.local/share/Steam/compatibilitytools.d`.
pub fn resolve_default() -> io::Result<Option<PathBuf>> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let config = config_home
        .join(crate::settings::APP_DIR)
        .join("proton-path");
    let env = std::env::var_os("PROTONPATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let compat = home.join(".local/share/Steam/compatibilitytools.d");
    resolve(&config, env.as_deref(), &compat)
}

/// What to do about Proton for one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtonChoice {
    /// Launch with this (`None`: let umu choose).
    Use(Option<PathBuf>),
    /// The game was pinned to a version this machine does not have.
    Mismatch {
        pinned: String,
        /// The newest version installed here, offered as "use newest anyway".
        newest: Option<PathBuf>,
    },
}

/// Like `resolve`, but honoring the game's pinned version, whether chosen or pinned on the
/// first launch. An installed pin wins over the per-machine choice (override file or
/// `$PROTONPATH`), which applies when there is no pin or it is not installed; a missing pin
/// with no machine choice is reported as a mismatch.
pub fn resolve_for_game(
    config_file: &Path,
    env_protonpath: Option<&Path>,
    compat_dir: &Path,
    pinned: Option<&str>,
) -> io::Result<ProtonChoice> {
    // A pin comes from a profile that may have been shared: only a plain folder name counts.
    let installed_pin = pinned.filter(|pin| is_plain_name(pin)).and_then(|pin| {
        let wanted = compat_dir.join(pin);
        wanted.is_dir().then_some(wanted)
    });
    if let Some(path) = installed_pin {
        return Ok(ProtonChoice::Use(Some(path)));
    }
    if let Some(path) = read_override(config_file)? {
        return Ok(ProtonChoice::Use(Some(path)));
    }
    if let Some(path) = env_protonpath {
        return Ok(ProtonChoice::Use(Some(path.to_path_buf())));
    }
    let newest = newest_installed(compat_dir)?;
    Ok(match pinned {
        Some(pinned) => ProtonChoice::Mismatch {
            pinned: pinned.to_owned(),
            newest,
        },
        None => ProtonChoice::Use(newest),
    })
}

fn is_plain_name(name: &str) -> bool {
    matches!(
        Path::new(name).components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    )
}

/// Every Proton the user can pick for a game: the folders under `compat_dir` that hold a
/// `proton` script, newest first. Names only, as the pin stores them.
pub fn selectable(compat_dir: &Path) -> io::Result<Vec<String>> {
    let entries = match std::fs::read_dir(compat_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.path().join("proton").is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            found.push((version_key(&name), name));
        }
    }
    found.sort_by(|a, b| b.cmp(a));
    Ok(found.into_iter().map(|(_, name)| name).collect())
}

/// Installed `GE-Proton*` versions under `compat_dir`, newest first.
pub fn installed(compat_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(compat_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix(GE_PREFIX)
            && entry.file_type()?.is_dir()
        {
            found.push((version_key(rest), entry.path()));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(found.into_iter().map(|(_, path)| path).collect())
}

/// The Steam compatibility-tools folder where GE-Proton versions are installed.
pub fn compat_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".local/share/Steam/compatibilitytools.d")
}

/// `resolve_for_game` against this machine's real config, `$PROTONPATH` and compat dir.
pub fn resolve_for_game_default(pinned: Option<&str>) -> io::Result<ProtonChoice> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let config = config_home
        .join(crate::settings::APP_DIR)
        .join("proton-path");
    let env = std::env::var_os("PROTONPATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    resolve_for_game(&config, env.as_deref(), &compat_dir(), pinned)
}
