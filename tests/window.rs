use dackel_launcher::settings::Settings;
use dackel_launcher::window::{
    DEFAULT_SIZE, SCALE_PRESETS, initial_fullscreen, initial_size, is_deck_like, is_native_wayland,
    scale_label, should_minimize_on_launch, size_for, zoom_for,
};
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |key| map.get(key).cloned()
}

#[test]
fn a_gamescope_session_is_deck_like() {
    assert!(is_deck_like(env(&[(
        "GAMESCOPE_WAYLAND_DISPLAY",
        "gamescope-0"
    )])));
    assert!(is_deck_like(env(&[("XDG_CURRENT_DESKTOP", "gamescope")])));
    assert!(is_deck_like(env(&[("SteamGamepadUI", "1")])));
}

#[test]
fn the_steam_deck_flag_makes_it_deck_like() {
    assert!(is_deck_like(env(&[("SteamDeck", "1")])));
}

#[test]
fn a_plain_desktop_is_not_deck_like() {
    assert!(!is_deck_like(env(&[("XDG_CURRENT_DESKTOP", "KDE")])));
    assert!(!is_deck_like(env(&[])));
}

#[test]
fn a_desktop_that_merely_has_gamescope_installed_is_not_deck_like() {
    // Real values from the author's Arch desktop: gamescope is configured, but this is KDE.
    let desktop = env(&[
        ("SteamDeck", "0"),
        ("XDG_CURRENT_DESKTOP", "KDE"),
        (
            "GAMESCOPECMD",
            "gamescope -e -f --mangoapp -W 2560 -H 1440 -r 144",
        ),
        ("ENABLE_GAMESCOPE_HDR", "0"),
    ]);

    assert!(!is_deck_like(desktop));
}

#[test]
fn empty_values_do_not_count_as_set() {
    assert!(!is_deck_like(env(&[("GAMESCOPE_WAYLAND_DISPLAY", "")])));
}

#[test]
fn a_size_on_screen_is_turned_into_a_zoom_by_the_system_scale_and_back() {
    assert_eq!(zoom_for(1.5, Some(3.0)), 0.5);
    assert_eq!(zoom_for(1.5, Some(1.0)), 1.5);
    assert_eq!(size_for(0.5, Some(3.25)), 1.63);
    for system in [1.0_f32, 1.25, 2.0, 3.25] {
        let back = size_for(zoom_for(1.75, Some(system)), Some(system));
        assert!((back - 1.75).abs() < 0.01, "{system}: {back}");
    }
}

#[test]
fn without_a_usable_system_scale_the_size_is_the_zoom() {
    for system in [None, Some(0.0), Some(-1.0), Some(f32::NAN)] {
        assert_eq!(zoom_for(1.5, system), 1.5, "{system:?}");
        assert_eq!(size_for(1.5, system), 1.5, "{system:?}");
    }
}

#[test]
fn the_zoom_stays_in_a_usable_range() {
    assert_eq!(zoom_for(1.5, Some(100.0)), 0.1);
    assert_eq!(zoom_for(2.5, Some(0.1)), 4.0);
}

#[test]
fn a_saved_size_wins_over_everything() {
    assert_eq!(
        initial_size(Some(2.0), Some(0.5), true, Some(3.25)),
        (2.0, false)
    );
}

#[test]
fn an_old_saved_zoom_becomes_the_size_it_gave_and_is_saved_once() {
    assert_eq!(
        initial_size(None, Some(0.5), true, Some(3.25)),
        (1.63, true)
    );
    assert_eq!(
        initial_size(None, Some(1.25), false, Some(1.0)),
        (1.25, true)
    );
}

#[test]
fn an_old_saved_zoom_is_used_as_it_is_when_the_system_scale_is_unknown() {
    assert_eq!(initial_size(None, Some(0.75), true, None), (0.75, false));
}

#[test]
fn with_nothing_saved_the_size_is_the_default_when_the_system_scale_is_known() {
    for deck_like in [false, true] {
        assert_eq!(
            initial_size(None, None, deck_like, Some(3.25)),
            (DEFAULT_SIZE, false)
        );
        assert_eq!(
            initial_size(None, None, deck_like, Some(1.0)),
            (DEFAULT_SIZE, false)
        );
    }
}

#[test]
fn with_nothing_saved_and_no_system_scale_the_old_fallbacks_stay() {
    assert_eq!(initial_size(None, None, true, None), (0.5, false));
    assert_eq!(initial_size(None, None, false, None), (1.0, false));
    assert_eq!(initial_size(None, None, true, Some(f32::NAN)), (0.5, false));
}

#[test]
fn fullscreen_follows_the_environment_until_the_user_chooses() {
    let none = Settings::default();

    assert!(initial_fullscreen(&none, true));
    assert!(!initial_fullscreen(&none, false));
}

#[test]
fn a_saved_fullscreen_choice_beats_the_environment() {
    let windowed = Settings {
        fullscreen: Some(false),
        ..Settings::default()
    };
    let full = Settings {
        fullscreen: Some(true),
        ..Settings::default()
    };

    assert!(!initial_fullscreen(&windowed, true));
    assert!(initial_fullscreen(&full, false));
}

#[test]
fn the_presets_are_sizes_from_three_quarters_to_two_and_a_half() {
    assert_eq!(SCALE_PRESETS, [0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5]);
    assert!(SCALE_PRESETS.contains(&DEFAULT_SIZE));
}

#[test]
fn the_scale_label_shows_the_system_scale_the_size_is_made_from() {
    assert_eq!(scale_label(1.5, Some(3.25)), "Size x1.50 (system x3.25)");
    assert_eq!(scale_label(1.25, Some(1.0)), "Size x1.25 (system x1.00)");
}

#[test]
fn the_scale_label_says_so_when_the_system_scale_is_unknown() {
    assert_eq!(scale_label(0.5, None), "Size x0.50 (system ?)");
}

mod restore {
    use dackel_launcher::window::{Restore, RestoreStep};

    #[test]
    fn nothing_happens_until_it_is_armed() {
        let mut r = Restore::default();

        assert_eq!(r.poll(10_000, true), RestoreStep::Idle);
    }

    #[test]
    fn it_waits_for_the_exiting_game_window_to_go_away_before_asking() {
        let mut r = Restore::default();
        r.arm(1_000);

        assert_eq!(
            r.poll(1_100, true),
            RestoreStep::Idle,
            "too soon: the game window may still be active"
        );
        assert_eq!(r.poll(1_300, true), RestoreStep::Request);
    }

    #[test]
    fn it_keeps_asking_at_a_steady_pace_while_the_window_stays_minimized() {
        let mut r = Restore::default();
        r.arm(0);

        assert_eq!(r.poll(300, true), RestoreStep::Request);
        assert_eq!(r.poll(600, true), RestoreStep::Idle);
        assert_eq!(r.poll(1_000, true), RestoreStep::Request);
        assert_eq!(r.poll(1_400, true), RestoreStep::Idle);
        assert_eq!(r.poll(1_700, true), RestoreStep::Request);
    }

    #[test]
    fn it_stops_as_soon_as_the_window_is_visible_again() {
        let mut r = Restore::default();
        r.arm(0);
        assert_eq!(r.poll(300, true), RestoreStep::Request);

        assert_eq!(r.poll(1_000, false), RestoreStep::Done);
        assert_eq!(
            r.poll(2_000, true),
            RestoreStep::Idle,
            "does not fight a user who minimized it again"
        );
    }

    #[test]
    fn it_gives_up_after_a_bounded_number_of_requests() {
        let mut r = Restore::default();
        r.arm(0);
        let mut requests = 0;
        let mut done = false;
        for t in (300..20_000).step_by(100) {
            match r.poll(t, true) {
                RestoreStep::Request => requests += 1,
                RestoreStep::Done => {
                    done = true;
                    break;
                }
                RestoreStep::Idle => {}
            }
        }

        assert_eq!(requests, 6);
        assert!(done);
        assert_eq!(r.poll(30_000, true), RestoreStep::Idle);
    }
}

mod wayland {
    use super::*;

    #[test]
    fn a_wayland_display_means_native_wayland() {
        assert!(is_native_wayland(env(&[("WAYLAND_DISPLAY", "wayland-0")])));
        assert!(
            is_native_wayland(env(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")])),
            "winit prefers Wayland when both are present"
        );
    }

    #[test]
    fn a_wayland_socket_also_counts() {
        assert!(is_native_wayland(env(&[("WAYLAND_SOCKET", "3")])));
    }

    #[test]
    fn x11_only_or_empty_values_are_not_native_wayland() {
        assert!(!is_native_wayland(env(&[("DISPLAY", ":0")])));
        assert!(!is_native_wayland(env(&[])));
        assert!(!is_native_wayland(env(&[("WAYLAND_DISPLAY", "")])));
    }

    #[test]
    fn a_session_type_alone_does_not_decide_it() {
        // winit looks at the display variables, so a stale session type must not mislead us.
        assert!(!is_native_wayland(env(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("DISPLAY", ":0")
        ])));
    }

    #[test]
    fn minimize_on_launch_only_on_the_deck_and_never_on_native_wayland() {
        assert!(
            should_minimize_on_launch(true, false),
            "Deck on X11 / Xwayland"
        );
        assert!(
            !should_minimize_on_launch(true, true),
            "Wayland cannot un-minimize"
        );
        assert!(!should_minimize_on_launch(false, false), "plain desktop");
        assert!(!should_minimize_on_launch(false, true));
    }
}
