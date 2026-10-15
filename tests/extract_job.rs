use dackel_launcher::extract::{Encoding, Progress};
use dackel_launcher::jobs::{ExtractJob, JobStatus, OverwriteCheck, extraction_in_progress};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
}

/// Polls until the job finishes, returning every status seen along the way.
fn wait(job: &mut ExtractJob) -> Vec<JobStatus> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut seen = Vec::new();
    loop {
        let status = job.poll();
        let finished = matches!(status, JobStatus::Finished(_) | JobStatus::Cancelled);
        seen.push(status);
        if finished {
            return seen;
        }
        assert!(Instant::now() < deadline, "job did not finish in time");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn start(dir: &Path) -> (ExtractJob, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    let job = ExtractJob::start(dir.to_path_buf(), Encoding::Auto, move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    (job, wakes)
}

#[test]
fn a_job_extracts_in_the_background_and_reports_success() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let (mut job, _) = start(dir.path());

    let seen = wait(&mut job);

    assert_eq!(seen.last(), Some(&JobStatus::Finished(Ok(()))));
    assert_eq!(fs::read(dir.path().join("Game/a.txt")).unwrap(), b"hello");
}

#[test]
fn progress_ends_at_the_totals_before_the_job_reports_finished() {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![3u8; 300_000];
    write_zip(
        &dir.path().join("RJ1.zip"),
        &[("Game/a.bin", &big), ("Game/b.txt", b"hi")],
    );
    let (mut job, _) = start(dir.path());

    wait(&mut job);

    assert_eq!(
        job.progress(),
        Progress {
            bytes_done: 300_002,
            bytes_total: 300_002,
            files_done: 2,
            files_total: 2,
            files_skipped: 0,
        }
    );
}

#[test]
fn a_failed_extraction_is_reported_as_a_message() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("RJ1.zip.001"), b"x").unwrap();
    fs::write(dir.path().join("RJ1.zip.002"), b"x").unwrap();
    let (mut job, _) = start(dir.path());

    let seen = wait(&mut job);

    match seen.last() {
        Some(JobStatus::Finished(Err(message))) => assert!(!message.is_empty()),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn the_window_is_woken_so_it_can_redraw() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let (mut job, wakes) = start(dir.path());

    wait(&mut job);

    assert!(wakes.load(Ordering::SeqCst) >= 1);
}

#[test]
fn polling_a_finished_job_keeps_returning_the_result() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let (mut job, _) = start(dir.path());
    wait(&mut job);

    assert_eq!(job.poll(), JobStatus::Finished(Ok(())));
    assert_eq!(job.poll(), JobStatus::Finished(Ok(())));
}

#[test]
fn a_pre_cancelled_job_reports_cancelled_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let flag = Arc::new(AtomicBool::new(true));
    let mut job = ExtractJob::start_with_flag(
        dir.path().to_path_buf(),
        Encoding::Auto,
        dackel_launcher::extract::Existing::Overwrite,
        flag,
        || {},
    );

    let seen = wait(&mut job);

    assert_eq!(seen.last(), Some(&JobStatus::Cancelled));
    assert!(!dir.path().join("Game").exists());
}

#[test]
fn cancelling_a_running_job_ends_cancelled_or_finished_but_never_failed() {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![9u8; 4_000_000];
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/big.bin", &big)]);
    let (mut job, _) = start(dir.path());

    job.cancel();
    let seen = wait(&mut job);

    // A tiny zip can finish before the worker looks at the flag; both outcomes are fine.
    assert!(
        matches!(
            seen.last(),
            Some(JobStatus::Cancelled | JobStatus::Finished(Ok(())))
        ),
        "got {:?}",
        seen.last()
    );
}

#[test]
fn cancelling_a_finished_job_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ1.zip"), &[("Game/a.txt", b"hello")]);
    let (mut job, _) = start(dir.path());
    wait(&mut job);

    job.cancel();

    assert_eq!(job.poll(), JobStatus::Finished(Ok(())));
    assert!(dir.path().join("Game/a.txt").exists());
}

#[test]
fn a_skipping_job_reports_how_many_files_it_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("RJ9.zip"),
        &[("Game/a.txt", b"zip"), ("Game/b.txt", b"b")],
    );
    fs::create_dir_all(dir.path().join("Game")).unwrap();
    fs::write(dir.path().join("Game/a.txt"), b"mine").unwrap();
    let mut job = ExtractJob::start_with(
        dir.path().to_path_buf(),
        Encoding::Auto,
        dackel_launcher::extract::Existing::Skip,
        || {},
    );

    let seen = wait(&mut job);

    assert_eq!(seen.last(), Some(&JobStatus::Finished(Ok(()))));
    assert_eq!(job.progress().files_skipped, 1);
    assert_eq!(fs::read(dir.path().join("Game/a.txt")).unwrap(), b"mine");
}

#[test]
fn an_overwrite_check_reports_the_files_a_zip_would_replace() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ8.zip"), &[("Game/a.txt", b"zip")]);
    fs::create_dir_all(dir.path().join("Game")).unwrap();
    fs::write(dir.path().join("Game/a.txt"), b"mine").unwrap();
    let check = dackel_launcher::jobs::OverwriteCheck::start(
        dir.path().to_path_buf(),
        Encoding::Auto,
        || {},
    );

    let deadline = Instant::now() + Duration::from_secs(20);
    let found = loop {
        if let Some(found) = check.poll() {
            break found;
        }
        assert!(Instant::now() < deadline, "check did not finish in time");
        std::thread::sleep(Duration::from_millis(5));
    };

    assert_eq!(found, Ok(vec![Path::new("Game/a.txt").to_path_buf()]));
}

#[test]
fn a_game_is_in_progress_while_it_extracts_or_while_its_files_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().to_string_lossy().into_owned();
    let mut extracting: HashMap<String, ExtractJob> = HashMap::new();
    let mut checking: HashMap<String, OverwriteCheck> = HashMap::new();

    assert!(!extraction_in_progress(&extracting, &checking, &key));

    extracting.insert(
        key.clone(),
        ExtractJob::start(dir.path().to_path_buf(), Encoding::Auto, || {}),
    );
    assert!(extraction_in_progress(&extracting, &checking, &key));

    extracting.clear();
    checking.insert(
        key.clone(),
        OverwriteCheck::start(dir.path().to_path_buf(), Encoding::Auto, || {}),
    );
    assert!(extraction_in_progress(&extracting, &checking, &key));
}

#[test]
fn another_games_extraction_does_not_make_this_game_in_progress() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut extracting: HashMap<String, ExtractJob> = HashMap::new();
    let checking: HashMap<String, OverwriteCheck> = HashMap::new();
    extracting.insert(
        other.path().to_string_lossy().into_owned(),
        ExtractJob::start(other.path().to_path_buf(), Encoding::Auto, || {}),
    );

    let key = dir.path().to_string_lossy().into_owned();
    assert!(!extraction_in_progress(&extracting, &checking, &key));
}
