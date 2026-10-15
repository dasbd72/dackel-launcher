use dackel_launcher::fetch::{Fetch, Fetched};
use dackel_launcher::library::Game;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

pub const PAGE_URL: &str = "https://www.dlsite.com/maniax-touch/work/=/product_id/RJ123456.html";
pub const COVER_URL: &str =
    "https://img.dlsite.jp/modpub/images2/work/doujin/RJ123456_img_main.jpg";

/// Stands in for the network: a fixed set of URLs; anything else is a 404.
#[derive(Default)]
pub struct Site {
    pub pages: HashMap<String, Vec<u8>>,
}

impl Site {
    pub fn with(mut self, url: &str, body: &[u8]) -> Self {
        self.pages.insert(url.to_owned(), body.to_vec());
        self
    }
}

impl Fetch for Site {
    fn get(&self, url: &str) -> io::Result<Fetched> {
        Ok(match self.pages.get(url) {
            Some(body) => Fetched::Body(body.clone()),
            None => Fetched::NotFound,
        })
    }
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read(path).unwrap()
}

pub fn game_in(lib: &Path, id: &str) -> Game {
    let dir = lib.join(id);
    fs::create_dir_all(&dir).unwrap();
    Game {
        id: id.to_owned(),
        dir,
        slot: 0,
    }
}
