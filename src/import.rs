//! Moves downloaded archives into a library: each goes into a game folder, which is created
//! or reused. Nothing is extracted here. No UI: the "Add zip" window drives this.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What to do with a file whose destination already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Overwrite,
    Skip,
}

/// The size and last-modified time of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// A file that would replace an existing file of the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The position of the file in the list given to `find_conflicts`.
    pub index: usize,
    pub dest: PathBuf,
    pub existing: Stat,
    pub incoming: Stat,
}

#[derive(Debug)]
pub enum Outcome {
    /// The file is in place at `dest`. `replaced` says an existing file was overwritten.
    Moved { dest: PathBuf, replaced: bool },
    /// A file of that name already exists and the policy is `Skip`. Nothing changed.
    Skipped,
    /// Nothing under the final name was changed, and the source is still where it was.
    Failed(io::Error),
}

#[derive(Debug)]
pub struct FileResult {
    pub source: PathBuf,
    pub outcome: Outcome,
}

const ARCHIVE_EXTENSIONS: &[&str] = &[".zip", ".7z", ".rar", ".tar.gz"];

/// Whether the file name is an archive this launcher can add: zip, 7z, rar or tar.gz.
pub fn is_archive(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    ARCHIVE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
}

/// The folder name an archive gets by default: its file name without the extension, with
/// `.tar.gz` removed in full.
pub fn default_folder_name(file_name: &str) -> String {
    if file_name.to_ascii_lowercase().ends_with(".tar.gz") {
        return file_name[..file_name.len() - ".tar.gz".len()].to_owned();
    }
    Path::new(file_name)
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Separator,
    /// Starts with a dot, which also covers `.` and `..`.
    LeadingDot,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NameError::Empty => "Type a folder name first.",
            NameError::Separator => "A folder name cannot contain / or \\.",
            NameError::LeadingDot => "A folder name cannot start with a dot.",
        })
    }
}

/// Checks a game folder name: it must be one plain, non-hidden name inside the library.
pub fn check_folder_name(name: &str) -> Result<(), NameError> {
    if name.trim().is_empty() {
        Err(NameError::Empty)
    } else if name.contains(['/', '\\']) {
        Err(NameError::Separator)
    } else if name.starts_with('.') {
        Err(NameError::LeadingDot)
    } else {
        Ok(())
    }
}

fn stat(path: &Path) -> Option<Stat> {
    let meta = fs::metadata(path).ok()?;
    meta.is_file().then(|| Stat {
        size: meta.len(),
        modified: meta.modified().ok(),
    })
}

/// Which files would replace an existing file of the same name. `items` are (source path,
/// folder name) pairs, as for `import`. Nothing is changed.
pub fn find_conflicts(library: &Path, items: &[(PathBuf, String)]) -> Vec<Conflict> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, (source, folder))| {
            let dest = library.join(folder).join(source.file_name()?);
            let existing = stat(&dest)?;
            let incoming = stat(source)?;
            Some(Conflict {
                index,
                dest,
                existing,
                incoming,
            })
        })
        .collect()
}

/// Moves each source into `library/<folder>/`, creating the folder when it is missing. Each
/// file is handled on its own, so one failure does not stop the rest. Results are in the
/// order given.
pub fn import(library: &Path, items: &[(PathBuf, String)], policy: Policy) -> Vec<FileResult> {
    items
        .iter()
        .map(|(source, folder)| FileResult {
            source: source.clone(),
            outcome: match place(library, source, folder, policy) {
                Ok(outcome) => outcome,
                Err(err) => Outcome::Failed(err),
            },
        })
        .collect()
}

fn place(library: &Path, source: &Path, folder: &str, policy: Policy) -> io::Result<Outcome> {
    check_folder_name(folder)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))?;
    let name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file"))?;
    let dir = library.join(folder);
    let dest = dir.join(name);
    let replaced = dest.exists();
    if replaced && policy == Policy::Skip {
        return Ok(Outcome::Skipped);
    }
    let existed = dir.is_dir();
    fs::create_dir_all(&dir)?;
    // A rename replaces the destination in one step, so an overwrite happens only once the
    // new copy is in place. Across filesystems, the copy goes through a temporary file.
    let moved = match fs::rename(source, &dest) {
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => move_by_copy(source, &dest),
        other => other,
    };
    if let Err(err) = moved {
        // Do not leave an empty game folder behind for a file that never arrived.
        if !existed {
            let _ = fs::remove_dir(&dir);
        }
        return Err(err);
    }
    Ok(Outcome::Moved { dest, replaced })
}

/// Moves `source` to `dest` by copying: the copy goes to a temporary name beside `dest`, its
/// size is checked, it is renamed to `dest` and only then is `source` removed. If anything
/// fails before the rename, `source` is untouched and the temporary file is removed.
pub fn move_by_copy(source: &Path, dest: &Path) -> io::Result<()> {
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = copy_checked(source, &tmp)
        .and_then(|()| fs::rename(&tmp, dest))
        .and_then(|()| fs::remove_file(source));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn copy_checked(source: &Path, tmp: &Path) -> io::Result<()> {
    let size = fs::metadata(source)?.len();
    if fs::copy(source, tmp)? != size {
        return Err(io::Error::other(
            "the copy is not the same size as the original",
        ));
    }
    Ok(())
}
