use dackel_launcher::library::AddLibraryError;
use dackel_launcher::library_list::{LibraryList, Removed};
use std::path::PathBuf;

/// Three real folders: the one beside the program and two to add.
struct Dirs {
    _tmp: tempfile::TempDir,
    beside: PathBuf,
    a: PathBuf,
    b: PathBuf,
}

fn dirs() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let make = |name: &str| {
        let dir = tmp.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        dir.canonicalize().unwrap()
    };
    Dirs {
        beside: make("beside"),
        a: make("a"),
        b: make("b"),
        _tmp: tmp,
    }
}

fn list_of(d: &Dirs, added: &[&PathBuf]) -> LibraryList {
    let added: Vec<PathBuf> = added.iter().map(|p| (*p).clone()).collect();
    LibraryList::new(d.beside.clone(), &added)
}

#[test]
fn the_folder_beside_the_program_is_first_and_the_rest_are_added() {
    let d = dirs();

    let list = list_of(&d, &[&d.a, &d.b]);

    assert_eq!(list.beside(), d.beside);
    assert_eq!(list.roots(), [d.beside.clone(), d.a.clone(), d.b.clone()]);
    assert_eq!(list.added(), [d.a.clone(), d.b.clone()]);
    assert_eq!(list.count(), 3);
}

#[test]
fn a_folder_listed_twice_shows_once() {
    let d = dirs();
    let again = PathBuf::from(format!("{}/", d.beside.display()));

    let list = list_of(&d, &[&again, &d.a, &d.a]);

    assert_eq!(list.roots(), [d.beside.clone(), d.a.clone()]);
}

#[test]
fn a_new_list_has_no_filter_nothing_offline_and_no_error() {
    let d = dirs();

    let list = list_of(&d, &[&d.a]);

    assert_eq!(list.filter(), None);
    assert!(list.offline().is_empty());
    assert_eq!(list.error(), None);
}

#[test]
fn offline_libraries_are_remembered_and_all_offline_means_every_one() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a]);

    list.set_offline(vec![d.a.clone()]);
    assert!(list.is_offline(&d.a));
    assert!(!list.is_offline(&d.beside));
    assert!(!list.all_offline());

    list.set_offline(vec![d.beside.clone(), d.a.clone()]);
    assert!(list.all_offline());
}

#[test]
fn the_manager_hides_an_offline_folder_beside_the_program_but_not_an_added_one() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a]);
    list.set_offline(vec![d.beside.clone(), d.a.clone()]);

    let slots: Vec<usize> = list.listed().into_iter().map(|(slot, _)| slot).collect();

    assert_eq!(slots, [1]);
}

#[test]
fn the_filter_shows_one_slot_or_all() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a]);
    assert!(list.shows_slot(0) && list.shows_slot(1));

    list.set_filter(Some(1));

    assert!(!list.shows_slot(0));
    assert!(list.shows_slot(1));
}

#[test]
fn a_single_library_has_no_label_and_several_are_named_by_slot() {
    let d = dirs();
    assert_eq!(list_of(&d, &[]).label_of(0), None);

    let list = list_of(&d, &[&d.a]);

    assert_eq!(list.label_of(0).as_deref(), Some("beside"));
    assert_eq!(list.label_of(1).as_deref(), Some("a"));
    assert_eq!(list.label_of(2), None);
}

#[test]
fn a_typed_folder_is_added_and_the_box_is_emptied() {
    let d = dirs();
    let mut list = list_of(&d, &[]);
    *list.input_mut() = format!("  {}  ", d.a.display());

    let added = list.add_typed();

    assert_eq!(added, Ok(d.a.clone()));
    assert_eq!(list.roots(), [d.beside.clone(), d.a.clone()]);
    assert_eq!(list.input_mut(), "");
    assert_eq!(list.error(), None);
}

#[test]
fn a_bad_typed_folder_keeps_the_text_and_says_why() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a]);

    *list.input_mut() = d.a.display().to_string();
    assert_eq!(list.add_typed(), Err(AddLibraryError::AlreadyListed));
    assert_eq!(list.error(), Some("That library is already in the list."));
    assert_eq!(list.input_mut(), &d.a.display().to_string());

    *list.input_mut() = d.b.join("missing").display().to_string();
    assert_eq!(list.add_typed(), Err(AddLibraryError::NotFound));
    assert_eq!(list.error(), Some("That folder does not exist."));

    *list.input_mut() = String::new();
    assert_eq!(list.add_typed(), Err(AddLibraryError::Empty));
    assert_eq!(list.roots().len(), 2);
}

#[test]
fn a_success_after_an_error_clears_the_error() {
    let d = dirs();
    let mut list = list_of(&d, &[]);
    assert!(list.add_typed().is_err());
    assert!(list.error().is_some());

    *list.input_mut() = d.b.display().to_string();
    assert!(list.add_typed().is_ok());

    assert_eq!(list.error(), None);
}

#[test]
fn clearing_the_error_removes_the_message() {
    let d = dirs();
    let mut list = list_of(&d, &[]);
    assert!(list.add_typed().is_err());

    list.clear_error();

    assert_eq!(list.error(), None);
}

#[test]
fn removing_an_added_library_moves_later_slots_up_and_clears_the_filter() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a, &d.b]);
    list.set_filter(Some(2));

    let removed = list.remove(1);

    assert_eq!(
        removed,
        Some(Removed {
            root: d.a.clone(),
            still_listed: false
        })
    );
    assert_eq!(list.roots(), [d.beside.clone(), d.b.clone()]);
    assert_eq!(list.filter(), None);
}

#[test]
fn the_folder_beside_the_program_and_unknown_slots_cannot_be_removed() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a]);
    list.set_filter(Some(1));

    assert_eq!(list.remove(0), None);
    assert_eq!(list.remove(5), None);

    assert_eq!(list.count(), 2);
    assert_eq!(list.filter(), Some(1));
}

#[test]
fn a_picked_folder_is_checked_like_the_same_typed_path() {
    let d = dirs();
    let mut list = list_of(&d, &[&d.a]);

    assert_eq!(list.add_picked(&d.b), Ok(d.b.clone()));
    assert_eq!(list.roots(), [d.beside.clone(), d.a.clone(), d.b.clone()]);
    assert_eq!(list.add_picked(&d.a), Err(AddLibraryError::AlreadyListed));
    assert_eq!(list.error(), Some("That library is already in the list."));
    assert_eq!(
        list.add_picked(&d.b.join("missing")),
        Err(AddLibraryError::NotFound)
    );
}

#[test]
fn picking_leaves_the_typed_text_alone() {
    let d = dirs();
    let mut list = list_of(&d, &[]);
    *list.input_mut() = "half a pa".to_owned();

    assert!(list.add_picked(&d.a).is_ok());

    assert_eq!(list.input_mut(), "half a pa");
}

#[test]
fn the_browser_starts_at_home_or_else_the_root() {
    use dackel_launcher::library::browser_start;
    use std::path::{Path, PathBuf};

    assert_eq!(
        browser_start(Some(Path::new("/home/me"))),
        PathBuf::from("/home/me")
    );
    assert_eq!(browser_start(None), PathBuf::from("/"));
}
