//! The one popup that can be open over the library or the details page.
//!
//! It is a single value, so two popups cannot be open at once, and the questions the
//! controller needs answered (how long is the open list, what does A choose, does the stick
//! scroll it) are asked of it in one place. The windows that draw each popup stay in the binary.

use crate::pickers::Picker;
use std::path::PathBuf;

/// The answers on the overwrite prompt, in the order they are listed and selected.
pub const OVERWRITE_CHOICES: [&str; 3] = ["Cancel", "Overwrite", "Skip existing"];

/// A launch waiting for the user to decide what to do about a missing Proton version.
pub struct ProtonPrompt {
    pub dir: PathBuf,
    pub pinned: String,
    pub newest: Option<PathBuf>,
    /// Showing the list of installed versions to pick from.
    pub choosing: bool,
}

/// Files an extraction would replace, shown for the user to decide.
pub struct OverwritePrompt {
    pub dir: PathBuf,
    /// Relative to the game folder.
    pub files: Vec<PathBuf>,
    pub search: String,
}

pub enum Popup {
    /// Choosing the exe of the game at this position in the list.
    Chooser(usize),
    /// A toolbar dropdown.
    Picker(Picker),
    /// The manager for the added libraries.
    Libraries,
    /// The window for the settings that are rarely changed.
    Settings,
    Proton(ProtonPrompt),
    Overwrite(OverwritePrompt),
}

/// What pressing A on entry `n` of the open popup chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickTarget {
    /// Candidate `candidate` for the game at `chooser`.
    Exe {
        chooser: usize,
        candidate: usize,
    },
    Picker(Picker, usize),
    Proton(usize),
    Overwrite(usize),
    /// A popup with no list to choose from.
    Nothing,
}

impl Popup {
    /// How many entries the open list has. Each popup asks only for its own size: the
    /// candidates of a game, the entries of a picker, or what the Proton prompt offers now.
    pub fn list_len(
        &self,
        candidates: impl FnOnce(usize) -> usize,
        picker_entries: impl FnOnce(Picker) -> usize,
        proton_options: impl FnOnce(&ProtonPrompt) -> usize,
    ) -> usize {
        match self {
            Popup::Overwrite(_) => OVERWRITE_CHOICES.len(),
            Popup::Proton(prompt) => proton_options(prompt),
            Popup::Picker(picker) => picker_entries(*picker),
            Popup::Chooser(game) => candidates(*game),
            Popup::Libraries | Popup::Settings => 0,
        }
    }

    pub fn pick(&self, selected: usize) -> PickTarget {
        match self {
            Popup::Overwrite(_) => PickTarget::Overwrite(selected),
            Popup::Proton(_) => PickTarget::Proton(selected),
            Popup::Picker(picker) => PickTarget::Picker(*picker, selected),
            Popup::Chooser(game) => PickTarget::Exe {
                chooser: *game,
                candidate: selected,
            },
            Popup::Libraries | Popup::Settings => PickTarget::Nothing,
        }
    }

    /// Whether pushing the left stick up or down scrolls this popup's list of files.
    pub fn scrolls_with_stick(&self) -> bool {
        matches!(self, Popup::Overwrite(_))
    }

    pub fn as_proton(&self) -> Option<&ProtonPrompt> {
        match self {
            Popup::Proton(prompt) => Some(prompt),
            _ => None,
        }
    }

    pub fn as_proton_mut(&mut self) -> Option<&mut ProtonPrompt> {
        match self {
            Popup::Proton(prompt) => Some(prompt),
            _ => None,
        }
    }

    pub fn as_overwrite(&self) -> Option<&OverwritePrompt> {
        match self {
            Popup::Overwrite(prompt) => Some(prompt),
            _ => None,
        }
    }

    pub fn as_overwrite_mut(&mut self) -> Option<&mut OverwritePrompt> {
        match self {
            Popup::Overwrite(prompt) => Some(prompt),
            _ => None,
        }
    }
}
