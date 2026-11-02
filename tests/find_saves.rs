use dackel_launcher::saves::{MAX, find_saves};
use std::fs;
use std::path::{Path, PathBuf};

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "x").unwrap();
}

#[test]
fn finds_saves_in_the_game_and_the_prefix_but_not_wine_noise() {
    let game = tempfile::tempdir().unwrap();
    touch(game.path(), "data/Save01.dat");
    touch(game.path(), "SAVEDATA/slot1");
    touch(
        game.path(),
        ".dackel-launcher/wine/drive_c/users/deck/Documents/mysave.sav",
    );
    touch(
        game.path(),
        ".dackel-launcher/wine/drive_c/windows/system32/savedll.dll",
    );
    touch(
        game.path(),
        ".dackel-launcher/wine/drive_c/Program Files (x86)/x/save.cfg",
    );
    touch(game.path(), "data/readme.txt");

    let found = find_saves(game.path());

    let mut paths = found.paths;
    paths.sort();
    assert_eq!(
        paths,
        [
            ".dackel-launcher/wine/drive_c/users/deck/Documents/mysave.sav",
            "SAVEDATA",
            "data/Save01.dat",
        ]
        .map(PathBuf::from)
    );
    assert!(!found.more);
}

#[test]
fn does_not_follow_symlinks_out_of_the_game() {
    let game = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    touch(outside.path(), "save.dat");
    std::os::unix::fs::symlink(outside.path(), game.path().join("z")).unwrap();

    assert!(find_saves(game.path()).paths.is_empty());
}

#[test]
fn stops_at_the_cap_and_says_there_is_more() {
    let game = tempfile::tempdir().unwrap();
    for i in 0..MAX + 5 {
        touch(game.path(), &format!("save{i}"));
    }

    let found = find_saves(game.path());

    assert_eq!(found.paths.len(), MAX);
    assert!(found.more);
}
