use dackel_launcher::library::{
    AddLibraryError, add_library, library_labels, scan_all, unique_roots,
};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

fn library_with(games: &[&str]) -> tempfile::TempDir {
    let lib = tempfile::tempdir().unwrap();
    for game in games {
        fs::create_dir(lib.path().join(game)).unwrap();
    }
    lib
}

mod scanning {
    use super::*;

    #[test]
    fn each_library_is_scanned_on_its_own_and_in_the_given_order() {
        let a = library_with(&["RJ111111"]);
        let b = library_with(&["RJ222222", "RJ333333"]);

        let scans = scan_all(&[a.path().to_path_buf(), b.path().to_path_buf()]);

        assert_eq!(scans.len(), 2);
        assert_eq!(scans[0].root, a.path());
        assert_eq!(scans[0].games.as_ref().unwrap().len(), 1);
        assert_eq!(scans[1].root, b.path());
        assert_eq!(scans[1].games.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn a_missing_library_is_reported_and_does_not_stop_the_others() {
        let a = library_with(&["RJ111111"]);
        let gone = a.path().join("unplugged");

        let scans = scan_all(&[gone.clone(), a.path().to_path_buf()]);

        assert_eq!(scans[0].root, gone);
        assert_eq!(
            scans[0].games.as_ref().unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(scans[1].games.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn the_same_folder_listed_twice_gives_every_game_twice_with_different_keys() {
        let lib = library_with(&["RJ111111"]);
        let root = lib.path().to_path_buf();

        let scans = scan_all(&[root.clone(), root.clone()]);
        let first = &scans[0].games.as_ref().unwrap()[0];
        let second = &scans[1].games.as_ref().unwrap()[0];

        assert_eq!(first.dir, second.dir);
        assert_ne!(first.key(), second.key());
        assert_eq!(first.dir_key(), second.dir_key());
        assert_eq!(first.slot, 0);
        assert_eq!(second.slot, 1);
    }

    #[test]
    fn the_same_game_id_in_two_libraries_stays_two_separate_games() {
        let a = library_with(&["RJ111111"]);
        let b = library_with(&["RJ111111"]);

        let scans = scan_all(&[a.path().to_path_buf(), b.path().to_path_buf()]);
        let first = &scans[0].games.as_ref().unwrap()[0];
        let second = &scans[1].games.as_ref().unwrap()[0];

        assert_eq!(first.id, second.id);
        assert_ne!(first.key(), second.key());
        assert_eq!(first.key(), first.dir.to_string_lossy());
    }
}

mod adding {
    use super::*;

    #[test]
    fn an_existing_folder_is_accepted_and_trimmed() {
        let lib = library_with(&[]);
        let typed = format!("  {}  ", lib.path().display());

        let added = add_library(&[], &typed).unwrap();

        assert_eq!(added, lib.path().canonicalize().unwrap());
    }

    #[test]
    fn nothing_typed_is_rejected() {
        assert_eq!(add_library(&[], "   "), Err(AddLibraryError::Empty));
    }

    #[test]
    fn a_path_that_does_not_exist_is_rejected() {
        let lib = library_with(&[]);
        let typed = lib.path().join("nope").display().to_string();

        assert_eq!(add_library(&[], &typed), Err(AddLibraryError::NotFound));
    }

    #[test]
    fn a_file_is_not_a_library() {
        let lib = library_with(&[]);
        let file = lib.path().join("a.txt");
        fs::write(&file, "").unwrap();

        assert_eq!(
            add_library(&[], &file.display().to_string()),
            Err(AddLibraryError::NotADirectory)
        );
    }

    #[test]
    fn a_library_already_listed_is_rejected_however_it_is_spelled() {
        let lib = library_with(&[]);
        let listed = vec![lib.path().canonicalize().unwrap()];
        let with_slash = format!("{}/", lib.path().display());
        let dotted = format!("{}/.", lib.path().display());

        assert_eq!(
            add_library(&listed, &with_slash),
            Err(AddLibraryError::AlreadyListed)
        );
        assert_eq!(
            add_library(&listed, &dotted),
            Err(AddLibraryError::AlreadyListed)
        );
    }

    #[test]
    fn a_listed_library_that_is_offline_does_not_block_adding_another() {
        let lib = library_with(&[]);
        let offline = vec![PathBuf::from("/run/media/nobody/GONE/DLSiteLibrary")];

        assert!(add_library(&offline, &lib.path().display().to_string()).is_ok());
    }
}

mod labels {
    use super::*;

    #[test]
    fn libraries_are_named_by_their_folder() {
        let roots = [
            PathBuf::from("/run/media/a/DLSiteLibrary"),
            PathBuf::from("/mnt/games/Extra"),
        ];

        assert_eq!(library_labels(&roots), ["DLSiteLibrary", "Extra"]);
    }

    #[test]
    fn the_same_folder_listed_twice_gets_distinct_labels() {
        let roots = [
            PathBuf::from("/run/media/a/DLSiteLibrary"),
            PathBuf::from("/run/media/a/DLSiteLibrary"),
        ];

        let labels = library_labels(&roots);

        assert_ne!(labels[0], labels[1]);
        assert!(labels[1].contains("(2)"), "{labels:?}");
    }

    #[test]
    fn libraries_with_the_same_folder_name_show_their_full_path() {
        let roots = [
            PathBuf::from("/run/media/a/DLSiteLibrary"),
            PathBuf::from("/run/media/b/DLSiteLibrary"),
            PathBuf::from("/mnt/Other"),
        ];

        assert_eq!(
            library_labels(&roots),
            [
                "/run/media/a/DLSiteLibrary",
                "/run/media/b/DLSiteLibrary",
                "Other"
            ]
        );
    }
}

mod unique {
    use super::*;

    #[test]
    fn a_folder_listed_again_is_dropped_keeping_the_first_in_order() {
        let a = library_with(&[]);
        let b = library_with(&[]);
        let roots = vec![
            a.path().to_path_buf(),
            b.path().to_path_buf(),
            a.path().to_path_buf(),
        ];

        assert_eq!(
            unique_roots(&roots),
            [a.path().to_path_buf(), b.path().to_path_buf()]
        );
    }

    #[test]
    fn different_spellings_of_one_folder_count_as_one() {
        let a = library_with(&[]);
        let roots = vec![
            a.path().to_path_buf(),
            PathBuf::from(format!("{}/", a.path().display())),
            PathBuf::from(format!("{}/.", a.path().display())),
        ];

        assert_eq!(unique_roots(&roots), [a.path().to_path_buf()]);
    }

    #[test]
    fn an_offline_folder_listed_twice_is_dropped_by_its_text() {
        let gone = PathBuf::from("/run/media/nobody/GONE/DLSiteLibrary");

        assert_eq!(unique_roots(&[gone.clone(), gone.clone()]), [gone]);
    }
}
