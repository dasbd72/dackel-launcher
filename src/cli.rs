//! The launcher's command line.

use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    /// `--library <dir>`: the library to use instead of the one beside the program.
    pub library: Option<PathBuf>,
    /// `--launch <game folder>`: start that game and exit when it does, with no window.
    pub launch: Option<PathBuf>,
}

/// Reads the arguments after the program name. Unknown arguments are ignored.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        for (flag, slot) in [
            ("--library", &mut parsed.library),
            ("--launch", &mut parsed.launch),
        ] {
            let value = if arg == flag {
                Some(args.next().ok_or(format!("{flag} requires a path"))?)
            } else {
                arg.strip_prefix(&format!("{flag}=")).map(str::to_owned)
            };
            if let Some(value) = value {
                let path =
                    std::path::absolute(&value).map_err(|e| format!("bad {flag} path: {e}"))?;
                *slot = Some(path);
                break;
            }
        }
    }
    Ok(parsed)
}
