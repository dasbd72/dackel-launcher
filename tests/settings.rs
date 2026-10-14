use dlsite_launcher::settings::{Settings, load, save, settings_path};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn missing_file_gives_the_defaults() {
    let dir = tempfile::tempdir().unwrap();

    let s = load(&dir.path().join("settings.json"));

    assert_eq!(s, Settings::default());
    assert_eq!(s.ui_scale, None);
    assert_eq!(s.fullscreen, None);
}

#[test]
fn saved_settings_round_trip_and_create_the_config_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dlsite-launcher").join("settings.json");
    let wanted = Settings {
        ui_scale: Some(1.5),
        fullscreen: Some(true),
    };

    save(&path, &wanted).unwrap();

    assert_eq!(load(&path), wanted);
}

#[test]
fn a_corrupt_file_falls_back_to_the_defaults_instead_of_failing_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, "{ not json").unwrap();

    assert_eq!(load(&path), Settings::default());
}

#[test]
fn unknown_fields_are_ignored_so_newer_files_still_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, r#"{"ui_scale": 1.25, "from_the_future": true}"#).unwrap();

    assert_eq!(load(&path).ui_scale, Some(1.25));
}

#[test]
fn an_absurd_ui_scale_is_clamped_to_a_usable_range() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");

    fs::write(&path, r#"{"ui_scale": 9.0}"#).unwrap();
    assert_eq!(load(&path).ui_scale, Some(3.0));

    fs::write(&path, r#"{"ui_scale": 0.0}"#).unwrap();
    assert_eq!(load(&path).ui_scale, Some(0.5));
}

#[test]
fn xdg_config_home_wins_over_home() {
    let path = settings_path(Some(Path::new("/xdg")), Some(Path::new("/home/u")));

    assert_eq!(
        path,
        Some(PathBuf::from("/xdg/dlsite-launcher/settings.json"))
    );
}

#[test]
fn without_xdg_it_uses_dot_config_under_home() {
    let path = settings_path(None, Some(Path::new("/home/u")));

    assert_eq!(
        path,
        Some(PathBuf::from(
            "/home/u/.config/dlsite-launcher/settings.json"
        ))
    );
}

#[test]
fn without_either_there_is_nowhere_to_save() {
    assert_eq!(settings_path(None, None), None);
    assert_eq!(
        settings_path(Some(Path::new("")), None),
        None,
        "empty XDG is unset"
    );
}
