//! Per-game tuning: environment variables, extra arguments, the Wine prefix.
//!
//! Everything here is pure text handling or touches only one game's own metadata folder.

use crate::library::META_DIR;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const PREFIX_DIR: &str = "wine";

/// Parses `KEY=value` lines. Blank lines and `#` comments are ignored; the error names the
/// offending line.
pub fn parse_env(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let n = i + 1;
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {n}: expected KEY=value"));
        };
        let key = key.trim();
        if !is_env_name(key) {
            return Err(format!("line {n}: \"{key}\" is not a valid variable name"));
        }
        map.insert(key.to_owned(), value.trim().to_owned());
    }
    Ok(map)
}

fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The text form `parse_env` reads back.
pub fn format_env(env: &BTreeMap<String, String>) -> String {
    env.iter().map(|(k, v)| format!("{k}={v}\n")).collect()
}

/// Splits an argument line the way a shell would: whitespace separates, single quotes are
/// literal, double quotes group, and a backslash escapes the next character.
pub fn split_args(text: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => current.push(c),
                        None => return Err("unclosed ' quote".to_owned()),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => current.push(c),
                            Some(c) => {
                                current.push('\\');
                                current.push(c);
                            }
                            None => return Err("unclosed \" quote".to_owned()),
                        },
                        Some(c) => current.push(c),
                        None => return Err("unclosed \" quote".to_owned()),
                    }
                }
            }
            '\\' => {
                started = true;
                match chars.next() {
                    Some(c) => current.push(c),
                    None => current.push('\\'),
                }
            }
            c => {
                started = true;
                current.push(c);
            }
        }
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

/// The game's own Wine prefix.
pub fn prefix_dir(game_dir: &Path) -> PathBuf {
    game_dir.join(META_DIR).join(PREFIX_DIR)
}

/// Deletes this game's Wine prefix (and nothing else); a game without one is fine.
/// Tuning, play time and the game's files are untouched.
pub fn reset_prefix(game_dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(prefix_dir(game_dir)) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// A shell command that runs winetricks against the game's prefix.
pub fn winetricks_command(game_dir: &Path) -> String {
    let prefix = prefix_dir(game_dir).display().to_string();
    format!("WINEPREFIX={} winetricks", shell_quote(&prefix))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}
