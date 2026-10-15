use dackel_launcher::launch::plan;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[test]
fn plan_runs_umu_on_the_exe_from_its_own_folder_with_the_game_prefix() {
    let game = Path::new("/library/RJ123456");

    let p = plan(game, Path::new("bin/Game.exe"), None);

    assert_eq!(p.program, "umu-run");
    assert_eq!(
        p.args,
        vec![OsString::from("/library/RJ123456/bin/Game.exe")]
    );
    assert_eq!(p.working_dir, PathBuf::from("/library/RJ123456/bin"));
    assert_eq!(
        p.env.get("WINEPREFIX").map(String::as_str),
        Some("/library/RJ123456/.dlsite-launcher/wine")
    );
    assert_eq!(p.env.get("LANG").map(String::as_str), Some("ja_JP.UTF-8"));
    assert_eq!(p.env.get("LC_ALL").map(String::as_str), Some("ja_JP.UTF-8"));
}

#[test]
fn gameid_is_umu_plus_the_rj_code() {
    let game = Path::new("/library/RJ123456");

    let p = plan(game, Path::new("bin/My-Game_v2.exe"), None);

    assert_eq!(
        p.env.get("GAMEID").map(String::as_str),
        Some("umu-RJ123456")
    );
}

#[test]
fn gameid_ignores_the_exe_name_so_games_sharing_an_exe_do_not_collide() {
    let a = plan(Path::new("/library/RJ111111"), Path::new("Game.exe"), None);
    let b = plan(Path::new("/library/RJ222222"), Path::new("Game.exe"), None);
    let garbled = plan(
        Path::new("/library/RJ297317"),
        Path::new("\u{e9}\u{f3}\u{e9}\u{3b1}.exe"),
        None,
    );

    assert_ne!(a.env.get("GAMEID"), b.env.get("GAMEID"));
    assert_eq!(
        garbled.env.get("GAMEID").map(String::as_str),
        Some("umu-RJ297317")
    );
}

#[test]
fn proton_path_is_set_only_when_a_proton_is_resolved() {
    let game = Path::new("/library/RJ123456");
    let proton = Path::new("/home/u/.local/share/Steam/compatibilitytools.d/GE-Proton10-1");

    let pinned = plan(game, Path::new("Game.exe"), Some(proton));
    let umu_default = plan(game, Path::new("Game.exe"), None);

    assert_eq!(
        pinned.env.get("PROTONPATH").map(String::as_str),
        Some("/home/u/.local/share/Steam/compatibilitytools.d/GE-Proton10-1")
    );
    assert!(!umu_default.env.contains_key("PROTONPATH"));
}

use dackel_launcher::launch::{LaunchError, LaunchPlan, Launcher, Process, Spawner};
use dackel_launcher::profile::{ExeState, play_stats};
use std::fs;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Records every plan it is asked to spawn; each process exits when the test says so.
#[derive(Clone, Default)]
struct Fake {
    plans: Arc<Mutex<Vec<LaunchPlan>>>,
    exits: Arc<Mutex<Vec<Arc<AtomicBool>>>>,
}

struct FakeProcess(Arc<AtomicBool>);

impl Process for FakeProcess {
    fn has_exited(&mut self) -> io::Result<bool> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

impl Fake {
    fn spawned(&self) -> Vec<LaunchPlan> {
        self.plans.lock().unwrap().clone()
    }
    fn exit_nth(&self, n: usize) {
        self.exits.lock().unwrap()[n].store(true, Ordering::SeqCst);
    }
}

impl Spawner for Fake {
    fn spawn(&self, plan: &LaunchPlan) -> io::Result<Box<dyn Process>> {
        self.plans.lock().unwrap().push(plan.clone());
        let exited = Arc::new(AtomicBool::new(false));
        self.exits.lock().unwrap().push(exited.clone());
        Ok(Box::new(FakeProcess(exited)))
    }
}

fn ready() -> ExeState {
    ExeState::Ready {
        exe: PathBuf::from("Game.exe"),
    }
}

fn game_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Game.exe"), b"").unwrap();
    dir
}

#[test]
fn starting_a_ready_game_spawns_umu_and_marks_it_running() {
    let game = game_dir();
    let fake = Fake::default();
    let mut launcher = Launcher::new(fake.clone());

    launcher.start(game.path(), &ready(), None, 1000).unwrap();

    let spawned = fake.spawned();
    assert_eq!(spawned.len(), 1);
    assert_eq!(spawned[0].program, "umu-run");
    assert_eq!(
        spawned[0].log_path,
        game.path().join(".dlsite-launcher").join("last-run.log")
    );
    assert!(launcher.is_running(game.path()));
}

#[test]
fn a_game_that_is_not_ready_is_not_launched() {
    let game = game_dir();
    let fake = Fake::default();
    let mut launcher = Launcher::new(fake.clone());
    let pending = ExeState::NeedsExeChoice { candidates: vec![] };

    let err = launcher
        .start(game.path(), &pending, None, 1000)
        .unwrap_err();

    assert!(matches!(err, LaunchError::NotReady));
    assert!(fake.spawned().is_empty());
    assert!(!launcher.is_running(game.path()));
}

#[test]
fn a_second_launch_of_a_running_game_is_blocked() {
    let game = game_dir();
    let fake = Fake::default();
    let mut launcher = Launcher::new(fake.clone());
    launcher.start(game.path(), &ready(), None, 1000).unwrap();

    let err = launcher
        .start(game.path(), &ready(), None, 1010)
        .unwrap_err();

    assert!(matches!(err, LaunchError::AlreadyRunning));
    assert_eq!(fake.spawned().len(), 1);
}

#[test]
fn play_time_and_last_played_are_saved_when_the_game_exits() {
    let game = game_dir();
    let fake = Fake::default();
    let mut launcher = Launcher::new(fake.clone());
    launcher.start(game.path(), &ready(), None, 1000).unwrap();

    launcher.poll(1000).unwrap();
    assert_eq!(play_stats(game.path()).unwrap().play_time_secs, 0);

    fake.exit_nth(0);
    launcher.poll(1090).unwrap();

    let stats = play_stats(game.path()).unwrap();
    assert_eq!(stats.play_time_secs, 90);
    assert_eq!(stats.last_played, Some(1000));
    assert!(!launcher.is_running(game.path()));
}

#[test]
fn play_time_accumulates_over_sessions_and_a_game_can_run_again() {
    let game = game_dir();
    let fake = Fake::default();
    let mut launcher = Launcher::new(fake.clone());
    launcher.start(game.path(), &ready(), None, 1000).unwrap();
    fake.exit_nth(0);
    launcher.poll(1060).unwrap();

    launcher.start(game.path(), &ready(), None, 2000).unwrap();
    fake.exit_nth(1);
    launcher.poll(2030).unwrap();

    let stats = play_stats(game.path()).unwrap();
    assert_eq!(stats.play_time_secs, 90);
    assert_eq!(stats.last_played, Some(2000));
    assert_eq!(fake.spawned().len(), 2);
}

#[test]
fn the_real_spawner_runs_the_plan_in_its_folder_with_env_and_log() {
    use dackel_launcher::launch::UmuSpawner;
    use std::time::{Duration, Instant};

    let game = tempfile::tempdir().unwrap();
    let mut p = plan(game.path(), Path::new("bin/Game.exe"), None);
    p.program = "sh".to_owned();
    p.args = vec![
        OsString::from("-c"),
        OsString::from("pwd; echo \"$WINEPREFIX\"; echo oops >&2"),
    ];
    fs::create_dir_all(p.working_dir.clone()).unwrap();

    let mut process = UmuSpawner.spawn(&p).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !process.has_exited().unwrap() {
        assert!(Instant::now() < deadline, "process did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }

    let log = fs::read_to_string(game.path().join(".dlsite-launcher/last-run.log")).unwrap();
    assert!(log.contains("bin"), "log was {log}");
    assert!(log.contains(".dlsite-launcher/wine"), "log was {log}");
    assert!(log.contains("oops"), "stderr missing from {log}");
}
