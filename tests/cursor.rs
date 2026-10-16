//! Controller focus: the rules that keep it on something that exists.

use dackel_launcher::cursor::{Cursor, Page, Slot, Target};
use dackel_launcher::nav::{Bounds, Control, Dir};

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

fn at(x: f32, y: f32) -> Bounds {
    Bounds {
        left: x,
        top: y,
        right: x + 10.0,
        bottom: y + 10.0,
    }
}

fn game(id: &str, x: f32, y: f32) -> Slot {
    Slot {
        target: Target::Game(id.to_owned()),
        bounds: at(x, y),
    }
}

fn control(c: Control, x: f32, y: f32) -> Slot {
    Slot {
        target: Target::Control(c),
        bounds: at(x, y),
    }
}

/// A toolbar with one control above a row of two game tiles.
fn grid_layout() -> Vec<Slot> {
    vec![
        control(Control::Sort, 0.0, 0.0),
        game("a", 0.0, 100.0),
        game("b", 100.0, 100.0),
    ]
}

/// A cursor that has drawn the grid and sits on its first game.
fn on_the_grid() -> Cursor {
    let mut cursor = Cursor::new();
    cursor.end_frame(grid_layout());
    cursor.keep_on_game(&ids(&["a", "b"]));
    cursor
}

#[test]
fn the_cursor_follows_the_first_game_until_the_user_moves_it() {
    let mut cursor = Cursor::new();
    cursor.keep_on_game(&ids(&["a", "b"]));
    assert_eq!(cursor.game(), Some("a"));

    // The list re-sorts: an unmoved cursor follows the new first game.
    cursor.keep_on_game(&ids(&["b", "a"]));
    assert_eq!(cursor.game(), Some("b"));
}

#[test]
fn a_cursor_the_user_moved_stays_on_its_game_while_it_is_visible() {
    let mut cursor = Cursor::new();
    cursor.select_game("b".to_owned());

    cursor.keep_on_game(&ids(&["a", "b"]));

    assert_eq!(cursor.game(), Some("b"));
}

#[test]
fn a_moved_cursor_returns_to_the_first_game_when_its_own_is_hidden() {
    let mut cursor = Cursor::new();
    cursor.select_game("c".to_owned());

    cursor.keep_on_game(&ids(&["a", "b"]));

    assert_eq!(cursor.game(), Some("a"));
}

#[test]
fn with_no_visible_games_the_cursor_has_no_game() {
    let mut cursor = Cursor::new();
    cursor.select_game("a".to_owned());

    cursor.keep_on_game(&[]);

    assert_eq!(cursor.game(), None);
}

#[test]
fn in_the_library_focus_on_a_control_that_is_gone_goes_back_to_the_games() {
    let mut cursor = on_the_grid();
    cursor.move_toward(Dir::Up);
    assert_eq!(cursor.control(), Some(Control::Sort));

    cursor.end_frame(vec![game("a", 0.0, 100.0)]);
    cursor.keep_on_page(Page::Grid);

    assert_eq!(cursor.control(), None);
}

#[test]
fn in_the_library_focus_on_a_control_that_is_drawn_stays() {
    let mut cursor = on_the_grid();
    cursor.move_toward(Dir::Up);

    cursor.keep_on_page(Page::Grid);

    assert_eq!(cursor.control(), Some(Control::Sort));
}

#[test]
fn the_details_page_always_keeps_focus_on_one_of_its_controls() {
    let mut cursor = Cursor::new();
    cursor.end_frame(vec![
        control(Control::Back, 0.0, 0.0),
        control(Control::Launch, 0.0, 50.0),
    ]);

    // No focus yet: it starts on Back.
    cursor.keep_on_page(Page::Details);
    assert_eq!(cursor.control(), Some(Control::Back));

    // A control that is drawn keeps focus.
    cursor.open_details();
    cursor.keep_on_page(Page::Details);
    assert_eq!(cursor.control(), Some(Control::Launch));

    // A control that is gone falls back to Back.
    cursor.end_frame(vec![control(Control::Back, 0.0, 0.0)]);
    cursor.keep_on_page(Page::Details);
    assert_eq!(cursor.control(), Some(Control::Back));
}

#[test]
fn the_details_page_leaves_focus_alone_until_it_has_been_drawn() {
    let mut cursor = Cursor::new();
    cursor.end_frame(grid_layout()); // still the library's layout: no Back button

    cursor.open_details();
    cursor.keep_on_page(Page::Details);

    assert_eq!(cursor.control(), Some(Control::Launch));
}

#[test]
fn closing_the_details_page_returns_focus_to_the_game_and_scrolls_to_it() {
    let mut cursor = Cursor::new();
    cursor.select_game("b".to_owned());
    cursor.open_details();

    cursor.close_details();

    assert_eq!(cursor.control(), None);
    assert_eq!(cursor.game(), Some("b"));
    assert!(cursor.scroll_requested());
}

#[test]
fn moving_focus_goes_to_the_nearest_item_in_that_direction() {
    let mut cursor = on_the_grid();

    cursor.move_toward(Dir::Right);
    assert_eq!(cursor.game(), Some("b"));
    assert_eq!(cursor.control(), None);

    cursor.move_toward(Dir::Up);
    assert_eq!(cursor.control(), Some(Control::Sort));
    // The game stays remembered for the way back down.
    assert_eq!(cursor.game(), Some("b"));
}

#[test]
fn moving_onto_a_game_stops_the_cursor_following_the_first_game() {
    let mut cursor = on_the_grid();

    cursor.move_toward(Dir::Right);
    cursor.keep_on_game(&ids(&["a", "b"]));

    assert_eq!(cursor.game(), Some("b"));
}

#[test]
fn moving_focus_asks_for_the_target_to_be_scrolled_into_view_until_the_frame_ends() {
    let mut cursor = on_the_grid();

    cursor.move_toward(Dir::Up);
    assert!(cursor.scroll_requested());

    cursor.end_frame(grid_layout());
    assert!(!cursor.scroll_requested());
}

#[test]
fn moving_with_nothing_focused_or_nothing_drawn_does_nothing() {
    let mut empty = Cursor::new();
    empty.move_toward(Dir::Down);
    assert_eq!((empty.game(), empty.control()), (None, None));

    let mut undrawn = Cursor::new();
    undrawn.keep_on_game(&ids(&["a"]));
    undrawn.move_toward(Dir::Down);
    assert_eq!(undrawn.game(), Some("a"));
    assert!(!undrawn.scroll_requested());
}

/// The empty library: a toolbar and no game tiles.
fn toolbar_only() -> Cursor {
    let mut cursor = Cursor::new();
    cursor.end_frame(vec![
        control(Control::Rescan, 0.0, 0.0),
        control(Control::Sort, 100.0, 0.0),
    ]);
    cursor.keep_on_game(&[]);
    cursor
}

#[test]
fn with_no_games_a_move_puts_focus_on_the_first_control() {
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        let mut cursor = toolbar_only();
        cursor.move_toward(dir);

        assert_eq!(cursor.control(), Some(Control::Rescan), "{dir:?}");
        assert_eq!(cursor.game(), None);
        assert!(cursor.scroll_requested());
    }
}

#[test]
fn with_no_games_the_second_move_goes_on_from_the_first_control() {
    let mut cursor = toolbar_only();
    cursor.move_toward(Dir::Right);
    cursor.move_toward(Dir::Right);

    assert_eq!(cursor.control(), Some(Control::Sort));
}

#[test]
fn a_cursor_whose_game_is_gone_and_whose_page_has_no_games_can_still_move() {
    let mut cursor = on_the_grid();
    cursor.end_frame(vec![control(Control::AddLibrary, 0.0, 0.0)]);
    cursor.keep_on_game(&[]);

    cursor.move_toward(Dir::Down);

    assert_eq!(cursor.control(), Some(Control::AddLibrary));
}

#[test]
fn a_cursor_left_on_a_game_that_is_no_longer_drawn_lands_on_the_first_item_when_it_moves() {
    // The library vanished: the page now draws only its own buttons, and nothing told the
    // cursor its game is gone (the page is not the game list, so no game ids are offered).
    for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        let mut cursor = on_the_grid();
        cursor.move_toward(Dir::Right);
        assert_eq!(cursor.game(), Some("b"));
        cursor.end_frame(vec![
            control(Control::Rescan, 0.0, 0.0),
            control(Control::AddLibrary, 0.0, 100.0),
        ]);

        cursor.move_toward(dir);

        assert_eq!(cursor.control(), Some(Control::Rescan), "{dir:?}");
        assert!(cursor.scroll_requested());
    }
}

#[test]
fn a_cursor_with_nothing_drawn_still_does_nothing_when_its_game_is_unknown() {
    let mut cursor = on_the_grid();
    cursor.end_frame(Vec::new());

    cursor.move_toward(Dir::Down);

    assert_eq!(cursor.game(), Some("a"));
    assert_eq!(cursor.control(), None);
    assert!(!cursor.scroll_requested());
}

#[test]
fn a_press_is_taken_once() {
    let mut cursor = Cursor::new();
    cursor.press(Control::Extract);

    assert_eq!(cursor.take_press(), Some(Control::Extract));
    assert_eq!(cursor.take_press(), None);
}

#[test]
fn clicking_a_tile_puts_focus_on_that_game_and_off_any_control() {
    let mut cursor = on_the_grid();
    cursor.move_toward(Dir::Up);

    cursor.select_game("b".to_owned());

    assert_eq!((cursor.game(), cursor.control()), (Some("b"), None));
}

#[test]
fn the_list_selection_moves_by_a_step_and_stays_inside_the_list() {
    let mut cursor = Cursor::new();

    cursor.move_list(1, 3);
    cursor.move_list(1, 3);
    cursor.move_list(1, 3);
    assert_eq!(cursor.list_sel(), 2);

    cursor.move_list(-1, 3);
    assert_eq!(cursor.list_sel(), 1);
    cursor.move_list(-5, 3);
    assert_eq!(cursor.list_sel(), 0);
}

#[test]
fn the_list_selection_in_an_empty_list_stays_on_row_zero() {
    let mut cursor = Cursor::new();
    cursor.move_list(1, 0);
    assert_eq!(cursor.list_sel(), 0);
}

#[test]
fn a_list_opens_on_the_given_row_and_reset_goes_back_to_the_first() {
    let mut cursor = Cursor::new();
    cursor.set_list(4);
    assert_eq!(cursor.list_sel(), 4);

    cursor.reset_list();
    assert_eq!(cursor.list_sel(), 0);
}

#[test]
fn focus_lands_on_a_control_that_is_drawn_a_frame_later() {
    let mut cursor = Cursor::new();
    cursor.end_frame(vec![
        control(Control::Back, 0.0, 0.0),
        control(Control::ResetPrefix, 0.0, 100.0),
    ]);
    cursor.focus(Control::KeepPrefix);

    // The next frame draws the prompt's buttons in place of the Reset button.
    cursor.end_frame(vec![
        control(Control::Back, 0.0, 0.0),
        control(Control::KeepPrefix, 0.0, 100.0),
    ]);
    cursor.keep_on_page(Page::Details);

    assert_eq!(cursor.control(), Some(Control::KeepPrefix));
}

#[test]
fn the_libraries_window_pulls_focus_onto_its_own_controls_once_drawn() {
    let mut cursor = Cursor::new();
    cursor.end_frame(vec![game("a", 0.0, 0.0)]);
    cursor.keep_on_page(Page::Libraries);
    assert_eq!(cursor.control(), None, "waits until the window was drawn");

    cursor.end_frame(vec![
        control(Control::LibraryPath, 0.0, 0.0),
        control(Control::LibraryClose, 0.0, 100.0),
    ]);
    cursor.keep_on_page(Page::Libraries);
    assert_eq!(cursor.control(), Some(Control::LibraryPath));

    cursor.move_toward(Dir::Down);
    cursor.keep_on_page(Page::Libraries);
    assert_eq!(cursor.control(), Some(Control::LibraryClose));
}

mod scrolled_grid {
    use super::*;

    /// A toolbar button at the top of the screen, and a grid of three rows of two tiles that
    /// is scrolled so the first row is above the screen (negative y) and two rows are shown.
    fn scrolled() -> Vec<Slot> {
        vec![
            control(Control::Sort, 0.0, 10.0),
            game("a", 0.0, -300.0),
            game("b", 100.0, -300.0),
            game("c", 0.0, 100.0),
            game("d", 100.0, 100.0),
            game("e", 0.0, 400.0),
            game("f", 100.0, 400.0),
        ]
    }

    fn on(id: &str) -> Cursor {
        let mut cursor = Cursor::new();
        cursor.select_game(id.to_owned());
        cursor.end_frame(scrolled());
        cursor
    }

    #[test]
    fn up_goes_to_the_row_above_even_when_it_is_scrolled_off_screen() {
        let mut cursor = on("d");

        cursor.move_toward(Dir::Up);

        assert_eq!(cursor.game(), Some("b"));
        assert_eq!(cursor.control(), None);
    }

    #[test]
    fn up_from_the_first_row_reaches_the_toolbar() {
        // Not scrolled: the first row is just below the toolbar.
        let mut cursor = Cursor::new();
        cursor.select_game("b".to_owned());
        cursor.end_frame(vec![
            control(Control::Sort, 0.0, 10.0),
            game("a", 0.0, 100.0),
            game("b", 100.0, 100.0),
            game("c", 0.0, 400.0),
        ]);

        cursor.move_toward(Dir::Up);

        assert_eq!(cursor.control(), Some(Control::Sort));
    }

    #[test]
    fn down_goes_to_the_next_row_in_the_same_column() {
        let mut cursor = on("c");

        cursor.move_toward(Dir::Down);

        assert_eq!(cursor.game(), Some("e"));
        cursor.move_toward(Dir::Down);
        assert_eq!(cursor.game(), Some("e"), "nothing below the last row");
    }

    #[test]
    fn a_short_last_row_is_entered_at_the_nearest_column() {
        let mut cursor = Cursor::new();
        cursor.select_game("d".to_owned());
        cursor.end_frame(vec![
            game("c", 0.0, 100.0),
            game("d", 100.0, 100.0),
            game("e", 0.0, 400.0),
        ]);

        cursor.move_toward(Dir::Down);

        assert_eq!(cursor.game(), Some("e"));
    }
}
