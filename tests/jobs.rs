use dackel_launcher::extract::Encoding;
use dackel_launcher::jobs::{Cancelled, ExtractJob, JobStatus, Jobs, OverwriteCheck};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

fn key_of(dir: &Path) -> String {
    dir.to_string_lossy().into_owned()
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
}

fn extraction(dir: &Path) -> ExtractJob {
    ExtractJob::start(dir.to_path_buf(), Encoding::Auto, || {})
}

fn check(dir: &Path) -> OverwriteCheck {
    OverwriteCheck::start(dir.to_path_buf(), Encoding::Auto, || {})
}

/// Calls `step` until it returns something, or fails after a few seconds.
fn wait_for<T>(mut step: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(found) = step() {
            return found;
        }
        assert!(Instant::now() < deadline, "jobs did not finish in time");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_game_is_in_progress_while_it_extracts_or_while_its_files_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let key = key_of(dir.path());
    let mut jobs = Jobs::new();
    assert!(!jobs.in_progress(&key));

    jobs.start_extraction(key.clone(), extraction(dir.path()));
    assert!(jobs.in_progress(&key));

    let mut jobs = Jobs::new();
    jobs.start_check(key.clone(), check(dir.path()));
    assert!(jobs.in_progress(&key));
}

#[test]
fn another_games_extraction_does_not_make_this_game_in_progress() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut jobs = Jobs::new();
    jobs.start_extraction(key_of(other.path()), extraction(other.path()));

    assert!(!jobs.in_progress(&key_of(dir.path())));
}

#[test]
fn only_an_extraction_counts_as_extracting() {
    let dir = tempfile::tempdir().unwrap();
    let key = key_of(dir.path());
    let mut jobs = Jobs::new();
    jobs.start_check(key.clone(), check(dir.path()));
    assert!(!jobs.is_extracting(&key));

    jobs.start_extraction(key.clone(), extraction(dir.path()));
    assert!(jobs.is_extracting(&key));
}

#[test]
fn nothing_is_running_until_a_job_starts() {
    let dir = tempfile::tempdir().unwrap();
    let mut jobs = Jobs::new();
    assert!(!jobs.any());
    assert_eq!(jobs.keys().count(), 0);

    jobs.start_check(key_of(dir.path()), check(dir.path()));

    assert!(jobs.any());
    assert_eq!(jobs.keys().collect::<Vec<_>>(), [&key_of(dir.path())]);
}

#[test]
fn the_games_inside_a_library_are_found_by_its_folder() {
    let lib = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let one = lib.path().join("RJ1");
    let two = lib.path().join("RJ2");
    let elsewhere = other.path().join("RJ3");
    for dir in [&one, &two, &elsewhere] {
        fs::create_dir(dir).unwrap();
    }
    let mut jobs = Jobs::new();
    jobs.start_extraction(key_of(&one), extraction(&one));
    jobs.start_check(key_of(&two), check(&two));
    jobs.start_extraction(key_of(&elsewhere), extraction(&elsewhere));

    let mut found = jobs.keys_in(lib.path());
    found.sort();

    assert_eq!(found, [key_of(&one), key_of(&two)]);
}

#[test]
fn cancelling_drops_a_check_and_asks_an_extraction_to_stop() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut jobs = Jobs::new();
    jobs.start_check(key_of(dir.path()), check(dir.path()));
    jobs.start_extraction(key_of(other.path()), extraction(other.path()));

    assert_eq!(jobs.cancel(&key_of(dir.path())), Cancelled::Check);
    assert!(!jobs.in_progress(&key_of(dir.path())));

    assert_eq!(jobs.cancel(&key_of(other.path())), Cancelled::Extraction);
    // The extraction stays listed until it reports back.
    assert!(jobs.in_progress(&key_of(other.path())));

    assert_eq!(jobs.cancel("/not/running"), Cancelled::Nothing);
}

#[test]
fn a_check_shows_an_empty_bar_and_an_extraction_shows_its_progress() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut jobs = Jobs::new();
    assert_eq!(jobs.progress(&key_of(dir.path())), None);

    jobs.start_check(key_of(dir.path()), check(dir.path()));
    let bar = jobs.progress(&key_of(dir.path())).unwrap();
    assert_eq!((bar.bytes_total, bar.bytes_done), (1, 0));

    jobs.start_extraction(key_of(other.path()), extraction(other.path()));
    assert!(jobs.progress(&key_of(other.path())).is_some());
}

#[test]
fn a_finished_check_is_handed_over_once() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let mut jobs = Jobs::new();
    jobs.start_check(key_of(dir.path()), check(dir.path()));

    let done = wait_for(|| Some(jobs.finished_checks()).filter(|d| !d.is_empty()));

    assert_eq!(done, [(key_of(dir.path()), Ok(vec![]))]);
    assert!(!jobs.in_progress(&key_of(dir.path())));
    assert!(jobs.finished_checks().is_empty());
}

#[test]
fn a_finished_extraction_is_handed_over_once_with_how_it_ended() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let mut jobs = Jobs::new();
    jobs.start_extraction(key_of(dir.path()), extraction(dir.path()));

    let done = wait_for(|| Some(jobs.finished_extractions()).filter(|d| !d.is_empty()));

    assert_eq!(done.len(), 1);
    let (key, progress, status) = &done[0];
    assert_eq!(key, &key_of(dir.path()));
    assert_eq!(progress.files_done, 1);
    assert_eq!(status, &JobStatus::Finished(Ok(())));
    assert!(!jobs.in_progress(&key_of(dir.path())));
    assert!(jobs.finished_extractions().is_empty());
}
