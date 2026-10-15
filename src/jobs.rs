//! Background extraction, so a big zip does not freeze the window.

use crate::extract::{self, Encoding, Existing, ExtractError, Progress};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// At most one progress message per this long; the copy loop reports far more often.
const REPORT_EVERY: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Running(Progress),
    Finished(Result<(), String>),
    /// Stopped on request; whatever the run had created was removed.
    Cancelled,
}

enum Update {
    Progress(Progress),
    Done(JobStatus),
}

/// An extraction running on its own thread. Poll it from the UI thread.
pub struct ExtractJob {
    rx: Receiver<Update>,
    cancel: Arc<AtomicBool>,
    progress: Progress,
    done: Option<JobStatus>,
}

impl ExtractJob {
    /// Starts extracting `dir`. `wake` is called from the worker whenever there is
    /// something new to show, so the UI can redraw (e.g. `ctx.request_repaint()`).
    pub fn start(dir: PathBuf, encoding: Encoding, wake: impl Fn() + Send + 'static) -> Self {
        Self::start_with(dir, encoding, Existing::Overwrite, wake)
    }

    /// Like [`start`](Self::start), choosing what happens to files that already exist.
    pub fn start_with(
        dir: PathBuf,
        encoding: Encoding,
        existing: Existing,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        Self::start_with_flag(
            dir,
            encoding,
            existing,
            Arc::new(AtomicBool::new(false)),
            wake,
        )
    }

    /// Like [`start`](Self::start), with a caller-supplied cancel flag.
    pub fn start_with_flag(
        dir: PathBuf,
        encoding: Encoding,
        existing: Existing,
        cancel: Arc<AtomicBool>,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let flag = cancel.clone();
        std::thread::spawn(move || {
            let mut latest = Progress::default();
            let mut last_sent: Option<Instant> = None;
            let result = extract::extract_with_existing(&dir, encoding, existing, &flag, |p| {
                latest = p;
                let due = last_sent.is_none_or(|at| at.elapsed() >= REPORT_EVERY);
                if due {
                    last_sent = Some(Instant::now());
                    let _ = tx.send(Update::Progress(p));
                    wake();
                }
            });
            // The last report may have been throttled away; make sure it arrives.
            let _ = tx.send(Update::Progress(latest));
            let status = match result {
                Ok(()) => JobStatus::Finished(Ok(())),
                Err(ExtractError::Cancelled) => JobStatus::Cancelled,
                Err(err) => JobStatus::Finished(Err(err.to_string())),
            };
            let _ = tx.send(Update::Done(status));
            wake();
        });
        ExtractJob {
            rx,
            cancel,
            progress: Progress::default(),
            done: None,
        }
    }

    /// Asks the worker to stop. It stops at the next chunk and removes what it created.
    /// Harmless if the job has already finished.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn poll(&mut self) -> JobStatus {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Progress(p) => self.progress = p,
                Update::Done(status) => self.done = Some(status),
            }
        }
        match &self.done {
            Some(status) => status.clone(),
            None => JobStatus::Running(self.progress),
        }
    }

    /// The most recent progress seen by `poll`.
    pub fn progress(&self) -> Progress {
        self.progress
    }
}

/// Looking for the files an extraction would overwrite, on its own thread.
pub struct OverwriteCheck {
    rx: Receiver<Result<Vec<PathBuf>, String>>,
}

impl OverwriteCheck {
    /// `wake` is called from the worker when the answer is ready.
    pub fn start(dir: PathBuf, encoding: Encoding, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let found = extract::overwrites(&dir, encoding).map_err(|err| err.to_string());
            let _ = tx.send(found);
            wake();
        });
        OverwriteCheck { rx }
    }

    /// The files that would be replaced (relative to the game folder), once known.
    pub fn poll(&self) -> Option<Result<Vec<PathBuf>, String>> {
        self.rx.try_recv().ok()
    }
}

/// What `Jobs::cancel` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancelled {
    /// The overwrite check was dropped, so nothing will be extracted.
    Check,
    /// The extraction was asked to stop; it removes what it created once it has.
    Extraction,
    /// Nothing was running for that game.
    Nothing,
}

/// The extractions and overwrite checks running in the background, by game folder.
#[derive(Default)]
pub struct Jobs {
    extracting: HashMap<String, ExtractJob>,
    checking: HashMap<String, OverwriteCheck>,
}

impl Jobs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the game folder `key` has an extraction running or its files being checked
    /// for overwrites. Either one means it must not be extracted, reloaded or launched again.
    pub fn in_progress(&self, key: &str) -> bool {
        self.extracting.contains_key(key) || self.checking.contains_key(key)
    }

    pub fn is_extracting(&self, key: &str) -> bool {
        self.extracting.contains_key(key)
    }

    /// Whether anything is running.
    pub fn any(&self) -> bool {
        !self.extracting.is_empty() || !self.checking.is_empty()
    }

    /// The game folders with an extraction or a check running.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.extracting.keys().chain(self.checking.keys())
    }

    /// The running game folders that sit directly inside `parent`.
    pub fn keys_in(&self, parent: &Path) -> Vec<String> {
        self.keys()
            .filter(|key| Path::new(key).parent() == Some(parent))
            .cloned()
            .collect()
    }

    pub fn start_check(&mut self, key: String, check: OverwriteCheck) {
        self.checking.insert(key, check);
    }

    pub fn start_extraction(&mut self, key: String, job: ExtractJob) {
        self.extracting.insert(key, job);
    }

    /// Stops the check for `key`, or asks its extraction to stop.
    pub fn cancel(&mut self, key: &str) -> Cancelled {
        if self.checking.remove(key).is_some() {
            Cancelled::Check
        } else if let Some(job) = self.extracting.get(key) {
            job.cancel();
            Cancelled::Extraction
        } else {
            Cancelled::Nothing
        }
    }

    /// The progress to draw for a game folder: the extraction's, or an empty bar while the
    /// overwrite check runs.
    pub fn progress(&self, key: &str) -> Option<Progress> {
        self.extracting
            .get(key)
            .map(ExtractJob::progress)
            .or_else(|| {
                self.checking.contains_key(key).then_some(Progress {
                    bytes_total: 1,
                    ..Progress::default()
                })
            })
    }

    /// The checks that have an answer, taken out of the list.
    pub fn finished_checks(&mut self) -> Vec<(String, Result<Vec<PathBuf>, String>)> {
        let done: Vec<(String, Result<Vec<PathBuf>, String>)> = self
            .checking
            .iter()
            .filter_map(|(key, check)| Some((key.clone(), check.poll()?)))
            .collect();
        for (key, _) in &done {
            self.checking.remove(key);
        }
        done
    }

    /// The extractions that are no longer running, with their last progress and how they
    /// ended, taken out of the list.
    pub fn finished_extractions(&mut self) -> Vec<(String, Progress, JobStatus)> {
        let mut finished = Vec::new();
        for (key, job) in &mut self.extracting {
            match job.poll() {
                JobStatus::Running(_) => {}
                done => finished.push((key.clone(), job.progress(), done)),
            }
        }
        for (key, _, _) in &finished {
            self.extracting.remove(key);
        }
        finished
    }
}
