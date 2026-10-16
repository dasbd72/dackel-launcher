//! Installing winetricks verbs into a game's prefix: reading what was typed, the command that
//! runs, and what is remembered afterwards.

use dackel_launcher::profile::{self, Tuning};
use dackel_launcher::tuning::{prefix_dir, reset_prefix};
use dackel_launcher::winetricks::{COMMON, install, log_path, parse_verbs, plan, toggle_verb};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

mod typing {
    use super::*;

    #[test]
    fn verbs_are_the_words_separated_by_any_spaces() {
        assert_eq!(
            parse_verbs("  dotnet48   win7\tvcrun2019 ").unwrap(),
            words(&["dotnet48", "win7", "vcrun2019"])
        );
        assert_eq!(
            parse_verbs("fontsmooth=rgb d3dx9_43").unwrap(),
            words(&["fontsmooth=rgb", "d3dx9_43"])
        );
    }

    #[test]
    fn nothing_typed_is_an_error() {
        assert!(parse_verbs("").is_err());
        assert!(parse_verbs("  \n ").is_err());
    }

    #[test]
    fn anything_a_shell_would_read_is_refused_and_named() {
        for bad in [
            "a;b",
            "$(x)",
            "a&&b",
            "`x`",
            "../x",
            "a|b",
            "-q",
            "--unattended",
            "a\"b",
        ] {
            let err = parse_verbs(&format!("dotnet48 {bad}")).unwrap_err();
            assert!(
                err.contains(bad.split_whitespace().next().unwrap()),
                "{bad}: {err}"
            );
        }
    }

    #[test]
    fn the_common_choices_are_all_valid_verbs() {
        for choice in COMMON {
            assert_eq!(parse_verbs(choice.verb).unwrap(), words(&[choice.verb]));
            assert!(!choice.hint.is_empty());
        }
    }

    #[test]
    fn a_chip_adds_its_verb_once_and_takes_it_out_again() {
        assert_eq!(toggle_verb("", "dotnet48"), "dotnet48");
        assert_eq!(toggle_verb("win7", "dotnet48"), "win7 dotnet48");
        assert_eq!(toggle_verb("win7 dotnet48", "dotnet48"), "win7");
        assert_eq!(toggle_verb("dotnet48", "dotnet48"), "");
        assert_eq!(toggle_verb("dotnet480", "dotnet48"), "dotnet480 dotnet48");
    }
}

mod the_command {
    use super::*;

    #[test]
    fn runs_winetricks_quietly_on_the_games_prefix_with_its_proton_and_overrides() {
        let game = Path::new("/library/RJ1");
        let proton = Path::new("/p/GE-Proton11-9");

        let p = plan(
            game,
            &words(&["dotnet48", "win7"]),
            Some(proton),
            &Tuning::default(),
        );

        assert_eq!(p.program, "umu-run");
        assert_eq!(p.args, ["winetricks", "-q", "dotnet48", "win7"]);
        assert_eq!(
            p.env.get("WINEPREFIX").map(String::as_str),
            Some("/library/RJ1/.dackel-launcher/wine")
        );
        assert_eq!(
            p.env.get("PROTONPATH").map(String::as_str),
            Some("/p/GE-Proton11-9")
        );
        assert_eq!(
            p.env.get("WINEDLLOVERRIDES").map(String::as_str),
            Some("winewayland.drv=d")
        );
        assert_eq!(p.env.get("GAMEID").map(String::as_str), Some("umu-RJ1"));
        assert_eq!(p.log_path, log_path(game));
        assert!(p.log_path.ends_with("RJ1/.dackel-launcher/winetricks.log"));
        assert_eq!(p.setup, None);
    }
}

fn fake_umu(dir: &Path, exit: i32) -> std::path::PathBuf {
    let script = dir.join("fake-umu");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho \"$@\" >> \"$CALLS\"\necho output-line\nexit {exit}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

mod installing {
    use super::*;

    fn setup(
        exit: i32,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        dackel_launcher::launch::LaunchPlan,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("RJ1");
        std::fs::create_dir_all(game.join(".dackel-launcher")).unwrap();
        let mut p = plan(&game, &words(&["dotnet48"]), None, &Tuning::default());
        p.program = fake_umu(dir.path(), exit).display().to_string();
        p.env.insert(
            "CALLS".to_owned(),
            dir.path().join("calls").display().to_string(),
        );
        (dir, game, p)
    }

    #[test]
    fn a_successful_run_is_logged_and_remembered() {
        let (dir, game, p) = setup(0);

        install(&game, &p, &words(&["dotnet48"])).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("calls"))
                .unwrap()
                .trim(),
            "winetricks -q dotnet48"
        );
        assert!(
            std::fs::read_to_string(log_path(&game))
                .unwrap()
                .contains("output-line")
        );
        assert_eq!(
            profile::read_winetricks(&game).unwrap(),
            words(&["dotnet48"])
        );
    }

    #[test]
    fn a_game_with_no_metadata_folder_yet_still_gets_its_log() {
        let (_dir, game, p) = setup(0);
        std::fs::remove_dir_all(game.join(".dackel-launcher")).unwrap();

        install(&game, &p, &words(&["dotnet48"])).unwrap();

        assert!(log_path(&game).exists());
    }

    #[test]
    fn a_failed_run_says_where_the_log_is_and_remembers_nothing() {
        let (_dir, game, p) = setup(3);

        let err = install(&game, &p, &words(&["dotnet48"])).unwrap_err();

        assert!(err.contains("winetricks.log"), "{err}");
        assert!(profile::read_winetricks(&game).unwrap().is_empty());
    }
}

mod remembering {
    use super::*;

    #[test]
    fn verbs_keep_their_order_and_are_not_listed_twice() {
        let dir = tempfile::tempdir().unwrap();

        profile::add_winetricks(dir.path(), &words(&["corefonts", "dotnet48"])).unwrap();
        profile::add_winetricks(dir.path(), &words(&["dotnet48", "vcrun2022"])).unwrap();

        assert_eq!(
            profile::read_winetricks(dir.path()).unwrap(),
            words(&["corefonts", "dotnet48", "vcrun2022"])
        );
    }

    #[test]
    fn a_game_with_nothing_installed_has_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();

        assert!(profile::read_winetricks(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn resetting_the_prefix_forgets_them_but_keeps_the_rest_of_the_profile() {
        let dir = tempfile::tempdir().unwrap();
        profile::set_favorite(dir.path(), true).unwrap();
        profile::add_winetricks(dir.path(), &words(&["dotnet48"])).unwrap();
        std::fs::create_dir_all(prefix_dir(dir.path())).unwrap();

        reset_prefix(dir.path()).unwrap();

        assert!(profile::read_winetricks(dir.path()).unwrap().is_empty());
        assert!(profile::read_stats(dir.path()).unwrap().favorite);
        assert!(!prefix_dir(dir.path()).exists());
    }

    #[test]
    fn resetting_a_game_without_a_profile_does_not_create_one() {
        let dir = tempfile::tempdir().unwrap();

        reset_prefix(dir.path()).unwrap();

        assert!(!dir.path().join(".dackel-launcher").exists());
    }
}
