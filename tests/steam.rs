//! Non-Steam game entries: building them, adding, updating and removing them in a parsed
//! `shortcuts.vdf`, and finding the Steam account to write to.

use dackel_launcher::steam::{
    account_with_newest, add_game, crc32, game_dir_of, grid_names, launch_options, new_shortcuts,
    refresh_launcher, remove_game, shortcut_app_id, steam_root, steam_running, write_grid,
};
use dackel_launcher::vdf::{Value, parse, write};
use std::fs;
use std::path::{Path, PathBuf};

fn launcher() -> PathBuf {
    PathBuf::from("/opt/dackel/dackel-launcher")
}

fn entries(root: &Value) -> Vec<&Value> {
    root.get("shortcuts")
        .unwrap()
        .items()
        .iter()
        .map(|(_, v)| v)
        .collect()
}

fn text<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry.get(key).and_then(Value::as_str).unwrap()
}

/// A shortcut of the user's own, which the launcher must never touch.
fn users_own() -> Value {
    Value::Map(vec![
        ("appid".into(), Value::U32(0x8000_0001)),
        ("AppName".into(), Value::Str("Their Game".into())),
        ("Exe".into(), Value::Str("\"/usr/bin/their\"".into())),
        ("LaunchOptions".into(), Value::Str("--fullscreen".into())),
        ("SomethingNew".into(), Value::U64(9)),
    ])
}

fn with_users_own() -> Value {
    let mut root = new_shortcuts();
    if let Some(Value::Map(items)) = root.get_mut("shortcuts") {
        items.push(("0".into(), users_own()));
    }
    root
}

#[test]
fn crc32_matches_the_standard_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn a_shortcut_app_id_has_the_high_bit_set_and_depends_on_exe_and_name() {
    let a = shortcut_app_id("\"/x/launcher\"", "RJ1");
    let b = shortcut_app_id("\"/x/launcher\"", "RJ2");

    assert!(a & 0x8000_0000 != 0);
    assert_ne!(a, b);
    assert_eq!(a, shortcut_app_id("\"/x/launcher\"", "RJ1"));
}

#[test]
fn the_launch_options_name_the_game_folder_and_read_back() {
    let dir = Path::new("/games/RJ123 Title");

    let options = launch_options(dir).unwrap();

    assert_eq!(options, "--launch \"/games/RJ123 Title\"");
    assert_eq!(game_dir_of(&options), Some(dir.to_path_buf()));
    assert_eq!(game_dir_of("--fullscreen"), None);
}

#[test]
fn a_folder_that_cannot_be_quoted_is_refused() {
    assert!(launch_options(Path::new("/games/a\"b")).is_err());
}

#[test]
fn adding_a_game_writes_a_complete_entry_after_the_users_own() {
    let mut root = with_users_own();

    let id = add_game(&mut root, &launcher(), Path::new("/games/RJ1"), "RJ1 Title").unwrap();

    let all = entries(&root);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0], &users_own());
    let ours = all[1];
    assert_eq!(text(ours, "AppName"), "RJ1 Title");
    assert_eq!(text(ours, "Exe"), "\"/opt/dackel/dackel-launcher\"");
    assert_eq!(text(ours, "StartDir"), "\"/opt/dackel\"");
    assert_eq!(text(ours, "LaunchOptions"), "--launch \"/games/RJ1\"");
    assert_eq!(ours.get("appid"), Some(&Value::U32(id)));
    assert!(ours.get("tags").is_some());
    assert_eq!(root.get("shortcuts").unwrap().items()[1].0, "1");
}

#[test]
fn adding_the_same_game_again_updates_it_and_keeps_its_app_id() {
    let mut root = new_shortcuts();
    let first = add_game(&mut root, &launcher(), Path::new("/games/RJ1"), "Old name").unwrap();

    let moved = PathBuf::from("/elsewhere/dackel-launcher");
    let second = add_game(&mut root, &moved, Path::new("/games/RJ1"), "New name").unwrap();

    assert_eq!(first, second);
    let all = entries(&root);
    assert_eq!(all.len(), 1);
    assert_eq!(text(all[0], "AppName"), "New name");
    assert_eq!(text(all[0], "Exe"), "\"/elsewhere/dackel-launcher\"");
}

#[test]
fn removing_a_game_takes_only_its_entry_and_renumbers_the_rest() {
    let mut root = with_users_own();
    add_game(&mut root, &launcher(), Path::new("/games/RJ1"), "One").unwrap();
    add_game(&mut root, &launcher(), Path::new("/games/RJ2"), "Two").unwrap();

    assert!(remove_game(&mut root, Path::new("/games/RJ1")));

    let all = entries(&root);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0], &users_own());
    assert_eq!(text(all[1], "AppName"), "Two");
    let keys: Vec<_> = root
        .get("shortcuts")
        .unwrap()
        .items()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, ["0", "1"]);
    assert!(
        !remove_game(&mut root, Path::new("/games/RJ1")),
        "already gone"
    );
}

#[test]
fn a_moved_launcher_is_written_into_every_entry_of_ours_and_no_other() {
    let mut root = with_users_own();
    let id = add_game(&mut root, &launcher(), Path::new("/games/RJ1"), "One").unwrap();

    let changed = refresh_launcher(&mut root, Path::new("/new/place/dackel-launcher"));

    assert_eq!(changed, 1);
    let all = entries(&root);
    assert_eq!(all[0], &users_own());
    assert_eq!(text(all[1], "Exe"), "\"/new/place/dackel-launcher\"");
    assert_eq!(all[1].get("appid"), Some(&Value::U32(id)));
}

#[test]
fn the_edited_file_still_parses_and_keeps_the_users_unknown_keys() {
    let mut root = with_users_own();
    add_game(&mut root, &launcher(), Path::new("/games/RJ1"), "One").unwrap();

    let again = parse(&write(&root)).unwrap();

    assert_eq!(again, root);
    assert_eq!(entries(&again)[0].get("SomethingNew"), Some(&Value::U64(9)));
}

#[test]
fn an_empty_file_is_a_file_with_no_shortcuts() {
    let root = new_shortcuts();

    assert!(entries(&root).is_empty());
    assert_eq!(parse(&write(&root)).unwrap(), root);
}

#[test]
fn the_account_with_the_newest_file_is_chosen() {
    let accounts = [
        ("111".to_owned(), 100),
        ("222".to_owned(), 300),
        ("333".to_owned(), 200),
    ];

    assert_eq!(account_with_newest(&accounts).as_deref(), Some("222"));
    assert_eq!(account_with_newest(&[]), None);
}

#[test]
fn a_tie_goes_to_the_lower_account_name() {
    let accounts = [("222".to_owned(), 5), ("111".to_owned(), 5)];

    assert_eq!(account_with_newest(&accounts).as_deref(), Some("111"));
}

#[test]
fn steam_is_found_by_its_userdata_folder_in_the_usual_places() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(steam_root(home.path()), None);

    fs::create_dir_all(home.path().join(".local/share/Steam/userdata")).unwrap();
    assert_eq!(
        steam_root(home.path()),
        Some(home.path().join(".local/share/Steam"))
    );
}

#[test]
fn steam_running_is_a_process_named_steam() {
    let proc = tempfile::tempdir().unwrap();
    for (pid, name) in [("10", "bash"), ("11", "steamwebhelper"), ("note", "steam")] {
        fs::create_dir(proc.path().join(pid)).unwrap();
        fs::write(proc.path().join(pid).join("comm"), format!("{name}\n")).unwrap();
    }
    assert!(
        !steam_running(proc.path()),
        "a helper or a non-process does not count"
    );

    fs::create_dir(proc.path().join("12")).unwrap();
    fs::write(proc.path().join("12/comm"), "steam\n").unwrap();
    assert!(steam_running(proc.path()));
}

#[test]
fn the_cover_is_written_as_the_wide_and_the_portrait_grid_image() {
    let dir = tempfile::tempdir().unwrap();
    let cover = dir.path().join("cover.jpg");
    image::RgbImage::from_pixel(8, 12, image::Rgb([200, 30, 30]))
        .save(&cover)
        .unwrap();
    let grid = dir.path().join("grid");

    write_grid(&cover, &grid, 2573499798).unwrap();

    assert_eq!(
        grid_names(2573499798),
        ["2573499798.png", "2573499798p.png"]
    );
    for name in grid_names(2573499798) {
        let img = image::open(grid.join(name)).unwrap();
        assert_eq!((img.width(), img.height()), (8, 12));
    }
}

mod files {
    use super::*;
    use dackel_launcher::steam::{accounts, load_shortcuts, save_shortcuts, shortcuts_path};

    fn steam_with(accounts: &[(&str, Option<&str>)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for (id, file) in accounts {
            let config = root.path().join("userdata").join(id).join("config");
            fs::create_dir_all(&config).unwrap();
            if let Some(file) = file {
                fs::write(config.join(file), b"x").unwrap();
            }
        }
        root
    }

    #[test]
    fn accounts_are_the_numbered_userdata_folders_except_zero() {
        let root = steam_with(&[("0", None), ("123", Some("localconfig.vdf")), ("456", None)]);
        fs::write(root.path().join("userdata/notes.txt"), b"").unwrap();

        let mut names: Vec<_> = accounts(root.path()).into_iter().map(|(n, _)| n).collect();
        names.sort();

        assert_eq!(names, ["123", "456"]);
    }

    #[test]
    fn a_missing_shortcuts_file_loads_as_no_shortcuts() {
        let root = steam_with(&[("123", None)]);
        let path = shortcuts_path(root.path(), "123");

        assert_eq!(load_shortcuts(&path).unwrap(), new_shortcuts());
    }

    #[test]
    fn saving_keeps_the_original_once_as_a_backup() {
        let root = steam_with(&[("123", None)]);
        let path = shortcuts_path(root.path(), "123");
        let mut first = with_users_own();
        save_shortcuts(&path, &first).unwrap();
        assert!(
            !path.with_extension("vdf.dackel-backup").exists(),
            "nothing to back up yet"
        );

        add_game(&mut first, &launcher(), Path::new("/games/RJ1"), "One").unwrap();
        save_shortcuts(&path, &first).unwrap();
        add_game(&mut first, &launcher(), Path::new("/games/RJ2"), "Two").unwrap();
        save_shortcuts(&path, &first).unwrap();

        assert_eq!(load_shortcuts(&path).unwrap(), first);
        let backup = fs::read(path.with_extension("vdf.dackel-backup")).unwrap();
        assert_eq!(
            backup,
            write(&with_users_own()),
            "the backup is the file as it was"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_and_is_left_alone() {
        let root = steam_with(&[("123", None)]);
        let path = shortcuts_path(root.path(), "123");
        fs::write(&path, b"\x42junk").unwrap();

        assert!(load_shortcuts(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"\x42junk");
    }
}

mod launcher_entry {
    use super::*;
    use dackel_launcher::steam::{add_launcher, has_launcher};

    #[test]
    fn the_launcher_is_added_once_with_no_launch_options() {
        let mut root = with_users_own();

        let (id, created) = add_launcher(&mut root, &launcher());
        let (again, created_again) = add_launcher(&mut root, &launcher());

        assert!(created && !created_again);
        assert_eq!(id, again);
        let all = entries(&root);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], &users_own());
        assert_eq!(text(all[1], "AppName"), "Dackel Launcher");
        assert_eq!(text(all[1], "Exe"), "\"/opt/dackel/dackel-launcher\"");
        assert_eq!(text(all[1], "LaunchOptions"), "");
        assert!(has_launcher(&root, &launcher()));
    }

    #[test]
    fn a_shortcut_the_user_made_for_the_launcher_counts_as_added() {
        let mut root = new_shortcuts();
        if let Some(Value::Map(items)) = root.get_mut("shortcuts") {
            items.push((
                "0".into(),
                Value::Map(vec![
                    ("appid".into(), Value::U32(0x8000_1234)),
                    ("AppName".into(), Value::Str("Dackel".into())),
                    (
                        "Exe".into(),
                        Value::Str("\"/opt/dackel/dackel-launcher\"".into()),
                    ),
                    ("LaunchOptions".into(), Value::Str("".into())),
                ]),
            ));
        }

        let (id, created) = add_launcher(&mut root, &launcher());

        assert!(!created);
        assert_eq!(id, 0x8000_1234);
        assert_eq!(entries(&root).len(), 1);
    }

    #[test]
    fn a_game_entry_is_not_the_launcher_and_the_launcher_is_not_a_game() {
        let mut root = new_shortcuts();
        add_game(&mut root, &launcher(), Path::new("/games/RJ1"), "One").unwrap();

        assert!(!has_launcher(&root, &launcher()));
        let (_, created) = add_launcher(&mut root, &launcher());
        assert!(created);
        assert!(remove_game(&mut root, Path::new("/games/RJ1")));
        assert!(
            has_launcher(&root, &launcher()),
            "removing a game leaves the launcher"
        );
    }
}

#[test]
fn writing_the_images_again_replaces_the_old_ones() {
    let dir = tempfile::tempdir().unwrap();
    let grid = dir.path().join("grid");
    let cover = dir.path().join("cover.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3]))
        .save(&cover)
        .unwrap();
    write_grid(&cover, &grid, 77).unwrap();

    image::RgbImage::from_pixel(6, 9, image::Rgb([9, 9, 9]))
        .save(&cover)
        .unwrap();
    write_grid(&cover, &grid, 77).unwrap();

    let img = image::open(grid.join("77p.png")).unwrap();
    assert_eq!((img.width(), img.height()), (6, 9));
}
