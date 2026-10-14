mod common;

use common::{COVER_URL, PAGE_URL, Site, fixture, game_in};
use dlsite_launcher::reload::Reloads;
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
