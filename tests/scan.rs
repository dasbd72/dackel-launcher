use dackel_launcher::library::{is_dlsite_id, scan};
use std::fs;

fn ids(library: &std::path::Path) -> Vec<String> {
    scan(library).unwrap().into_iter().map(|g| g.id).collect()
}

#[test]
fn lists_every_game_folder_sorted_by_id_but_not_hidden_folders_or_files() {
    let lib = tempfile::tempdir().unwrap();
    for name in [
        "RJ01389782",
        "RJ297317",
        "Some Other Game",
        ".hidden",
        ".dackel-launcher",
    ] {
        fs::create_dir(lib.path().join(name)).unwrap();
    }
    fs::write(lib.path().join("RJ555555"), "a file, not a game").unwrap();
    fs::write(lib.path().join("common.sh"), "").unwrap();

    assert_eq!(
        ids(lib.path()),
        ["RJ01389782", "RJ297317", "Some Other Game"]
    );
}

#[test]
fn only_rj_followed_by_digits_is_a_dlsite_id() {
    for id in ["RJ01389782", "RJ1"] {
        assert!(is_dlsite_id(id), "{id}");
    }
    for id in [
        "RJ",
        "RJ12ab",
        "rj123",
        "temp_RJ01612428",
        "RJ99.zip.d",
        "Game",
    ] {
        assert!(!is_dlsite_id(id), "{id}");
    }
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
    let meta = lib.path().join("RJ123456").join(".dackel-launcher");
    fs::create_dir_all(meta.join("wine").join("RJ999999")).unwrap();

    assert_eq!(ids(lib.path()), ["RJ123456"]);
}

#[test]
fn missing_library_is_an_error_not_an_empty_list() {
    let lib = tempfile::tempdir().unwrap();

    let err = scan(&lib.path().join("DLSiteLibrary")).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn an_imported_zip_needs_extracting_and_an_imported_other_archive_is_unsupported() {
    use dackel_launcher::extract::{Status, Unsupported, status};
    use dackel_launcher::import::{Policy, import};

    let downloads = tempfile::tempdir().unwrap();
    let lib = tempfile::tempdir().unwrap();
    let zip = downloads.path().join("RJ111111.zip");
    let rar = downloads.path().join("RJ222222.rar");
    fs::write(&zip, "zip").unwrap();
    fs::write(&rar, "rar").unwrap();

    import(
        lib.path(),
        &[(zip, "RJ111111".to_owned()), (rar, "RJ222222".to_owned())],
        Policy::Skip,
    );

    let games = scan(lib.path()).unwrap();
    let state = |id: &str| {
        let game = games.iter().find(|g| g.id == id).unwrap();
        status(&game.dir).unwrap()
    };
    assert_eq!(state("RJ111111"), Status::NotExtracted);
    assert_eq!(
        state("RJ222222"),
        Status::ExtractManually(Unsupported::OtherArchive)
    );
}
