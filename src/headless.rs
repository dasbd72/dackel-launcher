//! `--launch <game folder>`: start one game with no window, as a Steam entry does, and
//! keep running until the game ends so its play time is counted.

use crate::launch::{Launcher, Spawner};
use crate::profile::{self, ExeState};
use crate::proton::ProtonChoice;
use std::path::{Path, PathBuf};

/// The Proton to launch with. Nobody is there to ask about a missing version, so the newest
/// install stands in for it.
pub fn proton_for(choice: ProtonChoice) -> Option<PathBuf> {
    match choice {
        ProtonChoice::Use(path) => path,
        ProtonChoice::Mismatch { newest, .. } => newest,
    }
}

/// Starts the game in `game_dir`. A game that has not had its exe chosen cannot be started
/// without a window to ask in.
pub fn run<S: Spawner>(
    launcher: &mut Launcher<S>,
    game_dir: &Path,
    proton: Option<&Path>,
    now: u64,
) -> Result<(), String> {
    if !game_dir.is_dir() {
        return Err(format!("{} is not a folder", game_dir.display()));
    }
    let state = profile::ensure(game_dir).map_err(|e| e.to_string())?;
    if matches!(state, ExeState::NeedsExeChoice { .. }) {
        return Err(
            "this game has several programs and none is chosen: open the launcher and choose one"
                .to_owned(),
        );
    }
    launcher
        .start(game_dir, &state, proton, now)
        .map_err(|e| e.to_string())
}

/// Polls until no game is running. `pause` sleeps between polls; `now` is unix seconds.
pub fn wait<S: Spawner>(
    launcher: &mut Launcher<S>,
    mut pause: impl FnMut(),
    mut now: impl FnMut() -> u64,
) {
    while launcher.is_running_any() {
        pause();
        let _ = launcher.poll(now());
    }
}
