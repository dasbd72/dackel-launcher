use dlsite_launcher::profile::{ExeState, choose, ensure};
use std::fs;
use std::path::{Path, PathBuf};

fn file(root: &Path, rel: &str, bytes: usize) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

fn profile_path(game: &Path) -> PathBuf {
    game.join(".dlsite-launcher").join("profile.json")
}

#[test]
fn single_exe_is_picked_and_profiled_with_a_relative_path() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Game.exe", 1000);

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Game.exe")
        }
    );
    let json = fs::read_to_string(profile_path(game.path())).unwrap();
    assert!(json.contains("\"Game.exe\""), "profile was {json}");
}

#[test]
fn installers_and_helpers_are_never_candidates() {
    let game = tempfile::tempdir().unwrap();
    for helper in [
        "unins000.exe",
        "UNINS001.EXE",
        "UnityCrashHandler64.exe",
        "vc_redist.x64.exe",
        "DXWEBSETUP.exe",
        "notification_helper.exe",
        "GameConfig.exe",
        "Game_Setup.exe",
    ] {
        file(game.path(), helper, 100_000_000);
    }
    file(game.path(), "Game.exe", 1000);

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Game.exe")
        }
    );
}

#[test]
fn similar_exes_need_a_choice_and_the_pending_state_is_recorded() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Launcher.exe", 5000);
    file(game.path(), "Game.exe", 5000);

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::NeedsExeChoice {
            candidates: vec![PathBuf::from("Game.exe"), PathBuf::from("Launcher.exe")]
        }
    );
    assert!(profile_path(game.path()).exists());
}

#[test]
fn a_clearly_larger_exe_wins_without_asking() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Launcher.exe", 1_000);
    file(game.path(), "Game.exe", 50_000);

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Game.exe")
        }
    );
}

#[test]
fn exes_are_searched_three_levels_deep_and_never_inside_the_metadata_folder() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Bin/x64/Game.exe", 1_000);
    file(game.path(), "A/B/C/D/TooDeep.exe", 100_000_000);
    file(
        game.path(),
        ".dlsite-launcher/wine/drive_c/Prefix.exe",
        100_000_000,
    );

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Bin/x64/Game.exe")
        }
    );
}

#[test]
fn legacy_start_sh_game_exe_is_used_as_a_hint() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Bin/Game.exe", 5000);
    file(game.path(), "Launcher.exe", 5000);
    fs::write(
        game.path().join("start.sh"),
        "#!/bin/bash\nSCRIPT_DIR=\"$(dirname \"$0\")\"\nGAME_EXE=\"${SCRIPT_DIR}/Bin/Game.exe\"\numu-run \"$GAME_EXE\"\n",
    )
    .unwrap();

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Bin/Game.exe")
        }
    );
    let json = fs::read_to_string(profile_path(game.path())).unwrap();
    assert!(json.contains("Bin/Game.exe"), "profile was {json}");
}

#[test]
fn start_sh_hint_pointing_at_a_missing_file_is_ignored() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Launcher.exe", 5000);
    file(game.path(), "Game.exe", 5000);
    fs::write(
        game.path().join("start.sh"),
        "GAME_EXE=\"${SCRIPT_DIR}/Gone/Old.exe\"\n",
    )
    .unwrap();

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::NeedsExeChoice {
            candidates: vec![PathBuf::from("Game.exe"), PathBuf::from("Launcher.exe")]
        }
    );
}

#[test]
fn a_chosen_exe_survives_rescans_and_is_never_re_guessed() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Bin/A.exe", 5000);
    file(game.path(), "Bin/B.exe", 5000);
    assert!(matches!(
        ensure(game.path()).unwrap(),
        ExeState::NeedsExeChoice { .. }
    ));

    choose(game.path(), Path::new("Bin/B.exe")).unwrap();
    file(game.path(), "Huge.exe", 1_000_000);

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Bin/B.exe")
        }
    );
}

#[test]
fn only_detected_candidates_can_be_chosen() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Game.exe", 5000);
    file(game.path(), "unins000.exe", 5000);
    ensure(game.path()).unwrap();

    assert!(choose(game.path(), Path::new("unins000.exe")).is_err());
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_path_buf();
                out.push((rel, fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn only_the_metadata_folder_is_ever_written() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Bin/A.exe", 5000);
    file(game.path(), "Bin/B.exe", 5000);
    file(game.path(), "readme.txt", 10);
    fs::write(
        game.path().join("start.sh"),
        "GAME_EXE=\"${SCRIPT_DIR}/Bin/A.exe\"\n",
    )
    .unwrap();
    let before = snapshot(game.path());

    ensure(game.path()).unwrap();
    choose(game.path(), Path::new("Bin/B.exe")).unwrap();
    ensure(game.path()).unwrap();

    let after: Vec<_> = snapshot(game.path())
        .into_iter()
        .filter(|(rel, _)| !rel.starts_with(".dlsite-launcher"))
        .collect();
    assert_eq!(after, before);
}

#[test]
fn exported_game_exe_in_start_sh_is_used_as_a_hint() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Bin/Game.exe", 5000);
    file(game.path(), "Launcher.exe", 5000);
    fs::write(
        game.path().join("start.sh"),
        "#!/bin/bash\nexport GAME_EXE=\"${SCRIPT_DIR}/Bin/Game.exe\"\nexport WORKING_DIR=\"$(dirname \"$GAME_EXE\")\"\n",
    )
    .unwrap();

    let state = ensure(game.path()).unwrap();

    assert_eq!(
        state,
        ExeState::Ready {
            exe: PathBuf::from("Bin/Game.exe")
        }
    );
}
