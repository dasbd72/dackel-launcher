//! The libraries in use: the folder beside the program, the ones added on this machine,
//! which of them could not be read, the library filter, and the "add a folder" form.

use crate::library::{
    AddLibraryError, add_library, library_labels, may_create_library, unique_roots,
};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LibraryList {
    /// The library beside the binary (or `--library`); always first in `roots`.
    beside: PathBuf,
    /// Whether "Create it" is offered for the folder beside the program.
    can_create: bool,
    /// Every library in use: `beside`, then the ones added on this machine.
    roots: Vec<PathBuf>,
    /// Libraries that could not be read at the last scan (drive unplugged, folder gone).
    offline: Vec<PathBuf>,
    /// Show only games from this entry of `roots`.
    filter: Option<usize>,
    /// What was typed in the "add a folder" box.
    input: String,
    error: Option<String>,
}

/// What `remove` took out of the list.
#[derive(Debug, PartialEq, Eq)]
pub struct Removed {
    pub root: PathBuf,
    /// The same folder is still listed under another slot.
    pub still_listed: bool,
}

impl LibraryList {
    /// A folder listed twice (say, the one beside the program added again) shows once.
    pub fn new(beside: PathBuf, added: &[PathBuf]) -> Self {
        let all: Vec<PathBuf> = std::iter::once(beside.clone())
            .chain(added.iter().cloned())
            .collect();
        Self {
            can_create: may_create_library(&beside),
            roots: unique_roots(&all),
            beside,
            offline: Vec::new(),
            filter: None,
            input: String::new(),
            error: None,
        }
    }

    pub fn beside(&self) -> &Path {
        &self.beside
    }

    pub fn can_create(&self) -> bool {
        self.can_create
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// The libraries added on this machine, which the settings remember.
    pub fn added(&self) -> &[PathBuf] {
        &self.roots[1..]
    }

    pub fn count(&self) -> usize {
        self.roots.len()
    }

    pub fn offline(&self) -> &[PathBuf] {
        &self.offline
    }

    pub fn set_offline(&mut self, offline: Vec<PathBuf>) {
        self.offline = offline;
    }

    pub fn is_offline(&self, root: &Path) -> bool {
        self.offline.iter().any(|o| o == root)
    }

    /// Every library failed to scan.
    pub fn all_offline(&self) -> bool {
        self.offline.len() == self.roots.len()
    }

    /// The libraries the manager lists, with their slots. The one beside the program is
    /// listed only when it was found; added libraries are listed even when offline.
    pub fn listed(&self) -> Vec<(usize, &PathBuf)> {
        self.roots
            .iter()
            .enumerate()
            .filter(|(i, root)| *i > 0 || !self.is_offline(root))
            .collect()
    }

    pub fn filter(&self) -> Option<usize> {
        self.filter
    }

    pub fn set_filter(&mut self, slot: Option<usize>) {
        self.filter = slot;
    }

    /// Whether a game from `slot` passes the library filter.
    pub fn shows_slot(&self, slot: usize) -> bool {
        self.filter.is_none_or(|f| f == slot)
    }

    /// The name to show beside a game's library; `None` with a single library.
    pub fn label_of(&self, slot: usize) -> Option<String> {
        if self.roots.len() < 2 {
            return None;
        }
        library_labels(&self.roots).into_iter().nth(slot)
    }

    pub fn input_mut(&mut self) -> &mut String {
        &mut self.input
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Adds the typed folder. On success it is listed, the box is emptied and the folder is
    /// returned; otherwise the reason is kept to show beside the box.
    pub fn add_typed(&mut self) -> Result<PathBuf, AddLibraryError> {
        match add_library(&self.roots, &self.input) {
            Ok(path) => {
                self.roots.push(path.clone());
                self.input.clear();
                self.error = None;
                Ok(path)
            }
            Err(err) => {
                self.error = Some(err.to_string());
                Err(err)
            }
        }
    }

    /// Takes an added library out of the list and clears the filter, since later slots move
    /// up. The one beside the program, and a slot that is not listed, cannot be removed.
    pub fn remove(&mut self, slot: usize) -> Option<Removed> {
        if slot == 0 || slot >= self.roots.len() {
            return None;
        }
        let root = self.roots.remove(slot);
        self.filter = None;
        let still_listed = self.roots.contains(&root);
        Some(Removed { root, still_listed })
    }
}
