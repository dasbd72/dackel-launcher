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
