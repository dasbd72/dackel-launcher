//! What one frame of drawing asked for. The drawing code fills an [`Actions`] in and does
//! not touch the app; the app applies it once the frame has been drawn.

use crate::library::Game;
use crate::pickers::Picker;
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq)]
pub struct Actions {
    // The toolbar.
    pub rescan: bool,
    pub fullscreen: bool,
    pub toggle_blur: bool,

    // The page shown when there is no library.
    pub create_library: bool,
    pub open_libraries: bool,

    // The game grid and its tiles.
    /// A tile was clicked or one of its buttons was pressed: the cursor moves there.
    pub select: Option<String>,
    /// Index into the list of games.
    pub launch: Option<usize>,
    pub extract: Option<PathBuf>,
    pub cancel_extract: Vec<String>,
    pub open_details: Option<String>,
    pub favorite: Option<String>,

    // The details page.
    pub back: bool,
    pub reload: Option<Game>,
    pub details_extract: Option<PathBuf>,
    /// A game folder and the exe picked for it.
    pub details_choice: Option<(PathBuf, PathBuf)>,

    // The popups.
    pub choose_exe: Option<(PathBuf, PathBuf)>,
    pub close_chooser: bool,
    pub pick: Option<(Picker, usize)>,
    pub cancel_picker: bool,
}

/// The one action that changes the game list or starts work on a folder. Only one is carried
/// out per frame, the first of these in order.
#[derive(Debug, PartialEq, Eq)]
pub enum Exclusive {
    ChooseExe { dir: PathBuf, exe: PathBuf },
    Extract(PathBuf),
    OpenLibraries,
    CreateLibrary,
    Rescan,
}

impl Actions {
    /// Takes the action that wins this frame: an exe choice, then an extraction (a tile's before
    /// the details page's), then the libraries window, then creating the library, then a rescan.
    pub fn take_exclusive(&mut self) -> Option<Exclusive> {
        let choice = self.details_choice.take().or(self.choose_exe.take());
        let extract = self.extract.take().or(self.details_extract.take());
        if let Some((dir, exe)) = choice {
            Some(Exclusive::ChooseExe { dir, exe })
        } else if let Some(dir) = extract {
            Some(Exclusive::Extract(dir))
        } else if self.open_libraries {
            Some(Exclusive::OpenLibraries)
        } else if self.create_library {
            Some(Exclusive::CreateLibrary)
        } else if self.rescan {
            Some(Exclusive::Rescan)
        } else {
            None
        }
    }
}
