//! `--launch <game folder>`: start one game with no window and wait for it to end.

use dackel_launcher::headless::{proton_for, run, wait};
use dackel_launcher::launch::{LaunchPlan, Launcher, Process, Spawner};
use dackel_launcher::profile::read_stats;
use dackel_launcher::proton::ProtonChoice;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// A game that runs for `lives` polls.
#[derive(Clone)]
struct Fake {
    plans: Arc<Mutex<Vec<LaunchPlan>>>,
    lives: u32,
}

struct Ticking(Arc<AtomicU32>);

impl Process for Ticking {
    fn has_exited(&mut self) -> io::Result<bool> {
        Ok(self.0.fetch_sub(1, Ordering::SeqCst) == 0)
    }
}

impl Spawner for Fake {
    fn spawn(&self, plan: &LaunchPlan) -> io::Result<Box<dyn Process>> {
        self.plans.lock().unwrap().push(plan.clone());
        Ok(Box::new(Ticking(Arc::new(AtomicU32::new(self.lives)))))
    }
}

fn fake(lives: u32) -> Fake {
    Fake {
        plans: Arc::default(),
        lives,
    }
}

fn game_with_one_exe() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Game.exe"), b"").unwrap();
    dir
}

#[test]
fn a_ready_game_is_started_and_its_launch_recorded() {
    let game = game_with_one_exe();
    let spawner = fake(0);
    let mut launcher = Launcher::new(spawner.clone());

    run(&mut launcher, game.path(), None, 1000).unwrap();

    assert_eq!(spawner.plans.lock().unwrap().len(), 1);
    assert_eq!(read_stats(game.path()).unwrap().last_played, Some(1000));
}

#[test]
fn a_game_with_several_exes_and_no_choice_says_to_choose_one() {
    let game = tempfile::tempdir().unwrap();
    fs::write(game.path().join("A.exe"), b"same size").unwrap();
    fs::write(game.path().join("B.exe"), b"same size").unwrap();
    let spawner = fake(0);
    let mut launcher = Launcher::new(spawner.clone());

    let err = run(&mut launcher, game.path(), None, 1).unwrap_err();

    assert!(err.contains("choose"), "{err}");
    assert!(spawner.plans.lock().unwrap().is_empty());
}

#[test]
fn a_folder_that_is_not_there_is_an_error() {
    let mut launcher = Launcher::new(fake(0));

    assert!(run(&mut launcher, Path::new("/no/such/game"), None, 1).is_err());
}

#[test]
fn waiting_ends_when_the_game_ends_and_counts_the_play_time() {
    let game = game_with_one_exe();
    let mut launcher = Launcher::new(fake(2));
    run(&mut launcher, game.path(), None, 1000).unwrap();
    let clock = std::cell::Cell::new(1000);

    wait(
        &mut launcher,
        || clock.set(clock.get() + 10),
        || clock.get(),
    );

    assert!(!launcher.is_running_any());
    assert!(read_stats(game.path()).unwrap().play_time_secs.unwrap_or(0) > 0);
}

#[test]
fn a_missing_pinned_version_falls_back_to_the_newest() {
    let newest = Some(PathBuf::from("/compat/GE-Proton10-1"));

    assert_eq!(
        proton_for(ProtonChoice::Mismatch {
            pinned: "GE-Proton8-1".into(),
            newest: newest.clone()
        }),
        newest
    );
    assert_eq!(
        proton_for(ProtonChoice::Use(Some(PathBuf::from("/p")))),
        Some(PathBuf::from("/p"))
    );
    assert_eq!(proton_for(ProtonChoice::Use(None)), None);
}
