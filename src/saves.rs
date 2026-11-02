//! Finds where a game keeps its saves: files and folders with `sav` in the name, in the game's
//! folder and in its Wine prefix.

use std::path::{Path, PathBuf};

use crate::tuning::prefix_dir;

/// Matches past this many are left out.
pub const MAX: usize = 200;

pub struct Saves {
    /// Paths relative to the game's folder.
    pub paths: Vec<PathBuf>,
    /// There were matches beyond `MAX`.
    pub more: bool,
}

pub fn find_saves(game_dir: &Path) -> Saves {
    let mut found = Saves {
        paths: Vec::new(),
        more: false,
    };
    walk(game_dir, Path::new(""), &mut found);
    found
}

/// Wine's own files, which hold no saves and many matching names.
fn is_wine_noise(rel: &Path) -> bool {
    let drive_c = prefix_dir(Path::new("")).join("drive_c");
    rel.parent() == Some(drive_c.as_path())
        && rel
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "windows" || n.starts_with("Program Files"))
}

fn walk(game_dir: &Path, rel: &Path, found: &mut Saves) {
    let Ok(entries) = std::fs::read_dir(game_dir.join(rel)) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let rel = rel.join(entry.file_name());
        if entry
            .file_name()
            .to_string_lossy()
            .to_lowercase()
            .contains("sav")
        {
            if found.paths.len() == MAX {
                found.more = true;
                return;
            }
            found.paths.push(rel.clone());
        }
        // `file_type` does not follow links: the prefix's `z:` points at the whole disk.
        if entry.file_type().is_ok_and(|t| t.is_dir()) && !is_wine_noise(&rel) {
            walk(game_dir, &rel, found);
            if found.more {
                return;
            }
        }
    }
}
