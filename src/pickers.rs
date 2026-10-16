//! The toolbar dropdowns: sort, zip encoding, UI size, press debounce and library filter.
//!
//! Each one is described here once: its title, its entries, which entry is current, and what
//! picking an entry chooses. The window that draws the list stays in the binary.

use crate::browse::Sort;
use crate::extract::Encoding;
use crate::library::library_labels;
use crate::nav::{Control, DEBOUNCE_PRESETS_MS};
use crate::window::SCALE_PRESETS;
use std::path::PathBuf;

/// The Proton list's first entry: no choice, so the newest install is used.
pub const AUTOMATIC_PROTON: &str = "Automatic (newest GE)";

pub const SORTS: [Sort; 3] = [Sort::LastPlayed, Sort::Title, Sort::DateAdded];
pub const ENCODINGS: [Encoding; 3] = [Encoding::Auto, Encoding::Cp932, Encoding::Utf8];

/// A small list opened from a toolbar dropdown so the controller can choose from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picker {
    Sort,
    Encoding,
    Scale,
    Debounce,
    Library,
    /// The Proton version of the game on the details page.
    Proton,
}

impl Picker {
    /// The picker a focused toolbar control opens when pressed, if it is a dropdown.
    pub fn for_control(control: Control) -> Option<Picker> {
        match control {
            Control::Sort => Some(Picker::Sort),
            Control::Encoding => Some(Picker::Encoding),
            Control::Scale => Some(Picker::Scale),
            Control::Debounce => Some(Picker::Debounce),
            Control::LibraryFilter => Some(Picker::Library),
            Control::ProtonChoice => Some(Picker::Proton),
            _ => None,
        }
    }
}

/// What a dropdown popup shows: its title, its entries, and which one is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerList {
    pub title: &'static str,
    pub labels: Vec<String>,
    pub current: usize,
}

/// The values the dropdowns choose between, as they are now.
#[derive(Debug, Clone, Copy)]
pub struct Selection<'a> {
    pub sort: Sort,
    pub encoding: Encoding,
    pub scale: f32,
    pub debounce_ms: u64,
    pub libraries: &'a [PathBuf],
    pub library_filter: Option<usize>,
    /// The Proton versions installed here, for the open game.
    pub proton_versions: &'a [String],
    /// The open game's chosen version; `None` is automatic.
    pub proton_pin: Option<&'a str>,
}

/// What picking an entry chooses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice {
    Sort(Sort),
    Encoding(Encoding),
    Scale(f32),
    Debounce(u64),
    /// `None` shows every library.
    LibraryFilter(Option<usize>),
    /// A slot of `Selection::proton_versions`; `None` is automatic.
    Proton(Option<usize>),
}

pub fn sort_label(sort: Sort) -> &'static str {
    match sort {
        Sort::LastPlayed => "Last played",
        Sort::Title => "Title",
        Sort::DateAdded => "Date added",
    }
}

pub fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Auto => "Auto",
        Encoding::Cp932 => "cp932",
        Encoding::Utf8 => "UTF-8",
    }
}

pub fn debounce_label(ms: u64) -> String {
    match ms {
        0 => "Off".to_owned(),
        ms => format!("{ms} ms"),
    }
}

/// The entries of `picker` and which one is current. A value that is none of the entries
/// marks the first one.
pub fn list(picker: Picker, now: &Selection) -> PickerList {
    match picker {
        Picker::Sort => PickerList {
            title: "Sort",
            labels: SORTS.iter().map(|s| sort_label(*s).to_owned()).collect(),
            current: SORTS.iter().position(|s| *s == now.sort).unwrap_or(0),
        },
        Picker::Encoding => PickerList {
            title: "Zip encoding",
            labels: ENCODINGS
                .iter()
                .map(|e| encoding_label(*e).to_owned())
                .collect(),
            current: ENCODINGS
                .iter()
                .position(|e| *e == now.encoding)
                .unwrap_or(0),
        },
        Picker::Library => {
            let mut labels = vec!["All libraries".to_owned()];
            labels.extend(library_labels(now.libraries));
            PickerList {
                title: "Library",
                labels,
                current: now.library_filter.map_or(0, |slot| slot + 1),
            }
        }
        Picker::Debounce => PickerList {
            title: "Press debounce",
            labels: DEBOUNCE_PRESETS_MS
                .iter()
                .map(|ms| debounce_label(*ms))
                .collect(),
            current: DEBOUNCE_PRESETS_MS
                .iter()
                .position(|ms| *ms == now.debounce_ms)
                .unwrap_or(0),
        },
        Picker::Proton => {
            let mut labels = vec![AUTOMATIC_PROTON.to_owned()];
            labels.extend(now.proton_versions.iter().cloned());
            let current = match now.proton_pin {
                None => 0,
                Some(pin) => match now.proton_versions.iter().position(|v| v == pin) {
                    Some(slot) => slot + 1,
                    None => {
                        labels.push(format!("{pin} (not installed)"));
                        labels.len() - 1
                    }
                },
            };
            PickerList {
                title: "Proton",
                labels,
                current,
            }
        }
        Picker::Scale => PickerList {
            title: "Size on screen",
            labels: SCALE_PRESETS.iter().map(|p| format!("{p:.2}")).collect(),
            current: SCALE_PRESETS
                .iter()
                .position(|p| (now.scale - p).abs() < 1e-3)
                .unwrap_or(0),
        },
    }
}

/// What picking entry `index` of `picker` chooses, or `None` when there is no such entry.
/// The library list is the exception: an entry past its end shows every library, as the
/// first entry does. `library_count` is how many libraries are listed.
pub fn pick(picker: Picker, index: usize, library_count: usize) -> Option<Choice> {
    match picker {
        Picker::Sort => SORTS.get(index).copied().map(Choice::Sort),
        Picker::Encoding => ENCODINGS.get(index).copied().map(Choice::Encoding),
        Picker::Library => Some(Choice::LibraryFilter(
            index.checked_sub(1).filter(|slot| *slot < library_count),
        )),
        Picker::Debounce => DEBOUNCE_PRESETS_MS
            .get(index)
            .copied()
            .map(Choice::Debounce),
        Picker::Scale => SCALE_PRESETS.get(index).copied().map(Choice::Scale),
        Picker::Proton => Some(Choice::Proton(index.checked_sub(1))),
    }
}
