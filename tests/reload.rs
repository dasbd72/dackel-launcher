mod common;

use common::{COVER_URL, PAGE_URL, Site, fixture, game_in};
use dackel_launcher::info::refresh;
use dackel_launcher::library::Game;
use dackel_launcher::reload::{Infos, Reloads};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn background_reload_delivers_the_refreshed_info() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let site = Site::default()
        .with(PAGE_URL, &fixture("maniax_touch_work.html"))
        .with(COVER_URL, b"cover");
    let reloads = Reloads::new(Arc::new(site));

    let key = game.key();
    reloads.start(game);

    let deadline = Instant::now() + Duration::from_secs(5);
    let done = loop {
        if let Some(done) = reloads.try_next() {
            break done;
        }
        assert!(Instant::now() < deadline, "reload never finished");
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(done.key, key);
    assert_eq!(done.result.unwrap().title, "サンプル作品タイトル");
}

fn site() -> Site {
    Site::default()
        .with(PAGE_URL, &fixture("maniax_touch_work.html"))
        .with(COVER_URL, b"cover")
}

/// Collects until `count` refreshes have finished.
fn collect_until(infos: &mut Infos, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut done = 0;
    while done < count {
        done += infos.collect();
        assert!(Instant::now() < deadline, "reload never finished");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_game_is_busy_from_the_reload_until_it_is_collected() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let key = game.key();
    let mut infos = Infos::new(Arc::new(site()));
    assert!(!infos.is_busy(&key));
    assert!(!infos.any_busy());

    infos.reload(game);
    assert!(infos.is_busy(&key));
    assert!(infos.any_busy());

    collect_until(&mut infos, 1);
    assert!(!infos.is_busy(&key));
    assert!(!infos.any_busy());
}

#[test]
fn a_collected_reload_keeps_the_info_and_no_error() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let key = game.key();
    let mut infos = Infos::new(Arc::new(site()));

    infos.reload(game);
    collect_until(&mut infos, 1);

    assert_eq!(infos.get(&key).unwrap().title, "サンプル作品タイトル");
    assert_eq!(infos.error(&key), None);
}

#[test]
fn a_game_already_being_reloaded_is_not_reloaded_again() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let mut infos = Infos::new(Arc::new(site()));

    infos.reload(game.clone());
    infos.reload(game);

    collect_until(&mut infos, 1);
    thread::sleep(Duration::from_millis(100));
    assert_eq!(infos.collect(), 0);
}

#[test]
fn a_failed_reload_keeps_its_message_and_the_old_info_until_the_next_try() {
    let lib = tempfile::tempdir().unwrap();
    // A game folder that is a plain file cannot hold the saved info, so the reload fails.
    let dir = lib.path().join("RJ123456");
    std::fs::write(&dir, b"not a folder").unwrap();
    let game = Game {
        id: "RJ123456".to_owned(),
        dir,
        slot: 0,
    };
    let key = game.key();
    let mut infos = Infos::new(Arc::new(site()));
    let earlier_game = game_in(lib.path(), "RJ999999");
    let mut earlier = refresh(&earlier_game, &site()).unwrap();
    earlier.title = "from disk".to_owned();
    infos.set(key.clone(), earlier);

    infos.reload(game.clone());
    collect_until(&mut infos, 1);

    assert!(infos.error(&key).is_some());
    assert_eq!(infos.get(&key).unwrap().title, "from disk");

    // Starting the next refresh clears the old message.
    infos.reload(game);
    assert_eq!(infos.error(&key), None);
    collect_until(&mut infos, 1);
}

#[test]
fn info_read_from_disk_is_remembered_without_a_reload() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let info = refresh(&game, &site()).unwrap();
    let mut infos = Infos::new(Arc::new(Site::default()));

    infos.set(game.key(), info);

    assert_eq!(infos.get(&game.key()).unwrap().id, "RJ123456");
    assert!(infos.get("missing").is_none());
    assert!(!infos.any_busy());
}
