//! The files the AUR and Flatpak packages install must agree with each other and with the crate.

use std::fs;

const APP_ID: &str = "io.github.dasbd72.DackelLauncher";

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("{path}: {err}"))
}

fn desktop_value(desktop: &str, key: &str) -> String {
    desktop
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("desktop entry has no {key}"))
        .to_owned()
}

#[test]
fn desktop_entry_starts_the_binary_and_names_the_icon_after_the_app_id() {
    let desktop = read(&format!("packaging/{APP_ID}.desktop"));

    assert_eq!(desktop_value(&desktop, "Exec"), env!("CARGO_PKG_NAME"));
    assert_eq!(desktop_value(&desktop, "Icon"), APP_ID);
    assert_eq!(desktop_value(&desktop, "StartupWMClass"), APP_ID);
    assert_eq!(desktop_value(&desktop, "Categories"), "Game;");
    assert!(
        fs::metadata(format!("packaging/{APP_ID}.svg")).is_ok(),
        "the icon file is named after the app id"
    );
}

#[test]
fn metainfo_belongs_to_the_desktop_entry_and_declares_the_crate_license() {
    let metainfo = read(&format!("packaging/{APP_ID}.metainfo.xml"));

    assert!(metainfo.contains(&format!("<id>{APP_ID}</id>")));
    assert!(metainfo.contains(&format!("<launchable type=\"desktop-id\">{APP_ID}.desktop")));
    assert!(metainfo.contains(&format!(
        "<project_license>{}</project_license>",
        env!("CARGO_PKG_LICENSE")
    )));
    assert!(metainfo.contains(&format!(
        "<release version=\"{}\"",
        env!("CARGO_PKG_VERSION")
    )));
}

#[test]
fn the_mit_license_text_ships_with_the_project() {
    assert_eq!(env!("CARGO_PKG_LICENSE"), "MIT");
    assert!(read("LICENSE").starts_with("MIT License"));
    assert!(read("assets/icons/LICENSE-phosphor.txt").contains("Phosphor"));
}
