use crate::exe::{find_candidates, legacy_hint, pick};
use crate::library::META_DIR;
use crate::steam_art::IconSource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const PROFILE_FILE: &str = "profile.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExeState {
    Ready { exe: PathBuf },
    NeedsExeChoice { candidates: Vec<PathBuf> },
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Profile {
    /// Path of the chosen exe relative to the game folder; `None` while pending a choice.
    exe: Option<PathBuf>,
    /// Total time spent running the game, in seconds.
    #[serde(default)]
    play_time_secs: u64,
    /// Unix time (seconds) of the most recent launch.
    #[serde(default)]
    last_played: Option<u64>,
    #[serde(default)]
    favorite: bool,
    /// Extra environment variables for the game's launch.
    #[serde(default)]
    env: BTreeMap<String, String>,
    /// Extra command-line arguments, as the user typed them.
    #[serde(default)]
    args: String,
    /// Where the game's Steam icon comes from.
    #[serde(default)]
    steam_icon: IconSource,
    /// Name of the Proton version chosen for the game (e.g. `GE-Proton9-27`); none means automatic.
    #[serde(default)]
    proton: Option<String>,
    /// Winetricks verbs installed into the game's prefix, in the order they succeeded.
    #[serde(default)]
    winetricks: Vec<String>,
}

/// Per-game launch tuning kept in the profile.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tuning {
    pub env: BTreeMap<String, String>,
    pub args: String,
    /// The Proton version chosen for the game; `None` follows the newest install.
    pub proton: Option<String>,
}

pub fn read_tuning(game_dir: &Path) -> io::Result<Tuning> {
    let p = read_profile(game_dir)?.unwrap_or_default();
    Ok(Tuning {
        env: p.env,
        args: p.args,
        proton: p.proton,
    })
}

/// Saves the env map and argument line, keeping the rest of the profile (and the pin).
pub fn set_tuning(game_dir: &Path, env: BTreeMap<String, String>, args: &str) -> io::Result<()> {
    update_profile(game_dir, |p| {
        p.env = env;
        p.args = args.to_owned();
    })
}

pub fn read_steam_icon(game_dir: &Path) -> io::Result<IconSource> {
    Ok(read_profile(game_dir)?.unwrap_or_default().steam_icon)
}

pub fn set_steam_icon(game_dir: &Path, icon: IconSource) -> io::Result<()> {
    update_profile(game_dir, |p| p.steam_icon = icon)
}

/// Pins (or, with `None`, unpins) the Proton version for this game.
pub fn set_proton_pin(game_dir: &Path, version: Option<&str>) -> io::Result<()> {
    update_profile(game_dir, |p| p.proton = version.map(str::to_owned))
}

/// The winetricks verbs installed into this game's prefix so far.
pub fn read_winetricks(game_dir: &Path) -> io::Result<Vec<String>> {
    Ok(read_profile(game_dir)?.unwrap_or_default().winetricks)
}

/// Records verbs that installed; one already listed keeps its place.
pub fn add_winetricks(game_dir: &Path, verbs: &[String]) -> io::Result<()> {
    update_profile(game_dir, |p| {
        for verb in verbs {
            if !p.winetricks.contains(verb) {
                p.winetricks.push(verb.clone());
            }
        }
    })
}

/// Forgets the installed verbs, as the prefix they went into is gone. A game with nothing
/// recorded is left alone (no profile is created).
pub fn clear_winetricks(game_dir: &Path) -> io::Result<()> {
    match read_profile(game_dir)? {
        Some(p) if !p.winetricks.is_empty() => update_profile(game_dir, |p| p.winetricks.clear()),
        _ => Ok(()),
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayStats {
    pub play_time_secs: u64,
    pub last_played: Option<u64>,
}

pub fn play_stats(game_dir: &Path) -> io::Result<PlayStats> {
    Ok(read_profile(game_dir)?
        .map(|p| PlayStats {
            play_time_secs: p.play_time_secs,
            last_played: p.last_played,
        })
        .unwrap_or_default())
}

/// Records a launch at `started` (unix seconds).
pub fn record_launch(game_dir: &Path, started: u64) -> io::Result<()> {
    update_profile(game_dir, |p| p.last_played = Some(started))
}

/// Adds `secs` to the game's total play time.
pub fn add_play_time(game_dir: &Path, secs: u64) -> io::Result<()> {
    update_profile(game_dir, |p| {
        p.play_time_secs = p.play_time_secs.saturating_add(secs)
    })
}

/// User-facing state kept in the profile besides the exe choice.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub favorite: bool,
    /// Unix seconds of the last launch.
    pub last_played: Option<u64>,
    /// Total play time in seconds; `None` until the game has been played.
    pub play_time_secs: Option<u64>,
}

/// Reads the profile's stats; a game without a profile has the defaults.
pub fn read_stats(game_dir: &Path) -> io::Result<Stats> {
    let profile = read_profile(game_dir)?.unwrap_or_default();
    Ok(Stats {
        favorite: profile.favorite,
        last_played: profile.last_played,
        play_time_secs: (profile.play_time_secs > 0).then_some(profile.play_time_secs),
    })
}

/// Marks or unmarks a game as a favorite, keeping every other profile field.
pub fn set_favorite(game_dir: &Path, favorite: bool) -> io::Result<()> {
    update_profile(game_dir, |p| p.favorite = favorite)
}

fn update_profile(game_dir: &Path, change: impl FnOnce(&mut Profile)) -> io::Result<()> {
    let mut profile = read_profile(game_dir)?.unwrap_or_default();
    change(&mut profile);
    write_profile(game_dir, &profile)
}

pub fn ensure(game_dir: &Path) -> io::Result<ExeState> {
    if let Some(Profile { exe: Some(exe), .. }) = read_profile(game_dir)? {
        return Ok(ExeState::Ready { exe });
    }

    if let Some(exe) = legacy_hint(game_dir)? {
        write_exe(game_dir, Some(exe.clone()))?;
        return Ok(ExeState::Ready { exe });
    }

    let candidates = find_candidates(game_dir)?;
    match pick(&candidates) {
        Some(exe) => {
            write_exe(game_dir, Some(exe.clone()))?;
            Ok(ExeState::Ready { exe })
        }
        None => {
            write_exe(game_dir, None)?;
            Ok(ExeState::NeedsExeChoice {
                candidates: candidates.into_iter().map(|c| c.rel).collect(),
            })
        }
    }
}

fn read_profile(game_dir: &Path) -> io::Result<Option<Profile>> {
    let path = game_dir.join(META_DIR).join(PROFILE_FILE);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Sets the exe, keeping every other profile field (favorite, play time, ...).
fn write_exe(game_dir: &Path, exe: Option<PathBuf>) -> io::Result<()> {
    update_profile(game_dir, |p| p.exe = exe)
}

fn write_profile(game_dir: &Path, profile: &Profile) -> io::Result<()> {
    let dir = game_dir.join(META_DIR);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(profile).map_err(io::Error::other)?;
    std::fs::write(dir.join(PROFILE_FILE), json)
}

/// Records the user's exe choice. `exe` must be one of the detected candidates.
pub fn choose(game_dir: &Path, exe: &Path) -> io::Result<()> {
    let is_candidate = find_candidates(game_dir)?.iter().any(|c| c.rel == exe);
    if !is_candidate {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a detected exe", exe.display()),
        ));
    }
    write_exe(game_dir, Some(exe.to_path_buf()))
}

/// Every exe a user may pick for this game, relative to the game folder.
pub fn candidates(game_dir: &Path) -> io::Result<Vec<PathBuf>> {
    Ok(find_candidates(game_dir)?
        .into_iter()
        .map(|c| c.rel)
        .collect())
}
