use crate::extract::Progress;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

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
pub fn writable_by_me(dir: &Path) -> bool {
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

/// Erases a game's folder. Refuses anything that is not a direct child of a library root, so a
/// wrong path can never take a library or a parent folder with it.
pub fn delete_game(roots: &[PathBuf], dir: &Path) -> io::Result<()> {
    if !dir.parent().is_some_and(|p| roots.iter().any(|r| r == p)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a game folder of a library", dir.display()),
        ));
    }
    std::fs::remove_dir_all(dir)
}

/// What a finished move leaves behind.
#[derive(Debug)]
pub struct Moved {
    /// The game's folder in the destination library.
    pub dir: PathBuf,
    /// The old folder, when the copy was complete but the old one could not be erased.
    pub leftover: Option<PathBuf>,
}

/// Written first into the hidden folder a copy is made in, so only our own leftovers are cleaned.
const MOVING_MARK: &str = ".dackel-moving";

/// Why a game cannot move into `dest`, if it cannot: the destination must be another added
/// library that we can write to and that has no folder of the game's name. The one place that
/// decides this, for the buttons and for `move_game` alike.
pub fn move_blocked(roots: &[PathBuf], dir: &Path, dest: &Path) -> Option<&'static str> {
    let (Some(parent), Some(name)) = (dir.parent(), dir.file_name()) else {
        return Some("This is not a game folder");
    };
    if !roots.iter().any(|r| r == parent) {
        Some("This is not a game folder of a library")
    } else if dest == parent || !roots.iter().any(|r| r == dest) {
        Some("This is not another library")
    } else if !writable_by_me(dest) {
        Some("You cannot write to this library")
    } else if dest.join(name).symlink_metadata().is_ok() {
        Some("It already has a folder with this name")
    } else {
        None
    }
}

/// Moves a game's folder into another added library; nothing is overwritten.
/// `force_copy` skips the rename, so the copy path can be tested without a second drive.
/// A copy reports its progress and stops at the next chunk once `cancel` is set, removing
/// what it made.
pub fn move_game(
    roots: &[PathBuf],
    dir: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    force_copy: bool,
    progress: &mut dyn FnMut(Progress),
) -> io::Result<Moved> {
    if let Some(why) = move_blocked(roots, dir, dest) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, why));
    }
    let name = dir.file_name().unwrap_or_default();
    let target = dest.join(name);
    if !force_copy {
        match std::fs::rename(dir, &target) {
            Ok(()) => {
                return Ok(Moved {
                    dir: target,
                    leftover: None,
                });
            }
            Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {}
            Err(e) => return Err(e),
        }
    }
    // Copy under a hidden name (the scan skips it) and rename it into place only once complete.
    let temp = dest.join(format!(".{}.moving", name.to_string_lossy()));
    match temp.symlink_metadata() {
        Ok(_) if temp.join(MOVING_MARK).exists() => std::fs::remove_dir_all(&temp)?,
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} is in the way", temp.display()),
            ));
        }
        Err(_) => {}
    }
    let copied = copy_game(dir, &temp, cancel, progress).and_then(|()| {
        std::fs::remove_file(temp.join(MOVING_MARK))?;
        if tree_size(dir)? != tree_size(&temp)? {
            return Err(io::Error::other("the copy does not match the original"));
        }
        // The last chance to stop: after the rename the game is in its new place.
        check_cancel(cancel)?;
        std::fs::rename(&temp, &target)
    });
    if let Err(e) = copied {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(e);
    }
    let leftover = std::fs::remove_dir_all(dir)
        .err()
        .map(|_| dir.to_path_buf());
    Ok(Moved {
        dir: target,
        leftover,
    })
}

/// Removes the hidden folders a copy that never finished left in the libraries.
pub fn clean_moving(roots: &[PathBuf]) {
    for entry in roots
        .iter()
        .filter_map(|r| std::fs::read_dir(r).ok())
        .flatten()
        .flatten()
    {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            && name.ends_with(".moving")
            && entry.path().join(MOVING_MARK).exists()
        {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
    }
    Ok(())
}

fn copy_game(
    src: &Path,
    dst: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> io::Result<()> {
    let (files, bytes) = tree_size(src)?;
    let mut now = Progress {
        bytes_total: bytes,
        files_total: files,
        ..Progress::default()
    };
    progress(now);
    std::fs::create_dir(dst)?;
    std::fs::write(dst.join(MOVING_MARK), "")?;
    copy_tree(src, dst, cancel, &mut now, progress)
}

/// Copies a folder keeping symlinks (not followed) and permissions, in chunks so a cancel
/// does not wait for a big file.
fn copy_tree(
    src: &Path,
    dst: &Path,
    cancel: &AtomicBool,
    now: &mut Progress,
    progress: &mut dyn FnMut(Progress),
) -> io::Result<()> {
    use std::io::{Read, Write};
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        check_cancel(cancel)?;
        let entry = entry?;
        let (from, to) = (entry.path(), dst.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&from)?, &to)?;
        } else if kind.is_dir() {
            copy_tree(&from, &to, cancel, now, progress)?;
        } else {
            let mut input = std::fs::File::open(&from)?;
            let mut output = std::fs::File::create(&to)?;
            let mut buf = vec![0u8; 1 << 20];
            loop {
                check_cancel(cancel)?;
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                output.write_all(&buf[..n])?;
                now.bytes_done += n as u64;
                progress(*now);
            }
            output.set_permissions(input.metadata()?.permissions())?;
        }
        now.files_done += 1;
        progress(*now);
    }
    // Last, so a read-only folder can still be filled.
    std::fs::set_permissions(dst, std::fs::symlink_metadata(src)?.permissions())
}

/// Entry count and total file size of a folder, symlinks not followed.
fn tree_size(dir: &Path) -> io::Result<(u64, u64)> {
    let (mut count, mut bytes) = (0, 0);
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        count += 1;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            let (c, b) = tree_size(&entry.path())?;
            (count, bytes) = (count + c, bytes + b);
        } else if !kind.is_symlink() {
            bytes += entry.metadata()?.len();
        }
    }
    Ok((count, bytes))
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
