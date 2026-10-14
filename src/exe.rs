use std::io;
use std::path::{Path, PathBuf};

const META_DIR: &str = ".dlsite-launcher";
const MAX_DEPTH: usize = 3;

const HELPER_PREFIXES: [&str; 5] = [
    "unins",
    "unitycrashhandler",
    "vc_redist",
    "dxwebsetup",
    "notification_helper",
];
const HELPER_SUBSTRINGS: [&str; 2] = ["config", "setup"];

pub struct Candidate {
    pub rel: PathBuf,
    pub size: u64,
    pub depth: usize,
}

pub fn is_helper(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    HELPER_PREFIXES.iter().any(|p| lower.starts_with(p))
        || HELPER_SUBSTRINGS.iter().any(|s| lower.contains(s))
}

/// Lists non-helper `.exe` files up to `MAX_DEPTH` path components below `game_dir`,
/// skipping the metadata folder and symlinks.
pub fn find_candidates(game_dir: &Path) -> io::Result<Vec<Candidate>> {
    let mut found = Vec::new();
    walk(game_dir, Path::new(""), 0, &mut found)?;
    found.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(found)
}

fn walk(dir: &Path, rel: &Path, depth: usize, found: &mut Vec<Candidate>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let file_type = entry.file_type()?;
        let child_rel = rel.join(&name);
        if file_type.is_dir() {
            if name_str != META_DIR && depth + 1 < MAX_DEPTH {
                walk(&entry.path(), &child_rel, depth + 1, found)?;
            }
        } else if file_type.is_file()
            && depth < MAX_DEPTH
            && name_str.to_lowercase().ends_with(".exe")
            && !is_helper(&name_str)
        {
            found.push(Candidate {
                rel: child_rel,
                size: entry.metadata()?.len(),
                depth: depth + 1,
            });
        }
    }
    Ok(())
}

/// Auto-picks an exe only when one candidate clearly wins: the only candidate, or
/// the largest one at least twice the size of the runner-up. Ranking is by size,
/// then by shallower depth, then by path.
pub fn pick(candidates: &[Candidate]) -> Option<PathBuf> {
    let mut ranked: Vec<&Candidate> = candidates.iter().collect();
    ranked.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then(a.depth.cmp(&b.depth))
            .then(a.rel.cmp(&b.rel))
    });
    match ranked.as_slice() {
        [] => None,
        [only] => Some(only.rel.clone()),
        [first, second, ..] if first.size >= second.size.saturating_mul(2) => {
            Some(first.rel.clone())
        }
        _ => None,
    }
}

/// Reads `GAME_EXE="${SCRIPT_DIR}/<rel>"` from a legacy `start.sh` in `game_dir`.
/// Returns the relative path only if that file exists inside the game folder.
pub fn legacy_hint(game_dir: &Path) -> io::Result<Option<PathBuf>> {
    let bytes = match std::fs::read(game_dir.join("start.sh")) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let text = String::from_utf8_lossy(&bytes);
    let Some(value) = text
        .lines()
        .map(str::trim)
        .map(|line| line.strip_prefix("export ").unwrap_or(line).trim_start())
        .find_map(|line| line.strip_prefix("GAME_EXE="))
    else {
        return Ok(None);
    };
    let value = value.trim_matches(|c| c == '"' || c == '\'');
    let value = value
        .strip_prefix("${SCRIPT_DIR}/")
        .or_else(|| value.strip_prefix("$SCRIPT_DIR/"))
        .unwrap_or(value);
    let rel = PathBuf::from(value);
    let safe = rel
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)));
    if safe && !value.is_empty() && game_dir.join(&rel).is_file() {
        Ok(Some(rel))
    } else {
        Ok(None)
    }
}
