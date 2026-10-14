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
    let config = config_home.join("dlsite-launcher").join("proton-path");
    let env = std::env::var_os("PROTONPATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let compat = home.join(".local/share/Steam/compatibilitytools.d");
    resolve(&config, env.as_deref(), &compat)
}
