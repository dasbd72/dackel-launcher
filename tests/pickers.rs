//! The dropdowns the controller opens as lists (sort, library filter, a game's Proton): what each
//! one lists, which entry is current, and what picking an entry changes.

use dackel_launcher::browse::Sort;
use dackel_launcher::nav::Control;
use dackel_launcher::pickers::{Choice, Picker, Selection, list, pick};
use std::path::PathBuf;

fn libraries(names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|n| PathBuf::from("/games").join(n))
        .collect()
}

fn selection(libraries: &[PathBuf]) -> Selection<'_> {
    Selection {
        sort: Sort::LastPlayed,
        libraries,
        library_filter: None,
        proton_versions: &[],
        proton_pin: None,
    }
}

mod sort {
    use super::*;

    #[test]
    fn lists_the_three_orders_with_the_current_one_marked() {
        let libs = libraries(&[]);
        let now = Selection {
            sort: Sort::Title,
            ..selection(&libs)
        };

        let l = list(Picker::Sort, &now);

        assert_eq!(l.title, "Sort");
        assert_eq!(l.labels, ["Last played", "Title", "Date added"]);
        assert_eq!(l.current, 1);
    }

    #[test]
    fn picking_an_entry_chooses_that_order() {
        assert_eq!(
            pick(Picker::Sort, 2, 0),
            Some(Choice::Sort(Sort::DateAdded))
        );
        assert_eq!(
            pick(Picker::Sort, 0, 0),
            Some(Choice::Sort(Sort::LastPlayed))
        );
    }

    #[test]
    fn picking_past_the_end_changes_nothing() {
        assert_eq!(pick(Picker::Sort, 3, 0), None);
    }
}

mod library {
    use super::*;

    #[test]
    fn lists_all_libraries_then_each_library_by_name() {
        let libs = libraries(&["main", "sd"]);
        let now = Selection {
            library_filter: Some(1),
            ..selection(&libs)
        };

        let l = list(Picker::Library, &now);

        assert_eq!(l.title, "Library");
        assert_eq!(l.labels, ["All libraries", "main", "sd"]);
        assert_eq!(l.current, 2);
    }

    #[test]
    fn no_filter_marks_all_libraries() {
        let libs = libraries(&["main", "sd"]);

        assert_eq!(list(Picker::Library, &selection(&libs)).current, 0);
    }

    #[test]
    fn two_libraries_with_one_folder_name_are_told_apart_by_full_path() {
        let libs = vec![PathBuf::from("/a/games"), PathBuf::from("/b/games")];

        let l = list(Picker::Library, &selection(&libs));

        assert_eq!(l.labels, ["All libraries", "/a/games", "/b/games"]);
    }

    #[test]
    fn picking_the_first_entry_clears_the_filter() {
        assert_eq!(
            pick(Picker::Library, 0, 2),
            Some(Choice::LibraryFilter(None))
        );
    }

    #[test]
    fn picking_a_library_filters_to_its_slot() {
        assert_eq!(
            pick(Picker::Library, 1, 2),
            Some(Choice::LibraryFilter(Some(0)))
        );
        assert_eq!(
            pick(Picker::Library, 2, 2),
            Some(Choice::LibraryFilter(Some(1)))
        );
    }

    #[test]
    fn picking_past_the_end_clears_the_filter() {
        assert_eq!(
            pick(Picker::Library, 3, 2),
            Some(Choice::LibraryFilter(None))
        );
    }
}

mod controls {
    use super::*;

    #[test]
    fn each_dropdown_control_opens_its_own_picker() {
        assert_eq!(Picker::for_control(Control::Sort), Some(Picker::Sort));
        assert_eq!(
            Picker::for_control(Control::LibraryFilter),
            Some(Picker::Library)
        );
    }

    #[test]
    fn other_controls_open_no_picker() {
        assert_eq!(Picker::for_control(Control::Rescan), None);
        assert_eq!(Picker::for_control(Control::Quit), None);
        assert_eq!(Picker::for_control(Control::Blur), None);
        assert_eq!(Picker::for_control(Control::Settings), None);
    }
}

mod proton {
    use super::*;

    fn versions() -> Vec<String> {
        vec!["GE-Proton10-1".to_owned(), "GE-Proton9-27".to_owned()]
    }

    #[test]
    fn lists_automatic_then_every_version_with_no_pin_on_automatic() {
        let libs = libraries(&[]);
        let installed = versions();
        let now = Selection {
            proton_versions: &installed,
            ..selection(&libs)
        };

        let l = list(Picker::Proton, &now);

        assert_eq!(
            l.labels,
            ["Automatic (newest GE)", "GE-Proton10-1", "GE-Proton9-27"]
        );
        assert_eq!(l.current, 0);
    }

    #[test]
    fn marks_the_pinned_version_current() {
        let libs = libraries(&[]);
        let installed = versions();
        let now = Selection {
            proton_versions: &installed,
            proton_pin: Some("GE-Proton9-27"),
            ..selection(&libs)
        };

        assert_eq!(list(Picker::Proton, &now).current, 2);
    }

    #[test]
    fn a_pin_that_is_not_installed_is_added_last_and_marked_current() {
        let libs = libraries(&[]);
        let installed = versions();
        let now = Selection {
            proton_versions: &installed,
            proton_pin: Some("GE-Proton8-1"),
            ..selection(&libs)
        };

        let l = list(Picker::Proton, &now);

        assert_eq!(l.labels.last().unwrap(), "GE-Proton8-1 (not installed)");
        assert_eq!(l.current, 3);
    }

    #[test]
    fn picking_the_first_entry_is_automatic_and_the_rest_are_versions_by_slot() {
        assert_eq!(pick(Picker::Proton, 0, 0), Some(Choice::Proton(None)));
        assert_eq!(pick(Picker::Proton, 2, 0), Some(Choice::Proton(Some(1))));
    }

    #[test]
    fn the_control_opens_the_proton_list() {
        assert_eq!(
            Picker::for_control(Control::ProtonChoice),
            Some(Picker::Proton)
        );
    }
}
