//! Steam entries for games: the non-Steam shortcuts that start a game through the launcher.
//!
//! Everything here works on a parsed `shortcuts.vdf` (see `vdf`) or on plain paths, so it is
//! testable without Steam. The entries are the launcher's own when their launch options are
//! `--launch "<game folder>"`; nothing else in the file is ever changed.

pub use crate::vdf::Value;
use std::io;
use std::path::{Path, PathBuf};

const LAUNCH_PREFIX: &str = "--launch \"";

/// The standard CRC-32 (IEEE), which Steam uses to make a shortcut's app id.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The app id Steam gives a shortcut made from its quoted program path and its name.
pub fn shortcut_app_id(quoted_exe: &str, name: &str) -> u32 {
    crc32(format!("{quoted_exe}{name}").as_bytes()) | 0x8000_0000
}

/// The launch options of the entry for a game: `--launch "<folder>"`.
pub fn launch_options(game_dir: &Path) -> Result<String, String> {
    let dir = game_dir.to_string_lossy();
    if dir.contains('"') {
        return Err("the game folder has a quote mark in its name".to_owned());
    }
    Ok(format!("{LAUNCH_PREFIX}{dir}\""))
}

/// The game folder in launch options written by `launch_options`, if they are ours.
pub fn game_dir_of(options: &str) -> Option<PathBuf> {
    let dir = options.strip_prefix(LAUNCH_PREFIX)?.strip_suffix('"')?;
    Some(PathBuf::from(dir))
}

/// A shortcuts file with no shortcuts.
pub fn new_shortcuts() -> Value {
    Value::Map(vec![("shortcuts".to_owned(), Value::Map(Vec::new()))])
}

fn list_mut(root: &mut Value) -> &mut Vec<(String, Value)> {
    if root.get("shortcuts").is_none()
        && let Value::Map(items) = root
    {
        items.push(("shortcuts".to_owned(), Value::Map(Vec::new())));
    }
    match root.get_mut("shortcuts") {
        Some(Value::Map(items)) => items,
        _ => unreachable!("the shortcuts map was just made"),
    }
}

fn game_of(entry: &Value) -> Option<PathBuf> {
    game_dir_of(entry.get("LaunchOptions")?.as_str()?)
}

fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn set(entry: &mut Value, key: &str, value: Value) {
    if let Some(slot) = entry.get_mut(key) {
        *slot = value;
    } else if let Value::Map(items) = entry {
        items.push((key.to_owned(), value));
    }
}

fn new_entry(id: u32, name: &str, exe: &str, start: &str, options: &str) -> Value {
    let text = |s: &str| Value::Str(s.to_owned());
    Value::Map(vec![
        ("appid".to_owned(), Value::U32(id)),
        ("AppName".to_owned(), text(name)),
        ("Exe".to_owned(), text(exe)),
        ("StartDir".to_owned(), text(start)),
        ("icon".to_owned(), text("")),
        ("ShortcutPath".to_owned(), text("")),
        ("LaunchOptions".to_owned(), text(options)),
        ("IsHidden".to_owned(), Value::U32(0)),
        ("AllowDesktopConfig".to_owned(), Value::U32(1)),
        ("AllowOverlay".to_owned(), Value::U32(1)),
        ("OpenVR".to_owned(), Value::U32(0)),
        ("Devkit".to_owned(), Value::U32(0)),
        ("DevkitGameID".to_owned(), text("")),
        ("DevkitOverrideAppID".to_owned(), Value::U32(0)),
        ("LastPlayTime".to_owned(), Value::U32(0)),
        ("FlatpakAppID".to_owned(), text("")),
        ("sortas".to_owned(), text("")),
        ("tags".to_owned(), Value::Map(Vec::new())),
    ])
}

/// Adds the entry for a game, or updates it when the game already has one (its app id and
/// its artwork stay). Returns the entry's app id.
pub fn add_game(
    root: &mut Value,
    launcher: &Path,
    game_dir: &Path,
    name: &str,
) -> Result<u32, String> {
    let options = launch_options(game_dir)?;
    let exe = quoted(launcher);
    let start = quoted(launcher.parent().unwrap_or(Path::new("/")));
    let list = list_mut(root);
    if let Some((_, entry)) = list
        .iter_mut()
        .find(|(_, e)| game_of(e).as_deref() == Some(game_dir))
    {
        set(entry, "AppName", Value::Str(name.to_owned()));
        set(entry, "Exe", Value::Str(exe));
        set(entry, "StartDir", Value::Str(start));
        return match entry.get("appid") {
            Some(Value::U32(id)) => Ok(*id),
            _ => Err("an entry of ours has no app id".to_owned()),
        };
    }
    let id = shortcut_app_id(&exe, name);
    let next = list
        .iter()
        .filter_map(|(k, _)| k.parse::<usize>().ok())
        .max()
        .map_or(0, |n| n + 1);
    list.push((
        next.to_string(),
        new_entry(id, name, &exe, &start, &options),
    ));
    Ok(id)
}

/// A shortcut that starts the launcher itself: no launch options, and a program named like
/// the launcher, wherever it was when the shortcut was made.
fn is_launcher_entry(entry: &Value) -> bool {
    entry.get("LaunchOptions").and_then(Value::as_str) == Some("")
        && entry
            .get("Exe")
            .and_then(Value::as_str)
            .is_some_and(|exe| exe.trim_matches('"').rsplit('/').next() == Some("dackel-launcher"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherState {
    /// Steam has no shortcut for the launcher.
    Absent,
    /// It has one, and it points at this launcher.
    Current,
    /// It has one, but the launcher has moved since.
    Outdated,
}

pub fn launcher_state(root: &Value, launcher: &Path) -> LauncherState {
    let exe = quoted(launcher);
    let Some(list) = root.get("shortcuts") else {
        return LauncherState::Absent;
    };
    let ours: Vec<_> = list
        .items()
        .iter()
        .map(|(_, e)| e)
        .filter(|e| is_launcher_entry(e))
        .collect();
    if ours.is_empty() {
        LauncherState::Absent
    } else if ours
        .iter()
        .all(|e| e.get("Exe").and_then(Value::as_str) == Some(&exe))
    {
        LauncherState::Current
    } else {
        LauncherState::Outdated
    }
}

/// Adds a shortcut that starts the launcher, unless one exists (even one the user made, or
/// one for an older place of the launcher). Returns its app id and whether it was created.
pub fn add_launcher(root: &mut Value, launcher: &Path) -> (u32, bool) {
    let exe = quoted(launcher);
    let list = list_mut(root);
    if let Some(id) = list
        .iter()
        .find(|(_, e)| is_launcher_entry(e))
        .and_then(|(_, e)| match e.get("appid") {
            Some(Value::U32(id)) => Some(*id),
            _ => None,
        })
    {
        return (id, false);
    }
    let name = "Dackel Launcher";
    let id = shortcut_app_id(&exe, name);
    let next = list
        .iter()
        .filter_map(|(k, _)| k.parse::<usize>().ok())
        .max()
        .map_or(0, |n| n + 1);
    let start = quoted(launcher.parent().unwrap_or(Path::new("/")));
    list.push((next.to_string(), new_entry(id, name, &exe, &start, "")));
    (id, true)
}

/// The app ids of the shortcuts for the launcher.
pub fn launcher_app_ids(root: &Value) -> Vec<u32> {
    root.get("shortcuts")
        .map(|list| {
            list.items()
                .iter()
                .filter(|(_, e)| is_launcher_entry(e))
                .filter_map(|(_, e)| match e.get("appid") {
                    Some(Value::U32(id)) => Some(*id),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Sets the icon of every shortcut for the launcher. Returns how many changed.
pub fn set_launcher_icon(root: &mut Value, icon: &Path) -> usize {
    let icon = icon.to_string_lossy().into_owned();
    let mut changed = 0;
    for (_, entry) in list_mut(root).iter_mut() {
        if !is_launcher_entry(entry) || entry.get("icon").and_then(Value::as_str) == Some(&icon) {
            continue;
        }
        set(entry, "icon", Value::Str(icon.clone()));
        changed += 1;
    }
    changed
}

/// Points every shortcut for the launcher at its current place. Returns how many changed.
pub fn update_launcher(root: &mut Value, launcher: &Path) -> usize {
    let exe = quoted(launcher);
    let start = quoted(launcher.parent().unwrap_or(Path::new("/")));
    let mut changed = 0;
    for (_, entry) in list_mut(root).iter_mut() {
        if !is_launcher_entry(entry) || entry.get("Exe").and_then(Value::as_str) == Some(&exe) {
            continue;
        }
        set(entry, "Exe", Value::Str(exe.clone()));
        set(entry, "StartDir", Value::Str(start.clone()));
        changed += 1;
    }
    changed
}

/// Takes out the shortcuts for the launcher and numbers the rest again. False when none.
pub fn remove_launcher(root: &mut Value) -> bool {
    let list = list_mut(root);
    let before = list.len();
    list.retain(|(_, e)| !is_launcher_entry(e));
    for (i, (key, _)) in list.iter_mut().enumerate() {
        *key = i.to_string();
    }
    list.len() != before
}

/// Sets the icon of a game's entry. False when the game has no entry or it already has it.
pub fn set_game_icon(root: &mut Value, game_dir: &Path, icon: &Path) -> bool {
    let icon = icon.to_string_lossy().into_owned();
    for (_, entry) in list_mut(root).iter_mut() {
        if game_of(entry).as_deref() != Some(game_dir) {
            continue;
        }
        if entry.get("icon").and_then(Value::as_str) == Some(&icon) {
            return false;
        }
        set(entry, "icon", Value::Str(icon));
        return true;
    }
    false
}

/// The app id of a game's entry, if it has one.
pub fn app_id_of(root: &Value, game_dir: &Path) -> Option<u32> {
    root.get("shortcuts")?
        .items()
        .iter()
        .find(|(_, e)| game_of(e).as_deref() == Some(game_dir))
        .and_then(|(_, e)| match e.get("appid") {
            Some(Value::U32(id)) => Some(*id),
            _ => None,
        })
}

/// Takes out the game's entry and numbers the rest again, as Steam does. False when the
/// game had none.
pub fn remove_game(root: &mut Value, game_dir: &Path) -> bool {
    let list = list_mut(root);
    let before = list.len();
    list.retain(|(_, e)| game_of(e).as_deref() != Some(game_dir));
    for (i, (key, _)) in list.iter_mut().enumerate() {
        *key = i.to_string();
    }
    list.len() != before
}

/// Writes the launcher's current path into every entry of ours. Returns how many changed.
pub fn refresh_launcher(root: &mut Value, launcher: &Path) -> usize {
    let exe = quoted(launcher);
    let start = quoted(launcher.parent().unwrap_or(Path::new("/")));
    let mut changed = 0;
    for (_, entry) in list_mut(root).iter_mut() {
        if game_of(entry).is_none() || entry.get("Exe").and_then(Value::as_str) == Some(&exe) {
            continue;
        }
        set(entry, "Exe", Value::Str(exe.clone()));
        set(entry, "StartDir", Value::Str(start.clone()));
        changed += 1;
    }
    changed
}

/// The account whose file changed last (`mtime` in seconds); ties go to the lower name.
pub fn account_with_newest(accounts: &[(String, u64)]) -> Option<String> {
    accounts
        .iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(name, _)| name.clone())
}

/// Steam's folder under `home`, in the usual places, if there is one.
pub fn steam_root(home: &Path) -> Option<PathBuf> {
    [
        ".local/share/Steam",
        ".steam/steam",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
    ]
    .iter()
    .map(|rel| home.join(rel))
    .find(|root| root.join("userdata").is_dir())
}

/// Whether a process named `steam` exists, looking in `proc` (`/proc`).
pub fn steam_running(proc: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(proc) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
            && std::fs::read_to_string(entry.path().join("comm")).is_ok_and(|c| c.trim() == "steam")
    })
}

/// The file names Steam reads for a shortcut's wide and portrait images.
pub fn grid_names(app_id: u32) -> [String; 2] {
    [format!("{app_id}.png"), format!("{app_id}p.png")]
}

/// Copies a cover into Steam's grid folder as the entry's images, converted to PNG.
pub fn write_grid(cover: &Path, grid_dir: &Path, app_id: u32) -> io::Result<()> {
    let image = image::open(cover).map_err(io::Error::other)?;
    std::fs::create_dir_all(grid_dir)?;
    for name in grid_names(app_id) {
        image.save(grid_dir.join(name)).map_err(io::Error::other)?;
    }
    Ok(())
}

/// The accounts under Steam's `userdata`, each with the time (seconds) its shortcuts or
/// local settings file last changed. Account 0 is Steam's placeholder and is skipped.
pub fn accounts(steam_root: &Path) -> Vec<(String, u64)> {
    let Ok(entries) = std::fs::read_dir(steam_root.join("userdata")) else {
        return Vec::new();
    };
    let secs = |path: PathBuf| {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs())
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let numbered = !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit());
            if !numbered || name == "0" || !entry.path().is_dir() {
                return None;
            }
            let config = entry.path().join("config");
            let newest =
                secs(config.join("shortcuts.vdf")).max(secs(config.join("localconfig.vdf")));
            Some((name, newest))
        })
        .collect()
}

pub fn shortcuts_path(steam_root: &Path, account: &str) -> PathBuf {
    steam_root
        .join("userdata")
        .join(account)
        .join("config")
        .join("shortcuts.vdf")
}

/// Steam's grid folder for an account.
pub fn grid_dir(steam_root: &Path, account: &str) -> PathBuf {
    steam_root
        .join("userdata")
        .join(account)
        .join("config")
        .join("grid")
}

/// Reads a shortcuts file; a file that does not exist is a file with no shortcuts. A file
/// that cannot be read exactly is an error, so it is never replaced by a guess.
pub fn load_shortcuts(path: &Path) -> Result<Value, String> {
    match std::fs::read(path) {
        Ok(bytes) => crate::vdf::parse(&bytes).map_err(|e| format!("{}: {e}", path.display())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(new_shortcuts()),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

/// Writes a shortcuts file. The first time, the file as it was is kept next to it as
/// `shortcuts.vdf.dackel-backup`.
pub fn save_shortcuts(path: &Path, root: &Value) -> io::Result<()> {
    let backup = path.with_extension("vdf.dackel-backup");
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup)?;
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, crate::vdf::write(root))
}

/// The launcher's artwork, drawn by `scripts/make-steam-art.sh`, as Steam names each image.
fn launcher_art(app_id: u32) -> [(String, &'static [u8]); 5] {
    [
        (
            format!("{app_id}p.png"),
            include_bytes!("../assets/steam/portrait.png"),
        ),
        (
            format!("{app_id}.png"),
            include_bytes!("../assets/steam/wide.png"),
        ),
        (
            format!("{app_id}_hero.png"),
            include_bytes!("../assets/steam/hero.png"),
        ),
        (
            format!("{app_id}_logo.png"),
            include_bytes!("../assets/steam/logo.png"),
        ),
        (
            format!("{app_id}_icon.png"),
            include_bytes!("../assets/steam/icon.png"),
        ),
    ]
}

/// The file name of the launcher's icon for an app id (the shortcut's `icon` points at it).
pub fn launcher_icon_name(app_id: u32) -> String {
    format!("{app_id}_icon.png")
}

/// Writes the launcher's whole artwork set into Steam's grid folder.
pub fn write_launcher_art(grid_dir: &Path, app_id: u32) -> io::Result<()> {
    std::fs::create_dir_all(grid_dir)?;
    for (name, bytes) in launcher_art(app_id) {
        std::fs::write(grid_dir.join(name), bytes)?;
    }
    Ok(())
}
