use dackel_launcher::library::scan;
use std::fs;

fn ids(library: &std::path::Path) -> Vec<String> {
    scan(library).unwrap().into_iter().map(|g| g.id).collect()
}

#[test]
fn lists_only_rj_directories_sorted_by_id() {
    let lib = tempfile::tempdir().unwrap();
    for name in [
        "RJ01389782",
        "RJ297317",
        "wine",
        "temp_RJ01612428",
        "RJ",
        "RJ12ab",
        "rj123",
        "RJ99.zip.d",
    ] {
        fs::create_dir(lib.path().join(name)).unwrap();
    }
    fs::write(lib.path().join("RJ555555"), "a file, not a game").unwrap();
    fs::write(lib.path().join("common.sh"), "").unwrap();

    assert_eq!(ids(lib.path()), ["RJ01389782", "RJ297317"]);
}

#[test]
fn game_carries_its_directory() {
    let lib = tempfile::tempdir().unwrap();
    fs::create_dir(lib.path().join("RJ123456")).unwrap();

    let games = scan(lib.path()).unwrap();

    assert_eq!(games[0].dir, lib.path().join("RJ123456"));
}

#[test]
fn rescan_picks_up_folders_added_later() {
    let lib = tempfile::tempdir().unwrap();
    fs::create_dir(lib.path().join("RJ111111")).unwrap();
    assert_eq!(ids(lib.path()), ["RJ111111"]);

    fs::create_dir(lib.path().join("RJ222222")).unwrap();

    assert_eq!(ids(lib.path()), ["RJ111111", "RJ222222"]);
}

#[test]
fn metadata_folder_contents_are_never_listed_as_games() {
    let lib = tempfile::tempdir().unwrap();
    let meta = lib.path().join("RJ123456").join(".dlsite-launcher");
    fs::create_dir_all(meta.join("wine").join("RJ999999")).unwrap();

    assert_eq!(ids(lib.path()), ["RJ123456"]);
}

#[test]
fn missing_library_is_an_error_not_an_empty_list() {
    let lib = tempfile::tempdir().unwrap();

    let err = scan(&lib.path().join("DLSiteLibrary")).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
