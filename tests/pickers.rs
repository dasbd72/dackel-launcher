//! The toolbar dropdowns (sort, zip encoding, UI scale, debounce, library filter): what each
//! one lists, which entry is current, and what picking an entry changes.

use dackel_launcher::browse::Sort;
use dackel_launcher::extract::Encoding;
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
        encoding: Encoding::Auto,
        scale: 1.0,
        debounce_ms: 25,
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

mod encoding {
    use super::*;

    #[test]
    fn lists_auto_cp932_and_utf8_in_that_order() {
        let libs = libraries(&[]);
        let now = Selection {
            encoding: Encoding::Utf8,
            ..selection(&libs)
        };

        let l = list(Picker::Encoding, &now);

        assert_eq!(l.title, "Zip encoding");
        assert_eq!(l.labels, ["Auto", "cp932", "UTF-8"]);
        assert_eq!(l.current, 2);
    }

    #[test]
    fn picking_an_entry_chooses_that_encoding() {
        assert_eq!(
            pick(Picker::Encoding, 1, 0),
            Some(Choice::Encoding(Encoding::Cp932))
        );
        assert_eq!(
            pick(Picker::Encoding, 2, 0),
            Some(Choice::Encoding(Encoding::Utf8))
        );
    }

    #[test]
    fn picking_past_the_end_changes_nothing() {
        assert_eq!(pick(Picker::Encoding, 3, 0), None);
    }
}

mod scale {
    use super::*;

    #[test]
    fn lists_the_presets_with_two_decimals() {
        let libs = libraries(&[]);
        let now = Selection {
            scale: 1.5,
            ..selection(&libs)
        };

        let l = list(Picker::Scale, &now);

        assert_eq!(l.title, "UI scale");
        assert_eq!(l.labels, ["0.50", "0.75", "1.00", "1.25", "1.50", "1.75"]);
        assert_eq!(l.current, 4);
    }

    #[test]
    fn a_scale_within_a_thousandth_of_a_preset_counts_as_that_preset() {
        let libs = libraries(&[]);
        let now = Selection {
            scale: 1.2504,
            ..selection(&libs)
        };

        assert_eq!(list(Picker::Scale, &now).current, 3);
    }

    #[test]
    fn a_scale_between_presets_marks_the_first_entry() {
        let libs = libraries(&[]);
        let now = Selection {
            scale: 1.1,
            ..selection(&libs)
        };

        assert_eq!(list(Picker::Scale, &now).current, 0);
    }

    #[test]
    fn picking_an_entry_chooses_that_scale() {
        assert_eq!(pick(Picker::Scale, 0, 0), Some(Choice::Scale(0.5)));
        assert_eq!(pick(Picker::Scale, 5, 0), Some(Choice::Scale(1.75)));
    }

    #[test]
    fn picking_past_the_end_changes_nothing() {
        assert_eq!(pick(Picker::Scale, 6, 0), None);
    }
}

mod debounce {
    use super::*;

    #[test]
    fn lists_off_and_the_millisecond_presets() {
        let libs = libraries(&[]);
        let now = Selection {
            debounce_ms: 100,
            ..selection(&libs)
        };

        let l = list(Picker::Debounce, &now);

        assert_eq!(l.title, "Press debounce");
        assert_eq!(l.labels, ["Off", "25 ms", "50 ms", "100 ms", "150 ms"]);
        assert_eq!(l.current, 3);
    }

    #[test]
    fn a_window_that_is_no_preset_marks_the_first_entry() {
        let libs = libraries(&[]);
        let now = Selection {
            debounce_ms: 40,
            ..selection(&libs)
        };

        assert_eq!(list(Picker::Debounce, &now).current, 0);
    }

    #[test]
    fn picking_an_entry_chooses_that_window() {
        assert_eq!(pick(Picker::Debounce, 0, 0), Some(Choice::Debounce(0)));
        assert_eq!(pick(Picker::Debounce, 4, 0), Some(Choice::Debounce(150)));
    }

    #[test]
    fn picking_past_the_end_changes_nothing() {
        assert_eq!(pick(Picker::Debounce, 5, 0), None);
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
            Picker::for_control(Control::Encoding),
            Some(Picker::Encoding)
        );
        assert_eq!(Picker::for_control(Control::Scale), Some(Picker::Scale));
        assert_eq!(
            Picker::for_control(Control::Debounce),
            Some(Picker::Debounce)
        );
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
