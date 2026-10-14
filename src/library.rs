use std::io;
use std::path::{Path, PathBuf};

const LIBRARY_DIR: &str = "DLSiteLibrary";

pub fn resolve_library(exe_path: &Path, override_dir: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(dir) = override_dir {
        return Ok(dir.to_path_buf());
    }
    let real = exe_path.canonicalize()?;
    let exe_dir = real.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "executable has no parent directory",
        )
    })?;
    Ok(exe_dir.join(LIBRARY_DIR))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub id: String,
    pub dir: PathBuf,
}

impl Game {
    /// Identifies this game among all libraries: the same RJ code can sit in two of them.
    pub fn key(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }
}

/// What scanning one library root found.
#[derive(Debug)]
pub struct LibraryScan {
    pub root: PathBuf,
    /// An unplugged or deleted library is an `Err`, and never stops the others.
    pub games: io::Result<Vec<Game>>,
}

/// Scans every library, in the order given.
pub fn scan_all(roots: &[PathBuf]) -> Vec<LibraryScan> {
    roots
        .iter()
        .map(|root| LibraryScan {
            root: root.clone(),
            games: scan(root),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddLibraryError {
    Empty,
    NotFound,
    NotADirectory,
    AlreadyListed,
}

impl std::fmt::Display for AddLibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AddLibraryError::Empty => "Type a folder path first.",
            AddLibraryError::NotFound => "That folder does not exist.",
            AddLibraryError::NotADirectory => "That path is a file, not a folder.",
            AddLibraryError::AlreadyListed => "That library is already in the list.",
        })
    }
}

/// Checks a typed path and returns the folder to remember. `listed` are the libraries
/// already in use; a listed library that is offline does not matter here.
pub fn add_library(listed: &[PathBuf], typed: &str) -> Result<PathBuf, AddLibraryError> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err(AddLibraryError::Empty);
    }
    let path = Path::new(typed);
    if !path.exists() {
        return Err(AddLibraryError::NotFound);
    }
    if !path.is_dir() {
        return Err(AddLibraryError::NotADirectory);
    }
    let real = path.canonicalize().map_err(|_| AddLibraryError::NotFound)?;
    if listed
        .iter()
        .any(|l| l == &real || l.canonicalize().is_ok_and(|c| c == real))
    {
        return Err(AddLibraryError::AlreadyListed);
    }
    Ok(real)
}

/// Short names for a list of libraries: the folder name, or the full path when two
/// libraries share a folder name.
pub fn library_labels(roots: &[PathBuf]) -> Vec<String> {
    let name = |root: &Path| root.file_name().map(|n| n.to_string_lossy().into_owned());
    roots
        .iter()
        .map(|root| match name(root) {
            Some(n)
                if roots
                    .iter()
                    .filter(|r| name(r).as_deref() == Some(&n))
                    .count()
                    == 1 =>
            {
                n
            }
            _ => root.display().to_string(),
        })
        .collect()
}

pub fn scan(library_dir: &Path) -> io::Result<Vec<Game>> {
    let mut games = Vec::new();
    for entry in std::fs::read_dir(library_dir)? {
        let path = entry?.path();
        let Some(id) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_game_id(id) && path.is_dir() {
            games.push(Game {
                id: id.to_owned(),
                dir: path,
            });
        }
    }
    games.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(games)
}

fn is_game_id(name: &str) -> bool {
    name.strip_prefix("RJ")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}
