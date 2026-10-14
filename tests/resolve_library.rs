use dlsite_launcher::library::resolve_library;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

#[test]
fn library_is_next_to_the_executable() {
    let drive = tempfile::tempdir().unwrap();
    let exe = drive.path().join("dlsite-launcher");
    fs::write(&exe, "").unwrap();

    let library = resolve_library(&exe, None).unwrap();

    assert_eq!(
        library,
        drive.path().canonicalize().unwrap().join("DLSiteLibrary")
    );
}

#[test]
fn library_follows_a_symlinked_executable_to_its_real_location() {
    let drive = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let real = drive.path().join("dlsite-launcher");
    fs::write(&real, "").unwrap();
    let link = elsewhere.path().join("launcher-link");
    symlink(&real, &link).unwrap();

    let library = resolve_library(&link, None).unwrap();

    assert_eq!(
        library,
        drive.path().canonicalize().unwrap().join("DLSiteLibrary")
    );
}

#[test]
fn override_wins_over_the_executable_location() {
    let drive = tempfile::tempdir().unwrap();
    let exe = drive.path().join("dlsite-launcher");
    fs::write(&exe, "").unwrap();

    let library = resolve_library(&exe, Some(Path::new("/some/other/lib"))).unwrap();

    assert_eq!(library, Path::new("/some/other/lib"));
}
