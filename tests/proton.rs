use dlsite_launcher::proton::resolve;
use std::fs;
use std::path::{Path, PathBuf};

fn compat_with(dirs: &[&str]) -> tempfile::TempDir {
    let compat = tempfile::tempdir().unwrap();
    for d in dirs {
        fs::create_dir(compat.path().join(d)).unwrap();
    }
    compat
}

#[test]
fn machine_override_beats_env_and_installed_versions() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("proton-path");
    fs::write(&config, "/opt/GE-Proton9-1\n").unwrap();
    let compat = compat_with(&["GE-Proton10-1"]);

    let found = resolve(&config, Some(Path::new("/env/Proton")), compat.path()).unwrap();

    assert_eq!(found, Some(PathBuf::from("/opt/GE-Proton9-1")));
}

#[test]
fn protonpath_env_beats_installed_versions() {
    let root = tempfile::tempdir().unwrap();
    let compat = compat_with(&["GE-Proton10-1"]);

    let found = resolve(
        &root.path().join("missing"),
        Some(Path::new("/env/GE-Proton8-2")),
        compat.path(),
    )
    .unwrap();

    assert_eq!(found, Some(PathBuf::from("/env/GE-Proton8-2")));
}

#[test]
fn newest_installed_ge_proton_is_used_comparing_numbers_not_text() {
    let root = tempfile::tempdir().unwrap();
    let compat = compat_with(&[
        "GE-Proton9-27",
        "GE-Proton10-1",
        "GE-Proton9-5",
        "Proton-Experimental",
    ]);
    fs::write(compat.path().join("GE-Proton11-0"), "a file, not a Proton").unwrap();

    let found = resolve(&root.path().join("missing"), None, compat.path()).unwrap();

    assert_eq!(found, Some(compat.path().join("GE-Proton10-1")));
}

#[test]
fn nothing_configured_or_installed_leaves_umu_to_choose() {
    let root = tempfile::tempdir().unwrap();
    let compat = compat_with(&[]);

    let found = resolve(&root.path().join("missing"), None, compat.path()).unwrap();

    assert_eq!(found, None);
}
