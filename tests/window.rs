use dlsite_launcher::settings::Settings;
use dlsite_launcher::window::{default_ui_scale, initial_fullscreen, is_deck_like, next_scale};
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
fn default_scale_is_larger_on_the_deck() {
    assert_eq!(default_ui_scale(false), 1.0);
    assert_eq!(default_ui_scale(true), 1.25);
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
fn scale_cycles_through_the_presets_and_wraps() {
    assert_eq!(next_scale(1.0), 1.25);
    assert_eq!(next_scale(1.25), 1.5);
    assert_eq!(next_scale(1.5), 2.0);
    assert_eq!(next_scale(2.0), 1.0);
}

#[test]
fn an_off_preset_scale_moves_to_the_next_larger_preset() {
    assert_eq!(next_scale(1.1), 1.25);
    assert_eq!(next_scale(0.8), 1.0);
    assert_eq!(
        next_scale(2.6),
        1.0,
        "beyond the largest wraps to the smallest"
    );
}
