mod common;

use common::{COVER_URL, PAGE_URL, Site, fixture, game_in};
use dackel_launcher::fetch::{Fetch, Fetched};
use dackel_launcher::info::{cover_path, load, refresh};
use dackel_launcher::library::Game;
use std::fs;
use std::io;

fn read_info(game: &Game) -> serde_json::Value {
    let path = game.dir.join(".dackel-launcher").join("info.json");
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

    let meta = game.dir.join(".dackel-launcher");
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
    let meta = game.dir.join(".dackel-launcher");
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
    let meta = game.dir.join(".dackel-launcher");
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
    let meta = game.dir.join(".dackel-launcher");
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
        Some(game.dir.join(".dackel-launcher").join("cover.jpg"))
    );
}

#[test]
fn a_game_that_is_not_from_dlsite_is_never_looked_up_online() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "Some Other Game");

    let info = refresh(&game, &Offline).unwrap();

    assert_eq!(info.title, "Some Other Game");
    assert!(!info.found);
    assert_eq!(read_info(&game)["found"], false);
}

const PROJECT072_URL: &str = "https://api.072project.com/shop/product/RA25128779?source=";
const PROJECT072_COVER_URL: &str =
    "https://storage.072project.com/img/product/e3b0c442_onh3tp0o.jpg";

#[test]
fn project072_product_is_parsed_into_info_json() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let site = Site::default().with(PROJECT072_URL, &fixture("072project_product.json"));

    refresh(&game, &site).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "Sample Title");
    assert_eq!(info["circle"], "PAON");
    assert_eq!(info["genres"], serde_json::json!(["Alpha", "Beta"]));
    assert_eq!(
        info["description"],
        "First summary line.\nSecond summary line."
    );
    assert_eq!(info["found"], true);
}

#[test]
fn project072_cover_is_saved_beside_info_json() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let cover = b"\xFF\xD8\xFF fake jpeg bytes";
    let site = Site::default()
        .with(PROJECT072_URL, &fixture("072project_product.json"))
        .with(PROJECT072_COVER_URL, cover);

    refresh(&game, &site).unwrap();

    let meta = game.dir.join(".dackel-launcher");
    assert_eq!(read_info(&game)["cover"], "cover.jpg");
    assert_eq!(fs::read(meta.join("cover.jpg")).unwrap(), cover);
}

#[test]
fn missing_project072_product_falls_back_to_folder_name() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA99999999");

    refresh(&game, &Site::default()).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "RA99999999");
    assert_eq!(info["found"], false);
}

/// Records every URL asked for, and answers each with the given body.
#[derive(Default)]
struct Recording {
    urls: std::cell::RefCell<Vec<String>>,
}

impl Fetch for Recording {
    fn get(&self, url: &str) -> io::Result<Fetched> {
        self.urls.borrow_mut().push(url.to_owned());
        Ok(Fetched::NotFound)
    }
}

#[test]
fn a_072project_game_asks_only_the_072project_api() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let fetch = Recording::default();

    refresh(&game, &fetch).unwrap();

    assert_eq!(*fetch.urls.borrow(), vec![PROJECT072_URL.to_owned()]);
}

#[test]
fn a_dlsite_game_never_asks_the_072project_api() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RJ123456");
    let fetch = Recording::default();

    refresh(&game, &fetch).unwrap();

    assert_eq!(*fetch.urls.borrow(), vec![PAGE_URL.to_owned()]);
}

#[test]
fn a_072project_body_that_is_not_json_falls_back_to_folder_name() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let site = Site::default().with(PROJECT072_URL, b"<html>maintenance</html>");

    refresh(&game, &site).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "RA25128779");
    assert_eq!(info["found"], false);
}

#[test]
fn a_072project_product_with_an_empty_title_falls_back() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let body = br#"{"developer":"PAON","locale":{"locale_title":"  ","locale_summary":null,"cover_img":null},"tags":[]}"#;
    let site = Site::default().with(PROJECT072_URL, body);

    refresh(&game, &site).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "RA25128779");
    assert_eq!(info["found"], false);
}

#[test]
fn a_072project_product_with_no_developer_tags_or_cover_keeps_only_the_title() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let body = br#"{"developer":null,"locale":{"locale_title":"Bare","locale_summary":null,"cover_img":null}}"#;
    let site = Site::default().with(PROJECT072_URL, body);

    refresh(&game, &site).unwrap();

    let info = read_info(&game);
    assert_eq!(info["title"], "Bare");
    assert_eq!(info["circle"], serde_json::Value::Null);
    assert_eq!(info["genres"], serde_json::json!([]));
    assert_eq!(info["description"], "");
    assert_eq!(info["cover"], serde_json::Value::Null);
    assert_eq!(info["found"], true);
}

#[test]
fn a_072project_tag_without_the_t_prefix_is_kept_as_it_is() {
    let lib = tempfile::tempdir().unwrap();
    let game = game_in(lib.path(), "RA25128779");
    let body = br#"{"developer":"PAON","locale":{"locale_title":"T","locale_summary":"s","cover_img":null},"tags":[{"tag_id":9,"name_tkey":"Plain"}]}"#;
    let site = Site::default().with(PROJECT072_URL, body);

    refresh(&game, &site).unwrap();

    assert_eq!(read_info(&game)["genres"], serde_json::json!(["Plain"]));
}
