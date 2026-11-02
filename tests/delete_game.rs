use dackel_launcher::library::delete_game;
use std::fs;

#[test]
fn erases_a_game_folder_with_everything_in_it() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ1");
    fs::create_dir_all(game.join("data")).unwrap();
    fs::write(game.join("data/save.dat"), "x").unwrap();

    delete_game(&[lib.path().to_path_buf()], &game).unwrap();

    assert!(!game.exists());
    assert!(lib.path().exists());
}

#[test]
fn refuses_a_library_root_or_a_folder_that_is_not_its_direct_child() {
    let lib = tempfile::tempdir().unwrap();
    let nested = lib.path().join("RJ1/deeper");
    fs::create_dir_all(&nested).unwrap();
    let other = tempfile::tempdir().unwrap();
    let roots = [lib.path().to_path_buf()];

    for dir in [lib.path(), nested.as_path(), other.path()] {
        assert!(delete_game(&roots, dir).is_err(), "{}", dir.display());
        assert!(dir.exists(), "{}", dir.display());
    }
}

#[test]
fn a_symlink_to_a_folder_is_removed_but_not_followed() {
    let lib = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    fs::write(elsewhere.path().join("keep"), "x").unwrap();
    let link = lib.path().join("RJ1");
    std::os::unix::fs::symlink(elsewhere.path(), &link).unwrap();

    delete_game(&[lib.path().to_path_buf()], &link).unwrap();

    assert!(!link.exists());
    assert!(elsewhere.path().join("keep").exists());
}
