use dackel_launcher::atomic;
use std::fs;

#[test]
fn replaces_the_file_and_leaves_no_temporary_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.json");
    fs::write(&path, "old old old").unwrap();

    atomic::write(&path, "new").unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
