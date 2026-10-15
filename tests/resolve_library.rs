use dackel_launcher::library::{may_create_library, resolve_library};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;

#[test]
fn library_is_next_to_the_executable() {
    let drive = tempfile::tempdir().unwrap();
    let exe = drive.path().join("dackel-launcher");
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
    let real = drive.path().join("dackel-launcher");
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
    let exe = drive.path().join("dackel-launcher");
    fs::write(&exe, "").unwrap();

    let library = resolve_library(&exe, Some(Path::new("/some/other/lib"))).unwrap();

    assert_eq!(library, Path::new("/some/other/lib"));
}

#[test]
fn a_library_beside_a_writable_program_folder_may_be_created() {
    let drive = tempfile::tempdir().unwrap();
    let library = drive.path().join("DLSiteLibrary");

    assert!(may_create_library(&library));
}

#[test]
fn a_library_beside_a_read_only_program_folder_is_not_offered() {
    let drive = tempfile::tempdir().unwrap();
    let library = drive.path().join("DLSiteLibrary");
    fs::set_permissions(drive.path(), fs::Permissions::from_mode(0o555)).unwrap();

    let offered = may_create_library(&library);
    fs::set_permissions(drive.path(), fs::Permissions::from_mode(0o700)).unwrap();

    assert!(!offered);
}

#[test]
fn a_missing_program_folder_is_not_offered() {
    let drive = tempfile::tempdir().unwrap();
    let library = drive.path().join("gone").join("DLSiteLibrary");

    assert!(!may_create_library(&library));
}

#[test]
fn a_library_inside_a_system_folder_is_never_offered() {
    for dir in [
        "/usr/bin",
        "/bin",
        "/opt/dackel",
        "/app/bin",
        "/usr/local/bin",
    ] {
        let library = Path::new(dir).join("DLSiteLibrary");
        assert!(!may_create_library(&library), "{dir}");
    }
}
