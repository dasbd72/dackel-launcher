use dackel_launcher::profile::{ExeState, Stats, choose, ensure, read_stats, set_favorite};
use std::fs;
use std::path::Path;

fn file(root: &Path, rel: &str, bytes: usize) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

#[test]
fn favorite_is_persisted_in_the_profile() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Game.exe", 1000);

    set_favorite(game.path(), true).unwrap();

    assert!(read_stats(game.path()).unwrap().favorite);
    let json = fs::read_to_string(game.path().join(".dlsite-launcher/profile.json")).unwrap();
    assert!(json.contains("\"favorite\": true"), "profile was {json}");
}

#[test]
fn favorite_survives_exe_choice_and_rescans() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Bin/A.exe", 5000);
    file(game.path(), "Bin/B.exe", 5000);
    ensure(game.path()).unwrap();
    set_favorite(game.path(), true).unwrap();

    choose(game.path(), Path::new("Bin/B.exe")).unwrap();
    ensure(game.path()).unwrap();

    assert!(read_stats(game.path()).unwrap().favorite);
}

#[test]
fn unfavoriting_clears_the_flag() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Game.exe", 1000);
    set_favorite(game.path(), true).unwrap();

    set_favorite(game.path(), false).unwrap();

    assert!(!read_stats(game.path()).unwrap().favorite);
}

#[test]
fn profiles_written_before_favorites_existed_read_as_not_favorite() {
    let game = tempfile::tempdir().unwrap();
    file(game.path(), "Game.exe", 1000);
    fs::create_dir_all(game.path().join(".dlsite-launcher")).unwrap();
    fs::write(
        game.path().join(".dlsite-launcher/profile.json"),
        r#"{ "exe": "Game.exe" }"#,
    )
    .unwrap();

    assert!(!read_stats(game.path()).unwrap().favorite);
    assert_eq!(
        ensure(game.path()).unwrap(),
        ExeState::Ready {
            exe: "Game.exe".into()
        }
    );
}

#[test]
fn a_game_without_profile_has_default_stats() {
    let game = tempfile::tempdir().unwrap();

    let stats = read_stats(game.path()).unwrap();

    assert_eq!(stats, Stats::default());
}

#[test]
fn play_time_and_last_played_are_read_from_the_profile() {
    let game = tempfile::tempdir().unwrap();
    fs::create_dir_all(game.path().join(".dlsite-launcher")).unwrap();
    fs::write(
        game.path().join(".dlsite-launcher/profile.json"),
        r#"{ "exe": "Game.exe", "play_time_secs": 3725, "last_played": 1700000000 }"#,
    )
    .unwrap();

    let stats = read_stats(game.path()).unwrap();

    assert_eq!(stats.play_time_secs, Some(3725));
    assert_eq!(stats.last_played, Some(1_700_000_000));
}
