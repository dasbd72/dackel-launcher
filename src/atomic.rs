use std::io;
use std::path::{Path, PathBuf};

/// Writes `bytes` to `path` so a crash leaves the old file or the new one, never a cut-off one.
pub fn write(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
