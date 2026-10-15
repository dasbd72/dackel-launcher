//! `scripts/install.sh` run against a throwaway home and a fake release in a local folder.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const APP_ID: &str = "io.github.dasbd72.DackelLauncher";

/// Where the fake release keeps the files of the newest release, and of a tagged one.
const LATEST: &str = "latest/download";
const TAGGED: &str = "download/v9.9.9";

/// A home directory and a release site, both under one temporary folder.
struct World {
    root: TempDir,
}

impl World {
    fn new() -> Self {
        let world = World {
            root: tempfile::tempdir().unwrap(),
        };
        fs::create_dir_all(world.home()).unwrap();
        world
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn data(&self) -> PathBuf {
        self.home().join(".local/share")
    }

    fn install_dir(&self) -> PathBuf {
        self.data().join("dackel-launcher")
    }

    fn entry(&self) -> PathBuf {
        self.data()
            .join("applications")
            .join(format!("{APP_ID}.desktop"))
    }

    fn icon(&self) -> PathBuf {
        self.data()
            .join("icons/hicolor/scalable/apps")
            .join(format!("{APP_ID}.svg"))
    }

    fn release(&self, dir: &str) -> PathBuf {
        self.root.path().join("release").join(dir)
    }

    /// Publishes a release the way the workflow does: the two programs, the menu entry and
    /// its icon (the real ones from `packaging/`), and `SHA256SUMS` for all of them.
    fn publish(&self, dir: &str, launcher: &str, umu: &str) {
        let release = self.release(dir);
        fs::create_dir_all(&release).unwrap();
        fs::write(release.join("dackel-launcher"), launcher).unwrap();
        fs::write(release.join("umu-run"), umu).unwrap();
        for name in [format!("{APP_ID}.desktop"), format!("{APP_ID}.svg")] {
            fs::copy(Path::new("packaging").join(&name), release.join(&name)).unwrap();
        }
        let sums = Command::new("sha256sum")
            .current_dir(&release)
            .args(["dackel-launcher", "umu-run"])
            .arg(format!("{APP_ID}.desktop"))
            .arg(format!("{APP_ID}.svg"))
            .output()
            .unwrap();
        assert!(sums.status.success());
        fs::write(release.join("SHA256SUMS"), sums.stdout).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new("sh")
            .arg("scripts/install.sh")
            .args(args)
            .env("HOME", self.home())
            .env_remove("XDG_DATA_HOME")
            .env(
                "DACKEL_RELEASE_BASE",
                format!("file://{}", self.root.path().join("release").display()),
            )
            .output()
            .unwrap()
    }

    fn install(&self, args: &[&str]) {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
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
fn a_fresh_install_puts_both_programs_in_one_folder_and_creates_no_library() {
    let world = World::new();
    world.publish(LATEST, "launcher v1", "umu v1");

    world.install(&[]);

    let bin = world.install_dir().join("bin");
    assert_eq!(
        fs::read_to_string(bin.join("dackel-launcher")).unwrap(),
        "launcher v1"
    );
    assert_eq!(fs::read_to_string(bin.join("umu-run")).unwrap(), "umu v1");
    assert!(is_executable(&bin.join("dackel-launcher")));
    assert!(is_executable(&bin.join("umu-run")));
    assert_eq!(names_in(&world.install_dir()), ["bin"]);
    assert_eq!(names_in(&bin), ["dackel-launcher", "umu-run"]);
}

fn desktop_value(desktop: &str, key: &str) -> String {
    desktop
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("desktop entry has no {key}"))
        .to_owned()
}

#[test]
fn the_menu_entry_starts_the_installed_program_by_absolute_path_and_keeps_the_rest_of_the_template()
{
    let world = World::new();
    world.publish(LATEST, "launcher v1", "umu v1");

    world.install(&[]);

    let entry = fs::read_to_string(world.entry()).unwrap();
    let template = fs::read_to_string(format!("packaging/{APP_ID}.desktop")).unwrap();
    let installed = world.install_dir().join("bin/dackel-launcher");
    assert_eq!(
        desktop_value(&entry, "Exec"),
        installed.display().to_string()
    );
    for key in ["Name", "Icon", "Categories", "StartupWMClass", "Keywords"] {
        assert_eq!(desktop_value(&entry, key), desktop_value(&template, key));
    }
    assert_eq!(
        fs::read(world.icon()).unwrap(),
        fs::read(format!("packaging/{APP_ID}.svg")).unwrap()
    );
}

#[test]
fn a_file_that_does_not_match_the_checksums_stops_the_install_and_keeps_the_previous_one() {
    let world = World::new();
    world.publish(LATEST, "launcher v1", "umu v1");
    world.install(&[]);
    let entry_before = fs::read(world.entry()).unwrap();
    world.publish(LATEST, "launcher v2", "umu v2");
    fs::write(world.release(LATEST).join("umu-run"), "tampered").unwrap();

    let out = world.run(&[]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("umu-run does not match SHA256SUMS"));
    let bin = world.install_dir().join("bin");
    assert_eq!(
        fs::read_to_string(bin.join("dackel-launcher")).unwrap(),
        "launcher v1"
    );
    assert_eq!(fs::read_to_string(bin.join("umu-run")).unwrap(), "umu v1");
    assert_eq!(fs::read(world.entry()).unwrap(), entry_before);
}

#[test]
fn running_it_again_updates_the_programs_and_leaves_games_and_config_alone() {
    let world = World::new();
    world.publish(LATEST, "launcher v1", "umu v1");
    world.install(&[]);
    let game_data = world
        .home()
        .join("Games/RJ123456/.dackel-launcher/info.json");
    let config = world.home().join(".config/dackel-launcher/settings.json");
    for file in [&game_data, &config] {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "mine").unwrap();
    }
    world.publish(LATEST, "launcher v2", "umu v2");

    world.install(&[]);

    let bin = world.install_dir().join("bin");
    assert_eq!(
        fs::read_to_string(bin.join("dackel-launcher")).unwrap(),
        "launcher v2"
    );
    assert_eq!(fs::read_to_string(bin.join("umu-run")).unwrap(), "umu v2");
    assert_eq!(fs::read_to_string(&game_data).unwrap(), "mine");
    assert_eq!(fs::read_to_string(&config).unwrap(), "mine");
}

#[test]
fn a_tag_argument_installs_that_release_instead_of_the_latest() {
    let world = World::new();
    world.publish(LATEST, "launcher latest", "umu latest");
    world.publish(TAGGED, "launcher tagged", "umu tagged");

    world.install(&["v9.9.9"]);

    let bin = world.install_dir().join("bin");
    assert_eq!(
        fs::read_to_string(bin.join("dackel-launcher")).unwrap(),
        "launcher tagged"
    );
    assert_eq!(
        fs::read_to_string(bin.join("umu-run")).unwrap(),
        "umu tagged"
    );
}

#[test]
fn an_unknown_tag_fails_with_a_message_and_installs_nothing() {
    let world = World::new();
    world.publish(LATEST, "launcher latest", "umu latest");

    let out = world.run(&["v0.0.1"]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("could not download"));
    assert!(!world.install_dir().join("bin/dackel-launcher").exists());
    assert!(!world.entry().exists());
}

#[test]
fn uninstall_removes_the_program_folder_the_entry_and_the_icon_and_nothing_else() {
    let world = World::new();
    world.publish(LATEST, "launcher v1", "umu v1");
    world.install(&[]);
    let other_entry = world.data().join("applications/other.desktop");
    let config = world.home().join(".config/dackel-launcher/settings.json");
    let game_data = world
        .home()
        .join("Games/RJ123456/.dackel-launcher/info.json");
    for file in [&other_entry, &config, &game_data] {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "mine").unwrap();
    }

    world.install(&["--uninstall"]);

    assert!(!world.install_dir().exists());
    assert!(!world.entry().exists());
    assert!(!world.icon().exists());
    for file in [&other_entry, &config, &game_data] {
        assert_eq!(fs::read_to_string(file).unwrap(), "mine");
    }
}

#[test]
fn uninstalling_when_nothing_is_installed_is_not_an_error() {
    let world = World::new();

    world.install(&["--uninstall"]);
}
