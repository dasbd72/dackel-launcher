use dlsite_launcher::extract::{Encoding, ExtractError, Status, Unsupported, extract, status};
use std::fs;
use std::io::Write;
use std::path::Path;

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
    for name in ["/tmp/absolute.txt", ".dlsite-launcher/profile.json"] {
        let lib = tempfile::tempdir().unwrap();
        let game = lib.path().join("RJ123456");
        fs::create_dir(&game).unwrap();
        write_zip(&game.join("RJ123456.zip"), &[(name, b"bad")]);

        let err = extract(&game, Encoding::Auto).unwrap_err();

        assert!(matches!(err, ExtractError::UnsafeEntry(_)), "{name}");
        assert!(!game.join(".dlsite-launcher").exists(), "{name}");
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
