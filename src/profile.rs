use crate::exe::{find_candidates, legacy_hint, pick};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

const META_DIR: &str = ".dlsite-launcher";
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
}

pub fn ensure(game_dir: &Path) -> io::Result<ExeState> {
    if let Some(Profile { exe: Some(exe) }) = read_profile(game_dir)? {
        return Ok(ExeState::Ready { exe });
    }

    if let Some(exe) = legacy_hint(game_dir)? {
        write_profile(
            game_dir,
            &Profile {
                exe: Some(exe.clone()),
            },
        )?;
        return Ok(ExeState::Ready { exe });
    }

    let candidates = find_candidates(game_dir)?;
    match pick(&candidates) {
        Some(exe) => {
            write_profile(
                game_dir,
                &Profile {
                    exe: Some(exe.clone()),
                },
            )?;
            Ok(ExeState::Ready { exe })
        }
        None => {
            write_profile(game_dir, &Profile { exe: None })?;
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
    write_profile(
        game_dir,
        &Profile {
            exe: Some(exe.to_path_buf()),
        },
    )
}
