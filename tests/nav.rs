use dlsite_launcher::nav::{Dir, move_focus};

// A 7-item grid, 3 columns:
//   0 1 2
//   3 4 5
//   6
#[test]
fn right_and_left_move_within_a_row_and_stop_at_its_edges() {
    assert_eq!(move_focus(0, 7, 3, Dir::Right), 1);
    assert_eq!(move_focus(1, 7, 3, Dir::Right), 2);
    assert_eq!(
        move_focus(2, 7, 3, Dir::Right),
        2,
        "no wrap to the next row"
    );
    assert_eq!(move_focus(2, 7, 3, Dir::Left), 1);
    assert_eq!(
        move_focus(3, 7, 3, Dir::Left),
        3,
        "no wrap to the previous row"
    );
}

#[test]
fn up_and_down_move_by_a_row() {
    assert_eq!(move_focus(1, 7, 3, Dir::Down), 4);
    assert_eq!(move_focus(4, 7, 3, Dir::Up), 1);
    assert_eq!(move_focus(1, 7, 3, Dir::Up), 1, "top row stays");
}

#[test]
fn down_into_a_short_last_row_lands_on_its_last_item() {
    assert_eq!(move_focus(4, 7, 3, Dir::Down), 6);
    assert_eq!(move_focus(5, 7, 3, Dir::Down), 6);
    assert_eq!(move_focus(6, 7, 3, Dir::Down), 6, "bottom row stays");
}

#[test]
fn right_stops_at_the_last_item_of_a_short_row() {
    assert_eq!(move_focus(6, 7, 3, Dir::Right), 6);
}

#[test]
fn an_out_of_range_focus_is_pulled_back_into_the_list() {
    assert_eq!(move_focus(9, 7, 3, Dir::Up), 3);
    assert_eq!(move_focus(5, 0, 3, Dir::Down), 0, "empty list");
}

mod commands {
    use dlsite_launcher::nav::{Command, Context, Dir, Input, command};

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
    fn grid_buttons_launch_reload_open_details_and_switch_sort() {
        assert_eq!(command(Context::Grid, Input::A), Some(Command::Primary));
        assert_eq!(command(Context::Grid, Input::X), Some(Command::Reload));
        assert_eq!(command(Context::Grid, Input::Y), Some(Command::OpenDetails));
        assert_eq!(
            command(Context::Grid, Input::LeftBumper),
            Some(Command::CycleSort(-1))
        );
        assert_eq!(
            command(Context::Grid, Input::RightBumper),
            Some(Command::CycleSort(1))
        );
    }

    #[test]
    fn grid_has_controller_access_to_rescan_filters_and_zip_encoding() {
        assert_eq!(command(Context::Grid, Input::Start), Some(Command::Rescan));
        assert_eq!(
            command(Context::Grid, Input::Select),
            Some(Command::ToggleFavoritesOnly)
        );
        assert_eq!(
            command(Context::Grid, Input::B),
            Some(Command::ResetFilters)
        );
        assert_eq!(
            command(Context::Grid, Input::LeftTrigger),
            Some(Command::CycleEncoding(-1))
        );
        assert_eq!(
            command(Context::Grid, Input::RightTrigger),
            Some(Command::CycleEncoding(1))
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
    fn details_page_reaches_the_exe_chooser_and_re_extract_from_the_bumpers() {
        assert_eq!(
            command(Context::Details, Input::LeftBumper),
            Some(Command::ChooseExe)
        );
        assert_eq!(
            command(Context::Details, Input::RightBumper),
            Some(Command::ReExtract)
        );
    }

    #[test]
    fn details_page_ignores_the_dpad() {
        assert_eq!(command(Context::Details, Input::Up), None);
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
    use dlsite_launcher::nav::{Dir, Repeater, stick_dir};

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

mod window_controls {
    use dlsite_launcher::nav::{Command, Context, Input, command};

    #[test]
    fn stick_clicks_toggle_fullscreen_and_cycle_the_ui_scale_everywhere() {
        for context in [Context::Grid, Context::Details, Context::Chooser] {
            assert_eq!(
                command(context, Input::RightStickClick),
                Some(Command::ToggleFullscreen),
                "{context:?}"
            );
            assert_eq!(
                command(context, Input::LeftStickClick),
                Some(Command::CycleUiScale),
                "{context:?}"
            );
        }
    }
}
