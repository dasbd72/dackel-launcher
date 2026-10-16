//! The defaults a game's prefix gets before its first launch, and the spawner running that
//! setup step before the game.

use dackel_launcher::launch::{LaunchPlan, Process, Spawner, UmuSpawner, plan};
use dackel_launcher::prefix::{self, is_prepared, marker_path, prepare, registry_defaults};
use dackel_launcher::tuning::{prefix_dir, reset_prefix};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

fn game_plan(game: &Path) -> LaunchPlan {
    plan(game, Path::new("Game.exe"), None)
}

#[test]
fn a_fresh_prefix_gets_a_setup_step_that_imports_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("RJ123456");
    let mut p = game_plan(&game);

    prepare(&game, &mut p).unwrap();

    let reg = prefix::registry_file(&game);
    assert_eq!(std::fs::read_to_string(&reg).unwrap(), registry_defaults());
    let setup = p.setup.expect("setup step");
    assert_eq!(setup[0], "regedit");
    assert_eq!(setup[1], "/S");
    assert_eq!(
        setup[2].to_string_lossy(),
        format!("Z:{}", reg.display().to_string().replace('/', "\\"))
    );
    assert_eq!(p.marker, Some(marker_path(&game)));
}

#[test]
fn a_prefix_with_the_marker_gets_no_setup_step() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("RJ123456");
    std::fs::create_dir_all(prefix_dir(&game)).unwrap();
    std::fs::write(marker_path(&game), b"").unwrap();
    let mut p = game_plan(&game);

    prepare(&game, &mut p).unwrap();

    assert!(is_prepared(&game));
    assert_eq!(p.setup, None);
    assert_eq!(p.marker, None);
}

#[test]
fn reset_prefix_brings_the_setup_back() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("RJ123456");
    std::fs::create_dir_all(prefix_dir(&game)).unwrap();
    std::fs::write(marker_path(&game), b"").unwrap();

    reset_prefix(&game).unwrap();

    assert!(!is_prepared(&game));
}

#[test]
fn the_defaults_turn_off_menu_entries_and_crash_dialogs_and_nothing_else() {
    let reg = registry_defaults();

    assert!(reg.starts_with("Windows Registry Editor Version 5.00"));
    assert!(reg.contains(r#""winemenubuilder.exe"="""#));
    assert!(reg.contains(r#""ShowCrashDialog"=dword:00000000"#));
    assert!(
        !reg.contains("HKEY_LOCAL_MACHINE"),
        "no machine-wide changes"
    );
}

/// A stand-in for umu-run: appends its arguments to a file, one line per call.
fn fake_program(dir: &Path) -> std::path::PathBuf {
    let script = dir.join("fake-umu");
    std::fs::write(&script, "#!/bin/sh\necho \"$@\" >> \"$CALLS\"\nsleep 0.2\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn wait_for_exit(process: &mut dyn Process) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !process.has_exited().unwrap() {
        assert!(Instant::now() < deadline, "process never exited");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_spawner_runs_the_setup_step_then_the_game_and_leaves_the_marker() {
    let dir = tempfile::tempdir().unwrap();
    let calls = dir.path().join("calls");
    let marker = dir.path().join("marker");
    let mut p = game_plan(&dir.path().join("RJ1"));
    p.program = fake_program(dir.path()).display().to_string();
    p.working_dir = dir.path().to_path_buf();
    p.log_path = dir.path().join("last-run.log");
    p.env
        .insert("CALLS".to_owned(), calls.display().to_string());
    p.setup = Some(vec!["regedit".into(), "/S".into(), "x.reg".into()]);
    p.marker = Some(marker.clone());

    let mut process = UmuSpawner.spawn(&p).unwrap();
    assert!(!process.has_exited().unwrap(), "still preparing");
    wait_for_exit(process.as_mut());

    let lines = std::fs::read_to_string(&calls).unwrap();
    let lines: Vec<&str> = lines.lines().collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], "regedit /S x.reg");
    assert!(lines[1].ends_with("RJ1/Game.exe"), "{lines:?}");
    assert!(marker.exists());
}

#[test]
fn a_failed_setup_still_starts_the_game_and_leaves_no_marker() {
    let dir = tempfile::tempdir().unwrap();
    let calls = dir.path().join("calls");
    let marker = dir.path().join("marker");
    let script = dir.path().join("failing-umu");
    std::fs::write(
        &script,
        "#!/bin/sh\necho \"$@\" >> \"$CALLS\"\n[ \"$1\" = regedit ] && exit 1\nexit 0\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut p = game_plan(&dir.path().join("RJ1"));
    p.program = script.display().to_string();
    p.working_dir = dir.path().to_path_buf();
    p.log_path = dir.path().join("last-run.log");
    p.env
        .insert("CALLS".to_owned(), calls.display().to_string());
    p.setup = Some(vec!["regedit".into()]);
    p.marker = Some(marker.clone());

    let mut process = UmuSpawner.spawn(&p).unwrap();
    wait_for_exit(process.as_mut());

    assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 2);
    assert!(!marker.exists());
    assert!(
        std::fs::read_to_string(&p.log_path)
            .unwrap()
            .contains("prefix setup failed")
    );
}
