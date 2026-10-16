//! Knowing the running version, how it was installed, whether a newer release exists, and
//! (for `install.sh` installs only) updating in place.

use crate::fetch::{Fetch, Fetched};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::thread;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST_API: &str = "https://api.github.com/repos/dasbd72/dackel-launcher/releases/latest";
const INSTALLER: &str =
    "https://github.com/dasbd72/dackel-launcher/releases/latest/download/install.sh";
/// Written by `install.sh` next to the binary it installs.
pub const MARKER: &str = ".installed-by-script";
const CHECK_EVERY_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Script,
    System,
    Manual,
}

/// How the binary at `exe` was installed: the script leaves a marker beside it, a package
/// manager puts it under /usr, anything else was put there by hand.
pub fn detect(exe: &Path) -> Method {
    if exe.parent().is_some_and(|dir| dir.join(MARKER).exists()) {
        Method::Script
    } else if exe.starts_with("/usr") {
        Method::System
    } else {
        Method::Manual
    }
}

/// Whether the automatic check is due, given the last check and now (Unix seconds).
pub fn due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|last| now.saturating_sub(last) >= CHECK_EVERY_SECS)
}

fn parts(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// True when `latest` is a higher version than `current`; compared number by number, so
/// 0.1.10 is newer than 0.1.9.
pub fn newer(current: &str, latest: &str) -> bool {
    parts(latest) > parts(current)
}

/// The version in a GitHub "latest release" reply, without the leading `v`.
pub fn parse_latest(body: &[u8]) -> Option<String> {
    let json: serde_json::Value = serde_json::from_slice(body).ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    Some(tag.trim_start_matches('v').to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Updating,
    Updated(String),
    Failed(String),
}

fn get(fetch: &dyn Fetch, url: &str) -> Result<Vec<u8>, String> {
    match fetch.get(url) {
        Ok(Fetched::Body(body)) => Ok(body),
        Ok(Fetched::NotFound) => Err(format!("{url} was not found")),
        Err(err) => Err(err.to_string()),
    }
}

fn check(fetch: &dyn Fetch) -> State {
    match get(fetch, LATEST_API) {
        Ok(body) => match parse_latest(&body) {
            Some(latest) if newer(VERSION, &latest) => State::Available(latest),
            Some(_) => State::UpToDate,
            None => State::Failed("unexpected reply from GitHub".into()),
        },
        Err(err) => State::Failed(err),
    }
}

/// Runs the latest release's installer, which checks every file against SHA256SUMS before
/// it replaces anything.
fn install(fetch: &dyn Fetch, version: &str) -> State {
    let run = || -> Result<(), String> {
        let script = get(fetch, INSTALLER)?;
        let mut child = Command::new("sh")
            .arg("-s")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(&script)
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    };
    match run() {
        Ok(()) => State::Updated(version.to_owned()),
        Err(err) => State::Failed(err),
    }
}

/// Runs checks and updates on a worker thread, so the UI never waits on the network.
pub struct Updater {
    fetch: Arc<dyn Fetch + Send + Sync>,
    state: State,
    rx: Option<mpsc::Receiver<State>>,
}

impl Updater {
    pub fn new(fetch: Arc<dyn Fetch + Send + Sync>) -> Self {
        Updater {
            fetch,
            state: State::Idle,
            rx: None,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    fn spawn(&mut self, working: State, job: impl FnOnce(&dyn Fetch) -> State + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        let fetch = Arc::clone(&self.fetch);
        thread::spawn(move || {
            let _ = tx.send(job(fetch.as_ref()));
        });
        self.state = working;
        self.rx = Some(rx);
    }

    pub fn start_check(&mut self) {
        if !self.busy() {
            self.spawn(State::Checking, check);
        }
    }

    /// Updates to `version`; only for installs made by `install.sh`.
    pub fn start_install(&mut self, version: String) {
        if !self.busy() {
            self.spawn(State::Updating, move |fetch| install(fetch, &version));
        }
    }

    /// Takes in a finished job; true when one just finished.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok(state) => {
                self.state = state;
                self.rx = None;
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.state = State::Failed("the worker stopped".into());
                self.rx = None;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numbers_not_text() {
        assert!(newer("0.1.9", "0.1.10"));
        assert!(newer("0.1.11", "v0.2.0"));
        assert!(!newer("0.1.11", "0.1.11"));
        assert!(!newer("0.1.11", "0.1.9"));
    }

    #[test]
    fn reads_the_tag() {
        assert_eq!(
            parse_latest(br#"{"tag_name":"v0.1.12"}"#).as_deref(),
            Some("0.1.12")
        );
        assert_eq!(parse_latest(b"nope"), None);
    }

    #[test]
    fn detects_the_install_method() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("dackel-launcher");
        assert_eq!(detect(&exe), Method::Manual);
        std::fs::write(dir.path().join(MARKER), "").unwrap();
        assert_eq!(detect(&exe), Method::Script);
        assert_eq!(
            detect(Path::new("/usr/bin/dackel-launcher")),
            Method::System
        );
    }

    #[test]
    fn checks_at_most_once_a_day() {
        assert!(due(None, 5));
        assert!(!due(Some(1000), 1000 + CHECK_EVERY_SECS - 1));
        assert!(due(Some(1000), 1000 + CHECK_EVERY_SECS));
    }
}
