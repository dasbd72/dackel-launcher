use dackel_launcher::launch::{LaunchPlan, Launcher, Process, Spawner, plan_with};
use dackel_launcher::profile::{self, ExeState, Tuning, read_tuning, set_proton_pin, set_tuning};
use dackel_launcher::proton::{ProtonChoice, installed, resolve_for_game, selectable};
use dackel_launcher::tuning::{
    format_env, parse_env, prefix_dir, reset_prefix, split_args, winetricks_command,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

mod env_text {
    use super::*;

    #[test]
    fn one_key_value_per_line_blank_lines_and_comments_ignored() {
        let text = "DXVK_HUD=fps\n\n# a note\n  WINEDEBUG = -all  \nFOO=a=b\n";

        assert_eq!(
            parse_env(text).unwrap(),
            env(&[("DXVK_HUD", "fps"), ("WINEDEBUG", "-all"), ("FOO", "a=b")])
        );
    }

    #[test]
    fn a_line_without_equals_is_an_error_naming_the_line() {
        let err = parse_env("A=1\nnot a pair\n").unwrap_err();

        assert!(err.contains("line 2"), "{err}");
    }

    #[test]
    fn a_bad_variable_name_is_an_error() {
        assert!(parse_env("1BAD=x").is_err());
        assert!(parse_env("A B=x").is_err());
        assert!(parse_env("=x").is_err());
    }

    #[test]
    fn formatting_then_parsing_gives_the_same_map() {
        let map = env(&[("A", "1"), ("B", "two words")]);

        assert_eq!(parse_env(&format_env(&map)).unwrap(), map);
    }
}

mod args_text {
    use super::*;

    #[test]
    fn splits_on_whitespace_and_honors_quotes() {
        assert_eq!(
            split_args(r#"-windowed  --name "Mr Smith" 'a b' c\ d"#).unwrap(),
            ["-windowed", "--name", "Mr Smith", "a b", "c d"]
        );
    }

    #[test]
    fn empty_text_means_no_arguments() {
        assert!(split_args("   ").unwrap().is_empty());
    }

    #[test]
    fn an_unclosed_quote_is_an_error() {
        assert!(split_args(r#"--name "Mr Smith"#).is_err());
    }
}

mod stored {
    use super::*;

    #[test]
    fn a_game_without_tuning_has_empty_defaults() {
        let game = tempfile::tempdir().unwrap();

        assert_eq!(read_tuning(game.path()).unwrap(), Tuning::default());
    }

    #[test]
    fn env_and_args_round_trip_in_the_profile() {
        let game = tempfile::tempdir().unwrap();

        set_tuning(game.path(), env(&[("DXVK_HUD", "fps")]), "-windowed").unwrap();

        let t = read_tuning(game.path()).unwrap();
        assert_eq!(t.env, env(&[("DXVK_HUD", "fps")]));
        assert_eq!(t.args, "-windowed");
    }

    #[test]
    fn saving_tuning_keeps_the_rest_of_the_profile() {
        let game = tempfile::tempdir().unwrap();
        fs::write(game.path().join("Game.exe"), b"").unwrap();
        profile::ensure(game.path()).unwrap();
        profile::set_favorite(game.path(), true).unwrap();
        set_proton_pin(game.path(), Some("GE-Proton9-27")).unwrap();

        set_tuning(game.path(), env(&[("A", "1")]), "").unwrap();

        assert!(profile::read_stats(game.path()).unwrap().favorite);
        assert!(matches!(
            profile::ensure(game.path()).unwrap(),
            ExeState::Ready { .. }
        ));
        assert_eq!(
            read_tuning(game.path()).unwrap().proton.as_deref(),
            Some("GE-Proton9-27")
        );
    }

    #[test]
    fn the_pin_can_be_set_and_cleared() {
        let game = tempfile::tempdir().unwrap();

        set_proton_pin(game.path(), Some("GE-Proton9-27")).unwrap();
        assert_eq!(
            read_tuning(game.path()).unwrap().proton.as_deref(),
            Some("GE-Proton9-27")
        );
        set_proton_pin(game.path(), None).unwrap();
        assert_eq!(read_tuning(game.path()).unwrap().proton, None);
    }
}

mod launch_plan {
    use super::*;

    fn tuning(pairs: &[(&str, &str)], args: &str) -> Tuning {
        Tuning {
            env: env(pairs),
            args: args.to_owned(),
            proton: None,
        }
    }

    #[test]
    fn user_env_is_added_to_the_launch_env() {
        let p = plan_with(
            Path::new("/lib/RJ1"),
            Path::new("Game.exe"),
            None,
            &tuning(&[("DXVK_HUD", "fps")], ""),
        );

        assert_eq!(p.env.get("DXVK_HUD").map(String::as_str), Some("fps"));
        assert_eq!(p.env.get("GAMEID").map(String::as_str), Some("umu-RJ1"));
    }

    #[test]
    fn user_env_can_override_the_defaults_but_never_the_prefix() {
        let p = plan_with(
            Path::new("/lib/RJ1"),
            Path::new("Game.exe"),
            None,
            &tuning(&[("LANG", "en_US.UTF-8"), ("WINEPREFIX", "/elsewhere")], ""),
        );

        assert_eq!(p.env.get("LANG").map(String::as_str), Some("en_US.UTF-8"));
        assert_eq!(
            p.env.get("WINEPREFIX").map(String::as_str),
            Some("/lib/RJ1/.dackel-launcher/wine")
        );
    }

    #[test]
    fn extra_args_follow_the_exe() {
        let p = plan_with(
            Path::new("/lib/RJ1"),
            Path::new("Game.exe"),
            None,
            &tuning(&[], r#"-windowed --name "A B""#),
        );

        assert_eq!(
            p.args,
            vec![
                OsString::from("/lib/RJ1/Game.exe"),
                OsString::from("-windowed"),
                OsString::from("--name"),
                OsString::from("A B"),
            ]
        );
    }

    #[test]
    fn unparsable_args_are_passed_through_whitespace_split_rather_than_dropped() {
        let p = plan_with(
            Path::new("/lib/RJ1"),
            Path::new("Game.exe"),
            None,
            &tuning(&[], r#"--name "A"#),
        );

        assert!(p.args.len() >= 2, "{:?}", p.args);
    }
}

#[derive(Clone, Default)]
struct Fake {
    plans: Arc<Mutex<Vec<LaunchPlan>>>,
}

struct Never;

impl Process for Never {
    fn has_exited(&mut self) -> io::Result<bool> {
        Ok(false)
    }
}

impl Spawner for Fake {
    fn spawn(&self, plan: &LaunchPlan) -> io::Result<Box<dyn Process>> {
        self.plans.lock().unwrap().push(plan.clone());
        Ok(Box::new(Never))
    }
}

mod launching {
    use super::*;

    fn game() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Game.exe"), b"").unwrap();
        dir
    }

    fn ready() -> ExeState {
        ExeState::Ready {
            exe: PathBuf::from("Game.exe"),
        }
    }

    #[test]
    fn saved_env_and_args_reach_the_spawned_process() {
        let game = game();
        set_tuning(game.path(), env(&[("DXVK_HUD", "fps")]), "-windowed").unwrap();
        let fake = Fake::default();
        let mut launcher = Launcher::new(fake.clone());

        launcher.start(game.path(), &ready(), None, 1000).unwrap();

        let plans = fake.plans.lock().unwrap();
        assert_eq!(
            plans[0].env.get("DXVK_HUD").map(String::as_str),
            Some("fps")
        );
        assert_eq!(plans[0].args.last(), Some(&OsString::from("-windowed")));
    }

    #[test]
    fn a_launch_without_a_choice_pins_nothing() {
        let game = game();
        let mut launcher = Launcher::new(Fake::default());

        launcher
            .start(
                game.path(),
                &ready(),
                Some(Path::new("/compat/GE-Proton9-27")),
                1000,
            )
            .unwrap();

        assert_eq!(read_tuning(game.path()).unwrap().proton, None);
    }

    #[test]
    fn a_later_launch_does_not_move_an_existing_pin() {
        let game = game();
        set_proton_pin(game.path(), Some("GE-Proton9-27")).unwrap();
        let mut launcher = Launcher::new(Fake::default());

        launcher
            .start(
                game.path(),
                &ready(),
                Some(Path::new("/compat/GE-Proton10-1")),
                1000,
            )
            .unwrap();

        assert_eq!(
            read_tuning(game.path()).unwrap().proton.as_deref(),
            Some("GE-Proton9-27")
        );
    }

    #[test]
    fn with_no_proton_chosen_nothing_is_pinned() {
        let game = game();
        let mut launcher = Launcher::new(Fake::default());

        launcher.start(game.path(), &ready(), None, 1000).unwrap();

        assert_eq!(read_tuning(game.path()).unwrap().proton, None);
    }
}

mod proton_pin {
    use super::*;

    fn compat_with(dirs: &[&str]) -> tempfile::TempDir {
        let compat = tempfile::tempdir().unwrap();
        for d in dirs {
            fs::create_dir(compat.path().join(d)).unwrap();
        }
        compat
    }

    fn resolve(compat: &Path, pinned: Option<&str>) -> ProtonChoice {
        let nothing = compat.join("no-such-proton-path-file");
        resolve_for_game(&nothing, None, compat, pinned).unwrap()
    }

    #[test]
    fn no_pin_uses_the_newest_installed() {
        let compat = compat_with(&["GE-Proton9-27", "GE-Proton10-1"]);

        assert_eq!(
            resolve(compat.path(), None),
            ProtonChoice::Use(Some(compat.path().join("GE-Proton10-1")))
        );
    }

    #[test]
    fn an_installed_pin_is_used_even_when_newer_versions_exist() {
        let compat = compat_with(&["GE-Proton9-27", "GE-Proton10-1"]);

        assert_eq!(
            resolve(compat.path(), Some("GE-Proton9-27")),
            ProtonChoice::Use(Some(compat.path().join("GE-Proton9-27")))
        );
    }

    #[test]
    fn a_pin_that_is_not_installed_is_a_mismatch_offering_the_newest() {
        let compat = compat_with(&["GE-Proton10-1", "GE-Proton9-1"]);

        assert_eq!(
            resolve(compat.path(), Some("GE-Proton9-27")),
            ProtonChoice::Mismatch {
                pinned: "GE-Proton9-27".to_owned(),
                newest: Some(compat.path().join("GE-Proton10-1")),
            }
        );
    }

    #[test]
    fn a_mismatch_with_nothing_installed_has_no_newest() {
        let compat = compat_with(&[]);

        assert_eq!(
            resolve(compat.path(), Some("GE-Proton9-27")),
            ProtonChoice::Mismatch {
                pinned: "GE-Proton9-27".to_owned(),
                newest: None,
            }
        );
    }

    #[test]
    fn a_pin_that_is_a_path_rather_than_a_folder_name_is_never_followed() {
        let compat = compat_with(&["GE-Proton10-1"]);
        let outside = tempfile::tempdir().unwrap();

        for pin in [outside.path().to_str().unwrap(), "../", "GE-Proton10-1/.."] {
            assert!(
                matches!(
                    resolve(compat.path(), Some(pin)),
                    ProtonChoice::Mismatch { .. }
                ),
                "{pin}"
            );
        }
    }

    #[test]
    fn an_explicit_machine_choice_wins_and_raises_no_warning() {
        let compat = compat_with(&["GE-Proton10-1"]);
        let config = compat.path().join("proton-path");
        fs::write(&config, "/opt/GE-Proton8-1\n").unwrap();

        let choice = resolve_for_game(&config, None, compat.path(), Some("GE-Proton9-27")).unwrap();

        assert_eq!(
            choice,
            ProtonChoice::Use(Some(PathBuf::from("/opt/GE-Proton8-1")))
        );
    }

    #[test]
    fn an_installed_pin_beats_the_machine_choice() {
        let compat = compat_with(&["GE-Proton9-27"]);
        let config = compat.path().join("proton-path");
        fs::write(&config, "/opt/GE-Proton8-1\n").unwrap();

        let choice = resolve_for_game(&config, None, compat.path(), Some("GE-Proton9-27")).unwrap();

        assert_eq!(
            choice,
            ProtonChoice::Use(Some(compat.path().join("GE-Proton9-27")))
        );
    }

    #[test]
    fn selectable_versions_are_every_folder_with_a_proton_script_newest_first() {
        let compat = compat_with(&[
            "GE-Proton9-27",
            "GE-Proton10-1",
            "Proton-CachyOS-10",
            "NoScript",
        ]);
        for d in ["GE-Proton9-27", "GE-Proton10-1", "Proton-CachyOS-10"] {
            fs::write(compat.path().join(d).join("proton"), "#!/bin/sh\n").unwrap();
        }
        fs::write(compat.path().join("a-file"), "x").unwrap();

        assert_eq!(
            selectable(compat.path()).unwrap(),
            ["GE-Proton10-1", "Proton-CachyOS-10", "GE-Proton9-27"]
        );
    }

    #[test]
    fn nothing_is_selectable_without_a_compat_folder() {
        let root = tempfile::tempdir().unwrap();

        assert!(selectable(&root.path().join("missing")).unwrap().is_empty());
    }

    #[test]
    fn installed_versions_are_listed_newest_first() {
        let compat = compat_with(&["GE-Proton9-27", "GE-Proton10-1", "Other", "GE-Proton9-5"]);

        let names: Vec<String> = installed(compat.path())
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["GE-Proton10-1", "GE-Proton9-27", "GE-Proton9-5"]);
    }
}

mod prefix {
    use super::*;

    #[test]
    fn the_prefix_lives_in_the_games_own_metadata() {
        assert_eq!(
            prefix_dir(Path::new("/lib/RJ1")),
            PathBuf::from("/lib/RJ1/.dackel-launcher/wine")
        );
    }

    #[test]
    fn reset_removes_only_that_games_prefix() {
        let lib = tempfile::tempdir().unwrap();
        let a = lib.path().join("RJ1");
        let b = lib.path().join("RJ2");
        for game in [&a, &b] {
            fs::create_dir_all(prefix_dir(game).join("drive_c")).unwrap();
            fs::write(game.join("Game.exe"), b"").unwrap();
        }
        set_tuning(&a, env(&[("A", "1")]), "-x").unwrap();
        fs::write(a.join(".dackel-launcher/last-run.log"), b"log").unwrap();

        reset_prefix(&a).unwrap();

        assert!(!prefix_dir(&a).exists());
        assert!(a.join("Game.exe").exists(), "game files are untouched");
        assert!(a.join(".dackel-launcher/last-run.log").exists());
        assert_eq!(read_tuning(&a).unwrap().args, "-x", "tuning is kept");
        assert!(
            prefix_dir(&b).join("drive_c").exists(),
            "other game's prefix kept"
        );
    }

    #[test]
    fn resetting_a_game_that_has_no_prefix_is_fine() {
        let game = tempfile::tempdir().unwrap();

        assert!(reset_prefix(game.path()).is_ok());
    }
}

mod winetricks {
    use super::*;

    #[test]
    fn the_command_points_winetricks_at_the_games_prefix() {
        assert_eq!(
            winetricks_command(Path::new("/lib/RJ1")),
            "WINEPREFIX='/lib/RJ1/.dackel-launcher/wine' winetricks"
        );
    }

    #[test]
    fn paths_with_spaces_and_quotes_are_shell_safe() {
        assert_eq!(
            winetricks_command(Path::new("/my lib/it's/RJ1")),
            r#"WINEPREFIX='/my lib/it'\''s/RJ1/.dackel-launcher/wine' winetricks"#
        );
    }
}

#[test]
fn quotes_around_an_env_value_are_dropped_as_a_shell_would() {
    use dackel_launcher::tuning::parse_env;
    let env = parse_env("A=\"winewayland.drv=d\"\nB='x y'\nC=\"half\nD=\"\"\nE=plain\n").unwrap();

    assert_eq!(env["A"], "winewayland.drv=d");
    assert_eq!(env["B"], "x y");
    assert_eq!(env["C"], "\"half");
    assert_eq!(env["D"], "");
    assert_eq!(env["E"], "plain");
}
