//! Finding a system font that can draw Japanese; egui's built-in fonts cannot.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSource {
    pub path: PathBuf,
    /// Face index inside a `.ttc` collection; 0 for single-face files.
    pub index: u32,
}

/// File names to look for when `fc-match` is unavailable, best first.
const KNOWN_FONTS: &[&str] = &[
    "NotoSansCJK-Regular.ttc",
    "NotoSansCJKjp-Regular.otf",
    "NotoSansJP-Regular.otf",
    "NotoSansJP-Regular.ttf",
    "ipag.ttf",
    "ipagp.ttf",
    "TakaoGothic.ttf",
    "DroidSansFallbackFull.ttf",
];

const SCAN_DEPTH: usize = 4;

/// Parses `fc-match -f '%{file}\n%{index}\n'` output.
pub fn parse_fc_match(output: &str) -> Option<FontSource> {
    let mut lines = output.lines().map(str::trim);
    let path = lines.next().filter(|l| !l.is_empty())?;
    let index = lines.next().and_then(|l| l.parse().ok()).unwrap_or(0);
    Some(FontSource {
        path: PathBuf::from(path),
        index,
    })
}

/// Looks through `dirs` (recursively) for one of the known Japanese fonts.
pub fn locate_in_dirs(dirs: &[PathBuf]) -> Option<FontSource> {
    let found: Vec<PathBuf> = dirs.iter().flat_map(|d| walk(d, SCAN_DEPTH)).collect();
    KNOWN_FONTS.iter().find_map(|name| {
        found
            .iter()
            .find(|p| p.file_name().is_some_and(|n| n == *name))
            .map(|path| FontSource {
                path: path.clone(),
                index: 0,
            })
    })
}

fn walk(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                files.extend(walk(&path, depth - 1));
            }
        } else {
            files.push(path);
        }
    }
    files
}

/// The system's preferred Japanese font: `fc-match` first, then a directory scan.
pub fn locate_japanese_font() -> Option<FontSource> {
    let from_fc = Command::new("fc-match")
        .args(["-f", "%{file}\n%{index}\n", ":lang=ja"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| parse_fc_match(&String::from_utf8_lossy(&out.stdout)))
        .filter(|font| font.path.is_file());
    from_fc.or_else(|| {
        let mut dirs = vec![
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
        locate_in_dirs(&dirs)
    })
}
