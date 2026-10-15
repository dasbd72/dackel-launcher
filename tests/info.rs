mod common;

use common::{COVER_URL, PAGE_URL, Site, fixture, game_in};
use dackel_launcher::fetch::{Fetch, Fetched};
use dackel_launcher::info::{cover_path, load, refresh};
use dackel_launcher::library::Game;
use std::fs;
use std::io;

fn read_info(game: &Game) -> serde_json::Value {
    let path = game.dir.join(".dlsite-launcher").join("info.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn product_page_is_parsed_into_info_json() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let site = Site::default().with(PAGE_URL, &fixture("maniax_touch_work.html"));

    refresh(&game, &site).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "サンプル作品タイトル");
    assert_eq!(info["circle"], "サークル名テスト");
    assert_eq!(
        info["genres"],
        serde_json::json!(["ASMR", "ボイス・ASMR", "Genre C"])
    );
    assert_eq!(
        info["description"],
        "説明文の一行目です。\n二行目の説明です。"
    );
}

#[test]
fn cover_is_saved_beside_info_json_and_named_in_it() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let cover = b"\xFF\xD8\xFF fake jpeg bytes";
    let site = Site::default()
        .with(PAGE_URL, &fixture("maniax_touch_work.html"))
        .with(COVER_URL, cover);

    refresh(&game, &site).unwrap();

    let meta = game.dir.join(".dlsite-launcher");
    assert_eq!(read_info(&game)["cover"], "cover.jpg");
    assert_eq!(fs::read(meta.join("cover.jpg")).unwrap(), cover);
}

#[test]
fn missing_page_falls_back_to_folder_name_without_cover() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ999999");

    refresh(&game, &Site::default()).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "RJ999999");
    assert_eq!(info["found"], false);
    assert_eq!(info["cover"], serde_json::Value::Null);
    let meta = game.dir.join(".dlsite-launcher");
    assert_eq!(fs::read_dir(meta).unwrap().count(), 1, "only info.json");
}

#[test]
fn other_category_page_without_a_work_title_falls_back() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ424242");
    let landing =
        b"<html><head><title>DLsite</title></head><body><p>Not available</p></body></html>";
    let site = Site::default().with(
        "https://www.dlsite.com/maniax-touch/work/=/product_id/RJ424242.html",
        landing,
    );

    refresh(&game, &site).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "RJ424242");
    assert_eq!(info["found"], false);
}

#[test]
fn reload_never_touches_the_profile() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let meta = game.dir.join(".dlsite-launcher");
    fs::create_dir_all(&meta).unwrap();
    let profile = br#"{"exe":"game/start.exe","favorite":true}"#;
    fs::write(meta.join("profile.json"), profile).unwrap();
    let site = Site::default()
        .with(PAGE_URL, &fixture("maniax_touch_work.html"))
        .with(COVER_URL, b"cover");

    refresh(&game, &site).unwrap();
    refresh(&game, &Site::default()).unwrap();

    assert_eq!(fs::read(meta.join("profile.json")).unwrap(), profile);
}

#[test]
fn reload_removes_a_cover_the_page_no_longer_has() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let site = Site::default()
        .with(PAGE_URL, &fixture("maniax_touch_work.html"))
        .with(COVER_URL, b"cover");
    refresh(&game, &site).unwrap();
    let meta = game.dir.join(".dlsite-launcher");
    assert!(meta.join("cover.jpg").exists());

    let page_only = Site::default().with(PAGE_URL, &fixture("maniax_touch_work.html"));
    refresh(&game, &page_only).unwrap();

    assert!(!meta.join("cover.jpg").exists());
    assert_eq!(read_info(&game)["cover"], serde_json::Value::Null);
}

struct Offline;

impl Fetch for Offline {
    fn get(&self, _url: &str) -> io::Result<Fetched> {
        Err(io::Error::other("no network"))
    }
}

#[test]
fn network_failure_is_reported_and_keeps_the_old_info() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    refresh(
        &game,
        &Site::default().with(PAGE_URL, &fixture("maniax_touch_work.html")),
    )
    .unwrap();
    let before = read_info(&game);

    assert!(refresh(&game, &Offline).is_err());

    assert_eq!(read_info(&game), before);
}

#[test]
fn load_returns_what_refresh_saved() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    assert_eq!(load(&game), None);
    let site = Site::default()
        .with(PAGE_URL, &fixture("maniax_touch_work.html"))
        .with(COVER_URL, b"cover");

    let saved = refresh(&game, &site).unwrap();

    assert_eq!(load(&game), Some(saved));
    assert_eq!(
        cover_path(&game, &load(&game).unwrap()),
        Some(game.dir.join(".dlsite-launcher").join("cover.jpg"))
    );
}
