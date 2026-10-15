//! Which action wins when one frame asks for several.

use dackel_launcher::actions::{Actions, Exclusive};
use std::path::PathBuf;

fn path(name: &str) -> PathBuf {
    PathBuf::from(name)
}

#[test]
fn a_frame_that_asked_for_nothing_has_no_exclusive_action() {
    assert_eq!(Actions::default().take_exclusive(), None);
}

#[test]
fn each_exclusive_action_is_taken_when_it_is_the_only_one() {
    let mut a = Actions {
        rescan: true,
        ..Actions::default()
    };
    assert_eq!(a.take_exclusive(), Some(Exclusive::Rescan));

    let mut a = Actions {
        create_library: true,
        ..Actions::default()
    };
    assert_eq!(a.take_exclusive(), Some(Exclusive::CreateLibrary));

    let mut a = Actions {
        open_libraries: true,
        ..Actions::default()
    };
    assert_eq!(a.take_exclusive(), Some(Exclusive::OpenLibraries));

    let mut a = Actions {
        details_extract: Some(path("/g/a")),
        ..Actions::default()
    };
    assert_eq!(a.take_exclusive(), Some(Exclusive::Extract(path("/g/a"))));

    let mut a = Actions {
        choose_exe: Some((path("/g/a"), path("/g/a/run.exe"))),
        ..Actions::default()
    };
    assert_eq!(
        a.take_exclusive(),
        Some(Exclusive::ChooseExe {
            dir: path("/g/a"),
            exe: path("/g/a/run.exe")
        })
    );
}

#[test]
fn the_order_is_choose_exe_then_extract_then_libraries_then_create_then_rescan() {
    // Everything asked for at once, then one fewer each time: only the first in order is taken.
    let all = || Actions {
        rescan: true,
        create_library: true,
        open_libraries: true,
        extract: Some(path("/g/a")),
        choose_exe: Some((path("/g/b"), path("/g/b/run.exe"))),
        ..Actions::default()
    };
    let mut a = all();
    assert!(matches!(
        a.take_exclusive(),
        Some(Exclusive::ChooseExe { .. })
    ));
    let mut a = all();
    a.choose_exe = None;
    assert_eq!(a.take_exclusive(), Some(Exclusive::Extract(path("/g/a"))));
    let mut a = all();
    a.choose_exe = None;
    a.extract = None;
    assert_eq!(a.take_exclusive(), Some(Exclusive::OpenLibraries));
    let mut a = all();
    a.choose_exe = None;
    a.extract = None;
    a.open_libraries = false;
    assert_eq!(a.take_exclusive(), Some(Exclusive::CreateLibrary));
    let mut a = all();
    a.choose_exe = None;
    a.extract = None;
    a.open_libraries = false;
    a.create_library = false;
    assert_eq!(a.take_exclusive(), Some(Exclusive::Rescan));
}

#[test]
fn the_details_pages_exe_choice_wins_over_a_popups() {
    let mut a = Actions {
        details_choice: Some((path("/g/a"), path("/g/a/one.exe"))),
        choose_exe: Some((path("/g/b"), path("/g/b/two.exe"))),
        ..Actions::default()
    };
    assert_eq!(
        a.take_exclusive(),
        Some(Exclusive::ChooseExe {
            dir: path("/g/a"),
            exe: path("/g/a/one.exe")
        })
    );
}

#[test]
fn a_tiles_extraction_wins_over_the_details_pages() {
    let mut a = Actions {
        extract: Some(path("/g/tile")),
        details_extract: Some(path("/g/details")),
        ..Actions::default()
    };
    assert_eq!(
        a.take_exclusive(),
        Some(Exclusive::Extract(path("/g/tile")))
    );
}

#[test]
fn actions_that_are_not_exclusive_are_left_alone() {
    let mut a = Actions {
        back: true,
        launch: Some(3),
        favorite: Some("id".to_owned()),
        cancel_extract: vec!["/g/a".to_owned()],
        ..Actions::default()
    };
    assert_eq!(a.take_exclusive(), None);
    assert!(a.back);
    assert_eq!(a.launch, Some(3));
    assert_eq!(a.favorite.as_deref(), Some("id"));
    assert_eq!(a.cancel_extract, vec!["/g/a".to_owned()]);
}
