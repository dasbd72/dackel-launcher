use dackel_launcher::extract::{
    Encoding, Existing, ExtractError, Progress, Status, Unsupported, extract, extract_cancellable,
    extract_with_existing, extract_with_progress, overwrites, status,
};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
}

/// Writes a zip whose entry names are the given raw bytes, not UTF-8.
/// The zip crate only writes UTF-8 names, so each raw name is written as a
/// same-length ASCII placeholder and then patched in the file bytes.
fn write_zip_raw_names(path: &Path, entries: &[(Vec<u8>, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (i, (raw, body)) in entries.iter().enumerate() {
        let placeholder = format!("{:#>width$}", i, width = raw.len());
        zip.start_file(placeholder, options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();

    let mut bytes = fs::read(path).unwrap();
    for (i, (raw, _)) in entries.iter().enumerate() {
        let placeholder = format!("{:#>width$}", i, width = raw.len());
        let (patched, count) = replace_all(&bytes, placeholder.as_bytes(), raw);
        assert_eq!(count, 2, "name appears in the local and central headers");
        bytes = patched;
    }
    fs::write(path, bytes).unwrap();
}

fn replace_all(haystack: &[u8], needle: &[u8], with: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(haystack.len());
    let mut count = 0;
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(needle) {
            out.extend_from_slice(with);
            count += 1;
            i += needle.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    (out, count)
}

fn cp932(text: &str) -> Vec<u8> {
    encoding_rs::SHIFT_JIS.encode(text).0.into_owned()
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn folder_with_only_a_zip_is_not_extracted_and_untouched() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip(&game.join("RJ123456.zip"), &[("game/start.txt", b"hi")]);
    let before = names_in(&game);

    let state = status(&game).unwrap();

    assert_eq!(state, Status::NotExtracted);
    assert_eq!(names_in(&game), before);
}

#[test]
fn cp932_names_without_the_utf8_flag_decode_as_shift_jis() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip_raw_names(
        &game.join("RJ123456.zip"),
        &[
            (cp932("ゲーム/スタート.exe"), b"exe"),
            (cp932("ゲーム/データ/パック.bin"), b"data"),
        ],
    );

    extract(&game, Encoding::Auto).unwrap();

    assert_eq!(fs::read(game.join("ゲーム/スタート.exe")).unwrap(), b"exe");
    assert_eq!(
        fs::read(game.join("ゲーム/データ/パック.bin")).unwrap(),
        b"data"
    );
}

#[test]
fn single_top_level_folder_is_preserved_and_zip_is_kept() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip(
        &game.join("RJ123456.zip"),
        &[("Game/start.exe", b"exe"), ("Game/data/pack.bin", b"data")],
    );

    extract(&game, Encoding::Auto).unwrap();

    assert_eq!(fs::read(game.join("Game/start.exe")).unwrap(), b"exe");
    assert_eq!(fs::read(game.join("Game/data/pack.bin")).unwrap(), b"data");
    assert!(game.join("RJ123456.zip").is_file());
    assert_eq!(status(&game).unwrap(), Status::Ready { has_zip: true });
}

#[test]
fn utf8_flagged_names_decode_as_utf8() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip(
        &game.join("RJ123456.zip"),
        &[("ゲーム/スタート.exe", b"exe")],
    );

    extract(&game, Encoding::Auto).unwrap();

    assert_eq!(fs::read(game.join("ゲーム/スタート.exe")).unwrap(), b"exe");
}

#[test]
fn loose_files_are_wrapped_in_a_folder_named_after_the_zip() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip(
        &game.join("mygame.zip"),
        &[("start.exe", b"exe"), ("data/pack.bin", b"data")],
    );

    extract(&game, Encoding::Auto).unwrap();

    assert_eq!(fs::read(game.join("mygame/start.exe")).unwrap(), b"exe");
    assert_eq!(
        fs::read(game.join("mygame/data/pack.bin")).unwrap(),
        b"data"
    );
    assert!(!game.join("start.exe").exists());
}

#[test]
fn reextract_with_a_chosen_encoding_fixes_unflagged_utf8_names() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip_raw_names(
        &game.join("RJ123456.zip"),
        &[("ゲーム/スタート.exe".as_bytes().to_vec(), b"exe")],
    );

    extract(&game, Encoding::Utf8).unwrap();

    assert_eq!(fs::read(game.join("ゲーム/スタート.exe")).unwrap(), b"exe");
}

#[test]
fn entries_escaping_the_game_folder_are_refused_before_anything_is_written() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    write_zip(
        &game.join("RJ123456.zip"),
        &[("Game/ok.txt", b"ok"), ("Game/../../escaped.txt", b"bad")],
    );

    let err = extract(&game, Encoding::Auto).unwrap_err();

    assert!(matches!(err, ExtractError::UnsafeEntry(_)));
    assert!(!lib.path().join("escaped.txt").exists());
    assert!(!game.join("Game").exists());
}

#[test]
fn absolute_entries_and_metadata_folder_entries_are_refused() {
    for name in ["/tmp/absolute.txt", ".dackel-launcher/profile.json"] {
        let lib = tempfile::tempdir().unwrap();
        let game = lib.path().join("RJ123456");
        fs::create_dir(&game).unwrap();
        write_zip(&game.join("RJ123456.zip"), &[(name, b"bad")]);

        let err = extract(&game, Encoding::Auto).unwrap_err();

        assert!(matches!(err, ExtractError::UnsafeEntry(_)), "{name}");
        assert!(!game.join(".dackel-launcher").exists(), "{name}");
    }
}

#[test]
fn split_archives_are_flagged_and_not_attempted() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    fs::write(game.join("RJ123456.zip.001"), b"part one").unwrap();
    fs::write(game.join("RJ123456.zip.002"), b"part two").unwrap();
    let before = names_in(&game);

    assert_eq!(
        status(&game).unwrap(),
        Status::ExtractManually(Unsupported::SplitArchive)
    );
    assert!(matches!(
        extract(&game, Encoding::Auto),
        Err(ExtractError::Unsupported(Unsupported::SplitArchive))
    ));
    assert_eq!(names_in(&game), before);
}

#[test]
fn self_extracting_exe_is_flagged_and_not_attempted() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    let sfx = game.join("RJ123456.exe");
    write_zip(&sfx, &[("Game/start.exe", b"exe")]);
    let mut stub = b"MZ stub header".to_vec();
    stub.extend(fs::read(&sfx).unwrap());
    fs::write(&sfx, stub).unwrap();
    let before = names_in(&game);

    assert_eq!(
        status(&game).unwrap(),
        Status::ExtractManually(Unsupported::SelfExtracting)
    );
    assert!(matches!(
        extract(&game, Encoding::Auto),
        Err(ExtractError::Unsupported(Unsupported::SelfExtracting))
    ));
    assert_eq!(names_in(&game), before);
}

#[test]
fn plain_folder_without_archives_is_ready_with_no_zip() {
    let lib = tempfile::tempdir().unwrap();
    let game = lib.path().join("RJ123456");
    fs::create_dir(&game).unwrap();
    fs::write(game.join("start.exe"), b"exe").unwrap();

    assert_eq!(status(&game).unwrap(), Status::Ready { has_zip: false });
}

fn progress_of(dir: &Path) -> Vec<Progress> {
    let mut seen = Vec::new();
    extract_with_progress(dir, Encoding::Auto, |p| seen.push(p)).unwrap();
    seen
}

fn game_with_three_files() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![7u8; 200_000];
    write_zip(
        &dir.path().join("RJ1.zip"),
        &[
            ("Game/a.bin", &[1u8; 100]),
            ("Game/b.bin", &big),
            ("Game/c.txt", b"hello"),
        ],
    );
    dir
}

#[test]
fn progress_knows_the_totals_from_the_first_report() {
    let dir = game_with_three_files();

    let seen = progress_of(dir.path());

    assert!(!seen.is_empty());
    assert!(
        seen.iter()
            .all(|p| p.bytes_total == 200_105 && p.files_total == 3)
    );
    assert_eq!(
        (seen[0].bytes_done, seen[0].files_done),
        (0, 0),
        "starts at zero"
    );
}

#[test]
fn progress_never_goes_backwards_and_ends_exactly_at_the_totals() {
    let dir = game_with_three_files();

    let seen = progress_of(dir.path());

    for pair in seen.windows(2) {
        assert!(pair[0].bytes_done <= pair[1].bytes_done);
        assert!(pair[0].files_done <= pair[1].files_done);
    }
    let last = seen.last().unwrap();
    assert_eq!((last.bytes_done, last.files_done), (200_105, 3));
}

#[test]
fn a_large_file_reports_partial_progress_while_it_is_being_written() {
    let dir = game_with_three_files();

    let seen = progress_of(dir.path());

    assert!(
        seen.iter()
            .any(|p| p.bytes_done > 0 && p.bytes_done < p.bytes_total),
        "expected mid-file reports, got {seen:?}"
    );
}

#[test]
fn an_unsupported_archive_reports_no_progress() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("RJ1.zip.001"), b"x").unwrap();
    fs::write(dir.path().join("RJ1.zip.002"), b"x").unwrap();
    let mut calls = 0;

    let result = extract_with_progress(dir.path(), Encoding::Auto, |_| calls += 1);

    assert!(matches!(result, Err(ExtractError::Unsupported(_))));
    assert_eq!(calls, 0);
}

#[test]
fn progress_fraction_is_by_bytes_then_files_and_never_exceeds_one() {
    let half = Progress {
        bytes_done: 50,
        bytes_total: 200,
        files_done: 0,
        files_total: 4,
        files_skipped: 0,
    };
    assert_eq!(half.fraction(), 0.25);

    let only_empty_files = Progress {
        bytes_done: 0,
        bytes_total: 0,
        files_done: 1,
        files_total: 4,
        files_skipped: 0,
    };
    assert_eq!(only_empty_files.fraction(), 0.25);

    let nothing = Progress {
        bytes_done: 0,
        bytes_total: 0,
        files_done: 0,
        files_total: 0,
        files_skipped: 0,
    };
    assert_eq!(nothing.fraction(), 1.0);

    let over = Progress {
        bytes_done: 300,
        bytes_total: 200,
        files_done: 4,
        files_total: 4,
        files_skipped: 0,
    };
    assert_eq!(over.fraction(), 1.0);
}

fn cancel_when(dir: &Path, flag: &AtomicBool, after_bytes: u64) -> Result<(), ExtractError> {
    extract_cancellable(dir, Encoding::Auto, flag, |p| {
        if p.bytes_done >= after_bytes {
            flag.store(true, Ordering::SeqCst);
        }
    })
}

#[test]
fn cancelling_before_anything_is_written_changes_nothing() {
    let dir = game_with_three_files();
    let flag = AtomicBool::new(true);

    let result = extract_cancellable(dir.path(), Encoding::Auto, &flag, |_| {});

    assert!(matches!(result, Err(ExtractError::Cancelled)));
    assert_eq!(names_in(dir.path()), ["RJ1.zip"]);
}

#[test]
fn cancelling_midway_removes_everything_the_run_created() {
    let dir = game_with_three_files();
    let flag = AtomicBool::new(false);

    let result = cancel_when(dir.path(), &flag, 100_000);

    assert!(matches!(result, Err(ExtractError::Cancelled)));
    assert_eq!(
        names_in(dir.path()),
        ["RJ1.zip"],
        "no extracted folder, no .part leftovers"
    );
}

#[test]
fn cancelling_leaves_files_and_folders_that_were_already_there() {
    let dir = game_with_three_files();
    fs::create_dir(dir.path().join("Game")).unwrap();
    fs::write(dir.path().join("Game/keep.txt"), b"mine").unwrap();
    fs::write(dir.path().join("notes.txt"), b"notes").unwrap();
    let flag = AtomicBool::new(false);

    cancel_when(dir.path(), &flag, 100_000).unwrap_err();

    assert_eq!(fs::read(dir.path().join("Game/keep.txt")).unwrap(), b"mine");
    assert_eq!(fs::read(dir.path().join("notes.txt")).unwrap(), b"notes");
    assert!(
        !dir.path().join("Game/b.bin").exists(),
        "the new big file is gone"
    );
    assert!(!dir.path().join("Game/c.txt").exists());
}

#[test]
fn a_cancelled_re_extract_never_leaves_a_half_written_file() {
    let dir = game_with_three_files();
    fs::create_dir(dir.path().join("Game")).unwrap();
    fs::write(dir.path().join("Game/b.bin"), b"old contents").unwrap();
    let flag = AtomicBool::new(false);

    cancel_when(dir.path(), &flag, 100_000).unwrap_err();

    // The big file was being overwritten when the cancel came in: it must still be the
    // complete old file, not a truncated mix, and no temporary file is left behind.
    assert_eq!(
        fs::read(dir.path().join("Game/b.bin")).unwrap(),
        b"old contents"
    );
    assert!(
        names_in(&dir.path().join("Game"))
            .iter()
            .all(|n| !n.ends_with(".part")),
        "no .part files"
    );
}

#[test]
fn an_uncancelled_extraction_leaves_no_temporary_files() {
    let dir = game_with_three_files();

    extract(dir.path(), Encoding::Auto).unwrap();

    assert_eq!(names_in(dir.path()), ["Game", "RJ1.zip"]);
    assert_eq!(
        names_in(&dir.path().join("Game")),
        ["a.bin", "b.bin", "c.txt"]
    );
}

#[test]
fn overwrites_lists_the_existing_files_the_zip_would_replace() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("RJ1.zip"),
        &[("Game/save/s1.dat", b"new"), ("Game/new.txt", b"n")],
    );
    fs::create_dir_all(dir.path().join("Game/save")).unwrap();
    fs::write(dir.path().join("Game/save/s1.dat"), b"mine").unwrap();
    fs::write(dir.path().join("Game/other.txt"), b"keep").unwrap();

    let found = overwrites(dir.path(), Encoding::Auto).unwrap();

    assert_eq!(found, vec![Path::new("Game/save/s1.dat")]);
    assert_eq!(
        fs::read(dir.path().join("Game/save/s1.dat")).unwrap(),
        b"mine",
        "listing writes nothing"
    );
}

#[test]
fn overwrites_is_empty_for_a_fresh_folder_and_includes_a_wrapper_folder() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(&dir.path().join("RJ2.zip"), &[("loose.txt", b"x")]);

    assert!(overwrites(dir.path(), Encoding::Auto).unwrap().is_empty());

    fs::create_dir_all(dir.path().join("RJ2")).unwrap();
    fs::write(dir.path().join("RJ2/loose.txt"), b"old").unwrap();
    assert_eq!(
        overwrites(dir.path(), Encoding::Auto).unwrap(),
        vec![Path::new("RJ2/loose.txt")]
    );
}

#[test]
fn skipping_existing_files_keeps_them_and_writes_only_the_new_ones() {
    let dir = tempfile::tempdir().unwrap();
    write_zip(
        &dir.path().join("RJ3.zip"),
        &[("Game/save.dat", b"zip"), ("Game/new.txt", b"fresh")],
    );
    fs::create_dir_all(dir.path().join("Game")).unwrap();
    fs::write(dir.path().join("Game/save.dat"), b"mine").unwrap();
    let mut last = Progress::default();

    extract_with_existing(
        dir.path(),
        Encoding::Auto,
        Existing::Skip,
        &AtomicBool::new(false),
        |p| last = p,
    )
    .unwrap();

    assert_eq!(fs::read(dir.path().join("Game/save.dat")).unwrap(), b"mine");
    assert_eq!(fs::read(dir.path().join("Game/new.txt")).unwrap(), b"fresh");
    assert_eq!(last.files_skipped, 1);
    assert_eq!(
        last.files_total, 1,
        "only the file that gets written counts"
    );
    assert_eq!(last.files_done, 1);
    assert_eq!(last.fraction(), 1.0);
}
