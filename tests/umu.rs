use dackel_launcher::umu::{locate, missing_message};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn make_umu(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join("umu-run");
    fs::write(&path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn explicit_path_wins_over_everything_else() {
    let tmp = tempfile::tempdir().unwrap();
    let explicit = make_umu(&tmp.path().join("chosen"));
    make_umu(&tmp.path().join("app"));

    let found = locate(
        Some(explicit.as_os_str()),
        Some(&tmp.path().join("app")),
        None,
        Some(tmp.path()),
    );

    assert_eq!(found, Some(explicit));
}

#[test]
fn beside_the_program_comes_before_path_and_home() {
    let tmp = tempfile::tempdir().unwrap();
    let beside = make_umu(&tmp.path().join("app"));
    make_umu(&tmp.path().join("bin"));
    make_umu(&tmp.path().join("home/.local/bin"));
    let path_var = OsString::from(tmp.path().join("bin"));

    let found = locate(
        None,
        Some(&tmp.path().join("app")),
        Some(&path_var),
        Some(&tmp.path().join("home")),
    );

    assert_eq!(found, Some(beside));
}

#[test]
fn falls_back_to_path_then_the_local_bin() {
    let tmp = tempfile::tempdir().unwrap();
    let on_path = make_umu(&tmp.path().join("bin"));
    let local = make_umu(&tmp.path().join("home/.local/bin"));
    let path_var = OsString::from(tmp.path().join("bin"));

    let with_path = locate(None, None, Some(&path_var), Some(&tmp.path().join("home")));
    let without_path = locate(None, None, None, Some(&tmp.path().join("home")));

    assert_eq!(with_path, Some(on_path));
    assert_eq!(without_path, Some(local));
}

#[test]
fn a_file_that_is_not_executable_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("app");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("umu-run"), "x").unwrap();

    assert_eq!(locate(None, Some(&dir), None, None), None);
}

#[test]
fn nothing_found_gives_none_and_the_message_says_where_to_put_it() {
    let tmp = tempfile::tempdir().unwrap();

    assert_eq!(locate(None, Some(tmp.path()), None, None), None);
    let msg = missing_message();
    assert!(msg.contains("umu-run not found"));
    assert!(msg.contains("UMU_RUN"));
}
