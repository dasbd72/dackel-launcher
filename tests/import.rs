use dackel_launcher::import::{
    NameError, Outcome, Policy, check_folder_name, default_folder_name, find_conflicts, import,
    is_archive, move_by_copy,
};
use std::fs;
use std::path::{Path, PathBuf};

fn archive(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

fn item(source: &Path, folder: &str) -> (PathBuf, String) {
    (source.to_path_buf(), folder.to_owned())
}

#[test]
fn creates_the_game_folder_and_moves_the_archive_into_it() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let src = archive(downloads.path(), "RJ123456.zip", "game");

    let results = import(library.path(), &[item(&src, "RJ123456")], Policy::Overwrite);

    let dest = library.path().join("RJ123456").join("RJ123456.zip");
    assert!(matches!(
        &results[0].outcome,
        Outcome::Moved { dest: d, replaced: false } if *d == dest
    ));
    assert_eq!(fs::read_to_string(&dest).unwrap(), "game");
    assert!(!src.exists());
}

#[test]
fn reuses_an_existing_game_folder_without_touching_its_other_files() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let folder = library.path().join("RJ123456");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("notes.txt"), "keep").unwrap();
    let src = archive(downloads.path(), "RJ123456.zip", "game");

    import(library.path(), &[item(&src, "RJ123456")], Policy::Skip);

    assert_eq!(
        fs::read_to_string(folder.join("notes.txt")).unwrap(),
        "keep"
    );
    assert_eq!(
        fs::read_to_string(folder.join("RJ123456.zip")).unwrap(),
        "game"
    );
}

#[test]
fn two_files_with_one_folder_name_go_into_one_folder() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let part1 = archive(downloads.path(), "RJ123456.part1.zip", "one");
    let part2 = archive(downloads.path(), "RJ123456.part2.zip", "two");

    let results = import(
        library.path(),
        &[item(&part1, "RJ123456"), item(&part2, "RJ123456")],
        Policy::Skip,
    );

    assert!(
        results
            .iter()
            .all(|r| matches!(r.outcome, Outcome::Moved { .. }))
    );
    let folder = library.path().join("RJ123456");
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
    assert_eq!(
        fs::read_to_string(folder.join("RJ123456.part2.zip")).unwrap(),
        "two"
    );
}

#[test]
fn conflicts_report_both_sizes_and_modified_times() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let folder = library.path().join("RJ123456");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("RJ123456.zip"), "old copy").unwrap();
    let src = archive(downloads.path(), "RJ123456.zip", "new");
    let items = [
        item(&src, "RJ123456"),
        item(&archive(downloads.path(), "RJ999999.zip", "x"), "RJ999999"),
    ];

    let conflicts = find_conflicts(library.path(), &items);

    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.index, 0);
    assert_eq!(conflict.dest, folder.join("RJ123456.zip"));
    assert_eq!(conflict.existing.size, 8);
    assert_eq!(conflict.incoming.size, 3);
    assert_eq!(
        conflict.existing.modified,
        fs::metadata(folder.join("RJ123456.zip"))
            .unwrap()
            .modified()
            .ok()
    );
    assert_eq!(
        conflict.incoming.modified,
        fs::metadata(&src).unwrap().modified().ok()
    );
}

#[test]
fn overwrite_replaces_the_existing_file() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let folder = library.path().join("RJ123456");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("RJ123456.zip"), "old").unwrap();
    let src = archive(downloads.path(), "RJ123456.zip", "new");

    let results = import(library.path(), &[item(&src, "RJ123456")], Policy::Overwrite);

    assert!(matches!(
        results[0].outcome,
        Outcome::Moved { replaced: true, .. }
    ));
    assert_eq!(
        fs::read_to_string(folder.join("RJ123456.zip")).unwrap(),
        "new"
    );
    assert!(!src.exists());
}

#[test]
fn skip_leaves_both_the_existing_file_and_the_source_alone() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let folder = library.path().join("RJ123456");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("RJ123456.zip"), "old").unwrap();
    let src = archive(downloads.path(), "RJ123456.zip", "new");

    let results = import(library.path(), &[item(&src, "RJ123456")], Policy::Skip);

    assert!(matches!(results[0].outcome, Outcome::Skipped));
    assert_eq!(
        fs::read_to_string(folder.join("RJ123456.zip")).unwrap(),
        "old"
    );
    assert_eq!(fs::read_to_string(&src).unwrap(), "new");
}

#[test]
fn a_bad_name_fails_that_file_and_leaves_no_folder_behind() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let src = archive(downloads.path(), "a.zip", "x");

    let results = import(
        library.path(),
        &[item(&src, "../escape")],
        Policy::Overwrite,
    );

    assert!(matches!(results[0].outcome, Outcome::Failed(_)));
    assert!(src.exists());
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 0);
}

#[test]
fn a_failed_copy_leaves_the_source_and_no_partial_file() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let folder = library.path().join("RJ123456");
    fs::create_dir(&folder).unwrap();
    // A folder where the file should land: the rename into place cannot succeed.
    let blocker = folder.join("RJ123456.zip");
    fs::create_dir(&blocker).unwrap();
    let src = archive(downloads.path(), "RJ123456.zip", "game");

    let result = move_by_copy(&src, &blocker);

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&src).unwrap(), "game");
    assert!(blocker.is_dir());
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
}

#[test]
fn the_copy_fallback_moves_the_file_and_removes_the_source() {
    let downloads = tempfile::tempdir().unwrap();
    let library = tempfile::tempdir().unwrap();
    let dest = library.path().join("RJ123456.zip");
    let src = archive(downloads.path(), "RJ123456.zip", "game");

    move_by_copy(&src, &dest).unwrap();

    assert_eq!(fs::read_to_string(&dest).unwrap(), "game");
    assert!(!src.exists());
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 1);
}

#[test]
fn default_name_strips_one_extension_and_all_of_tar_gz() {
    assert_eq!(default_folder_name("RJ123456.zip"), "RJ123456");
    assert_eq!(default_folder_name("RJ123456.part1.rar"), "RJ123456.part1");
    assert_eq!(default_folder_name("RA000001.7z"), "RA000001");
    assert_eq!(default_folder_name("RJ123456.tar.gz"), "RJ123456");
    assert_eq!(default_folder_name("RJ123456.TAR.GZ"), "RJ123456");
}

#[test]
fn only_zip_7z_rar_and_tar_gz_are_archives() {
    assert!(is_archive(Path::new("/x/RJ1.zip")));
    assert!(is_archive(Path::new("/x/RJ1.7z")));
    assert!(is_archive(Path::new("/x/RJ1.rar")));
    assert!(is_archive(Path::new("/x/RJ1.tar.gz")));
    assert!(!is_archive(Path::new("/x/RJ1.exe")));
    assert!(!is_archive(Path::new("/x/RJ1.gz")));
}

#[test]
fn folder_names_are_one_plain_name() {
    for name in ["RJ123456", "Some Game", "RJ1.2"] {
        assert_eq!(check_folder_name(name), Ok(()), "{name}");
    }
    assert_eq!(check_folder_name(""), Err(NameError::Empty));
    assert_eq!(check_folder_name("   "), Err(NameError::Empty));
    for name in ["a/b", "a\\b"] {
        assert_eq!(check_folder_name(name), Err(NameError::Separator), "{name}");
    }
    for name in [".", "..", ".hidden"] {
        assert_eq!(
            check_folder_name(name),
            Err(NameError::LeadingDot),
            "{name}"
        );
    }
}
