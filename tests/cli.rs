//! The launcher's command line: `--library <dir>` and `--launch <game folder>`.

use dackel_launcher::cli::{Args, parse};
use std::path::PathBuf;

fn args(list: &[&str]) -> impl Iterator<Item = String> {
    list.iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>()
        .into_iter()
}

#[test]
fn no_arguments_mean_the_window_with_the_default_library() {
    assert_eq!(parse(args(&[])).unwrap(), Args::default());
}

#[test]
fn library_takes_a_path_in_either_spelling() {
    let a = parse(args(&["--library", "/games"])).unwrap();
    let b = parse(args(&["--library=/games"])).unwrap();

    assert_eq!(a.library, Some(PathBuf::from("/games")));
    assert_eq!(a, b);
}

#[test]
fn launch_takes_a_game_folder_in_either_spelling() {
    let a = parse(args(&["--launch", "/games/RJ1"])).unwrap();
    let b = parse(args(&["--launch=/games/RJ1"])).unwrap();

    assert_eq!(a.launch, Some(PathBuf::from("/games/RJ1")));
    assert_eq!(a, b);
}

#[test]
fn a_flag_without_its_value_is_an_error() {
    assert!(parse(args(&["--launch"])).unwrap_err().contains("--launch"));
    assert!(
        parse(args(&["--library"]))
            .unwrap_err()
            .contains("--library")
    );
}

#[test]
fn other_arguments_are_ignored() {
    let a = parse(args(&["--whatever", "x", "--launch", "/g"])).unwrap();

    assert_eq!(a.launch, Some(PathBuf::from("/g")));
}
