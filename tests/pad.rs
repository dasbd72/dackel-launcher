use dackel_launcher::nav::{Dir, Input};
use dackel_launcher::pad::{Pad, dir_input, pad_input};
use dackel_launcher::steam_input::{HINT, Watch};
use gilrs::Button;

fn pad() -> Pad {
    Pad::without_gamepad(100, Watch::new(false))
}

#[test]
fn the_face_buttons_dpad_and_menu_buttons_make_inputs() {
    let table = [
        (Button::DPadUp, Input::Up),
        (Button::DPadDown, Input::Down),
        (Button::DPadLeft, Input::Left),
        (Button::DPadRight, Input::Right),
        (Button::South, Input::A),
        (Button::East, Input::B),
        (Button::West, Input::X),
        (Button::North, Input::Y),
        (Button::Start, Input::Start),
        (Button::Select, Input::Select),
    ];
    for (button, input) in table {
        assert_eq!(pad_input(button), Some(input), "{button:?}");
    }
}

#[test]
fn other_buttons_make_no_input() {
    for button in [
        Button::LeftTrigger,
        Button::RightTrigger2,
        Button::Mode,
        Button::LeftThumb,
        Button::Unknown,
    ] {
        assert_eq!(pad_input(button), None, "{button:?}");
    }
}

#[test]
fn a_direction_makes_the_matching_arrow_input() {
    assert_eq!(dir_input(Dir::Up), Input::Up);
    assert_eq!(dir_input(Dir::Down), Input::Down);
    assert_eq!(dir_input(Dir::Left), Input::Left);
    assert_eq!(dir_input(Dir::Right), Input::Right);
}

#[test]
fn a_pad_without_gamepad_support_polls_nothing() {
    assert_eq!(pad().poll(0, true, false), None);
}

#[test]
fn a_pushed_stick_moves_once_then_repeats_while_held() {
    let mut pad = pad();
    let right = (1.0, 0.0);

    assert_eq!(pad.stick_inputs(0, right, true, false), [Input::Right]);
    assert_eq!(pad.stick_inputs(100, right, true, false), []);
    assert_eq!(pad.stick_inputs(400, right, true, false), [Input::Right]);
    assert_eq!(pad.stick_inputs(520, right, true, false), [Input::Right]);
}

#[test]
fn a_stick_back_in_the_dead_zone_stops_and_the_next_push_moves_again() {
    let mut pad = pad();
    pad.stick_inputs(0, (0.0, 1.0), true, false);

    assert_eq!(pad.stick_inputs(10, (0.0, 0.0), true, false), []);
    assert_eq!(pad.stick_inputs(20, (0.0, 1.0), true, false), [Input::Up]);
}

#[test]
fn up_and_down_scroll_instead_of_moving_when_a_page_scrolls() {
    let mut pad = pad();

    let moved = pad.stick_inputs(0, (0.0, -1.0), true, true);

    assert_eq!(moved, []);
    assert_eq!(pad.scroll(), -1.0);
    // Sideways still moves focus, and asks for no scroll.
    assert_eq!(pad.stick_inputs(10, (1.0, 0.0), true, true), [Input::Right]);
    assert_eq!(pad.scroll(), 0.0);
}

#[test]
fn up_and_down_move_focus_when_nothing_scrolls() {
    let mut pad = pad();

    assert_eq!(pad.stick_inputs(0, (0.0, -1.0), true, false), [Input::Down]);
    assert_eq!(pad.scroll(), 0.0);
}

#[test]
fn an_unfocused_pad_does_not_move_or_scroll() {
    let mut pad = pad();

    assert_eq!(pad.stick_inputs(0, (0.0, -1.0), false, true), []);
    assert_eq!(pad.scroll(), 0.0);
    assert_eq!(pad.stick_inputs(10, (1.0, 0.0), false, false), []);
}

#[test]
fn the_same_press_within_the_window_counts_once() {
    let mut pad = pad();

    assert_eq!(
        pad.debounced(0, vec![Input::A, Input::Up]),
        [Input::A, Input::Up]
    );
    assert_eq!(pad.debounced(50, vec![Input::A, Input::B]), [Input::B]);
    assert_eq!(pad.debounced(100, vec![Input::A]), [Input::A]);
}

#[test]
fn changing_the_debounce_window_changes_what_counts_as_the_same_press() {
    let mut pad = pad();
    assert_eq!(pad.debounce_window(), 100);
    pad.debounced(0, vec![Input::A]);

    pad.set_debounce_window(0);

    assert_eq!(pad.debounce_window(), 0);
    assert_eq!(pad.debounced(1, vec![Input::A]), [Input::A]);
}

#[test]
fn three_arrow_presses_under_steam_give_the_hint_once() {
    let mut pad = Pad::without_gamepad(25, Watch::new(true));

    assert_eq!(pad.arrow_pressed(), None);
    assert_eq!(pad.arrow_pressed(), None);
    assert_eq!(pad.arrow_pressed(), Some(HINT));
    assert_eq!(pad.arrow_pressed(), None);
}

#[test]
fn arrow_presses_outside_steam_give_no_hint() {
    let mut pad = pad();

    for _ in 0..5 {
        assert_eq!(pad.arrow_pressed(), None);
    }
}
