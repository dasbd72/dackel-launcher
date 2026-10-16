use dackel_launcher::window::{RestoreStep, WindowCommand, WindowState};

fn state(minimize_on_launch: bool) -> WindowState {
    WindowState::new(false, 1.0, minimize_on_launch)
}

#[test]
fn toggling_fullscreen_flips_it_and_queues_the_new_value() {
    let mut w = state(false);

    assert!(w.toggle_fullscreen());
    assert!(!w.toggle_fullscreen());

    assert!(!w.fullscreen());
    assert_eq!(
        w.drain_commands(),
        vec![
            WindowCommand::Fullscreen(true),
            WindowCommand::Fullscreen(false)
        ]
    );
    assert_eq!(w.drain_commands(), vec![]);
}

#[test]
fn setting_the_size_remembers_it_and_asks_for_one_zoom() {
    let mut w = state(false);
    w.see_system_scale(Some(2.0));
    w.take_zoom_request();

    w.set_size(1.5);

    assert_eq!(w.size(), 1.5);
    assert_eq!(w.take_zoom_request(), Some(0.75));
    assert_eq!(w.take_zoom_request(), None);
}

#[test]
fn seeing_the_system_scale_for_the_first_time_asks_for_the_zoom_that_gives_the_size() {
    let mut w = WindowState::new(false, 1.5, false);
    assert_eq!(w.take_zoom_request(), None);

    assert!(w.see_system_scale(Some(3.0)));

    assert!(w.synced());
    assert_eq!(w.take_zoom_request(), Some(0.5));
    assert!(!w.see_system_scale(Some(3.0)), "only the first time");
    assert_eq!(w.take_zoom_request(), None);
}

#[test]
fn a_changed_system_scale_keeps_the_size_by_changing_the_zoom() {
    let mut w = WindowState::new(false, 1.5, false);
    w.see_system_scale(Some(3.0));
    w.take_zoom_request();

    w.see_system_scale(Some(1.5));

    assert_eq!(w.size(), 1.5);
    assert_eq!(w.take_zoom_request(), Some(1.0));
}

#[test]
fn a_missing_system_scale_is_waited_for_a_few_frames_then_the_size_is_the_zoom() {
    let mut w = WindowState::new(false, 0.5, false);

    for _ in 0..4 {
        assert!(!w.see_system_scale(None));
        assert!(!w.synced());
        assert_eq!(w.take_zoom_request(), None);
    }

    assert!(w.see_system_scale(None));
    assert!(w.synced());
    assert_eq!(w.take_zoom_request(), Some(0.5));
}

#[test]
fn a_zoom_close_to_the_one_the_size_gives_is_not_the_user_zooming() {
    let mut w = WindowState::new(false, 1.5, false);
    assert!(!w.zoom_differs(0.9), "not before the system scale is known");
    w.see_system_scale(Some(3.0));

    assert!(!w.zoom_differs(0.5));
    assert!(!w.zoom_differs(0.5005));
    assert!(w.zoom_differs(0.6));
    assert_eq!(w.size_of_zoom(0.6), 1.8);
}

#[test]
fn a_launch_minimizes_only_where_the_machine_does_that() {
    let mut deck = state(true);
    deck.game_started();
    assert_eq!(deck.drain_commands(), vec![WindowCommand::Minimize]);
    assert!(deck.minimized_by_us());

    let mut desktop = state(false);
    desktop.game_started();
    assert_eq!(desktop.drain_commands(), vec![]);
    assert!(!desktop.minimized_by_us());
}

#[test]
fn nothing_is_waiting_when_the_launcher_did_not_minimize_itself() {
    let mut w = state(false);
    w.game_started();

    assert_eq!(w.after_game_poll(0, true, false, || true), None);
    assert_eq!(w.after_game_poll(10_000, false, false, || true), None);
}

#[test]
fn nothing_is_requested_while_a_game_is_still_running() {
    let mut w = state(true);
    w.game_started();

    assert_eq!(w.after_game_poll(0, true, true, || true), None);
    assert!(w.minimized_by_us());
}

#[test]
fn after_the_game_exits_the_launcher_is_asked_back_until_it_is_visible() {
    let mut w = state(true);
    w.game_started();

    // The frame the game exits: the first ask waits a moment.
    assert_eq!(
        w.after_game_poll(1_000, true, false, || true),
        Some(RestoreStep::Idle)
    );
    assert_eq!(
        w.after_game_poll(1_300, false, false, || true),
        Some(RestoreStep::Request)
    );
    // The window is visible again: done, and nothing waits afterwards.
    assert_eq!(
        w.after_game_poll(1_400, false, false, || false),
        Some(RestoreStep::Done)
    );
    assert!(!w.minimized_by_us());
    assert_eq!(w.after_game_poll(1_500, false, false, || true), None);
}

#[test]
fn whether_the_window_is_minimized_is_asked_only_when_something_waits() {
    let mut w = state(false);
    let mut asked = false;

    w.after_game_poll(0, false, false, || {
        asked = true;
        true
    });

    assert!(!asked);
}
