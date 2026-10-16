use std::io;
use std::path::{Path, PathBuf};

const LIBRARY_DIR: &str = "DLSiteLibrary";

/// The hidden folder inside each game folder that holds everything the launcher keeps for
/// that game: profile, info, cover, logs and the Wine prefix.
pub const META_DIR: &str = ".dackel-launcher";

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

/// Folders a packaged program is installed into. Nothing is created there, even when the
/// permissions would allow it.
const SYSTEM_DIRS: &[&str] = &["/usr", "/bin", "/sbin", "/lib", "/lib64", "/opt", "/app"];

/// Whether the launcher may offer to create `library` (the folder beside the program).
/// Only when its parent is writable and not a system folder; a packaged install never
/// offers to write into its own install directory.
pub fn may_create_library(library: &Path) -> bool {
    let Some(parent) = library.parent() else {
        return false;
    };
    if SYSTEM_DIRS.iter().any(|dir| parent.starts_with(dir)) {
        return false;
    }
    writable_by_me(parent)
}

/// Whether the current user may write into `dir`, as the kernel decides (owner, group and
/// other bits, and read-only mounts). Nothing is written.
fn writable_by_me(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a valid NUL-terminated string for the duration of the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub id: String,
    pub dir: PathBuf,
    /// Which entry of the library list the game came from.
    pub slot: usize,
}

impl Game {
    /// Identifies this game on screen among all libraries and slots. Opaque: not a path.
    pub fn key(&self) -> String {
        match self.slot {
            0 => self.dir_key(),
            slot => format!("#{slot} {}", self.dir_key()),
        }
    }

    /// Identifies the game's folder. Two slots of one folder share it, so work on the
    /// folder (extracting, running) is tracked once and shows on both.
    pub fn dir_key(&self) -> String {
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
        .enumerate()
        .map(|(slot, root)| LibraryScan {
            root: root.clone(),
            games: scan(root).map(|games| {
                games
                    .into_iter()
                    .map(|game| Game { slot, ..game })
                    .collect()
            }),
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

/// The folder the browser opens in: the home folder, or the root when home is unknown.
pub fn browser_start(home: Option<&Path>) -> PathBuf {
    home.map_or_else(|| PathBuf::from("/"), Path::to_path_buf)
}

/// The libraries to scan: the list with repeats dropped, the first of each kept. Two
/// spellings of one folder (a trailing slash, a symlink) count as the same folder.
pub fn unique_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let real = |root: &PathBuf| root.canonicalize().unwrap_or_else(|_| root.clone());
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut kept = Vec::new();
    for root in roots {
        let key = real(root);
        if !seen.contains(&key) {
            seen.push(key);
            kept.push(root.clone());
        }
    }
    kept
}

/// Short names for a list of libraries: the folder name, or the full path when two
/// libraries share a folder name. (The app lists each folder once; a repeat would get "(2)".)
pub fn library_labels(roots: &[PathBuf]) -> Vec<String> {
    let name = |root: &Path| root.file_name().map(|n| n.to_string_lossy().into_owned());
    roots
        .iter()
        .enumerate()
        .map(|(i, root)| {
            let unique = match name(root) {
                Some(n) => {
                    roots
                        .iter()
                        .filter(|r| name(r).as_deref() == Some(&n))
                        .count()
                        == 1
                }
                None => false,
            };
            let label = match name(root) {
                Some(n) if unique => n,
                _ => root.display().to_string(),
            };
            match roots[..i].iter().filter(|r| *r == root).count() {
                0 => label,
                earlier => format!("{label} ({})", earlier + 1),
            }
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
        if !id.starts_with('.') && path.is_dir() {
            games.push(Game {
                id: id.to_owned(),
                dir: path,
                slot: 0,
            });
        }
    }
    games.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(games)
}

/// A store a game id comes from. Only DLsite and 072Project ids get store information looked up online.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    DLsite,
    Project072,
}

impl Store {
    /// The name shown on the game.
    pub fn label(self) -> &'static str {
        match self {
            Store::DLsite => "DLsite",
            Store::Project072 => "072Project",
        }
    }
}

/// The store a game id belongs to, by its prefix: `RJ` digits, or `RA` digits.
pub fn store_of(id: &str) -> Option<Store> {
    if is_dlsite_id(id) {
        Some(Store::DLsite)
    } else if is_072project_id(id) {
        Some(Store::Project072)
    } else {
        None
    }
}

fn is_072project_id(name: &str) -> bool {
    name.strip_prefix("RA")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// True for ids the DLsite store knows: `RJ` followed by digits.
pub fn is_dlsite_id(name: &str) -> bool {
    name.strip_prefix("RJ")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}
