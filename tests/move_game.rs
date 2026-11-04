use dackel_launcher::extract::Progress;
use dackel_launcher::library::{clean_moving, move_blocked, move_game, scan};
use std::fs;
use std::sync::atomic::AtomicBool;

fn game(lib: &std::path::Path, name: &str) -> std::path::PathBuf {
    let dir = lib.join(name);
    fs::create_dir_all(dir.join("prefix")).unwrap();
    fs::write(dir.join("prefix/save.dat"), "x").unwrap();
    dir
}

#[test]
fn moves_a_game_folder_into_another_library() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];

    let moved = move_game(
        &roots,
        &dir,
        b.path(),
        &AtomicBool::new(false),
        false,
        &mut |_| {},
    )
    .unwrap();

    assert_eq!(moved.dir, b.path().join("RJ1"));
    assert!(!dir.exists());
    assert_eq!(
        fs::read_to_string(b.path().join("RJ1/prefix/save.dat")).unwrap(),
        "x"
    );
}

#[test]
fn refuses_a_name_collision_and_changes_nothing() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    fs::create_dir(b.path().join("RJ1")).unwrap();
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];

    assert!(
        move_game(
            &roots,
            &dir,
            b.path(),
            &AtomicBool::new(false),
            false,
            &mut |_| {}
        )
        .is_err()
    );
    assert!(dir.join("prefix/save.dat").exists());
    assert_eq!(fs::read_dir(b.path().join("RJ1")).unwrap().count(), 0);
}

#[test]
fn refuses_a_destination_that_is_not_another_added_library() {
    let (a, b, other) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let dir = game(a.path(), "RJ1");
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
    let cancel = AtomicBool::new(false);

    assert!(move_game(&roots, &dir, other.path(), &cancel, false, &mut |_| {}).is_err());
    assert!(move_game(&roots, &dir, a.path(), &cancel, false, &mut |_| {}).is_err());
    assert!(dir.exists());
}

#[test]
fn a_copy_keeps_symlinks_and_permissions_and_leaves_no_temp_folder() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    let exe = dir.join("game.exe");
    fs::write(&exe, "MZ").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o750)).unwrap();
    symlink("prefix/save.dat", dir.join("link")).unwrap();
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
    let before = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&dir).unwrap());

    let moved = move_game(
        &roots,
        &dir,
        b.path(),
        &AtomicBool::new(false),
        true,
        &mut |_| {},
    )
    .unwrap();

    assert!(!dir.exists());
    assert!(moved.leftover.is_none());
    let new = b.path().join("RJ1");
    // A rename would keep the folder's inode; a real copy gets a new one.
    assert_ne!(
        std::os::unix::fs::MetadataExt::ino(&fs::metadata(&new).unwrap()),
        before
    );
    assert_eq!(
        fs::metadata(new.join("game.exe"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o750
    );
    assert_eq!(
        fs::read_link(new.join("link")).unwrap(),
        std::path::Path::new("prefix/save.dat")
    );
    let names: Vec<_> = fs::read_dir(b.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["RJ1"]);
}

#[test]
fn a_cancelled_copy_leaves_the_source_whole_and_no_partial_copy() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];

    let err = move_game(
        &roots,
        &dir,
        b.path(),
        &AtomicBool::new(true),
        true,
        &mut |_| {},
    )
    .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
    assert!(dir.join("prefix/save.dat").exists());
    assert_eq!(fs::read_dir(b.path()).unwrap().count(), 0);
}

#[test]
fn when_the_old_folder_cannot_be_erased_the_new_copy_is_the_game() {
    use std::os::unix::fs::PermissionsExt;
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
    // No write access to the library, so its game folder cannot be removed.
    fs::set_permissions(a.path(), fs::Permissions::from_mode(0o555)).unwrap();

    let moved = move_game(
        &roots,
        &dir,
        b.path(),
        &AtomicBool::new(false),
        true,
        &mut |_| {},
    );

    fs::set_permissions(a.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let moved = moved.unwrap();
    assert_eq!(moved.leftover.as_deref(), Some(dir.as_path()));
    assert!(b.path().join("RJ1/prefix/save.dat").exists());
}

#[test]
fn leftover_partial_copies_are_removed_and_games_are_not() {
    let a = tempfile::tempdir().unwrap();
    game(a.path(), "RJ1");
    let partial = game(a.path(), ".RJ2.moving");
    fs::write(partial.join(".dackel-moving"), "").unwrap();
    let roots = [a.path().to_path_buf(), a.path().join("missing")];

    clean_moving(&roots);

    assert!(!partial.exists());
    assert!(a.path().join("RJ1/prefix/save.dat").exists());
}

#[test]
fn a_copy_reports_progress_up_to_the_whole_size() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    fs::write(dir.join("big.bin"), vec![7u8; 3 << 20]).unwrap();
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
    let mut last = Progress::default();

    move_game(
        &roots,
        &dir,
        b.path(),
        &AtomicBool::new(false),
        true,
        &mut |p| last = p,
    )
    .unwrap();

    assert_eq!(last.bytes_total, (3 << 20) + 1);
    assert_eq!(last.bytes_done, last.bytes_total);
}

#[test]
fn cancelling_in_the_middle_of_a_big_file_stops_it_and_leaves_the_source() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    fs::write(dir.join("big.bin"), vec![7u8; 8 << 20]).unwrap();
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];
    let cancel = AtomicBool::new(false);

    let err = move_game(&roots, &dir, b.path(), &cancel, true, &mut |p| {
        if p.bytes_done > 0 {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(fs::metadata(dir.join("big.bin")).unwrap().len(), 8 << 20);
    assert_eq!(fs::read_dir(b.path()).unwrap().count(), 0);
}

#[test]
fn a_folder_of_the_same_name_that_is_not_ours_is_never_touched() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let dir = game(a.path(), "RJ1");
    let theirs = game(b.path(), ".RJ1.moving");
    let roots = [a.path().to_path_buf(), b.path().to_path_buf()];

    assert!(
        move_game(
            &roots,
            &dir,
            b.path(),
            &AtomicBool::new(false),
            true,
            &mut |_| {}
        )
        .is_err()
    );
    clean_moving(&roots);

    assert!(theirs.join("prefix/save.dat").exists());
    assert!(dir.exists());
}

#[test]
fn the_scan_does_not_list_a_copy_in_progress() {
    let a = tempfile::tempdir().unwrap();
    game(a.path(), "RJ1");
    game(a.path(), ".RJ2.moving");

    let ids: Vec<_> = scan(a.path()).unwrap().into_iter().map(|g| g.id).collect();

    assert_eq!(ids, ["RJ1"]);
}

#[test]
fn move_blocked_gives_the_reason_for_each_refusal_and_none_when_clear() {
    use std::os::unix::fs::PermissionsExt;
    let (a, b, c) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let dir = game(a.path(), "RJ1");
    let roots = [
        a.path().to_path_buf(),
        b.path().to_path_buf(),
        c.path().to_path_buf(),
    ];
    fs::create_dir(c.path().join("RJ1")).unwrap();

    assert_eq!(move_blocked(&roots, &dir, b.path()), None);
    assert!(
        move_blocked(&roots, &dir, a.path()).is_some(),
        "its own library"
    );
    assert!(move_blocked(&roots, &dir, c.path()).is_some(), "name taken");
    assert!(move_blocked(&roots, &dir, std::path::Path::new("/nowhere")).is_some());
    fs::set_permissions(b.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let readonly = move_blocked(&roots, &dir, b.path());
    fs::set_permissions(b.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(readonly.is_some());
}
