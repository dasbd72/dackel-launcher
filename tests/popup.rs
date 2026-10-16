//! The one popup that can be open: how long its list is, what picking an entry means, whether
//! the stick scrolls it, and how the two prompts are reached.

use dackel_launcher::pickers::Picker;
use dackel_launcher::popup::{OVERWRITE_CHOICES, OverwritePrompt, PickTarget, Popup, ProtonPrompt};
use std::path::PathBuf;

fn proton() -> Popup {
    Popup::Proton(ProtonPrompt {
        dir: PathBuf::from("/games/RJ1"),
        pinned: "GE-Proton9-1".to_owned(),
        newest: None,
        choosing: false,
    })
}

fn overwrite() -> Popup {
    Popup::Overwrite(OverwritePrompt {
        dir: PathBuf::from("/games/RJ1"),
        files: vec![PathBuf::from("a.txt")],
        search: String::new(),
    })
}

/// The sizes the caller would work out; each popup asks only for its own.
fn len(popup: &Popup) -> usize {
    popup.list_len(|_chooser| 4, |_picker| 7, |_prompt| 2)
}

#[test]
fn the_overwrite_prompt_lists_its_fixed_choices() {
    assert_eq!(OVERWRITE_CHOICES, ["Cancel", "Overwrite", "Skip existing"]);
    assert_eq!(len(&overwrite()), 3);
}

#[test]
fn the_proton_prompt_lists_what_it_offers_now() {
    assert_eq!(len(&proton()), 2);
}

#[test]
fn a_picker_lists_its_own_entries() {
    assert_eq!(len(&Popup::Picker(Picker::Sort)), 7);
}

#[test]
fn the_exe_chooser_lists_the_candidates_of_its_game() {
    let asked = std::cell::Cell::new(None);
    let n = Popup::Chooser(3).list_len(
        |game| {
            asked.set(Some(game));
            4
        },
        |_| 0,
        |_| 0,
    );
    assert_eq!((n, asked.get()), (4, Some(3)));
}

#[test]
fn the_libraries_window_has_no_list() {
    assert_eq!(len(&Popup::Libraries), 0);
    assert_eq!(len(&Popup::Settings), 0);
}

#[test]
fn each_popup_asks_only_for_its_own_size() {
    // A size that panics when asked for proves the other sizes are never worked out.
    let boom = |_: usize| -> usize { panic!("not this popup's size") };
    assert_eq!(
        Popup::Picker(Picker::Sort).list_len(boom, |_| 5, |_| panic!()),
        5
    );
    assert_eq!(overwrite().list_len(boom, |_| panic!(), |_| panic!()), 3);
}

#[test]
fn picking_means_what_the_open_popup_says() {
    assert_eq!(overwrite().pick(1), PickTarget::Overwrite(1));
    assert_eq!(proton().pick(0), PickTarget::Proton(0));
    assert_eq!(
        Popup::Picker(Picker::Library).pick(2),
        PickTarget::Picker(Picker::Library, 2)
    );
    assert_eq!(
        Popup::Chooser(5).pick(1),
        PickTarget::Exe {
            chooser: 5,
            candidate: 1
        }
    );
}

#[test]
fn picking_in_the_libraries_window_does_nothing() {
    assert_eq!(Popup::Libraries.pick(0), PickTarget::Nothing);
    assert_eq!(Popup::Settings.pick(0), PickTarget::Nothing);
}

#[test]
fn only_the_overwrite_prompt_scrolls_with_the_stick() {
    assert!(overwrite().scrolls_with_stick());
    for popup in [
        proton(),
        Popup::Chooser(0),
        Popup::Picker(Picker::Sort),
        Popup::Libraries,
    ] {
        assert!(!popup.scrolls_with_stick());
    }
}

#[test]
fn a_prompt_is_reached_only_through_its_own_popup() {
    assert!(proton().as_proton().is_some());
    assert!(overwrite().as_proton().is_none());
    assert!(overwrite().as_overwrite().is_some());
    assert!(proton().as_overwrite().is_none());
    assert!(Popup::Libraries.as_proton().is_none());

    let mut popup = proton();
    popup.as_proton_mut().unwrap().choosing = true;
    assert!(popup.as_proton().unwrap().choosing);

    let mut popup = overwrite();
    popup.as_overwrite_mut().unwrap().search.push_str("txt");
    assert_eq!(popup.as_overwrite().unwrap().search, "txt");
}
