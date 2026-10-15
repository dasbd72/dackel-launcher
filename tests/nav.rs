mod commands {
    use dackel_launcher::nav::{Command, Context, Dir, Input, command};

    #[test]
    fn dpad_moves_focus_on_the_grid() {
        assert_eq!(
            command(Context::Grid, Input::Up),
            Some(Command::Move(Dir::Up))
        );
        assert_eq!(
            command(Context::Grid, Input::Right),
            Some(Command::Move(Dir::Right))
        );
    }

    #[test]
    fn grid_buttons_launch_reload_and_open_details() {
        assert_eq!(command(Context::Grid, Input::A), Some(Command::Primary));
        assert_eq!(command(Context::Grid, Input::X), Some(Command::Reload));
        assert_eq!(command(Context::Grid, Input::Y), Some(Command::OpenDetails));
    }

    #[test]
    fn grid_has_controller_access_to_rescan_and_the_filters() {
        assert_eq!(command(Context::Grid, Input::Start), Some(Command::Rescan));
        assert_eq!(
            command(Context::Grid, Input::Select),
            Some(Command::ToggleFavoritesOnly)
        );
        assert_eq!(
            command(Context::Grid, Input::B),
            Some(Command::ResetFilters)
        );
    }

    #[test]
    fn details_page_backs_out_launches_reloads_and_toggles_favorite() {
        assert_eq!(command(Context::Details, Input::B), Some(Command::Back));
        assert_eq!(command(Context::Details, Input::A), Some(Command::Primary));
        assert_eq!(command(Context::Details, Input::X), Some(Command::Reload));
        assert_eq!(
            command(Context::Details, Input::Y),
            Some(Command::ToggleFavorite)
        );
    }

    #[test]
    fn the_dpad_moves_focus_between_the_buttons_on_the_details_page() {
        assert_eq!(
            command(Context::Details, Input::Up),
            Some(Command::Move(Dir::Up))
        );
        assert_eq!(
            command(Context::Details, Input::Right),
            Some(Command::Move(Dir::Right))
        );
    }

    #[test]
    fn exe_chooser_is_a_modal_list() {
        assert_eq!(
            command(Context::Chooser, Input::Up),
            Some(Command::ChooserMove(-1))
        );
        assert_eq!(
            command(Context::Chooser, Input::Down),
            Some(Command::ChooserMove(1))
        );
        assert_eq!(
            command(Context::Chooser, Input::A),
            Some(Command::ChooserPick)
        );
        assert_eq!(
            command(Context::Chooser, Input::B),
            Some(Command::ChooserCancel)
        );
        assert_eq!(
            command(Context::Chooser, Input::Y),
            None,
            "grid buttons are blocked behind the modal"
        );
    }
}

mod stick {
    use dackel_launcher::nav::{Dir, Repeater, stick_dir};

    #[test]
    fn small_deflections_inside_the_dead_zone_do_nothing() {
        assert_eq!(stick_dir(0.3, 0.3), None);
        assert_eq!(stick_dir(0.0, 0.0), None);
    }

    #[test]
    fn the_dominant_axis_picks_the_direction_and_y_is_up_positive() {
        assert_eq!(stick_dir(0.0, 0.9), Some(Dir::Up));
        assert_eq!(stick_dir(0.1, -0.9), Some(Dir::Down));
        assert_eq!(stick_dir(0.9, 0.2), Some(Dir::Right));
        assert_eq!(stick_dir(-0.8, -0.3), Some(Dir::Left));
    }

    #[test]
    fn a_new_direction_fires_immediately_then_waits_before_repeating() {
        let mut r = Repeater::default();

        assert_eq!(r.update(0, Some(Dir::Right)), Some(Dir::Right));
        assert_eq!(r.update(100, Some(Dir::Right)), None);
        assert_eq!(r.update(399, Some(Dir::Right)), None);
        assert_eq!(r.update(400, Some(Dir::Right)), Some(Dir::Right));
    }

    #[test]
    fn once_repeating_it_fires_at_a_steady_rate() {
        let mut r = Repeater::default();
        r.update(0, Some(Dir::Down));
        r.update(400, Some(Dir::Down));

        assert_eq!(r.update(450, Some(Dir::Down)), None);
        assert_eq!(r.update(520, Some(Dir::Down)), Some(Dir::Down));
        assert_eq!(r.update(560, Some(Dir::Down)), None);
        assert_eq!(r.update(640, Some(Dir::Down)), Some(Dir::Down));
    }

    #[test]
    fn releasing_or_changing_direction_restarts_the_delay() {
        let mut r = Repeater::default();
        r.update(0, Some(Dir::Down));
        r.update(400, Some(Dir::Down));

        assert_eq!(r.update(500, None), None);
        assert_eq!(r.update(510, Some(Dir::Down)), Some(Dir::Down));
        assert_eq!(r.update(520, Some(Dir::Left)), Some(Dir::Left));
        assert_eq!(
            r.update(800, Some(Dir::Left)),
            None,
            "delay restarted at 520"
        );
    }
}

mod stick_scroll {
    use dackel_launcher::nav::stick_scroll;

    #[test]
    fn inside_the_dead_zone_nothing_scrolls() {
        assert_eq!(stick_scroll(0.0, 0.0), 0.0);
        assert_eq!(stick_scroll(0.1, 0.4), 0.0);
    }

    #[test]
    fn pushing_up_scrolls_up_and_down_scrolls_down_at_full_speed_when_fully_pushed() {
        assert_eq!(stick_scroll(0.0, 1.0), 1.0);
        assert_eq!(stick_scroll(0.0, -1.0), -1.0);
    }

    #[test]
    fn the_speed_grows_with_how_far_the_stick_is_pushed() {
        let slow = stick_scroll(0.0, 0.6);
        let fast = stick_scroll(0.0, 0.9);

        assert!(0.0 < slow && slow < fast && fast < 1.0, "{slow} {fast}");
    }

    #[test]
    fn a_mostly_sideways_push_does_not_scroll() {
        assert_eq!(stick_scroll(0.9, 0.6), 0.0);
        assert_eq!(stick_scroll(-0.9, -0.6), 0.0);
    }
}

mod window_controls {
    use dackel_launcher::nav::{Command, Context, Input, command};

    #[test]
    fn the_fullscreen_key_works_everywhere_including_behind_a_modal() {
        for context in [Context::Grid, Context::Details, Context::Chooser] {
            assert_eq!(
                command(context, Input::ToggleFullscreen),
                Some(Command::ToggleFullscreen),
                "{context:?}"
            );
        }
    }
}

mod blur {
    use dackel_launcher::nav::{Command, Context, Input, command};

    #[test]
    fn the_blur_key_works_on_every_screen_including_behind_a_modal() {
        for context in [Context::Grid, Context::Details, Context::Chooser] {
            assert_eq!(
                command(context, Input::ToggleBlur),
                Some(Command::ToggleBlur),
                "{context:?}"
            );
        }
    }
}

mod spatial {
    use dackel_launcher::nav::{Bounds, Dir, spatial_next};

    fn b(left: f32, top: f32, right: f32, bottom: f32) -> Bounds {
        Bounds {
            left,
            top,
            right,
            bottom,
        }
    }

    // A miniature of the real screen:
    //   toolbar row 1:  A  B
    //   toolbar row 2:  C                 D
    //   tiles row 1:    T1  T2  T3
    //   tiles row 2:    T4
    const A: usize = 0;
    const B: usize = 1;
    const C: usize = 2;
    const D: usize = 3;
    const T1: usize = 4;
    const T2: usize = 5;
    const T3: usize = 6;
    const T4: usize = 7;

    fn screen() -> Vec<Bounds> {
        vec![
            b(0.0, 0.0, 80.0, 34.0),
            b(90.0, 0.0, 200.0, 34.0),
            b(0.0, 44.0, 150.0, 78.0),
            b(700.0, 44.0, 850.0, 78.0),
            b(10.0, 100.0, 210.0, 400.0),
            b(220.0, 100.0, 420.0, 400.0),
            b(430.0, 100.0, 630.0, 400.0),
            b(10.0, 410.0, 210.0, 710.0),
        ]
    }

    #[test]
    fn right_and_left_walk_along_a_row_and_stop_at_its_ends() {
        let s = screen();
        assert_eq!(spatial_next(&s, A, Dir::Right), B);
        assert_eq!(spatial_next(&s, T1, Dir::Right), T2);
        assert_eq!(spatial_next(&s, T2, Dir::Right), T3);
        assert_eq!(
            spatial_next(&s, T3, Dir::Right),
            T3,
            "nothing further right"
        );
        assert_eq!(spatial_next(&s, T3, Dir::Left), T2);
        assert_eq!(spatial_next(&s, T1, Dir::Left), T1, "nothing further left");
    }

    #[test]
    fn down_into_a_short_last_row_lands_on_its_only_tile() {
        let s = screen();
        assert_eq!(spatial_next(&s, T1, Dir::Down), T4);
        assert_eq!(spatial_next(&s, T3, Dir::Down), T4);
        assert_eq!(spatial_next(&s, T4, Dir::Down), T4, "bottom stays");
    }

    #[test]
    fn up_from_the_first_row_of_tiles_reaches_the_nearest_toolbar_control() {
        let s = screen();
        assert_eq!(
            spatial_next(&s, T1, Dir::Up),
            C,
            "C is above T1's left side"
        );
        assert_eq!(
            spatial_next(&s, T3, Dir::Up),
            D,
            "D is the closer one above T3"
        );
    }

    #[test]
    fn down_from_the_toolbar_steps_through_its_rows_then_into_the_tiles() {
        let s = screen();
        assert_eq!(spatial_next(&s, A, Dir::Down), C);
        assert_eq!(spatial_next(&s, C, Dir::Down), T1);
        assert_eq!(spatial_next(&s, A, Dir::Up), A, "top stays");
    }

    #[test]
    fn a_control_in_line_beats_a_nearer_one_that_is_far_off_to_the_side() {
        let s = vec![
            b(0.0, 0.0, 100.0, 50.0),      // current
            b(120.0, 400.0, 220.0, 450.0), // close in x, far below
            b(300.0, 0.0, 400.0, 50.0),    // farther, but in the same row
        ];
        assert_eq!(spatial_next(&s, 0, Dir::Right), 2);
    }

    #[test]
    fn with_nothing_to_move_to_focus_stays_put() {
        let one = vec![b(0.0, 0.0, 10.0, 10.0)];
        assert_eq!(spatial_next(&one, 0, Dir::Down), 0);
        assert_eq!(spatial_next(&[], 0, Dir::Left), 0);
        assert_eq!(
            spatial_next(&one, 5, Dir::Left),
            5,
            "unknown current is returned as is"
        );
    }
}

mod duplicates {
    use dackel_launcher::nav::{DEBOUNCE_PRESETS_MS, DEFAULT_DEBOUNCE_MS, Debounce, Input};

    #[test]
    fn a_wider_window_drops_presses_that_are_further_apart() {
        let mut d = Debounce::new(100);
        assert!(d.accept(1000, Input::Down));
        assert!(!d.accept(1080, Input::Down));
        assert!(d.accept(1100, Input::Down));
    }

    #[test]
    fn a_zero_window_turns_it_off() {
        let mut d = Debounce::new(0);
        assert!(d.accept(1000, Input::Down));
        assert!(d.accept(1000, Input::Down));
    }

    #[test]
    fn the_window_can_be_changed_while_running() {
        let mut d = Debounce::new(0);
        assert!(d.accept(1000, Input::Up));
        d.set_window(50);
        assert!(!d.accept(1010, Input::Up));
    }

    #[test]
    fn the_default_is_one_of_the_offered_choices_and_off_is_offered() {
        assert!(DEBOUNCE_PRESETS_MS.contains(&DEFAULT_DEBOUNCE_MS));
        assert!(DEBOUNCE_PRESETS_MS.contains(&0));
        assert_eq!(Debounce::default().window(), DEFAULT_DEBOUNCE_MS);
    }

    #[test]
    fn the_same_press_arriving_twice_at_once_counts_once() {
        // Steam shows a real pad and its virtual copy; both report the same press.
        let mut d = Debounce::default();
        assert!(d.accept(1000, Input::Down));
        assert!(!d.accept(1000, Input::Down));
        assert!(!d.accept(1010, Input::Down));
    }

    #[test]
    fn a_real_second_press_a_moment_later_still_counts() {
        let mut d = Debounce::default();
        assert!(d.accept(1000, Input::Down));
        assert!(d.accept(1080, Input::Down));
    }

    #[test]
    fn different_buttons_at_once_are_both_kept() {
        let mut d = Debounce::default();
        assert!(d.accept(1000, Input::Down));
        assert!(d.accept(1000, Input::A));
        assert!(d.accept(1000, Input::Right));
    }

    #[test]
    fn each_button_is_tracked_on_its_own() {
        let mut d = Debounce::default();
        assert!(d.accept(1000, Input::Down));
        assert!(d.accept(1005, Input::A));
        assert!(!d.accept(1010, Input::Down), "Down was seen 10 ms ago");
        assert!(!d.accept(1015, Input::A), "A was seen 10 ms ago");
    }
}
